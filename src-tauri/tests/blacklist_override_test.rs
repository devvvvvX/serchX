use serchx_lib::crawler::{CrawlController, CrawlerEngine};
use serchx_lib::db::DatabaseManager;
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::models::WatchedFolder;
use serchx_lib::safety::filters::{
    is_watched_folder_blacklist_override, resolve_path_indexing_permission,
    should_skip_directory, PathPermissionResult,
};
use serchx_lib::safety::FileFilter;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_algorithm_blacklist_and_child_exception_override() {
    let a = PathBuf::from("/workspace/A");
    let ab_file = PathBuf::from("/workspace/A/B/file1.txt");
    let ac = PathBuf::from("/workspace/A/C");
    let ac_file = PathBuf::from("/workspace/A/C/file2.txt");
    let ac_nested_file = PathBuf::from("/workspace/A/C/nested/file3.txt");
    let ad_file = PathBuf::from("/workspace/A/D/file4.txt");
    let outside_file = PathBuf::from("/workspace/other/file5.txt");

    let watched_roots = vec![ac.clone()];
    let blacklisted_roots = vec![a.clone()];

    // 1. A/C is an override child of blacklisted A
    assert!(is_watched_folder_blacklist_override(&ac, &blacklisted_roots));
    assert!(!is_watched_folder_blacklist_override(&a, &blacklisted_roots));
    assert!(!is_watched_folder_blacklist_override(
        &PathBuf::from("/workspace/other"),
        &blacklisted_roots
    ));

    // 2. Directory pruning checks
    // Root A contains watched child A/C, so should NOT skip A during crawl descent
    assert!(!should_skip_directory(&a, &watched_roots, &blacklisted_roots));

    // A/B has no watched child inside it and is blacklisted -> SKIP descent
    let ab_dir = PathBuf::from("/workspace/A/B");
    assert!(should_skip_directory(&ab_dir, &watched_roots, &blacklisted_roots));

    // A/C is watched child override -> DO NOT skip descent
    assert!(!should_skip_directory(&ac, &watched_roots, &blacklisted_roots));

    // A/C/nested is under watched child override -> DO NOT skip descent
    let ac_nested_dir = PathBuf::from("/workspace/A/C/nested");
    assert!(!should_skip_directory(&ac_nested_dir, &watched_roots, &blacklisted_roots));

    // A/D has no watched child inside it and is blacklisted -> SKIP descent
    let ad_dir = PathBuf::from("/workspace/A/D");
    assert!(should_skip_directory(&ad_dir, &watched_roots, &blacklisted_roots));

    // 3. File permission resolution
    // A/B/file1.txt is outside watched roots (since only A/C is watched)
    assert_eq!(
        resolve_path_indexing_permission(&ab_file, &watched_roots, &blacklisted_roots),
        PathPermissionResult::OutsideWatchedRoots
    );

    // If both A and A/C were in watched roots (e.g. A watched AND blacklisted, but A/C explicitly watched):
    let watched_roots_with_a = vec![a.clone(), ac.clone()];
    assert_eq!(
        resolve_path_indexing_permission(&ab_file, &watched_roots_with_a, &blacklisted_roots),
        PathPermissionResult::BlockedByBlacklist {
            blacklisted_root: a.clone()
        }
    );
    assert_eq!(
        resolve_path_indexing_permission(&ad_file, &watched_roots_with_a, &blacklisted_roots),
        PathPermissionResult::BlockedByBlacklist {
            blacklisted_root: a.clone()
        }
    );

    // Corner Case: A/C and its nested files MUST be Allowed!
    assert_eq!(
        resolve_path_indexing_permission(&ac_file, &watched_roots, &blacklisted_roots),
        PathPermissionResult::Allowed
    );
    assert_eq!(
        resolve_path_indexing_permission(&ac_nested_file, &watched_roots, &blacklisted_roots),
        PathPermissionResult::Allowed
    );
    assert_eq!(
        resolve_path_indexing_permission(&ac_file, &watched_roots_with_a, &blacklisted_roots),
        PathPermissionResult::Allowed
    );
    assert_eq!(
        resolve_path_indexing_permission(&ac_nested_file, &watched_roots_with_a, &blacklisted_roots),
        PathPermissionResult::Allowed
    );

    // Outside path
    assert_eq!(
        resolve_path_indexing_permission(&outside_file, &watched_roots, &blacklisted_roots),
        PathPermissionResult::OutsideWatchedRoots
    );
}

