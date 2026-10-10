use serchx_lib::db::{DatabaseManager, IngestionBatchItem};
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::models::{ArtifactVersion, DocumentChunk, FileEvent};
use serchx_lib::safety::FileFilter;
use serchx_lib::search::Fts5SearchEngine;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_db_batch_writer() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_batch_writer.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());

    let mut items = Vec::new();
    for i in 1..=10 {
        let path = format!("/tmp/test_doc_{}.txt", i);
        let artifact_id = format!("art-{}", i);
        let version_id = format!("ver-{}", i);
        let content = format!("This is unique text content for document number {}", i);

        let version = ArtifactVersion {
            id: version_id.clone(),
            artifact_id: artifact_id.clone(),
            path: path.clone(),
            content_hash: format!("hash-{}", i),
            text_hash: Some(format!("thash-{}", i)),
            size_bytes: content.len() as i64,
            modified_at: 1000 + i as i64,
            previous_version_id: None,
            extracted_text: Some(content.clone()),
            is_pinned: false,
        };

        let chunk = DocumentChunk {
            id: format!("chunk-{}", i),
            version_id: version_id.clone(),
            chunk_index: 0,
            content: content.clone(),
        };

        let event = FileEvent {
            id: format!("event-{}", i),
            artifact_id: Some(artifact_id),
            event_type: "ingested".to_string(),
            timestamp: 2000 + i as i64,
            metadata: None,
        };

        items.push(IngestionBatchItem {
            artifact_path: path,
            artifact_kind: "file".to_string(),
            version,
            chunks: vec![chunk],
            event: Some(event),
        });
    }

    db.persist_ingestion_batch(&items).expect("batch insert should succeed");

    // Verify FTS5 search index was populated via triggers
    let fts = Fts5SearchEngine::new(db.clone());
    let results = fts.search("unique", 20).expect("fts search");
    assert_eq!(results.len(), 10, "All 10 documents should be indexed in FTS5");

    let specific_result = fts.search("number 7", 5).expect("fts specific search");
    assert_eq!(specific_result.len(), 1);
    assert_eq!(specific_result[0].version_id, "ver-7");
}

#[test]
fn test_parallel_indexing_and_race_condition_freedom() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_parallel_crawl.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());

    let pipeline = Arc::new(IngestionPipeline::with_filter(db.clone(), extractors, ocr, filter.clone()));
    let controller = serchx_lib::crawler::CrawlController::new();
    let coordinator = Arc::new(serchx_lib::crawler::CrawlCoordinator::new(None));

    let crawler = serchx_lib::crawler::CrawlerEngine::with_coordinator(
        pipeline,
        controller,
        filter,
        Some(db_path),
        coordinator.clone(),
    );

    let watched_root = dir.path().join("corpus");
    fs::create_dir_all(&watched_root).unwrap();

    // Create 100 test files across 5 subdirectories
    let total_files = 100;
    for sub in 0..5 {
        let sub_dir = watched_root.join(format!("subdir_{}", sub));
        fs::create_dir_all(&sub_dir).unwrap();
        for f in 0..20 {
            let file_path = sub_dir.join(format!("item_{}.txt", f));
            fs::write(
                &file_path,
                format!("Parallel indexed corpus document section {} item {}. Important search term keywordAlpha.", sub, f),
            )
            .unwrap();
        }
    }

    crawler.crawl_paths(&[watched_root]);
    let progress = crawler.get_progress();

    assert_eq!(
        progress.indexed_count, total_files,
        "All 100 files should be indexed in parallel without dropped records"
    );
    assert_eq!(progress.failed_count, 0, "No files should fail");

    // Search FTS5 to verify all 100 files are searchable
    let fts = Fts5SearchEngine::new(db.clone());
    let search_results = fts.search("keywordAlpha", 200).expect("fts search keywordAlpha");
    assert_eq!(search_results.len(), total_files);
}
