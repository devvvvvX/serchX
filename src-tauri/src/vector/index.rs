use crate::db::{DatabaseManager, DbError};
use rusqlite::params;
use std::sync::Arc;

pub trait VectorIndex: Send + Sync {
    fn insert_vector(&self, chunk_id: &str, embedding: &[f32]) -> Result<(), DbError>;
    fn insert_vectors_batch(&self, items: &[(&str, &[f32])]) -> Result<(), DbError> {
        for (id, emb) in items {
            self.insert_vector(id, emb)?;
        }
        Ok(())
    }
    fn search_knn(&self, query_vector: &[f32], limit: usize) -> Result<Vec<(String, f32)>, DbError>;
    fn delete_vector(&self, chunk_id: &str) -> Result<(), DbError>;
    fn count_vectors(&self) -> Result<usize, DbError>;
    fn vector_storage_bytes(&self) -> Result<u64, DbError>;
}

pub struct SqliteVectorIndex {
    db: Arc<DatabaseManager>,
}

impl SqliteVectorIndex {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
}

impl VectorIndex for SqliteVectorIndex {
    fn insert_vectors_batch(&self, items: &[(&str, &[f32])]) -> Result<(), DbError> {
        if items.is_empty() {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut conn = self.db.lock_conn()?;
        let tx = conn.transaction()?;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO chunk_vectors (chunk_id, dimensions, embedding, created_at)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(chunk_id) DO UPDATE SET
                    dimensions = excluded.dimensions,
                    embedding = excluded.embedding,
                    created_at = excluded.created_at
                "#,
            )?;

            for (chunk_id, embedding) in items {
                let mut bytes = Vec::with_capacity(embedding.len() * 4);
                for &val in *embedding {
                    bytes.extend_from_slice(&val.to_le_bytes());
                }
                stmt.execute(params![chunk_id, embedding.len() as i64, bytes, now])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn insert_vector(&self, chunk_id: &str, embedding: &[f32]) -> Result<(), DbError> {
        self.insert_vectors_batch(&[(chunk_id, embedding)])
    }

    fn search_knn(&self, query_vector: &[f32], limit: usize) -> Result<Vec<(String, f32)>, DbError> {
        if query_vector.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let query_norm_sq: f32 = query_vector.iter().map(|&x| x * x).sum();
        if query_norm_sq == 0.0 {
            return Ok(Vec::new());
        }
        let query_norm = query_norm_sq.sqrt();

        let conn = self.db.lock_conn()?;
        let mut stmt = conn.prepare("SELECT chunk_id, embedding FROM chunk_vectors")?;
        let mut rows = stmt.query([])?;
        let mut scored: Vec<(String, f32)> = Vec::new();

        let expected_bytes = query_vector.len() * 4;

        while let Some(row) = rows.next()? {
            let chunk_id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;

            if blob.len() != expected_bytes {
                continue; // Skip dimension mismatch
            }

            let mut dot = 0.0f32;
            let mut cand_norm_sq = 0.0f32;

            for (q, chunk) in query_vector.iter().zip(blob.chunks_exact(4)) {
                let val = f32::from_le_bytes(chunk.try_into().unwrap());
                dot += q * val;
                cand_norm_sq += val * val;
            }

            let cand_norm = cand_norm_sq.sqrt();
            let similarity = if cand_norm > 0.0 {
                dot / (query_norm * cand_norm)
            } else {
                0.0
            };

            scored.push((chunk_id, similarity));
        }

        // Sort descending by cosine similarity
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        Ok(scored)
    }

    fn delete_vector(&self, chunk_id: &str) -> Result<(), DbError> {
        let conn = self.db.lock_conn()?;
        conn.execute("DELETE FROM chunk_vectors WHERE chunk_id = ?1", params![chunk_id])?;
        Ok(())
    }

    fn count_vectors(&self) -> Result<usize, DbError> {
        let conn = self.db.lock_conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunk_vectors", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    fn vector_storage_bytes(&self) -> Result<u64, DbError> {
        let conn = self.db.lock_conn()?;
        let bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(embedding)), 0) FROM chunk_vectors",
            [],
            |r| r.get(0),
        )?;
        Ok(bytes as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactVersion, DocumentChunk};

    #[test]
    fn test_sqlite_vector_index_basic() {
        let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
        let index = SqliteVectorIndex::new(db.clone());

        // Setup prerequisite artifact, version, and chunks for foreign key integrity
        let path = "/test/file.txt";
        let artifact = db.upsert_artifact(path, "file").expect("upsert artifact");
        let v_id = uuid::Uuid::new_v4().to_string();
        let version = ArtifactVersion {
            id: v_id.clone(),
            artifact_id: artifact.id,
            path: path.to_string(),
            content_hash: "hash_1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: None,
            is_pinned: false,
        };
        db.record_version(&version).expect("record version");

        let chunks = vec![
            DocumentChunk {
                id: "c1".to_string(),
                version_id: v_id.clone(),
                chunk_index: 0,
                content: "v1".to_string(),
            },
            DocumentChunk {
                id: "c2".to_string(),
                version_id: v_id.clone(),
                chunk_index: 1,
                content: "v2".to_string(),
            },
            DocumentChunk {
                id: "c3".to_string(),
                version_id: v_id.clone(),
                chunk_index: 2,
                content: "v3".to_string(),
            },
        ];
        db.insert_chunks(&chunks).expect("insert chunks");

        assert_eq!(index.count_vectors().unwrap(), 0);
        assert_eq!(index.vector_storage_bytes().unwrap(), 0);

        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![0.7071, 0.7071, 0.0];

        index.insert_vector("c1", &v1).expect("insert c1");
        index.insert_vector("c2", &v2).expect("insert c2");
        index.insert_vector("c3", &v3).expect("insert c3");

        assert_eq!(index.count_vectors().unwrap(), 3);
        assert_eq!(index.vector_storage_bytes().unwrap(), 3 * 3 * 4); // 3 vectors * 3 floats * 4 bytes = 36 bytes

        // Query with v1: c1 should have similarity ~1.0, c3 ~0.7071, c2 ~0.0
        let results = index.search_knn(&v1, 10).expect("search knn");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "c1");
        assert!((results[0].1 - 1.0).abs() < 1e-4);
        assert_eq!(results[1].0, "c3");
        assert!((results[1].1 - 0.7071).abs() < 1e-3);
        assert_eq!(results[2].0, "c2");
        assert!((results[2].1 - 0.0).abs() < 1e-4);

        // Test limit
        let limited = index.search_knn(&v1, 2).expect("search knn limited");
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].0, "c1");
        assert_eq!(limited[1].0, "c3");

        // Delete c1
        index.delete_vector("c1").expect("delete c1");
        assert_eq!(index.count_vectors().unwrap(), 2);

        let results_after = index.search_knn(&v1, 10).expect("search after delete");
        assert_eq!(results_after.len(), 2);
        assert_eq!(results_after[0].0, "c3");
    }
}
