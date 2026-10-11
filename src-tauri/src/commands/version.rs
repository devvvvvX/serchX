use crate::db::DatabaseManager;
use crate::diff::{DiffCache, DiffEngine, VersionDiff};
use crate::models::ArtifactVersion;
use std::sync::Arc;
use tauri::State;

/// Lightweight version summary returned to the compare window.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct VersionSummary {
    pub version_id: String,
    pub version_number: usize,
    pub modified_at: i64,
    pub is_pinned: bool,
}

pub fn resolve_versions_for_path(
    db: &DatabaseManager,
    path: &str,
) -> Result<Vec<VersionSummary>, String> {
    // 1. Find the artifact_id from the latest version with this path
    let latest = db
        .get_latest_version_by_path(path)
        .map_err(|e| e.to_string())?;

    let artifact_id = match latest {
        Some(v) => v.artifact_id,
        None => {
            // Check if canonical path matches
            let canonical_match = std::fs::canonicalize(path).ok().and_then(|canon| {
                let canon_str = canon.to_string_lossy().to_string();
                db.get_latest_version_by_path(&canon_str).ok().flatten()
            });

            if let Some(v) = canonical_match {
                v.artifact_id
            } else if let Ok(Some(v)) = db.get_version_by_id(path) {
                v.artifact_id
            } else {
                let by_artifact = db.get_artifact_versions_ordered(path).unwrap_or_default();
                if !by_artifact.is_empty() {
                    let summaries = by_artifact
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| VersionSummary {
                            version_id: v.id,
                            version_number: i + 1,
                            modified_at: v.modified_at,
                            is_pinned: v.is_pinned,
                        })
                        .collect();
                    return Ok(summaries);
                }
                return Ok(vec![]);
            }
        }
    };

    // 2. Return all versions for this artifact ordered oldest → newest
    let versions: Vec<ArtifactVersion> = db
        .get_artifact_versions_ordered(&artifact_id)
        .map_err(|e| e.to_string())?;

    let summaries = versions
        .into_iter()
        .enumerate()
        .map(|(i, v)| VersionSummary {
            version_id: v.id,
            version_number: i + 1,
            modified_at: v.modified_at,
            is_pinned: v.is_pinned,
        })
        .collect();

    Ok(summaries)
}

#[tauri::command]
pub async fn get_versions_for_path(
    path: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<VersionSummary>, String> {
    resolve_versions_for_path(db.inner(), &path)
}

fn validate_version_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err("Invalid version ID length (must be 1-64 characters)".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Invalid version ID characters (alphanumeric, hyphens, and underscores only)"
                .to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn get_version_diff(
    version_a_id: String,
    version_b_id: String,
    db: State<'_, Arc<DatabaseManager>>,
    diff_engine: State<'_, Arc<DiffEngine>>,
    diff_cache: State<'_, Arc<DiffCache>>,
) -> Result<VersionDiff, String> {
    validate_version_id(&version_a_id)?;
    validate_version_id(&version_b_id)?;

    if let Some(cached) = diff_cache.get(&version_a_id, &version_b_id) {
        return Ok(cached);
    }

    let ver_a = db
        .get_version_by_id(&version_a_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Version '{}' not found", version_a_id))?;

    let ver_b = db
        .get_version_by_id(&version_b_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Version '{}' not found", version_b_id))?;

    let text_a = match ver_a.extracted_text {
        Some(t) => t,
        None => {
            let mut chunks = db.get_chunks_for_version(&version_a_id).unwrap_or_default();
            chunks.sort_by_key(|c| c.chunk_index);
            // Use "\n" (not "\n\n") to match extracted_text format and avoid
            // false diffs at paragraph boundaries when one version has
            // extracted_text and the other falls back to chunk reconstruction
            chunks
                .into_iter()
                .map(|c| c.content)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    let text_b = match ver_b.extracted_text {
        Some(t) => t,
        None => {
            let mut chunks = db.get_chunks_for_version(&version_b_id).unwrap_or_default();
            chunks.sort_by_key(|c| c.chunk_index);
            chunks
                .into_iter()
                .map(|c| c.content)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    let diff = diff_engine.compute_diff(&text_a, &text_b, &version_a_id, &version_b_id);
    diff_cache.put(diff.clone());
    Ok(diff)
}

#[tauri::command]
pub async fn toggle_version_pin(
    version_id: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<bool, String> {
    validate_version_id(&version_id)?;

    let ver = db
        .get_version_by_id(&version_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Version '{}' not found", version_id))?;

    let new_pinned = !ver.is_pinned;
    crate::db::pin_version(db.inner(), &version_id, new_pinned).map_err(|e| e.to_string())?;

    Ok(new_pinned)
}
