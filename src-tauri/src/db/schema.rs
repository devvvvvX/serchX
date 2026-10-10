use rusqlite::{Connection, Result};

pub fn migrate(conn: &Connection) -> Result<()> {
    // High-performance concurrency pragmas
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        "#,
    )?;

    // Core table migrations
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS artifacts (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS artifact_versions (
            id TEXT PRIMARY KEY,
            artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
            path TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            text_hash TEXT,
            size_bytes INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            previous_version_id TEXT REFERENCES artifact_versions(id),
            extracted_text TEXT,
            is_pinned INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            version_id TEXT NOT NULL REFERENCES artifact_versions(id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
            event_type TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            metadata JSON
        );

        CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            entity_type TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS relationships (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relationship_type TEXT NOT NULL,
            weight REAL DEFAULT 1.0,
            PRIMARY KEY (source_id, target_id, relationship_type)
        );

        -- FTS5 Full-Text Search Virtual Table
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            chunk_id UNINDEXED,
            artifact_id UNINDEXED,
            version_id UNINDEXED,
            path,
            content,
            tokenize = 'unicode61 remove_diacritics 2'
        );

        -- Synchronization Triggers for FTS5
        CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_insert AFTER INSERT ON chunks
        BEGIN
            INSERT INTO chunks_fts(chunk_id, artifact_id, version_id, path, content)
            SELECT new.id, av.artifact_id, new.version_id, av.path, new.content
            FROM artifact_versions av
            WHERE av.id = new.version_id;
        END;

        CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_delete AFTER DELETE ON chunks
        BEGIN
            DELETE FROM chunks_fts WHERE chunk_id = old.id;
        END;

        -- Performance Indexes
        CREATE INDEX IF NOT EXISTS idx_artifact_versions_path ON artifact_versions(path);
        CREATE INDEX IF NOT EXISTS idx_artifact_versions_hash ON artifact_versions(content_hash);
        CREATE INDEX IF NOT EXISTS idx_artifact_versions_pinned ON artifact_versions(artifact_id, is_pinned);
        CREATE INDEX IF NOT EXISTS idx_chunks_version_id ON chunks(version_id);
        CREATE INDEX IF NOT EXISTS idx_events_artifact_id ON events(artifact_id);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);

        -- Watched Folders Table (SCOPE-03)
        CREATE TABLE IF NOT EXISTS watched_folders (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            enabled INTEGER NOT NULL DEFAULT 1,
            recursive INTEGER NOT NULL DEFAULT 1,
            added_at INTEGER NOT NULL,
            last_scanned_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_watched_folders_enabled ON watched_folders(enabled);

        -- Blacklist Folders Table (D-06)
        CREATE TABLE IF NOT EXISTS blacklist_folders (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_blacklist_folders_path ON blacklist_folders(path);

        -- App Settings Table (D-01, D-02, D-03)
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value JSON NOT NULL,
            updated_at INTEGER NOT NULL
        );

        -- Chunk Vectors Table (INDEX-03, INDEX-04)
        CREATE TABLE IF NOT EXISTS chunk_vectors (
            chunk_id TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
            dimensions INTEGER NOT NULL,
            embedding BLOB NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunk_vectors_created ON chunk_vectors(created_at);

        -- Dead Letter Queue Table for failed indexing files
        CREATE TABLE IF NOT EXISTS dead_letter_queue (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            error_type TEXT NOT NULL,
            error_message TEXT NOT NULL,
            failed_at INTEGER NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending'
        );
        CREATE INDEX IF NOT EXISTS idx_dlq_path ON dead_letter_queue(path);
        CREATE INDEX IF NOT EXISTS idx_dlq_failed_at ON dead_letter_queue(failed_at);
        CREATE INDEX IF NOT EXISTS idx_dlq_status ON dead_letter_queue(status);
        "#,
    )?;

    // Safe migration check for existing pre-migration databases
    let has_is_pinned: bool = conn
        .prepare("PRAGMA table_info(artifact_versions);")?
        .query_map([], |r| r.get::<_, String>(1))?
        .any(|col| col.map(|c| c == "is_pinned").unwrap_or(false));

    if !has_is_pinned {
        let _ = conn.execute(
            "ALTER TABLE artifact_versions ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifact_versions_pinned ON artifact_versions(artifact_id, is_pinned);",
            [],
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_migration_in_memory() {
        let conn = Connection::open_in_memory().expect("failed to open in-memory database");
        migrate(&conn).expect("migration failed");

        // Verify journal mode or foreign keys
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .expect("pragma failed");
        assert_eq!(fk_enabled, 1);

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"artifacts".to_string()));
        assert!(tables.contains(&"artifact_versions".to_string()));
        assert!(tables.contains(&"chunks".to_string()));
        assert!(tables.contains(&"events".to_string()));
        assert!(tables.contains(&"entities".to_string()));
        assert!(tables.contains(&"relationships".to_string()));
        assert!(tables.contains(&"chunks_fts".to_string()));
        assert!(tables.contains(&"watched_folders".to_string()));
        assert!(tables.contains(&"blacklist_folders".to_string()));
        assert!(tables.contains(&"app_settings".to_string()));
        assert!(tables.contains(&"chunk_vectors".to_string()));
        assert!(tables.contains(&"dead_letter_queue".to_string()));

        // Verify is_pinned column
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(artifact_versions);")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(cols.contains(&"is_pinned".to_string()));
    }
}
