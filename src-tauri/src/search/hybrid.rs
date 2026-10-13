use crate::db::DatabaseManager;
use crate::diff::{DiffCache, DiffEngine};
use crate::models::AppSettings;
use crate::search::fts5::{Fts5SearchEngine, SearchError};
use crate::search::parser::{parse_query, ChangeDirection};
use crate::vector::{ModelScheduler, VectorIndex};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchReason {
    ExactKeyword {
        terms: Vec<String>,
    },
    SemanticSimilarity {
        score: f32,
    },
    TimeRangeMatch {
        label: String,
    },
    TypeMatch {
        extension: String,
    },
    PathMatch {
        folder: String,
    },
    VersionCue {
        cue: String,
    },
    VersionRelationship {
        version_number: usize,
        total_versions: usize,
        relation_summary: String,
        matched_diff_snippet: Option<String>,
        later_version_date: Option<i64>,
        is_pinned: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub version_id: String,
    pub modified_at: i64,
    pub size_bytes: i64,
    pub is_pinned: bool,
    pub version_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedSearchResult {
    pub primary_result: HybridSearchResult,
    pub other_versions: Vec<VersionSummary>,
    pub total_version_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub chunk_id: String,
    pub artifact_id: String,
    pub version_id: String,
    pub path: String,
    pub score: f32,
    pub snippet: String,
    pub reasons: Vec<MatchReason>,
    pub modified_at: i64,
    pub is_pinned: bool,
}

#[derive(Default)]
struct CandidateAccumulator {
    chunk_id: String,
    artifact_id: String,
    version_id: String,
    path: String,
    snippet: String,
    modified_at: i64,
    is_pinned: bool,
    fts_rank: Option<usize>,
    vector_rank: Option<usize>,
    vector_sim: Option<f32>,
    matched_exact_terms: Vec<String>,
}

pub struct HybridSearchEngine {
    db: Arc<DatabaseManager>,
    fts: Arc<Fts5SearchEngine>,
    vector_index: Arc<dyn VectorIndex>,
    scheduler: Arc<ModelScheduler>,
    diff_engine: Arc<DiffEngine>,
    diff_cache: Arc<DiffCache>,
}

impl HybridSearchEngine {
    pub fn new(
        db: Arc<DatabaseManager>,
        fts: Arc<Fts5SearchEngine>,
        vector_index: Arc<dyn VectorIndex>,
        scheduler: Arc<ModelScheduler>,
    ) -> Self {
        Self::with_diff_support(
            db,
            fts,
            vector_index,
            scheduler,
            Arc::new(DiffEngine::new()),
            Arc::new(DiffCache::default()),
        )
    }

    pub fn with_diff_support(
        db: Arc<DatabaseManager>,
        fts: Arc<Fts5SearchEngine>,
        vector_index: Arc<dyn VectorIndex>,
        scheduler: Arc<ModelScheduler>,
        diff_engine: Arc<DiffEngine>,
        diff_cache: Arc<DiffCache>,
    ) -> Self {
        Self {
            db,
            fts,
            vector_index,
            scheduler,
            diff_engine,
            diff_cache,
        }
    }

    pub async fn search(
        &self,
        raw_query: &str,
        limit: usize,
        settings: &AppSettings,
    ) -> Result<Vec<HybridSearchResult>, SearchError> {
        let trimmed = raw_query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let now = Utc::now();
        let parsed = parse_query(trimmed, now);

        let mut candidates: HashMap<String, CandidateAccumulator> = HashMap::new();

        // 1. FTS5 Search Execution
        let fts_query = if !parsed.exact_phrases.is_empty() {
            format!("{} {}", parsed.clean_text, parsed.exact_phrases.join(" "))
        } else {
            parsed.clean_text.clone()
        };

        if let Ok(fts_results) = self.fts.search(&fts_query, limit * 2) {
            for (idx, r) in fts_results.into_iter().enumerate() {
                let entry =
                    candidates
                        .entry(r.chunk_id.clone())
                        .or_insert_with(|| CandidateAccumulator {
                            chunk_id: r.chunk_id.clone(),
                            artifact_id: r.artifact_id.clone(),
                            version_id: r.version_id.clone(),
                            path: r.path.clone(),
                            snippet: r.snippet.clone(),
                            modified_at: 0,
                            is_pinned: false,
                            fts_rank: None,
                            vector_rank: None,
                            vector_sim: None,
                            matched_exact_terms: Vec::new(),
                        });
                entry.fts_rank = Some(idx + 1);
                if !parsed.exact_phrases.is_empty() {
                    entry.matched_exact_terms = parsed.exact_phrases.clone();
                }
            }
        }

        // 2. Vector kNN Search Execution
        if settings.semantic_search_enabled {
            match self.scheduler.get_model().await {
                Ok(model) => match model.embed_text(&parsed.clean_text) {
                    Ok(query_vec) => match self.vector_index.search_knn(&query_vec, limit * 2) {
                        Ok(vec_results) => {
                            tracing::debug!(
                                "search: vector kNN returned {} candidates for '{}'",
                                vec_results.len(),
                                parsed.clean_text
                            );
                            for (idx, (chunk_id, sim)) in vec_results.into_iter().enumerate() {
                                let entry =
                                    candidates.entry(chunk_id.clone()).or_insert_with(|| {
                                        CandidateAccumulator {
                                            chunk_id: chunk_id.clone(),
                                            artifact_id: String::new(),
                                            version_id: String::new(),
                                            path: String::new(),
                                            snippet: String::new(),
                                            modified_at: 0,
                                            is_pinned: false,
                                            fts_rank: None,
                                            vector_rank: None,
                                            vector_sim: None,
                                            matched_exact_terms: Vec::new(),
                                        }
                                    });
                                entry.vector_rank = Some(idx + 1);
                                entry.vector_sim = Some(sim);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("search: vector_index search_knn error: {:?}", e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("search: failed to generate query embedding: {:?}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("search: scheduler failed to get embedding model: {:?}", e);
                }
            }
        }

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // 3. Hydrate missing metadata for vector-only candidates
        let missing_ids: Vec<String> = candidates
            .values()
            .filter(|c| c.artifact_id.is_empty())
            .map(|c| c.chunk_id.clone())
            .collect();

        if !missing_ids.is_empty() {
            let conn = self.db.lock_conn()?;
            for chunk_id in missing_ids {
                let res = conn.query_row(
                    r#"
                    SELECT c.id, v.artifact_id, v.id, v.path, v.modified_at, v.is_pinned, SUBSTR(c.content, 1, 200)
                    FROM chunks c
                    JOIN artifact_versions v ON c.version_id = v.id
                    WHERE c.id = ?1
                    "#,
                    params![chunk_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, bool>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    },
                );

                if let Ok((art_id, ver_id, path, mod_at, pinned, snippet)) = res {
                    if let Some(entry) = candidates.get_mut(&chunk_id) {
                        entry.artifact_id = art_id;
                        entry.version_id = ver_id;
                        entry.path = path;
                        entry.modified_at = mod_at;
                        entry.is_pinned = pinned;
                        entry.snippet = snippet;
                    }
                }
            }
        }

        // Hydrate modified_at and is_pinned for FTS candidates if still 0
        {
            let conn = self.db.lock_conn()?;
            for entry in candidates.values_mut() {
                if entry.modified_at == 0 && !entry.version_id.is_empty() {
                    let ver_info = conn.query_row(
                        "SELECT modified_at, is_pinned FROM artifact_versions WHERE id = ?1",
                        params![entry.version_id],
                        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)),
                    );
                    if let Ok((mod_at, pinned)) = ver_info {
                        entry.modified_at = mod_at;
                        entry.is_pinned = pinned;
                    }
                }
            }
        }

        // 3b. Evaluate Version Cues & Candidate Injection
        let mut version_boosts: HashMap<String, (f32, MatchReason)> = HashMap::new();

        if let Some(ref d_cue) = parsed.directional_version_cue {
            let unique_artifacts: HashSet<String> = candidates
                .values()
                .map(|c| c.artifact_id.clone())
                .filter(|id| !id.is_empty())
                .collect();

            for art_id in unique_artifacts {
                if let Ok(versions) = self.db.get_artifact_versions_ordered(&art_id) {
                    let total_versions = versions.len();
                    if total_versions < 2 {
                        continue;
                    }

                    // Look for version matching ordinal cue if provided
                    if let Some(target_ord) = d_cue.ordinal {
                        if target_ord > 0 && (target_ord as usize) <= total_versions {
                            let ver = &versions[(target_ord as usize) - 1];
                            let reason = MatchReason::VersionRelationship {
                                version_number: target_ord as usize,
                                total_versions,
                                relation_summary: format!(
                                    "Matches explicit version v{}",
                                    target_ord
                                ),
                                matched_diff_snippet: None,
                                later_version_date: if (target_ord as usize) < total_versions {
                                    Some(versions[target_ord as usize].modified_at)
                                } else {
                                    None
                                },
                                is_pinned: ver.is_pinned,
                            };
                            version_boosts.insert(ver.id.clone(), (1.30, reason));
                        }
                    }

                    // Evaluate directional change cues (Before / After)
                    if let Some(dir) = d_cue.direction {
                        for i in 0..(total_versions - 1) {
                            let v_a = &versions[i];
                            let v_b = &versions[i + 1];

                            let diff = if let Some(cached) = self.diff_cache.get(&v_a.id, &v_b.id) {
                                cached
                            } else {
                                let text_a = v_a.extracted_text.as_deref().unwrap_or("");
                                let text_b = v_b.extracted_text.as_deref().unwrap_or("");
                                let computed = self
                                    .diff_engine
                                    .compute_diff(text_a, text_b, &v_a.id, &v_b.id);
                                self.diff_cache.put(computed.clone());
                                computed
                            };

                            let mut target_ver = None;
                            let mut target_ver_idx = 0;
                            let mut relation_text = String::new();
                            let mut diff_snippet = None;

                            match dir {
                                ChangeDirection::Before => {
                                    if let Some(ref target) = d_cue.target_subject {
                                        let words: Vec<&str> = target.split_whitespace().collect();
                                        let matches_term = words.iter().any(|w| {
                                            diff.removed_terms.contains(&w.to_lowercase())
                                        });
                                        if matches_term {
                                            target_ver = Some(v_a);
                                            target_ver_idx = i + 1;
                                            relation_text = format!(
                                                "Preserved before '{}' was modified/removed",
                                                target
                                            );
                                            let mut snippet_lines = Vec::new();
                                            for hunk in &diff.hunks {
                                                if hunk.tag == "removed"
                                                    && words.iter().any(|w| {
                                                        hunk.content
                                                            .to_lowercase()
                                                            .contains(&w.to_lowercase())
                                                    })
                                                {
                                                    for line in hunk.content.lines().take(3) {
                                                        let trimmed = line.trim();
                                                        if !trimmed.is_empty() {
                                                            snippet_lines
                                                                .push(format!("- {}", trimmed));
                                                        }
                                                    }
                                                    break;
                                                }
                                            }
                                            if !snippet_lines.is_empty() {
                                                diff_snippet = Some(snippet_lines.join("\n"));
                                            }
                                        }
                                    } else {
                                        target_ver = Some(v_a);
                                        target_ver_idx = i + 1;
                                        relation_text =
                                            "Preserved prior to subsequent revision".to_string();
                                    }
                                }
                                ChangeDirection::After => {
                                    if let Some(ref target) = d_cue.target_subject {
                                        let words: Vec<&str> = target.split_whitespace().collect();
                                        let matches_term = words
                                            .iter()
                                            .any(|w| diff.added_terms.contains(&w.to_lowercase()));
                                        if matches_term {
                                            target_ver = Some(v_b);
                                            target_ver_idx = i + 2;
                                            relation_text = format!(
                                                "Introduced in revision containing '{}'",
                                                target
                                            );
                                            let mut snippet_lines = Vec::new();
                                            for hunk in &diff.hunks {
                                                if hunk.tag == "added"
                                                    && words.iter().any(|w| {
                                                        hunk.content
                                                            .to_lowercase()
                                                            .contains(&w.to_lowercase())
                                                    })
                                                {
                                                    for line in hunk.content.lines().take(3) {
                                                        let trimmed = line.trim();
                                                        if !trimmed.is_empty() {
                                                            snippet_lines
                                                                .push(format!("+ {}", trimmed));
                                                        }
                                                    }
                                                    break;
                                                }
                                            }
                                            if !snippet_lines.is_empty() {
                                                diff_snippet = Some(snippet_lines.join("\n"));
                                            }
                                        }
                                    } else {
                                        target_ver = Some(v_b);
                                        target_ver_idx = i + 2;
                                        relation_text =
                                            "Updated version in document history".to_string();
                                    }
                                }
                            }

                            if let Some(matched_v) = target_ver {
                                let reason = MatchReason::VersionRelationship {
                                    version_number: target_ver_idx,
                                    total_versions,
                                    relation_summary: relation_text,
                                    matched_diff_snippet: diff_snippet,
                                    later_version_date: if target_ver_idx < total_versions {
                                        Some(versions[target_ver_idx].modified_at)
                                    } else {
                                        None
                                    },
                                    is_pinned: matched_v.is_pinned,
                                };
                                version_boosts.insert(matched_v.id.clone(), (1.50, reason));

                                // Candidate injection: if target version chunk not in candidates, inject first chunk
                                let already_present =
                                    candidates.values().any(|c| c.version_id == matched_v.id);
                                if !already_present {
                                    if let Ok(chunks) =
                                        self.db.get_chunks_for_version(&matched_v.id)
                                    {
                                        if let Some(first_chunk) = chunks.into_iter().next() {
                                            let snippet = if first_chunk.content.len() > 200 {
                                                first_chunk.content[..200].to_string()
                                            } else {
                                                first_chunk.content.clone()
                                            };
                                            candidates.insert(
                                                first_chunk.id.clone(),
                                                CandidateAccumulator {
                                                    chunk_id: first_chunk.id,
                                                    artifact_id: matched_v.artifact_id.clone(),
                                                    version_id: matched_v.id.clone(),
                                                    path: matched_v.path.clone(),
                                                    snippet,
                                                    modified_at: matched_v.modified_at,
                                                    is_pinned: matched_v.is_pinned,
                                                    fts_rank: Some(limit * 2 + 1),
                                                    vector_rank: None,
                                                    vector_sim: None,
                                                    matched_exact_terms: Vec::new(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Reciprocal Rank Fusion & Structured MatchReason Generation
        let k = settings.rrf_k as f32;
        let w_fts = settings.fts_weight;
        let w_vec = settings.vector_weight;

        let mut scored_results: Vec<HybridSearchResult> = Vec::with_capacity(candidates.len());

        for cand in candidates.into_values() {
            // Ignore candidates where hydration completely failed
            if cand.path.is_empty() {
                continue;
            }

            let mut rrf_score = 0.0f32;
            let mut reasons = Vec::new();

            if let Some(fts_r) = cand.fts_rank {
                rrf_score += w_fts / (k + fts_r as f32);
                let terms = if !cand.matched_exact_terms.is_empty() {
                    cand.matched_exact_terms.clone()
                } else {
                    parsed
                        .clean_text
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect()
                };
                reasons.push(MatchReason::ExactKeyword { terms });
            }

            if let Some(vec_r) = cand.vector_rank {
                rrf_score += w_vec / (k + vec_r as f32);
                let sim = cand.vector_sim.unwrap_or(0.0);
                reasons.push(MatchReason::SemanticSimilarity { score: sim });
            }

            // Multiplicative boosts
            let mut boost = 1.0f32;

            // Folder location boost
            if let Some(ref folder) = parsed.folder_hint {
                if cand.path.to_lowercase().contains(&folder.to_lowercase()) {
                    boost *= 1.25;
                    reasons.push(MatchReason::PathMatch {
                        folder: folder.clone(),
                    });
                }
            }

            // File type extension boost
            if !parsed.type_extensions.is_empty() {
                if let Some(ext) = Path::new(&cand.path).extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if parsed
                        .type_extensions
                        .iter()
                        .any(|t| t.to_lowercase() == ext_lower)
                    {
                        boost *= 1.25;
                        reasons.push(MatchReason::TypeMatch {
                            extension: ext_lower,
                        });
                    }
                }
            }

            // Temporal date filter boost
            if let Some(ref df) = parsed.date_filter {
                if cand.modified_at >= df.start_ms && cand.modified_at <= df.end_ms {
                    boost *= 1.30;
                    reasons.push(MatchReason::TimeRangeMatch {
                        label: df.label.clone(),
                    });
                }
            }

            // Version cue boost
            if !parsed.version_cues.is_empty() {
                let lower_path = cand.path.to_lowercase();
                for cue in &parsed.version_cues {
                    if lower_path.contains(cue) || cand.snippet.to_lowercase().contains(cue) {
                        boost *= 1.20;
                        reasons.push(MatchReason::VersionCue { cue: cue.clone() });
                        break;
                    }
                }
            }

            // Version relationship directional boost
            if let Some((v_boost, ref v_reason)) = version_boosts.get(&cand.version_id) {
                boost *= v_boost;
                reasons.push(v_reason.clone());
            }

            let final_score = rrf_score * boost;

            scored_results.push(HybridSearchResult {
                chunk_id: cand.chunk_id,
                artifact_id: cand.artifact_id,
                version_id: cand.version_id,
                path: cand.path,
                score: final_score,
                snippet: cand.snippet,
                reasons,
                modified_at: cand.modified_at,
                is_pinned: cand.is_pinned,
            });
        }

        // Sort descending by score
        scored_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored_results.truncate(limit);

        Ok(scored_results)
    }

    pub fn group_results_by_artifact(
        &self,
        results: Vec<HybridSearchResult>,
    ) -> Vec<GroupedSearchResult> {
        let mut groups: Vec<GroupedSearchResult> = Vec::new();
        let mut seen_artifacts: HashSet<String> = HashSet::new();

        for r in results {
            if seen_artifacts.contains(&r.artifact_id) {
                continue;
            }
            seen_artifacts.insert(r.artifact_id.clone());

            let mut other_versions = Vec::new();
            let mut total_count = 1;

            if let Ok(all_versions) = self.db.get_artifact_versions_ordered(&r.artifact_id) {
                total_count = all_versions.len();
                for (idx, v) in all_versions.into_iter().enumerate() {
                    if v.id != r.version_id {
                        other_versions.push(VersionSummary {
                            version_id: v.id,
                            modified_at: v.modified_at,
                            size_bytes: v.size_bytes,
                            is_pinned: v.is_pinned,
                            version_number: idx + 1,
                        });
                    }
                }
            }

            groups.push(GroupedSearchResult {
                primary_result: r,
                other_versions,
                total_version_count: total_count,
            });
        }

        groups
    }

    pub async fn search_grouped(
        &self,
        raw_query: &str,
        limit: usize,
        settings: &AppSettings,
    ) -> Result<Vec<GroupedSearchResult>, SearchError> {
        let raw_results = self.search(raw_query, limit, settings).await?;
        Ok(self.group_results_by_artifact(raw_results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactVersion, DocumentChunk};
    use crate::vector::embedder::MockEmbedder;
    use crate::vector::index::SqliteVectorIndex;

    #[tokio::test]
    async fn test_hybrid_search_rrf_and_reasons() {
        let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
        let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
        let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
        let scheduler = Arc::new(ModelScheduler::with_factory(60, || {
            Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn crate::vector::Embedder>)
        }));

        let engine =
            HybridSearchEngine::new(db.clone(), fts, vector_index.clone(), scheduler.clone());

        // Setup test data
        let path1 = "/Users/test/Documents/invoice.pdf";
        let art1 = db.upsert_artifact(path1, "file").unwrap();
        let v1_id = "v1-id";
        let now_ms = Utc::now().timestamp_millis();
        let v1 = ArtifactVersion {
            id: v1_id.to_string(),
            artifact_id: art1.id.clone(),
            path: path1.to_string(),
            content_hash: "h1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: now_ms,
            previous_version_id: None,
            extracted_text: Some("Total invoice amount for consulting services".to_string()),
            is_pinned: true,
        };
        db.record_version(&v1).unwrap();

        let chunk1 = DocumentChunk {
            id: "c1".to_string(),
            version_id: v1_id.to_string(),
            chunk_index: 0,
            content: "Total invoice amount for consulting services".to_string(),
        };
        db.insert_chunks(&[chunk1]).unwrap();

        // Insert vector for c1
        let model = scheduler.get_model().await.unwrap();
        let vec1 = model
            .embed_text("Total invoice amount for consulting services")
            .unwrap();
        vector_index.insert_vector("c1", &vec1).unwrap();

        let settings = AppSettings::default();

        // 1. Search with exact keyword and folder hint
        let results = engine
            .search("invoice in Documents", 10, &settings)
            .await
            .expect("search hybrid");

        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert_eq!(res.chunk_id, "c1");
        assert_eq!(res.path, path1);
        assert!(res.score > 0.0);
        assert!(res.is_pinned);

        // Check match reasons
        let has_keyword = res
            .reasons
            .iter()
            .any(|r| matches!(r, MatchReason::ExactKeyword { .. }));
        let has_vector = res
            .reasons
            .iter()
            .any(|r| matches!(r, MatchReason::SemanticSimilarity { .. }));
        let has_path = res
            .reasons
            .iter()
            .any(|r| matches!(r, MatchReason::PathMatch { .. }));
        assert!(has_keyword);
        assert!(has_vector);
        assert!(has_path);
    }
}
