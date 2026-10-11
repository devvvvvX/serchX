use serchx_lib::crawler::{
    CrawlController, CrawlCoordinator, CrawlerEngine, FileProcessStatus, QueueFolderStatus,
};
use serchx_lib::db::DatabaseManager;
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::models::WatchedFolder;
use serchx_lib::safety::FileFilter;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_crawl_coordinator_state_and_ring_buffer() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));

    // 1. Initial state
    let snap0 = coordinator.get_snapshot();
    assert!(!snap0.is_active);
    assert_eq!(snap0.total_queue_depth, 0);
    assert!(snap0.active_folder.is_none());
    assert!(snap0.pending_queue.is_empty());
    assert!(snap0.recent_events.is_empty());

    // 2. Enqueue 2 folders
    let folders = vec![
        WatchedFolder {
            id: "folder-1".to_string(),
            path: "/path/to/folder1".to_string(),
            enabled: true,
            recursive: true,
            added_at: 1000,
            last_scanned_at: None,
        },
        WatchedFolder {
            id: "folder-2".to_string(),
            path: "/path/to/folder2".to_string(),
            enabled: true,
            recursive: true,
            added_at: 1001,
            last_scanned_at: None,
        },
    ];
    coordinator.enqueue_folders(&folders);

    let snap1 = coordinator.get_snapshot();
    assert!(snap1.is_active);
    assert_eq!(snap1.total_queue_depth, 2);
    assert_eq!(snap1.pending_queue.len(), 2);
    assert_eq!(snap1.pending_queue[0].id, "folder-1");
    assert_eq!(snap1.pending_queue[0].status, QueueFolderStatus::Queued);

    // 3. Start folder 1 crawl
    coordinator.start_folder_crawl("/path/to/folder1");
    let snap2 = coordinator.get_snapshot();
    assert!(snap2.active_folder.is_some());
    let active = snap2.active_folder.unwrap();
    assert_eq!(active.id, "folder-1");
    assert_eq!(active.status, QueueFolderStatus::Scanning);
    assert_eq!(snap2.pending_queue.len(), 1); // folder-2 still pending
    assert_eq!(snap2.total_queue_depth, 2);

    // 4. Record progress with 60 files to test 50-item rolling buffer cap
    for i in 0..60 {
        let path = format!("/path/to/folder1/doc_{}.txt", i);
        let status = if i % 2 == 0 {
            FileProcessStatus::Indexed
        } else {
            FileProcessStatus::Skipped
        };
        coordinator.increment_discovered();
        coordinator.record_file_progress(&path, status, (i + 1) * 100);
    }

    let snap3 = coordinator.get_snapshot();
    let active3 = snap3.active_folder.unwrap();
    assert_eq!(active3.total_files_discovered, 60);
    assert_eq!(active3.files_indexed, 30);
    assert_eq!(active3.files_skipped, 30);
    assert_eq!(active3.files_failed, 0);

    // Assert ring buffer capped strictly at 50
    assert_eq!(snap3.recent_events.len(), 50);
    // Oldest 10 (0..9) were pushed out; element 0 should be doc_10.txt
    assert_eq!(snap3.recent_events[0].path, "/path/to/folder1/doc_10.txt");
    assert_eq!(snap3.recent_events[49].path, "/path/to/folder1/doc_59.txt");
    assert_eq!(
        snap3.current_file,
        Some("/path/to/folder1/doc_59.txt".to_string())
    );

    // 5. Complete folder 1 crawl
    coordinator.complete_folder_crawl("/path/to/folder1", true);
    let snap4 = coordinator.get_snapshot();
    assert!(snap4.active_folder.is_none());
    assert_eq!(snap4.pending_queue.len(), 1); // folder-2 remaining

    // 6. Cancel pending folder 2
    coordinator.cancel_folder("folder-2");
    let snap5 = coordinator.get_snapshot();
    assert!(!snap5.is_active);
    assert_eq!(snap5.total_queue_depth, 0);
    assert!(snap5.pending_queue.is_empty());
}

