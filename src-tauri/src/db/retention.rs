use crate::db::{DatabaseManager, DbError};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub max_versions_per_artifact: usize,
    pub retention_days: Option<i64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_versions_per_artifact: 3,
            retention_days: None,
        }
    }
}

pub fn prune_unpinned_versions(
    db: &DatabaseManager,
    policy: &RetentionPolicy,
) -> Result<usize, DbError> {
    let mut conn = db.lock_conn()?;
    let tx = conn.transaction()?;

    let mut pruned_count = 0;
    let now = chrono::Utc::now().timestamp_millis();
    let cutoff_timestamp = policy.retention_days.map(|days| now - (days * 86_400 * 1000));

    // Get all distinct artifact IDs
    let artifact_ids: Vec<String> = {
        let mut stmt = tx.prepare("SELECT DISTINCT artifact_id FROM artifact_versions")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<Vec<String>, _>>()?
    };

    for artifact_id in artifact_ids {
        let versions: Vec<(String, i64, bool)> = {
            let mut get_versions_stmt = tx.prepare(
                "SELECT id, modified_at, is_pinned
                 FROM artifact_versions
                 WHERE artifact_id = ?1
                 ORDER BY modified_at DESC",
            )?;
            let rows = get_versions_stmt.query_map(params![&artifact_id], |r| {
                let is_pinned: i64 = r.get(2)?;
                Ok((r.get(0)?, r.get(1)?, is_pinned != 0))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        // Rule 1: Always preserve the latest version (index 0)
        // Rule 2: Always preserve pinned versions (is_pinned == true)
        // Rule 3: For older unpinned versions, delete if:
        //   - count of kept versions >= policy.max_versions_per_artifact OR
        //   - (if retention_days set) modified_at < cutoff
        let mut kept_unpinned = 0;
        for (idx, (version_id, modified_at, is_pinned)) in versions.into_iter().enumerate() {
            if idx == 0 {
                // Latest version is always preserved regardless of pin status
                continue;
            }
            if is_pinned {
                // Pinned versions are always preserved
                continue;
            }

            let exceeds_cap = kept_unpinned >= policy.max_versions_per_artifact.saturating_sub(1);
            let exceeds_age = match cutoff_timestamp {
                Some(cutoff) => modified_at < cutoff,
                None => false,
            };

            if exceeds_cap || exceeds_age {
                // Delete associated chunks (cascading into FTS5 via trigger)
                tx.execute("DELETE FROM chunks WHERE version_id = ?1", params![&version_id])?;
                // Delete version
                tx.execute(
                    "DELETE FROM artifact_versions WHERE id = ?1",
                    params![&version_id],
                )?;
                pruned_count += 1;
            } else {
                kept_unpinned += 1;
            }
        }
    }

    tx.commit()?;
    Ok(pruned_count)
}

pub fn pin_version(db: &DatabaseManager, version_id: &str, pinned: bool) -> Result<(), DbError> {
    let conn = db.lock_conn()?;
    conn.execute(
        "UPDATE artifact_versions SET is_pinned = ?1 WHERE id = ?2",
        params![if pinned { 1 } else { 0 }, version_id],
    )?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::models::{ArtifactVersion, DocumentChunk};

    #[test]
    fn test_retention_policy_pruning() {
        let db = DatabaseManager::new_in_memory().expect("in-memory db");
        let path = "/test/file.md";
        let artifact = db.upsert_artifact(path, "file").expect("upsert artifact");

        // Create 6 versions with increasing timestamps
        let mut version_ids = Vec::new();
        for i in 1..=6 {
            let vid = format!("v_{}", i);
            version_ids.push(vid.clone());
            let version = ArtifactVersion {
                id: vid.clone(),
                artifact_id: artifact.id.clone(),
                path: path.to_string(),
                content_hash: format!("hash_{}", i),
                text_hash: None,
                size_bytes: 100 * i as i64,
                modified_at: 1000 * i as i64, // v_6 is latest
                previous_version_id: None,
                extracted_text: Some(format!("Content of version {}", i)),
                is_pinned: false,
            };
            db.record_version(&version).expect("record version");

            db.insert_chunks(&[DocumentChunk {
                id: format!("c_{}", i),
                version_id: vid,
                chunk_index: 0,
                content: format!("Chunk content {}", i),
            }])
            .expect("insert chunk");
        }

        // Pin version 3 (an older version)
        pin_version(&db, "v_3", true).expect("pin version 3");

        // Prune with policy: max 3 versions
        let policy = RetentionPolicy {
            max_versions_per_artifact: 3,
            retention_days: None,
        };

        let pruned = prune_unpinned_versions(&db, &policy).expect("prune");
        // We had: v_6 (latest, kept), v_5 (kept), v_4 (kept), v_3 (pinned, kept), v_2 (pruned), v_1 (pruned) -> 2 pruned
        assert_eq!(pruned, 2);

        // Verify latest (v_6) is kept
        let latest = db.get_latest_version_by_path(path).unwrap().unwrap();
        assert_eq!(latest.id, "v_6");

        // Verify pinned (v_3) is still in the database
        let conn = db.lock_conn().unwrap();
        let v3_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_versions WHERE id = 'v_3'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        assert!(v3_exists, "Pinned version v_3 must be preserved");

        // Verify chunks for pruned version v_1 were removed
        let v1_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE version_id = 'v_1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v1_chunks, 0, "Chunks for pruned version must be removed");

        // Verify chunks_fts has no entries for v_1
        let v1_fts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE version_id = 'v_1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v1_fts, 0, "FTS entries for pruned version must be removed");
    }
}
