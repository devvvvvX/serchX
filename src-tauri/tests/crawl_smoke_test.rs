use serchx_lib::crawler::{CrawlController, CrawlerEngine};
use serchx_lib::db::DatabaseManager;
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::safety::FileFilter;
use serchx_lib::search::Fts5SearchEngine;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_crawl_smoke_1000_files() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    // 1. Create a synthetic corpus:
    // - 500 valid docs (.txt)
    // - 200 valid code/markdown (.md)
    // - 100 blacklisted .git files
    // - 100 blacklisted node_modules files
    // - 100 disallowed extension files (.bin)
    let docs_dir = root.join("documents");
    let code_dir = root.join("code");
    let git_dir = root.join(".git");
    let node_modules_dir = root.join("node_modules");
    let bin_dir = root.join("binaries");

    fs::create_dir_all(&docs_dir).unwrap();
    fs::create_dir_all(&code_dir).unwrap();
    fs::create_dir_all(&git_dir).unwrap();
    fs::create_dir_all(&node_modules_dir).unwrap();
    fs::create_dir_all(&bin_dir).unwrap();

    // 500 valid docs
    for i in 0..500 {
        let p = docs_dir.join(format!("doc_{}.txt", i));
        let mut f = File::create(p).unwrap();
        if i == 42 {
            writeln!(
                f,
                "Special secret keyword KITTEN_QUANTUM_DISCOVERY in document 42."
            )
            .unwrap();
        } else {
            writeln!(
                f,
                "Regular standard corporate text content line {} with miscellaneous notes.",
                i
            )
            .unwrap();
        }
    }

    // 200 valid code/markdown files
    for i in 0..200 {
        let p = code_dir.join(format!("spec_{}.md", i));
        let mut f = File::create(p).unwrap();
        writeln!(
            f,
            "# System Specification Section {}\nDetailed technical description and architecture notes.",
            i
        )
        .unwrap();
    }

    // 100 blacklisted git files (pruned at directory descent level)
    for i in 0..100 {
        let p = git_dir.join(format!("commit_{}", i));
        let mut f = File::create(p).unwrap();
        writeln!(f, "commit object metadata {}", i).unwrap();
    }

    // 100 blacklisted node_modules files (pruned at directory descent level)
    for i in 0..100 {
        let p = node_modules_dir.join(format!("package_{}.json", i));
        let mut f = File::create(p).unwrap();
        writeln!(f, "{{\"name\": \"package_{}\"}}", i).unwrap();
    }

    // 100 disallowed binary files
    for i in 0..100 {
        let p = bin_dir.join(format!("lib_{}.bin", i));
        let mut f = File::create(p).unwrap();
        f.write_all(&[0x00, 0x01, 0x02, 0x03, 0xFF]).unwrap();
    }

    // Setup pipeline & crawler
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());
    let pipeline = Arc::new(IngestionPipeline::with_filter(
        db.clone(),
        extractors,
        ocr,
        filter.clone(),
    ));
    let controller = CrawlController::new();
    let engine = CrawlerEngine::new(pipeline, controller, filter, None);

    let progress = engine.crawl_paths(&[root.to_path_buf()]);

    // Verifications:
    // 700 files indexed (500 docs + 200 specs)
    assert_eq!(progress.indexed_count, 700);
    // 100 binary files skipped by extension filter
    // .git and node_modules are pruned before entry descent
    assert_eq!(progress.skipped_count, 100);
    assert_eq!(progress.failed_count, 0);

    // Verify FTS search works on indexed files
    let search_engine = Fts5SearchEngine::new(db);
    let results = search_engine
        .search("KITTEN_QUANTUM_DISCOVERY", 10)
        .expect("search");
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("KITTEN_QUANTUM_DISCOVERY"));
}

#[test]
fn test_crawl_smoke_pause_resume() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    for i in 0..150 {
        let p = root.join(format!("page_{}.txt", i));
        let mut f = File::create(p).unwrap();
        writeln!(
            f,
            "Document number {} content with unique hash token {}",
            i,
            i * 7
        )
        .unwrap();
    }

    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());
    let pipeline = Arc::new(IngestionPipeline::with_filter(
        db.clone(),
        extractors,
        ocr,
        filter.clone(),
    ));
    let controller = CrawlController::new();
    let engine = Arc::new(CrawlerEngine::new(pipeline, controller, filter, None));

    let engine_clone = engine.clone();
    let root_path = root.to_path_buf();
    let handle = std::thread::spawn(move || {
        engine_clone.crawl_paths(&[root_path]);
    });

    // Wait for crawl to start
    for _ in 0..50 {
        if engine.get_progress().is_running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    // Pause
    engine.pause();
    assert!(engine.get_progress().is_paused);
    std::thread::sleep(std::time::Duration::from_millis(30));

    // Resume
    engine.resume();
    assert!(!engine.get_progress().is_paused);

    handle.join().unwrap();
    let final_progress = engine.get_progress();
    assert_eq!(final_progress.indexed_count, 150);
    assert_eq!(final_progress.failed_count, 0);
}