#[test]
fn test_crawler_engine_updates_coordinator_during_crawl() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    let docs_dir = root.join("my_docs");
    fs::create_dir_all(&docs_dir).unwrap();

    for i in 0..5 {
        let mut f = File::create(docs_dir.join(format!("file_{}.txt", i))).unwrap();
        writeln!(f, "Content for document file {}", i).unwrap();
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
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    let engine =
        CrawlerEngine::with_coordinator(pipeline, controller, filter, None, coordinator.clone());

    // Enqueue before crawling
    let watched = WatchedFolder {
        id: "w-docs".to_string(),
        path: docs_dir.to_string_lossy().to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };
    coordinator.enqueue_folders(&[watched]);

    // Perform crawl
    let progress = engine.crawl_paths(&[docs_dir.clone()]);
    assert_eq!(progress.indexed_count, 5);

    // Check coordinator state post-crawl
    let snap = coordinator.get_snapshot();
    assert!(!snap.is_active);
    assert_eq!(snap.recent_events.len(), 5);
    for event in &snap.recent_events {
        assert_eq!(event.status, FileProcessStatus::Indexed);
        assert_eq!(event.extension, "txt");
    }
}

#[test]
fn test_coordinator_pre_count_and_file_processing_lifecycle() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    coordinator.start_folder_crawl_with_expected("/path/to/folder", 42);

    let snap = coordinator.get_snapshot();
    let active = snap.active_folder.expect("active folder present");
    assert_eq!(active.total_files_expected, 42);
    assert_eq!(active.total_files_discovered, 0);

    // Start file processing notifies current_file immediately before ingestion
    coordinator.start_file_processing("/path/to/folder/big.json", 1_500_000);
    let snap2 = coordinator.get_snapshot();
    assert_eq!(
        snap2.current_file,
        Some("/path/to/folder/big.json".to_string())
    );
    assert_eq!(snap2.current_file_size, 1_500_000);

    // Completion records file progress
    coordinator.increment_discovered();
    coordinator.record_file_progress(
        "/path/to/folder/big.json",
        FileProcessStatus::Indexed,
        1_500_000,
    );
    let snap3 = coordinator.get_snapshot();
    let active3 = snap3.active_folder.unwrap();
    assert_eq!(active3.files_indexed, 1);
    assert_eq!(active3.total_files_discovered, 1);
}

#[test]
fn test_per_folder_pause_and_resume_controls() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    let controller = CrawlController::new();
    coordinator.set_controller(controller.clone());

    let folder1 = WatchedFolder {
        id: "folder-1".to_string(),
        path: "/data/docs".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };
    let folder2 = WatchedFolder {
        id: "folder-2".to_string(),
        path: "/data/photos".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };

    coordinator.enqueue_folders(&[folder1.clone(), folder2.clone()]);
    assert_eq!(coordinator.get_snapshot().pending_queue.len(), 2);

    // Start crawl on folder 1
    coordinator.start_folder_crawl_with_expected(&folder1.path, 50);
    let snap = coordinator.get_snapshot();
    assert!(snap.is_active);
    assert!(!snap.is_paused);
    let active = snap.active_folder.expect("active folder");
    assert_eq!(active.id, "folder-1");
    assert!(!active.is_paused);
    assert_eq!(snap.pending_queue.len(), 1);

    // Pause individual active folder
    coordinator.pause_folder("folder-1");
    assert!(controller.is_paused());
    let snap_paused = coordinator.get_snapshot();
    assert!(snap_paused.is_paused);
    assert!(snap_paused.active_folder.unwrap().is_paused);

    // Resume individual active folder
    coordinator.resume_folder("folder-1");
    assert!(!controller.is_paused());
    let snap_resumed = coordinator.get_snapshot();
    assert!(!snap_resumed.is_paused);
    assert!(!snap_resumed.active_folder.unwrap().is_paused);

    // Pre-pause queued folder 2
    coordinator.pause_folder("folder-2");
    let snap_prepaused = coordinator.get_snapshot();
    assert_eq!(snap_prepaused.pending_queue.len(), 1);
    assert!(snap_prepaused.pending_queue[0].is_paused);

    // Complete folder 1 crawl
    coordinator.complete_folder_crawl(&folder1.path, true);
    assert!(coordinator.has_pending_folders());

    // When folder 2 starts, it should inherit pre-paused state
    coordinator.start_folder_crawl_with_expected(&folder2.path, 100);
    let snap_f2 = coordinator.get_snapshot();
    let active_f2 = snap_f2.active_folder.expect("folder 2 active");
    assert_eq!(active_f2.id, "folder-2");
    assert!(active_f2.is_paused);
    assert!(controller.is_paused());
}