#[test]
fn test_crawler_respects_blacklist_and_child_override() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    // Create directory tree:
    // root/A/B/file1.txt
    // root/A/C/file2.txt
    // root/A/C/nested/file3.txt
    // root/A/D/file4.txt
    let dir_a = root.join("A");
    let dir_b = dir_a.join("B");
    let dir_c = dir_a.join("C");
    let dir_c_nested = dir_c.join("nested");
    let dir_d = dir_a.join("D");

    fs::create_dir_all(&dir_b).unwrap();
    fs::create_dir_all(&dir_c_nested).unwrap();
    fs::create_dir_all(&dir_d).unwrap();

    let mut f1 = File::create(dir_b.join("file1.txt")).unwrap();
    writeln!(f1, "Secret blacklisted content in A/B.").unwrap();

    let mut f2 = File::create(dir_c.join("file2.txt")).unwrap();
    writeln!(f2, "Special included target content in A/C child override.").unwrap();

    let mut f3 = File::create(dir_c_nested.join("file3.txt")).unwrap();
    writeln!(f3, "Deeply nested included target content in A/C/nested.").unwrap();

    let mut f4 = File::create(dir_d.join("file4.txt")).unwrap();
    writeln!(f4, "Secret blacklisted content in A/D.").unwrap();

    // Setup in-memory DB and register blacklist and watched folders
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    db.add_blacklist_folder(&dir_a.to_string_lossy()).unwrap();
    db.add_watched_folder(&WatchedFolder {
        id: "folder-c".to_string(),
        path: dir_c.to_string_lossy().to_string(),
        enabled: true,
        recursive: true,
        added_at: chrono::Utc::now().timestamp(),
        last_scanned_at: None,
    }).unwrap();

    // Verify DB state
    let blacklists = db.get_blacklist_folders().unwrap();
    assert_eq!(blacklists.len(), 1);
    assert_eq!(blacklists[0].path, dir_a.to_string_lossy());

    let watched = db.get_watched_folders().unwrap();
    assert_eq!(watched.len(), 1);
    assert_eq!(watched[0].path, dir_c.to_string_lossy());

    // Verify exception override detection helper
    let is_override = is_watched_folder_blacklist_override(
        &dir_c,
        &[dir_a.clone()],
    );
    assert!(is_override, "A/C must be flagged as a blacklist override child!");

    // Construct Crawler Engine
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

    // Crawl starting at dir_c (or crawl all watched folders)
    let progress = engine.crawl_paths(&[dir_c.clone()]);

    // Exactly 2 files indexed: file2.txt and file3.txt
    assert_eq!(progress.indexed_count, 2, "Only A/C/file2.txt and A/C/nested/file3.txt should be indexed");
    assert_eq!(progress.failed_count, 0);

    // If crawl is triggered on parent root (or if A is traversed),
    // A/B and A/D files should NOT be indexed because A is blacklisted!
    let progress_all = engine.crawl_paths(&[dir_a.clone()]);
    // file2.txt and file3.txt were already indexed (so skipped as identical versions),
    // file1 and file4 must NOT be indexed.
    assert_eq!(progress_all.failed_count, 0);

    // Verify FTS search confirms only A/C files were ingested
    let search = serchx_lib::search::Fts5SearchEngine::new(db);
    let res_override = search.search("Special included target", 10).unwrap();
    assert_eq!(res_override.len(), 1);
    assert!(res_override[0].path.contains("file2.txt"));

    let res_nested = search.search("Deeply nested included", 10).unwrap();
    assert_eq!(res_nested.len(), 1);
    assert!(res_nested[0].path.contains("file3.txt"));

    let res_blocked_b = search.search("Secret blacklisted content in A/B", 10).unwrap();
    assert_eq!(res_blocked_b.len(), 0, "A/B content must NOT be indexed");

    let res_blocked_d = search.search("Secret blacklisted content in A/D", 10).unwrap();
    assert_eq!(res_blocked_d.len(), 0, "A/D content must NOT be indexed");
}
