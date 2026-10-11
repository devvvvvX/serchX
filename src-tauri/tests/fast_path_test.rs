use serchx_lib::db::DatabaseManager;
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::safety::FileFilter;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn test_metadata_fast_path_skips_unchanged_file() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_fast_path.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());

    let pipeline = IngestionPipeline::with_filter(db.clone(), extractors, ocr, filter);

    let test_file = dir.path().join("sample_doc.txt");
    {
        let mut f = File::create(&test_file).unwrap();
        f.write_all(b"Hello world! This is a test file for fast-path indexing.")
            .unwrap();
    }

    // 1. Initial ingestion
    let res1 = pipeline.ingest_file(&test_file).expect("first ingest");
    assert!(
        res1.is_new_version,
        "First ingestion should create a new version"
    );

    // 2. Immediate second ingestion (Fast-path hit)
    let start = Instant::now();
    let res2 = pipeline.ingest_file(&test_file).expect("second ingest");
    let elapsed = start.elapsed();

    assert!(
        !res2.is_new_version,
        "Second ingestion should hit fast-path and NOT create a new version"
    );
    assert_eq!(res1.artifact_id, res2.artifact_id);
    assert_eq!(res1.version_id, res2.version_id);
    assert!(
        elapsed.as_millis() < 50,
        "Fast-path check should be extremely fast, took {:?}",
        elapsed
    );
}

#[test]
fn test_metadata_fast_path_detects_mtime_change() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_fast_path_mtime.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());

    let pipeline = IngestionPipeline::with_filter(db.clone(), extractors, ocr, filter);

    let test_file = dir.path().join("changing_doc.txt");
    fs::write(&test_file, "Initial content 12345").unwrap();

    let res1 = pipeline.ingest_file(&test_file).expect("first ingest");
    assert!(res1.is_new_version);

    // Sleep 15ms so mtime timestamp changes
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Append new content
    {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&test_file)
            .unwrap();
        f.write_all(b" - Appended content").unwrap();
    }

    let res2 = pipeline
        .ingest_file(&test_file)
        .expect("second ingest after modification");
    assert!(
        res2.is_new_version,
        "Modified file must trigger a new version"
    );
    assert_ne!(res1.version_id, res2.version_id);
    assert_eq!(res1.artifact_id, res2.artifact_id);
}

#[test]
fn test_single_pass_streaming_crawler() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_crawler_stream.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());

    let pipeline = Arc::new(IngestionPipeline::with_filter(
        db.clone(),
        extractors,
        ocr,
        filter.clone(),
    ));
    let controller = serchx_lib::crawler::CrawlController::new();
    let coordinator = Arc::new(serchx_lib::crawler::CrawlCoordinator::new(None));

    let crawler = serchx_lib::crawler::CrawlerEngine::with_coordinator(
        pipeline,
        controller,
        filter,
        Some(db_path),
        coordinator.clone(),
    );

    let watched_dir = dir.path().join("watched");
    fs::create_dir_all(&watched_dir).unwrap();
    for i in 1..=5 {
        fs::write(
            watched_dir.join(format!("file_{}.txt", i)),
            format!("Content of file {}", i),
        )
        .unwrap();
    }

    crawler.crawl_paths(&[watched_dir]);
    let progress = crawler.get_progress();

    assert_eq!(progress.indexed_count, 5);
    assert_eq!(progress.failed_count, 0);
}