#[test]
fn test_global_pause_all_and_resume_all() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    let folder = WatchedFolder {
        id: "f-1".to_string(),
        path: "/data/code".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };
    let folder_q = WatchedFolder {
        id: "f-2".to_string(),
        path: "/data/notes".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };

    coordinator.enqueue_folders(&[folder.clone(), folder_q.clone()]);
    coordinator.start_folder_crawl_with_expected(&folder.path, 20);

    // Global pause all
    coordinator.set_paused(true);
    let snap_paused = coordinator.get_snapshot();
    assert!(snap_paused.is_paused);
    assert!(snap_paused.active_folder.unwrap().is_paused);
    assert!(snap_paused.pending_queue[0].is_paused);

    // Global resume all
    coordinator.set_paused(false);
    let snap_resumed = coordinator.get_snapshot();
    assert!(!snap_resumed.is_paused);
    assert!(!snap_resumed.active_folder.unwrap().is_paused);
    assert!(!snap_resumed.pending_queue[0].is_paused);
}

#[test]
fn test_cancel_pending_and_active_folders() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    let controller = CrawlController::new();
    coordinator.set_controller(controller.clone());

    let folder1 = WatchedFolder {
        id: "f-a".to_string(),
        path: "/data/a".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };
    let folder2 = WatchedFolder {
        id: "f-b".to_string(),
        path: "/data/b".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };

    coordinator.enqueue_folders(&[folder1.clone(), folder2.clone()]);
    coordinator.start_folder_crawl_with_expected(&folder1.path, 10);

    // Cancel pending folder 2
    coordinator.cancel_folder("f-b");
    assert!(coordinator.is_cancelled("/data/b"));
    let snap = coordinator.get_snapshot();
    assert!(snap.pending_queue.is_empty());

    // Cancel active folder 1
    coordinator.cancel_folder("f-a");
    assert!(controller.is_stopped());
    assert!(coordinator.is_cancelled("/data/a"));
    let snap2 = coordinator.get_snapshot();
    assert!(snap2.active_folder.is_none());
}

#[test]
fn test_two_phase_analysis_then_indexing_lifecycle() {
    let coordinator = Arc::new(CrawlCoordinator::new(None));
    let folder = WatchedFolder {
        id: "folder-big".to_string(),
        path: "/data/massive".to_string(),
        enabled: true,
        recursive: true,
        added_at: 0,
        last_scanned_at: None,
    };

    coordinator.enqueue_folders(&[folder.clone()]);
    let snap0 = coordinator.get_snapshot();
    assert_eq!(snap0.pending_queue[0].status, QueueFolderStatus::Queued);

    // Phase 1: Background Analysis starts
    coordinator.start_folder_analysis(&folder.path);
    let snap1 = coordinator.get_snapshot();
    let active1 = snap1.active_folder.expect("active folder present");
    assert_eq!(active1.status, QueueFolderStatus::Analyzing);
    assert_eq!(active1.path, "/data/massive");
    assert!(active1.analysis_report.is_none());

    // Phase 1 completes: sets expected files, ETA, and breakdown report
    let mock_report = serchx_lib::models::FolderAnalysisReport {
        folder_path: "/data/massive".to_string(),
        total_files_scanned: 12000,
        total_indexable_files: 9500,
        total_indexable_bytes: 450_000_000,
        ignored_files_count: 2500,
        documents_count: 5000,
        documents_bytes: 300_000_000,
        images_count: 3000,
        images_bytes: 100_000_000,
        code_count: 1500,
        code_bytes: 50_000_000,
        data_count: 0,
        data_bytes: 0,
        estimated_time_seconds: 140,
        scan_duration_ms: 320,
    };
    coordinator.complete_folder_analysis(&folder.path, mock_report.clone());

    let snap2 = coordinator.get_snapshot();
    let active2 = snap2.active_folder.expect("active folder present");
    assert_eq!(active2.status, QueueFolderStatus::Ready);
    assert_eq!(active2.total_files_expected, 9500);
    assert_eq!(active2.eta_seconds, Some(140));
    assert!(active2.analysis_report.is_some());
    assert_eq!(
        coordinator
            .get_analysis_report(&folder.path)
            .unwrap()
            .total_indexable_files,
        9500
    );

    // Phase 2: Indexing starts seamlessly
    coordinator.start_folder_crawl_with_expected(&folder.path, 9500);
    let snap3 = coordinator.get_snapshot();
    let active3 = snap3.active_folder.expect("active folder present");
    assert_eq!(active3.status, QueueFolderStatus::Scanning);
    assert_eq!(active3.total_files_expected, 9500);
    assert!(active3.analysis_report.is_some());
}
