use crate::db::DatabaseManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Database error: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    #[error("Embedding error: {0}")]
    Embed(String),
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub chunk_id: String,
    pub artifact_id: String,
    pub version_id: String,
    pub path: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Clone)]
pub struct Fts5SearchEngine {
    db: Arc<DatabaseManager>,
}

impl Fts5SearchEngine {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Sanitizes an input query string to avoid SQLite FTS5 syntax errors
    /// while preserving exact quoted phrases and word matching.
    pub fn sanitize_query(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // If the user deliberately wrapped in quotes, keep as exact phrase
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
            let inner = &trimmed[1..trimmed.len() - 1].replace('"', "");
            return format!("\"{}\"", inner);
        }

        // Tokenize and sanitize special FTS5 operators (*, :, ^, -, etc.)
        let tokens: Vec<String> = trimmed
            .split_whitespace()
            .map(|word| {
                let clean: String = word
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                    .collect();
                if clean.is_empty() {
                    String::new()
                } else {
                    format!("\"{}\"", clean)
                }
            })
            .filter(|w| !w.is_empty())
            .collect();

        if tokens.is_empty() {
            String::new()
        } else {
            tokens.join(" AND ")
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, SearchError> {
        let sanitized = Self::sanitize_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .db
            .lock_conn()
            .map_err(|e| SearchError::Db(e))?;

        let sql = r#"
            SELECT chunk_id, artifact_id, version_id, path,
                   snippet(chunks_fts, 4, '<mark>', '</mark>', '...', 24) AS snip,
                   bm25(chunks_fts) AS rank
            FROM chunks_fts
            WHERE chunks_fts MATCH ?1
            ORDER BY rank ASC
            LIMIT ?2;
        "#;

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![sanitized, limit], |r| {
            Ok(SearchResult {
                chunk_id: r.get(0)?,
                artifact_id: r.get(1)?,
                version_id: r.get(2)?,
                path: r.get(3)?,
                snippet: r.get(4)?,
                score: r.get(5)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactVersion, DocumentChunk};
    use std::time::Instant;

    #[test]
    fn test_fts5_query_sanitization() {
        assert_eq!(Fts5SearchEngine::sanitize_query("error_code:123*"), "\"error_code123\"");
        assert_eq!(Fts5SearchEngine::sanitize_query("\"exact phrase\""), "\"exact phrase\"");
        assert_eq!(Fts5SearchEngine::sanitize_query("invoice Apple"), "\"invoice\" AND \"Apple\"");
        assert_eq!(Fts5SearchEngine::sanitize_query("   "), "");
    }

    #[test]
    fn test_fts5_exact_search_and_latency_benchmark() {
        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        let search_engine = Fts5SearchEngine::new(db.clone());

        // Seed 100 sample chunks
        let artifact = db.upsert_artifact("/Users/test/Documents/sample.txt", "file").unwrap();
        let version_id = "v_sample_1";
        let version = ArtifactVersion {
            id: version_id.to_string(),
            artifact_id: artifact.id.clone(),
            path: "/Users/test/Documents/sample.txt".to_string(),
            content_hash: "hash_sample".to_string(),
            text_hash: None,
            size_bytes: 10000,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: None,
            is_pinned: false,
        };
        db.record_version(&version).unwrap();

        let mut chunks = Vec::new();
        for i in 0..100 {
            let content = if i == 42 {
                "Target error code ERR_AUTH_TIMEOUT occurred while contacting the database server."
            } else if i == 84 {
                "Meeting notes discussing Q3 revenue targets and budget allocations for marketing."
            } else {
                "Regular background documentation passage containing general project text information."
            };

            chunks.push(DocumentChunk {
                id: format!("chunk_{}", i),
                version_id: version_id.to_string(),
                chunk_index: i as i32,
                content: content.to_string(),
            });
        }
        db.insert_chunks(&chunks).unwrap();

        // 1. Exact Error Code Search Benchmark (<50 ms target)
        let start = Instant::now();
        let results = search_engine.search("ERR_AUTH_TIMEOUT", 10).unwrap();
        let elapsed = start.elapsed();

        println!("FTS5 search latency: {:?}", elapsed);
        assert!(elapsed.as_millis() < 50, "Query latency must be <50ms, took {:?}", elapsed);
        assert!(!results.is_empty(), "Should return matching error code chunk");
        assert_eq!(results[0].chunk_id, "chunk_42");
        assert!(results[0].snippet.contains("<mark>ERR_AUTH_TIMEOUT</mark>"));

        // 2. Exact Phrase Search Benchmark
        let phrase_results = search_engine.search("\"Q3 revenue targets\"", 10).unwrap();
        assert!(!phrase_results.is_empty());
        assert_eq!(phrase_results[0].chunk_id, "chunk_84");
    }
}
