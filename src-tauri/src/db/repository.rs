use crate::db::schema;
use crate::models::{Artifact, ArtifactVersion, BlacklistFolder, DocumentChunk, Entity, FileEvent, Relationship, WatchedFolder};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Lock error: mutex poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone)]
pub struct IngestionBatchItem {
    pub artifact_path: String,
    pub artifact_kind: String,
    pub version: ArtifactVersion,
    pub chunks: Vec<DocumentChunk>,
    pub event: Option<FileEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStat {
    pub extension: String,
    pub file_count: usize,
    pub chunk_count: usize,
    pub total_bytes: u64,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStat {
    pub category: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub extensions: Vec<ExtensionStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeFolderResult {
    pub files_removed: usize,
    pub chunks_removed: usize,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusAnalytics {
    pub total_files: usize,
    pub total_bytes: u64,
    pub total_chunks: usize,
    pub categories: Vec<CategoryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub original_files_bytes: u64,
    pub sqlite_db_bytes: u64,
    pub text_chunks_bytes: u64,
    pub vector_index_bytes: u64,
}

pub fn categorize_file_extension(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "pdf" | "docx" | "doc" | "txt" | "md" | "rtf" | "pages" | "odt" | "tex" => "Documents",
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "h" | "hpp" | "go" | "java"
        | "rb" | "sh" | "zsh" | "swift" | "kt" | "html" | "css" | "scss" | "json" | "yaml"
        | "yml" | "toml" | "xml" | "sql" => "Code",
        "png" | "jpg" | "jpeg" | "tiff" | "webp" | "bmp" | "gif" | "heic" => "Images & OCR",
        "csv" | "tsv" | "xlsx" | "xls" | "parquet" | "jsonl" | "arrow" => "Data & Sheets",
        _ => "Others",
    }
}

#[derive(Clone)]
pub struct DatabaseManager {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseManager {
    pub fn new(db_path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn new_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DbError> {
        self.conn.lock().map_err(|_| DbError::LockPoisoned)
    }

    pub fn upsert_artifact(&self, path: &str, kind: &str) -> Result<Artifact, DbError> {
        let conn = self.lock_conn()?;
        
        // Check if an existing version has this path to get the artifact_id
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT artifact_id FROM artifact_versions WHERE path = ?1 LIMIT 1",
                params![path],
                |r| r.get(0),
            )
            .optional()?;

        let now = chrono::Utc::now().timestamp_millis();
        let artifact = match existing_id {
            Some(id) => {
                let created_at: i64 = conn.query_row(
                    "SELECT created_at FROM artifacts WHERE id = ?1",
                    params![&id],
                    |r| r.get(0),
                )?;
                Artifact {
                    id,
                    kind: kind.to_string(),
                    created_at,
                }
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO artifacts (id, kind, created_at) VALUES (?1, ?2, ?3)",
                    params![&id, kind, now],
                )?;
                Artifact {
                    id,
                    kind: kind.to_string(),
                    created_at: now,
                }
            }
        };

        Ok(artifact)
    }

    pub fn get_latest_version_by_path(&self, path: &str) -> Result<Option<ArtifactVersion>, DbError> {
        let conn = self.lock_conn()?;
        let version = conn
            .query_row(
                r#"
                SELECT id, artifact_id, path, content_hash, text_hash, size_bytes, modified_at, previous_version_id, extracted_text, is_pinned
                FROM artifact_versions
                WHERE path = ?1
                ORDER BY modified_at DESC
                LIMIT 1
                "#,
                params![path],
                |r| {
                    let is_pinned: i64 = r.get(9)?;
                    Ok(ArtifactVersion {
                        id: r.get(0)?,
                        artifact_id: r.get(1)?,
                        path: r.get(2)?,
                        content_hash: r.get(3)?,
                        text_hash: r.get(4)?,
                        size_bytes: r.get(5)?,
                        modified_at: r.get(6)?,
                        previous_version_id: r.get(7)?,
                        extracted_text: r.get(8)?,
                        is_pinned: is_pinned != 0,
                    })
                },
            )
            .optional()?;
        Ok(version)
    }

    pub fn record_version(&self, version: &ArtifactVersion) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            r#"
            INSERT INTO artifact_versions (
                id, artifact_id, path, content_hash, text_hash, size_bytes, modified_at, previous_version_id, extracted_text, is_pinned
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                &version.id,
                &version.artifact_id,
                &version.path,
                &version.content_hash,
                &version.text_hash,
                version.size_bytes,
                version.modified_at,
                &version.previous_version_id,
                &version.extracted_text,
                if version.is_pinned { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn get_artifact_versions_ordered(&self, artifact_id: &str) -> Result<Vec<ArtifactVersion>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, artifact_id, path, content_hash, text_hash, size_bytes, modified_at, previous_version_id, extracted_text, is_pinned
            FROM artifact_versions
            WHERE artifact_id = ?1
            ORDER BY modified_at ASC
            "#,
        )?;
        let rows = stmt.query_map(params![artifact_id], |r| {
            let is_pinned: i64 = r.get(9)?;
            Ok(ArtifactVersion {
                id: r.get(0)?,
                artifact_id: r.get(1)?,
                path: r.get(2)?,
                content_hash: r.get(3)?,
                text_hash: r.get(4)?,
                size_bytes: r.get(5)?,
                modified_at: r.get(6)?,
                previous_version_id: r.get(7)?,
                extracted_text: r.get(8)?,
                is_pinned: is_pinned != 0,
            })
        })?;

        let mut versions = Vec::new();
        for v in rows {
            versions.push(v?);
        }
        Ok(versions)
    }

    pub fn get_version_by_id(&self, version_id: &str) -> Result<Option<ArtifactVersion>, DbError> {
        let conn = self.lock_conn()?;
        let version = conn
            .query_row(
                r#"
                SELECT id, artifact_id, path, content_hash, text_hash, size_bytes, modified_at, previous_version_id, extracted_text, is_pinned
                FROM artifact_versions
                WHERE id = ?1
                "#,
                params![version_id],
                |r| {
                    let is_pinned: i64 = r.get(9)?;
                    Ok(ArtifactVersion {
                        id: r.get(0)?,
                        artifact_id: r.get(1)?,
                        path: r.get(2)?,
                        content_hash: r.get(3)?,
                        text_hash: r.get(4)?,
                        size_bytes: r.get(5)?,
                        modified_at: r.get(6)?,
                        previous_version_id: r.get(7)?,
                        extracted_text: r.get(8)?,
                        is_pinned: is_pinned != 0,
                    })
                },
            )
            .optional()?;
        Ok(version)
    }

    pub fn insert_chunks(&self, chunks: &[DocumentChunk]) -> Result<(), DbError> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO chunks (id, version_id, chunk_index, content) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for chunk in chunks {
                stmt.execute(params![
                    &chunk.id,
                    &chunk.version_id,
                    chunk.chunk_index,
                    &chunk.content,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn persist_ingestion_batch(&self, items: &[IngestionBatchItem]) -> Result<(), DbError> {
        if items.is_empty() {
            return Ok(());
        }

        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().timestamp_millis();

        for item in items {
            // 1. Resolve or create artifact
            let existing_id: Option<String> = tx
                .query_row(
                    "SELECT artifact_id FROM artifact_versions WHERE path = ?1 LIMIT 1",
                    params![&item.artifact_path],
                    |r| r.get(0),
                )
                .optional()?;

            let artifact_id = match existing_id {
                Some(id) => id,
                None => {
                    let id = item.version.artifact_id.clone();
                    tx.execute(
                        "INSERT OR IGNORE INTO artifacts (id, kind, created_at) VALUES (?1, ?2, ?3)",
                        params![&id, &item.artifact_kind, now],
                    )?;
                    id
                }
            };

            // 2. Insert artifact version
            tx.execute(
                r#"
                INSERT INTO artifact_versions (
                    id, artifact_id, path, content_hash, text_hash, size_bytes, modified_at, previous_version_id, extracted_text, is_pinned
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    &item.version.id,
                    &artifact_id,
                    &item.version.path,
                    &item.version.content_hash,
                    &item.version.text_hash,
                    item.version.size_bytes,
                    item.version.modified_at,
                    &item.version.previous_version_id,
                    &item.version.extracted_text,
                    if item.version.is_pinned { 1 } else { 0 },
                ],
            )?;

            // 3. Insert chunks into chunks (triggers FTS5 insert trigger)
            if !item.chunks.is_empty() {
                let mut chunk_stmt = tx.prepare(
                    "INSERT INTO chunks (id, version_id, chunk_index, content) VALUES (?1, ?2, ?3, ?4)",
                )?;
                for chunk in &item.chunks {
                    chunk_stmt.execute(params![
                        &chunk.id,
                        &item.version.id,
                        chunk.chunk_index,
                        &chunk.content,
                    ])?;
                }
            }

            // 4. Record event
            if let Some(ref event) = item.event {
                let metadata_str = event
                    .metadata
                    .as_ref()
                    .map(|m| serde_json::to_string(m))
                    .transpose()?;

                tx.execute(
                    "INSERT INTO events (id, artifact_id, event_type, timestamp, metadata) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        &event.id,
                        &artifact_id,
                        &event.event_type,
                        event.timestamp,
                        metadata_str,
                    ],
                )?;
            }

            // Automatically resolve and remove from Dead Letter Queue upon successful batch ingestion
            let _ = tx.execute(
                "DELETE FROM dead_letter_queue WHERE path = ?1",
                params![&item.artifact_path],
            );
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_chunks_for_version(&self, version_id: &str) -> Result<Vec<DocumentChunk>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, version_id, chunk_index, content FROM chunks WHERE version_id = ?1 ORDER BY chunk_index ASC",
        )?;
        let chunk_iter = stmt.query_map(params![version_id], |r| {
            Ok(DocumentChunk {
                id: r.get(0)?,
                version_id: r.get(1)?,
                chunk_index: r.get(2)?,
                content: r.get(3)?,
            })
        })?;

        let mut chunks = Vec::new();
        for chunk in chunk_iter {
            chunks.push(chunk?);
        }
        Ok(chunks)
    }

    pub fn record_event(&self, event: &FileEvent) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        let metadata_str = event
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        conn.execute(
            "INSERT INTO events (id, artifact_id, event_type, timestamp, metadata) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &event.id,
                &event.artifact_id,
                &event.event_type,
                event.timestamp,
                metadata_str,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_entity(&self, entity: &Entity) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO entities (id, name, entity_type) VALUES (?1, ?2, ?3)",
            params![&entity.id, &entity.name, &entity.entity_type],
        )?;
        Ok(())
    }

    pub fn record_relationship(&self, rel: &Relationship) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO relationships (source_id, target_id, relationship_type, weight)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![&rel.source_id, &rel.target_id, &rel.relationship_type, rel.weight],
        )?;
        Ok(())
    }

    pub fn add_watched_folder(&self, folder: &WatchedFolder) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO watched_folders (id, path, enabled, recursive, added_at, last_scanned_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                &folder.id,
                &folder.path,
                if folder.enabled { 1 } else { 0 },
                if folder.recursive { 1 } else { 0 },
                folder.added_at,
                folder.last_scanned_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_watched_folders(&self) -> Result<Vec<WatchedFolder>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, enabled, recursive, added_at, last_scanned_at FROM watched_folders ORDER BY added_at ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            let enabled_int: i32 = row.get(2)?;
            let recursive_int: i32 = row.get(3)?;
            Ok(WatchedFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                enabled: enabled_int != 0,
                recursive: recursive_int != 0,
                added_at: row.get(4)?,
                last_scanned_at: row.get(5)?,
            })
        })?;

        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn get_enabled_watched_folders(&self) -> Result<Vec<WatchedFolder>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, enabled, recursive, added_at, last_scanned_at FROM watched_folders WHERE enabled = 1 ORDER BY added_at ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            let enabled_int: i32 = row.get(2)?;
            let recursive_int: i32 = row.get(3)?;
            Ok(WatchedFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                enabled: enabled_int != 0,
                recursive: recursive_int != 0,
                added_at: row.get(4)?,
                last_scanned_at: row.get(5)?,
            })
        })?;

        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn update_watched_folder_enabled(&self, id: &str, enabled: bool) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE watched_folders SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    pub fn remove_watched_folder(&self, id: &str) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM watched_folders WHERE id = ?1", params![id])?;
        Ok(())
    }
    // ── Delete queue helpers ──────────────────────────────────────────────────

    /// Count distinct files indexed under a path prefix (for progress bar total).
    pub fn count_files_for_path(&self, folder_path: &str) -> Result<usize, DbError> {
        let clean = folder_path.trim_end_matches('/');
        let prefix = format!("{}/%", clean);
        let conn = self.lock_conn()?;
        let count: usize = conn.query_row(
            "SELECT COUNT(DISTINCT path) FROM artifact_versions WHERE path = ?1 OR path LIKE ?2",
            rusqlite::params![clean, prefix],
            |r| r.get(0),
        ).unwrap_or(0);
        Ok(count)
    }

    /// List all indexed file paths under a folder prefix.
    pub fn list_files_for_path(&self, folder_path: &str) -> Result<Vec<String>, DbError> {
        let clean = folder_path.trim_end_matches('/');
        let prefix = format!("{}/%", clean);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT path FROM artifact_versions WHERE path = ?1 OR path LIKE ?2 ORDER BY path",
        )?;
        let rows = stmt.query_map(rusqlite::params![clean, prefix], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count all indexed files across all folders.
    pub fn count_all_indexed_files(&self) -> Result<usize, DbError> {
        let conn = self.lock_conn()?;
        let count: usize = conn.query_row(
            "SELECT COUNT(DISTINCT path) FROM artifact_versions",
            [],
            |r| r.get(0),
        ).unwrap_or(0);
        Ok(count)
    }

    /// List all indexed file paths across the entire database.
    pub fn list_all_indexed_files(&self) -> Result<Vec<String>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT path FROM artifact_versions ORDER BY path",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete all index data for a single file path WITHOUT running VACUUM.
    /// Call `vacuum()` once when the batch is complete.
    pub fn purge_single_file_index(&self, file_path: &str) -> Result<(), DbError> {
        self.purge_files_batch(&[file_path.to_string()])
    }

    /// Current SQLite database size on disk via page count * page size (<0.1ms).
    pub fn get_sqlite_size_bytes(&self) -> u64 {
        if let Ok(conn) = self.lock_conn() {
            crate::benchmark::telemetry::get_sqlite_disk_bytes(&conn)
        } else {
            0
        }
    }

    /// Fast batch purge for multiple file paths in a single transaction.
    /// Temporarily disables the FTS delete trigger to avoid full-table scans of unindexed chunk_ids.
    pub fn purge_files_batch(&self, file_paths: &[String]) -> Result<(), DbError> {
        if file_paths.is_empty() {
            return Ok(());
        }
        let start = std::time::Instant::now();
        tracing::debug!("purge_files_batch: Purging batch of {} files", file_paths.len());
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        // Temporarily drop trg_chunks_fts_delete so deleting artifact_versions -> chunks
        // does NOT execute an O(N) full-table scan on chunks_fts(chunk_id) for every single chunk deleted.
        let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_delete", []);

        let placeholders = file_paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let params: Vec<&dyn rusqlite::ToSql> = file_paths.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        // 1. Fast direct FTS delete via indexed path column in a single batch scan
        let fts_query = format!("DELETE FROM chunks_fts WHERE path IN ({})", placeholders);
        let _ = tx.execute(&fts_query, params.as_slice());

        // 2. Cascade delete artifact_versions -> chunks -> chunk_vectors
        let ver_query = format!("DELETE FROM artifact_versions WHERE path IN ({})", placeholders);
        let _ = tx.execute(&ver_query, params.as_slice());

        // 3. Clean DLQ entries
        let dlq_query = format!("DELETE FROM dead_letter_queue WHERE path IN ({})", placeholders);
        let _ = tx.execute(&dlq_query, params.as_slice());

        // Clean up orphan artifacts that have no remaining versions
        let _ = tx.execute(
            "DELETE FROM artifacts WHERE id NOT IN (SELECT DISTINCT artifact_id FROM artifact_versions)",
            [],
        );

        // Recreate the FTS delete trigger
        let _ = tx.execute(
            r#"
            CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_delete AFTER DELETE ON chunks
            BEGIN
                DELETE FROM chunks_fts WHERE chunk_id = old.id;
            END;
            "#,
            [],
        );

        tx.commit()?;
        tracing::debug!(
            "purge_files_batch: Finished purging {} files in {:?}",
            file_paths.len(),
            start.elapsed()
        );
        Ok(())
    }

    /// Run VACUUM then WAL checkpoint(TRUNCATE) to fully reclaim disk space after a batch purge.
    /// VACUUM compacts in-file free pages; WAL TRUNCATE removes the WAL file so the .db
    /// file size on disk matches what PRAGMA page_count reports (visible in telemetry header).
    pub fn vacuum(&self) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("VACUUM", [])?;
        // Truncate WAL so the physical file shrinks immediately
        let _: i64 = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |r| r.get(0))
            .unwrap_or(0);
        Ok(())
    }

    /// Fast full-wipe used after per-file batch deletes are complete.
    /// Drops FTS tables/triggers, rebuilds them clean, then VACUUMs.
    pub fn purge_all_index_data_fast(&self) -> Result<(), DbError> {
        tracing::info!("purge_all_index_data_fast: Starting fast purge of all index data");
        {
            let mut conn = self.lock_conn()?;
            let tx = conn.transaction()?;
            tracing::info!("purge_all_index_data_fast: Dropping FTS triggers & clearing chunks_fts");
            let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_insert", []);
            let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_delete", []);
            let _ = tx.execute("DELETE FROM chunks_fts", []);
            let _ = tx.execute("UPDATE watched_folders SET last_scanned_at = NULL", []);
            tx.commit()?;
            tracing::info!("purge_all_index_data_fast: Re-applying migrations / triggers");
            crate::db::schema::migrate(&conn)?;
        } // conn guard released here
        // Now re-acquire for VACUUM (cannot VACUUM inside a transaction)
        tracing::info!("purge_all_index_data_fast: Running VACUUM to reclaim disk space");
        let conn = self.lock_conn()?;
        let _ = conn.execute("VACUUM", []);
        tracing::info!("purge_all_index_data_fast: VACUUM completed successfully");
        Ok(())
    }

    pub fn purge_folder_data(&self, folder_path: &str) -> Result<PurgeFolderResult, DbError> {
        self.purge_folder_data_with_exceptions(folder_path, &[])
    }

    pub fn purge_folder_data_with_exceptions(
        &self,
        folder_path: &str,
        exceptions: &[String],
    ) -> Result<PurgeFolderResult, DbError> {
        let clean_path = folder_path.trim_end_matches('/');
        let exact_path = clean_path.to_string();
        let prefix_pattern = format!("{}/%", clean_path);

        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        // If exceptions are provided, filter candidate paths
        let paths_to_purge: Vec<String> = if exceptions.is_empty() {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT path FROM artifact_versions WHERE path = ?1 OR path LIKE ?2",
            )?;
            let rows = stmt.query_map(params![exact_path, prefix_pattern], |r| r.get(0))?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT path FROM artifact_versions WHERE path = ?1 OR path LIKE ?2",
            )?;
            let rows = stmt.query_map(params![exact_path, prefix_pattern], |r| r.get(0))?;
            rows.filter_map(|r| r.ok())
                .filter(|p| {
                    let p_path = Path::new(p);
                    !exceptions.iter().any(|exc| {
                        crate::safety::filters::is_path_inside_or_equal(p_path, Path::new(exc))
                    })
                })
                .collect()
        };

        if paths_to_purge.is_empty() {
            // Also clean any dead letter entries matching exact or prefix that aren't inside exceptions
            if exceptions.is_empty() {
                let _ = tx.execute(
                    "DELETE FROM dead_letter_queue WHERE path = ?1 OR path LIKE ?2",
                    params![exact_path, prefix_pattern],
                );
            }
            tx.commit()?;
            return Ok(PurgeFolderResult {
                files_removed: 0,
                chunks_removed: 0,
                path: folder_path.to_string(),
            });
        }

        let files_count = paths_to_purge.len();

        tx.execute(
            "CREATE TEMP TABLE IF NOT EXISTS _purge_paths (path TEXT PRIMARY KEY)",
            [],
        )?;
        tx.execute("DELETE FROM _purge_paths", [])?;
        {
            let mut insert_stmt = tx.prepare("INSERT INTO _purge_paths (path) VALUES (?1)")?;
            for p in &paths_to_purge {
                let _ = insert_stmt.execute(params![p]);
            }
        }

        let chunks_count: usize = tx
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE version_id IN (SELECT av.id FROM artifact_versions av JOIN _purge_paths pp ON av.path = pp.path)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_insert", []);
        let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_delete", []);

        let _ = tx.execute(
            "DELETE FROM chunk_vectors WHERE chunk_id IN (SELECT c.id FROM chunks c JOIN artifact_versions av ON c.version_id = av.id JOIN _purge_paths pp ON av.path = pp.path)",
            [],
        );

        let _ = tx.execute(
            "DELETE FROM chunks WHERE version_id IN (SELECT av.id FROM artifact_versions av JOIN _purge_paths pp ON av.path = pp.path)",
            [],
        );

        let _ = tx.execute(
            "DELETE FROM chunks_fts WHERE path IN (SELECT path FROM _purge_paths)",
            [],
        );

        let _ = tx.execute(
            "DELETE FROM artifacts WHERE id IN (SELECT DISTINCT av.artifact_id FROM artifact_versions av JOIN _purge_paths pp ON av.path = pp.path)",
            [],
        );

        let _ = tx.execute(
            "DELETE FROM artifact_versions WHERE path IN (SELECT path FROM _purge_paths)",
            [],
        );

        let _ = tx.execute(
            "DELETE FROM dead_letter_queue WHERE path IN (SELECT path FROM _purge_paths)",
            [],
        );

        let _ = tx.execute(
            "UPDATE watched_folders SET last_scanned_at = NULL WHERE path = ?1 OR path LIKE ?2",
            params![exact_path, prefix_pattern],
        );

        let _ = tx.execute("DROP TABLE IF EXISTS _purge_paths", []);

        tx.commit()?;
        crate::db::schema::migrate(&conn)?;
        let _ = conn.execute("VACUUM", []);

        tracing::info!(
            "purge_folder_data: Purged {} files, {} chunks for path '{}'",
            files_count,
            chunks_count,
            folder_path
        );

        Ok(PurgeFolderResult {
            files_removed: files_count,
            chunks_removed: chunks_count,
            path: folder_path.to_string(),
        })
    }

    pub fn purge_all_index_data(&self) -> Result<PurgeFolderResult, DbError> {
        tracing::info!("purge_all_index_data: Attempting to lock connection");
        let mut conn = self.lock_conn()?;
        tracing::info!("purge_all_index_data: Connection locked. Starting transaction.");
        let tx = conn.transaction()?;

        tracing::info!("purge_all_index_data: Counting files...");
        let files_count: usize = tx
            .query_row(
                "SELECT COUNT(DISTINCT path) FROM artifact_versions",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        tracing::info!("purge_all_index_data: Counting chunks...");
        let chunks_count: usize = tx
            .query_row(
                "SELECT COUNT(*) FROM chunks",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        tracing::info!("purge_all_index_data: Dropping FTS5 triggers to prevent O(N^2) deletion...");
        let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_insert", []);
        let _ = tx.execute("DROP TRIGGER IF EXISTS trg_chunks_fts_delete", []);

        tracing::info!("purge_all_index_data: Deleting from chunk_vectors...");
        let _ = tx.execute("DELETE FROM chunk_vectors", []);
        tracing::info!("purge_all_index_data: Deleting from chunks...");
        let _ = tx.execute("DELETE FROM chunks", []);
        tracing::info!("purge_all_index_data: Deleting from chunks_fts...");
        let _ = tx.execute("DELETE FROM chunks_fts", []);
        tracing::info!("purge_all_index_data: Deleting from artifact_versions...");
        let _ = tx.execute("DELETE FROM artifact_versions", []);
        tracing::info!("purge_all_index_data: Deleting from artifacts...");
        let _ = tx.execute("DELETE FROM artifacts", []);
        tracing::info!("purge_all_index_data: Deleting from events...");
        let _ = tx.execute("DELETE FROM events", []);
        tracing::info!("purge_all_index_data: Deleting from entities...");
        let _ = tx.execute("DELETE FROM entities", []);
        tracing::info!("purge_all_index_data: Deleting from relationships...");
        let _ = tx.execute("DELETE FROM relationships", []);
        tracing::info!("purge_all_index_data: Updating watched_folders...");
        let _ = tx.execute("UPDATE watched_folders SET last_scanned_at = NULL", []);

        tracing::info!("purge_all_index_data: Committing transaction...");
        tx.commit()?;

        tracing::info!("purge_all_index_data: Recreating FTS5 triggers...");
        crate::db::schema::migrate(&conn)?;

        tracing::info!("purge_all_index_data: Transaction committed. Executing VACUUM...");
        // Free up unused database pages
        let _ = conn.execute("VACUUM", []);
        tracing::info!("purge_all_index_data: VACUUM complete.");

        Ok(PurgeFolderResult {
            files_removed: files_count,
            chunks_removed: chunks_count,
            path: "ALL".to_string(),
        })
    }

    pub fn update_folder_last_scanned(&self, id: &str, timestamp: i64) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE watched_folders SET last_scanned_at = ?1 WHERE id = ?2",
            params![timestamp, id],
        )?;
        Ok(())
    }

    pub fn update_watched_folder_last_scanned(&self, path: &str, timestamp: i64) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE watched_folders SET last_scanned_at = ?1 WHERE path = ?2",
            params![timestamp, path],
        )?;
        Ok(())
    }

    pub fn get_blacklist_folders(&self) -> Result<Vec<BlacklistFolder>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT id, path, created_at FROM blacklist_folders ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(BlacklistFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;

        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn add_blacklist_folder(&self, path: &str) -> Result<BlacklistFolder, DbError> {
        let record = {
            let conn = self.lock_conn()?;
            let id = uuid::Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now().timestamp();
            conn.execute(
                r#"
                INSERT INTO blacklist_folders (id, path, created_at)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(path) DO NOTHING
                "#,
                params![id, path, created_at],
            )?;

            conn.query_row(
                "SELECT id, path, created_at FROM blacklist_folders WHERE path = ?1",
                params![path],
                |r| {
                    Ok(BlacklistFolder {
                        id: r.get(0)?,
                        path: r.get(1)?,
                        created_at: r.get(2)?,
                    })
                },
            )?
        };

        // Automatically purge any previously indexed data under this blacklisted directory,
        // preserving any watched folder exceptions nested inside it.
        let watched_folders = self.get_watched_folders().unwrap_or_default();
        let blacklisted_path = Path::new(path);
        let exception_paths: Vec<String> = watched_folders
            .into_iter()
            .filter(|w| {
                let wp = Path::new(&w.path);
                wp != blacklisted_path && crate::safety::filters::is_path_inside_or_equal(wp, blacklisted_path)
            })
            .map(|w| w.path)
            .collect();

        if let Err(e) = self.purge_folder_data_with_exceptions(path, &exception_paths) {
            tracing::warn!(
                "add_blacklist_folder: error auto-purging index for blacklisted path {}: {}",
                path,
                e
            );
        }

        Ok(record)
    }

    pub fn remove_blacklist_folder(&self, id: &str) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM blacklist_folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn is_folder_blacklisted(&self, path: &str) -> Result<bool, DbError> {
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM blacklist_folders WHERE path = ?1",
            params![path],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_corpus_analytics(&self) -> Result<CorpusAnalytics, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            WITH latest_versions AS (
                SELECT id, path, size_bytes,
                       ROW_NUMBER() OVER (PARTITION BY path ORDER BY modified_at DESC) as rn
                FROM artifact_versions
            )
            SELECT lv.path, lv.size_bytes, COUNT(c.id) as chunk_count
            FROM latest_versions lv
            LEFT JOIN chunks c ON c.version_id = lv.id
            WHERE lv.rn = 1
            GROUP BY lv.id;
            "#
        )?;

        struct RawRow {
            path: String,
            size_bytes: i64,
            chunk_count: i64,
        }

        let rows = stmt.query_map([], |r| {
            Ok(RawRow {
                path: r.get(0)?,
                size_bytes: r.get(1)?,
                chunk_count: r.get(2)?,
            })
        })?;

        use std::collections::HashMap;
        let mut ext_map: HashMap<(String, String), (usize, usize, u64)> = HashMap::new();
        let mut total_files = 0usize;
        let mut total_bytes = 0u64;
        let mut total_chunks = 0usize;

        for r in rows {
            let row = r?;
            total_files += 1;
            let size = row.size_bytes.max(0) as u64;
            let chunks = row.chunk_count.max(0) as usize;
            total_bytes += size;
            total_chunks += chunks;

            let ext = std::path::Path::new(&row.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("no-ext")
                .to_lowercase();
            let cat = categorize_file_extension(&ext).to_string();

            let entry = ext_map.entry((cat, ext)).or_insert((0, 0, 0));
            entry.0 += 1;
            entry.1 += chunks;
            entry.2 += size;
        }

        let mut cat_map: HashMap<String, (usize, u64, Vec<ExtensionStat>)> = HashMap::new();
        for ((cat, ext), (count, chunks, bytes)) in ext_map {
            let percentage = if total_bytes > 0 {
                (bytes as f32 / total_bytes as f32) * 100.0
            } else if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };

            let entry = cat_map.entry(cat).or_insert((0, 0, Vec::new()));
            entry.0 += count;
            entry.1 += bytes;
            entry.2.push(ExtensionStat {
                extension: ext,
                file_count: count,
                chunk_count: chunks,
                total_bytes: bytes,
                percentage,
            });
        }

        let cat_order = ["Documents", "Code", "Images & OCR", "Data & Sheets", "Others"];
        let mut categories = Vec::new();

        for cat_name in &cat_order {
            if let Some((f_count, b_count, mut exts)) = cat_map.remove(*cat_name) {
                exts.sort_by(|a, b| b.file_count.cmp(&a.file_count).then_with(|| b.total_bytes.cmp(&a.total_bytes)));
                categories.push(CategoryStat {
                    category: cat_name.to_string(),
                    file_count: f_count,
                    total_bytes: b_count,
                    extensions: exts,
                });
            }
        }

        for (cat_name, (f_count, b_count, mut exts)) in cat_map {
            exts.sort_by(|a, b| b.file_count.cmp(&a.file_count));
            categories.push(CategoryStat {
                category: cat_name,
                file_count: f_count,
                total_bytes: b_count,
                extensions: exts,
            });
        }

        Ok(CorpusAnalytics {
            total_files,
            total_bytes,
            total_chunks,
            categories,
        })
    }

    pub fn get_storage_breakdown(&self, db_path: Option<&std::path::Path>) -> Result<StorageBreakdown, DbError> {
        let conn = self.lock_conn()?;
        let original_files_bytes: i64 = conn.query_row(
            r#"
            SELECT COALESCE(SUM(size_bytes), 0) FROM (
                SELECT size_bytes, ROW_NUMBER() OVER (PARTITION BY path ORDER BY modified_at DESC) rn
                FROM artifact_versions
            ) WHERE rn = 1;
            "#,
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let text_chunks_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chunks;",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let vector_index_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(embedding)), 0) FROM chunk_vectors;",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let sqlite_db_bytes = if let Some(p) = db_path {
            std::fs::metadata(p).map(|m| m.len()).unwrap_or_else(|_| {
                crate::benchmark::telemetry::get_sqlite_disk_bytes(&conn)
            })
        } else {
            crate::benchmark::telemetry::get_sqlite_disk_bytes(&conn)
        };

        Ok(StorageBreakdown {
            original_files_bytes: original_files_bytes.max(0) as u64,
            sqlite_db_bytes,
            text_chunks_bytes: text_chunks_bytes.max(0) as u64,
            vector_index_bytes: vector_index_bytes.max(0) as u64,
        })
    }

    pub fn get_app_settings(&self) -> Result<crate::models::AppSettings, DbError> {
        let conn = self.lock_conn()?;
        let value_json: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'global'",
                [],
                |r| r.get(0),
            )
            .optional()?;

        match value_json {
            Some(json_str) => {
                let settings = serde_json::from_str(&json_str)?;
                Ok(settings)
            }
            None => Ok(crate::models::AppSettings::default()),
        }
    }

    pub fn save_app_settings(&self, settings: &crate::models::AppSettings) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        let json_str = serde_json::to_string(settings)?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('global', ?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
            params![json_str, now],
        )?;
        Ok(())
    }

    pub fn get_window_position(&self) -> Result<Option<(i32, i32)>, DbError> {
        let conn = self.lock_conn()?;
        let value_json: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'window_position'",
                [],
                |r| r.get(0),
            )
            .optional()?;

        match value_json {
            Some(json_str) => {
                let pos: (i32, i32) = serde_json::from_str(&json_str)?;
                Ok(Some(pos))
            }
            None => Ok(None),
        }
    }

    pub fn save_window_position(&self, x: i32, y: i32) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        let json_str = serde_json::to_string(&(x, y))?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('window_position', ?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
            params![json_str, now],
        )?;
        Ok(())
    }

    // Dead Letter Queue operations
    pub fn insert_or_update_dead_letter(&self, item: &crate::models::DeadLetterItem) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute(
            r#"
            INSERT INTO dead_letter_queue (id, path, file_name, file_size, error_type, error_message, failed_at, retry_count, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(path) DO UPDATE SET
                file_size = excluded.file_size,
                error_type = excluded.error_type,
                error_message = excluded.error_message,
                failed_at = excluded.failed_at,
                retry_count = dead_letter_queue.retry_count + 1,
                status = excluded.status
            "#,
            params![
                &item.id,
                &item.path,
                &item.file_name,
                item.file_size as i64,
                &item.error_type,
                &item.error_message,
                item.failed_at,
                item.retry_count as i64,
                &item.status,
            ],
        )?;
        Ok(())
    }

    pub fn get_dead_letter_items(&self) -> Result<Vec<crate::models::DeadLetterItem>, DbError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, file_name, file_size, error_type, error_message, failed_at, retry_count, status FROM dead_letter_queue ORDER BY failed_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let file_size: i64 = r.get(3)?;
            let retry_count: i64 = r.get(7)?;
            Ok(crate::models::DeadLetterItem {
                id: r.get(0)?,
                path: r.get(1)?,
                file_name: r.get(2)?,
                file_size: file_size.max(0) as u64,
                error_type: r.get(4)?,
                error_message: r.get(5)?,
                failed_at: r.get(6)?,
                retry_count: retry_count.max(0) as u32,
                status: r.get(8)?,
            })
        })?;
        let mut items = Vec::new();
        for item in rows {
            items.push(item?);
        }
        Ok(items)
    }

    pub fn remove_dead_letter_by_id(&self, id: &str) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM dead_letter_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn remove_dead_letter_by_path(&self, path: &str) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM dead_letter_queue WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn clear_dead_letter_queue(&self) -> Result<(), DbError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM dead_letter_queue", [])?;
        Ok(())
    }

    pub fn get_dead_letter_count(&self) -> Result<usize, DbError> {
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM dead_letter_queue", [], |r| r.get(0))?;
        Ok(count.max(0) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watched_folders_crud() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        let folder = WatchedFolder {
            id: "folder-123".to_string(),
            path: "/Users/test/Documents".to_string(),
            enabled: true,
            recursive: true,
            added_at: 1000,
            last_scanned_at: None,
        };

        // 1. Add watched folder
        db.add_watched_folder(&folder).expect("add watched folder");

        // 2. Query back
        let folders = db.get_watched_folders().expect("get folders");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].path, "/Users/test/Documents");
        assert!(folders[0].enabled);

        // 3. Toggle enabled
        db.update_watched_folder_enabled("folder-123", false).expect("toggle enabled");
        let enabled_folders = db.get_enabled_watched_folders().expect("get enabled");
        assert!(enabled_folders.is_empty());

        // 4. Update last scanned
        db.update_folder_last_scanned("folder-123", 2000).expect("update scan time");
        let updated = db.get_watched_folders().expect("get folders");
        assert_eq!(updated[0].last_scanned_at, Some(2000));

        // 5. Remove folder
        db.remove_watched_folder("folder-123").expect("remove folder");
        let remaining = db.get_watched_folders().expect("get folders");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_database_manager_crud() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        // 1. Create artifact
        let path = "/Users/test/Documents/invoice.pdf";
        let artifact = db.upsert_artifact(path, "file").expect("upsert artifact");
        assert_eq!(artifact.kind, "file");

        // 2. Add version 1
        let v1_id = uuid::Uuid::new_v4().to_string();
        let version1 = ArtifactVersion {
            id: v1_id.clone(),
            artifact_id: artifact.id.clone(),
            path: path.to_string(),
            content_hash: "hash_v1".to_string(),
            text_hash: Some("text_hash_v1".to_string()),
            size_bytes: 1024,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Invoice total: $500".to_string()),
            is_pinned: false,
        };
        db.record_version(&version1).expect("record v1");

        // 3. Query back latest version
        let latest = db.get_latest_version_by_path(path).expect("get latest").expect("should exist");
        assert_eq!(latest.id, v1_id);
        assert_eq!(latest.content_hash, "hash_v1");

        // 4. Insert chunks
        let chunks = vec![
            DocumentChunk {
                id: "c1".to_string(),
                version_id: v1_id.clone(),
                chunk_index: 0,
                content: "Invoice total".to_string(),
            },
            DocumentChunk {
                id: "c2".to_string(),
                version_id: v1_id.clone(),
                chunk_index: 1,
                content: "$500 due on Monday".to_string(),
            },
        ];
        db.insert_chunks(&chunks).expect("insert chunks");

        let queried_chunks = db.get_chunks_for_version(&v1_id).expect("query chunks");
        assert_eq!(queried_chunks.len(), 2);
        assert_eq!(queried_chunks[0].content, "Invoice total");

        // 5. Log file event
        let event = FileEvent {
            id: uuid::Uuid::new_v4().to_string(),
            artifact_id: Some(artifact.id.clone()),
            event_type: "created".to_string(),
            timestamp: 1000,
            metadata: Some(serde_json::json!({ "source": "filesystem_watcher" })),
        };
        db.record_event(&event).expect("record event");
    }

    #[test]
    fn test_settings_persistence_and_defaults() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        // 1. Should return default settings initially
        let defaults = db.get_app_settings().expect("get default settings");
        assert_eq!(defaults.preset, crate::models::SettingPreset::Balanced);
        assert_eq!(defaults.max_file_size_mb, 50);
        assert_eq!(defaults.embedding_model, "AllMiniLML6V2");
        assert_eq!(defaults.max_vector_chunks, 250_000);
        assert_eq!(defaults.max_indexed_files, 20_000);

        // 2. Modify and save settings
        let mut modified = defaults.clone();
        modified.preset = crate::models::SettingPreset::Power;
        modified.max_file_size_mb = 100;
        modified.embedding_model = "nomic-embed-text".to_string();
        db.save_app_settings(&modified).expect("save app settings");

        // 3. Read back and verify persistence
        let loaded = db.get_app_settings().expect("get saved settings");
        assert_eq!(loaded.preset, crate::models::SettingPreset::Power);
        assert_eq!(loaded.max_file_size_mb, 100);
        assert_eq!(loaded.embedding_model, "nomic-embed-text");
    }

    #[test]
    fn test_dead_letter_queue_crud() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        // 1. Initially empty
        assert_eq!(db.get_dead_letter_count().unwrap(), 0);
        assert!(db.get_dead_letter_items().unwrap().is_empty());

        // 2. Insert failed item
        let item1 = crate::models::DeadLetterItem {
            id: "dlq-1".to_string(),
            path: "/path/to/broken.pdf".to_string(),
            file_name: "broken.pdf".to_string(),
            file_size: 1024,
            error_type: "Extraction".to_string(),
            error_message: "Failed to extract text from PDF: PDF error: unsupported key length".to_string(),
            failed_at: 1000,
            retry_count: 1,
            status: "pending".to_string(),
        };
        db.insert_or_update_dead_letter(&item1).expect("insert dlq");
        assert_eq!(db.get_dead_letter_count().unwrap(), 1);

        let items = db.get_dead_letter_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name, "broken.pdf");
        assert_eq!(items[0].retry_count, 1);
        assert_eq!(items[0].error_type, "Extraction");

        // 3. Upsert on same path (e.g. subsequent timeout failure)
        let item1_retry = crate::models::DeadLetterItem {
            id: "dlq-1-retry".to_string(),
            path: "/path/to/broken.pdf".to_string(),
            file_name: "broken.pdf".to_string(),
            file_size: 1024,
            error_type: "Timeout".to_string(),
            error_message: "Processing timed out after 30 seconds".to_string(),
            failed_at: 2000,
            retry_count: 1,
            status: "pending".to_string(),
        };
        db.insert_or_update_dead_letter(&item1_retry).expect("upsert dlq");
        assert_eq!(db.get_dead_letter_count().unwrap(), 1);

        let items = db.get_dead_letter_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].retry_count, 2);
        assert_eq!(items[0].error_type, "Timeout");

        // 4. Insert second item
        let item2 = crate::models::DeadLetterItem {
            id: "dlq-2".to_string(),
            path: "/path/to/timeout.docx".to_string(),
            file_name: "timeout.docx".to_string(),
            file_size: 2048,
            error_type: "Timeout".to_string(),
            error_message: "Processing timed out after 30 seconds".to_string(),
            failed_at: 3000,
            retry_count: 1,
            status: "pending".to_string(),
        };
        db.insert_or_update_dead_letter(&item2).expect("insert dlq 2");
        assert_eq!(db.get_dead_letter_count().unwrap(), 2);

        // 5. Remove by path
        db.remove_dead_letter_by_path("/path/to/broken.pdf").expect("remove by path");
        assert_eq!(db.get_dead_letter_count().unwrap(), 1);

        // 6. Clear queue
        db.clear_dead_letter_queue().expect("clear dlq");
        assert_eq!(db.get_dead_letter_count().unwrap(), 0);
    }

    #[test]
    fn test_window_position_persistence() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");
        assert_eq!(db.get_window_position().expect("get initial position"), None);

        db.save_window_position(340, 180).expect("save position");
        assert_eq!(db.get_window_position().expect("get saved position"), Some((340, 180)));

        db.save_window_position(500, 220).expect("update position");
        assert_eq!(db.get_window_position().expect("get updated position"), Some((500, 220)));
    }

    #[test]
    fn test_blacklist_repository() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        assert_eq!(db.get_blacklist_folders().expect("get empty blacklist").len(), 0);

        let folder = db.add_blacklist_folder("/Users/test/Secret").expect("add blacklist folder");
        assert_eq!(folder.path, "/Users/test/Secret");
        assert!(db.is_folder_blacklisted("/Users/test/Secret").expect("check blacklisted"));
        assert!(!db.is_folder_blacklisted("/Users/test/Public").expect("check not blacklisted"));

        let list = db.get_blacklist_folders().expect("get list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, folder.id);

        db.remove_blacklist_folder(&folder.id).expect("remove blacklist folder");
        assert_eq!(db.get_blacklist_folders().expect("get empty after remove").len(), 0);
        assert!(!db.is_folder_blacklisted("/Users/test/Secret").expect("check not blacklisted after remove"));
    }

    #[test]
    fn test_purge_folder_data() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        let path1 = "/Users/test/Docs/report.txt";
        let path2 = "/Users/test/Docs/nested/sheet.xlsx";
        let path3 = "/Users/test/Other/photo.jpg";

        let a1 = db.upsert_artifact(path1, "file").unwrap();
        let a2 = db.upsert_artifact(path2, "file").unwrap();
        let a3 = db.upsert_artifact(path3, "file").unwrap();

        let v1 = ArtifactVersion {
            id: "v1".to_string(),
            artifact_id: a1.id,
            path: path1.to_string(),
            content_hash: "h1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Report contents".to_string()),
            is_pinned: false,
        };
        let v2 = ArtifactVersion {
            id: "v2".to_string(),
            artifact_id: a2.id,
            path: path2.to_string(),
            content_hash: "h2".to_string(),
            text_hash: None,
            size_bytes: 200,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Spreadsheet data".to_string()),
            is_pinned: false,
        };
        let v3 = ArtifactVersion {
            id: "v3".to_string(),
            artifact_id: a3.id,
            path: path3.to_string(),
            content_hash: "h3".to_string(),
            text_hash: None,
            size_bytes: 300,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: None,
            is_pinned: false,
        };

        db.record_version(&v1).unwrap();
        db.record_version(&v2).unwrap();
        db.record_version(&v3).unwrap();

        let c1 = vec![DocumentChunk {
            id: "c1".to_string(),
            version_id: "v1".to_string(),
            chunk_index: 0,
            content: "Report contents".to_string(),
        }];
        let c2 = vec![DocumentChunk {
            id: "c2".to_string(),
            version_id: "v2".to_string(),
            chunk_index: 0,
            content: "Spreadsheet data".to_string(),
        }];
        db.insert_chunks(&c1).unwrap();
        db.insert_chunks(&c2).unwrap();

        let purge_res = db.purge_folder_data("/Users/test/Docs").expect("purge folder");
        assert_eq!(purge_res.files_removed, 2);
        assert_eq!(purge_res.chunks_removed, 2);

        assert!(db.get_latest_version_by_path(path1).unwrap().is_none());
        assert!(db.get_latest_version_by_path(path2).unwrap().is_none());
        assert!(db.get_chunks_for_version("v1").unwrap().is_empty());
        assert!(db.get_chunks_for_version("v2").unwrap().is_empty());

        assert!(db.get_latest_version_by_path(path3).unwrap().is_some());
    }

    #[test]
    fn test_purge_all_index_data() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        let path1 = "/Users/test/Docs/report.txt";
        let path2 = "/Users/test/Other/photo.jpg";

        let a1 = db.upsert_artifact(path1, "file").unwrap();
        let a2 = db.upsert_artifact(path2, "file").unwrap();

        let v1 = ArtifactVersion {
            id: "v1".to_string(),
            artifact_id: a1.id,
            path: path1.to_string(),
            content_hash: "h1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Report contents".to_string()),
            is_pinned: false,
        };
        let v2 = ArtifactVersion {
            id: "v2".to_string(),
            artifact_id: a2.id,
            path: path2.to_string(),
            content_hash: "h2".to_string(),
            text_hash: None,
            size_bytes: 200,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: None,
            is_pinned: false,
        };

        db.record_version(&v1).unwrap();
        db.record_version(&v2).unwrap();

        let c1 = vec![DocumentChunk {
            id: "c1".to_string(),
            version_id: "v1".to_string(),
            chunk_index: 0,
            content: "Report contents".to_string(),
        }];
        db.insert_chunks(&c1).unwrap();

        let folder = WatchedFolder {
            id: "wf-1".to_string(),
            path: "/Users/test/Docs".to_string(),
            enabled: true,
            recursive: true,
            added_at: 1000,
            last_scanned_at: Some(1500),
        };
        db.add_watched_folder(&folder).unwrap();

        let purge_res = db.purge_all_index_data().expect("purge all index data");
        assert_eq!(purge_res.files_removed, 2);
        assert_eq!(purge_res.chunks_removed, 1);

        assert!(db.get_latest_version_by_path(path1).unwrap().is_none());
        assert!(db.get_latest_version_by_path(path2).unwrap().is_none());
        assert!(db.get_chunks_for_version("v1").unwrap().is_empty());

        let folders = db.get_watched_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].last_scanned_at, None);
    }

    #[test]
    fn test_add_blacklist_folder_purges_and_preserves_overrides() {
        let db = DatabaseManager::new_in_memory().expect("failed to init db");

        let parent_file = "/Users/test/Workspace/parent.txt";
        let blacklisted_file = "/Users/test/Workspace/Secret/secret.txt";
        let override_file = "/Users/test/Workspace/Secret/PublicOverride/public.txt";

        let a1 = db.upsert_artifact(parent_file, "file").unwrap();
        let a2 = db.upsert_artifact(blacklisted_file, "file").unwrap();
        let a3 = db.upsert_artifact(override_file, "file").unwrap();

        let v1 = ArtifactVersion {
            id: "v1".to_string(),
            artifact_id: a1.id,
            path: parent_file.to_string(),
            content_hash: "h1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Parent".to_string()),
            is_pinned: false,
        };
        let v2 = ArtifactVersion {
            id: "v2".to_string(),
            artifact_id: a2.id,
            path: blacklisted_file.to_string(),
            content_hash: "h2".to_string(),
            text_hash: None,
            size_bytes: 200,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Secret".to_string()),
            is_pinned: false,
        };
        let v3 = ArtifactVersion {
            id: "v3".to_string(),
            artifact_id: a3.id,
            path: override_file.to_string(),
            content_hash: "h3".to_string(),
            text_hash: None,
            size_bytes: 300,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("Public".to_string()),
            is_pinned: false,
        };

        db.record_version(&v1).unwrap();
        db.record_version(&v2).unwrap();
        db.record_version(&v3).unwrap();

        db.insert_chunks(&[DocumentChunk {
            id: "c1".to_string(),
            version_id: "v1".to_string(),
            chunk_index: 0,
            content: "Parent".to_string(),
        }]).unwrap();
        db.insert_chunks(&[DocumentChunk {
            id: "c2".to_string(),
            version_id: "v2".to_string(),
            chunk_index: 0,
            content: "Secret".to_string(),
        }]).unwrap();
        db.insert_chunks(&[DocumentChunk {
            id: "c3".to_string(),
            version_id: "v3".to_string(),
            chunk_index: 0,
            content: "Public".to_string(),
        }]).unwrap();

        // Register PublicOverride as an explicit watched folder
        db.add_watched_folder(&WatchedFolder {
            id: "wf-override".to_string(),
            path: "/Users/test/Workspace/Secret/PublicOverride".to_string(),
            enabled: true,
            recursive: true,
            added_at: 1000,
            last_scanned_at: None,
        }).unwrap();

        // Now add /Users/test/Workspace/Secret to blacklist
        db.add_blacklist_folder("/Users/test/Workspace/Secret").unwrap();

        // 1. Blacklisted secret file should be purged!
        assert!(db.get_latest_version_by_path(blacklisted_file).unwrap().is_none());
        assert!(db.get_chunks_for_version("v2").unwrap().is_empty());

        // 2. Parent file outside blacklist remains intact!
        assert!(db.get_latest_version_by_path(parent_file).unwrap().is_some());
        assert_eq!(db.get_chunks_for_version("v1").unwrap().len(), 1);

        // 3. Watched override child folder inside blacklist is preserved!
        assert!(db.get_latest_version_by_path(override_file).unwrap().is_some());
        assert_eq!(db.get_chunks_for_version("v3").unwrap().len(), 1);
    }

    #[test]
    fn test_purge_files_batch() {
        let db = DatabaseManager::new_in_memory().unwrap();
        let file1 = "/test/doc1.txt";
        let file2 = "/test/doc2.txt";

        let art1 = db.upsert_artifact(file1, "file").unwrap();
        let v1 = ArtifactVersion {
            id: "v1".to_string(),
            artifact_id: art1.id.clone(),
            path: file1.to_string(),
            content_hash: "hash1".to_string(),
            text_hash: None,
            size_bytes: 100,
            modified_at: 1000,
            previous_version_id: None,
            extracted_text: Some("sample text 1".to_string()),
            is_pinned: false,
        };
        db.record_version(&v1).unwrap();
        db.insert_chunks(&[DocumentChunk {
            id: "c1".to_string(),
            version_id: "v1".to_string(),
            chunk_index: 0,
            content: "sample text 1".to_string(),
        }]).unwrap();

        let art2 = db.upsert_artifact(file2, "file").unwrap();
        let v2 = ArtifactVersion {
            id: "v2".to_string(),
            artifact_id: art2.id.clone(),
            path: file2.to_string(),
            content_hash: "hash2".to_string(),
            text_hash: None,
            size_bytes: 200,
            modified_at: 2000,
            previous_version_id: None,
            extracted_text: Some("sample text 2".to_string()),
            is_pinned: false,
        };
        db.record_version(&v2).unwrap();
        db.insert_chunks(&[DocumentChunk {
            id: "c2".to_string(),
            version_id: "v2".to_string(),
            chunk_index: 0,
            content: "sample text 2".to_string(),
        }]).unwrap();

        assert!(db.get_latest_version_by_path(file1).unwrap().is_some());
        assert!(db.get_latest_version_by_path(file2).unwrap().is_some());

        db.purge_files_batch(&[file1.to_string()]).unwrap();

        assert!(db.get_latest_version_by_path(file1).unwrap().is_none());
        assert!(db.get_chunks_for_version("v1").unwrap().is_empty());
        assert!(db.get_latest_version_by_path(file2).unwrap().is_some());
        assert_eq!(db.get_chunks_for_version("v2").unwrap().len(), 1);

        assert!(db.get_sqlite_size_bytes() > 0);
    }
}


