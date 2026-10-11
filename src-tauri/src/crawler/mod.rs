pub mod control;
pub mod coordinator;
pub mod delete_queue;
pub mod progress;

pub use control::CrawlController;
pub use coordinator::{
    CrawlCoordinator, FileProcessStatus, IndexingQueueSnapshot, ProcessedFileRecord,
    QueueFolderStatus, QueuedFolderItem,
};
pub use progress::CrawlProgress;

use crate::ingest::{IngestionError, IngestionPipeline};
use crate::safety::FileFilter;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Maximum time allowed for processing a single file (SHA-256 + extraction + chunking).
/// Files exceeding this are skipped to prevent one slow file from blocking the entire crawl.
pub const PER_FILE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct CrawlerEngine {
    pipeline: Arc<IngestionPipeline>,
    controller: CrawlController,
    progress: Arc<RwLock<CrawlProgress>>,
    filter: Arc<FileFilter>,
    db_path: Option<PathBuf>,
    coordinator: Option<Arc<CrawlCoordinator>>,
    crawl_lock: Arc<Mutex<()>>,
}

impl CrawlerEngine {
    pub fn new(
        pipeline: Arc<IngestionPipeline>,
        controller: CrawlController,
        filter: Arc<FileFilter>,
        db_path: Option<PathBuf>,
    ) -> Self {
        Self {
            pipeline,
            controller,
            progress: Arc::new(RwLock::new(CrawlProgress::default())),
            filter,
            db_path,
            coordinator: None,
            crawl_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn with_coordinator(
        pipeline: Arc<IngestionPipeline>,
        controller: CrawlController,
        filter: Arc<FileFilter>,
        db_path: Option<PathBuf>,
        coordinator: Arc<CrawlCoordinator>,
    ) -> Self {
        coordinator.set_controller(controller.clone());
        Self {
            pipeline,
            controller,
            progress: Arc::new(RwLock::new(CrawlProgress::default())),
            filter,
            db_path,
            coordinator: Some(coordinator),
            crawl_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn coordinator(&self) -> Option<Arc<CrawlCoordinator>> {
        self.coordinator.clone()
    }

    pub fn controller(&self) -> &CrawlController {
        &self.controller
    }

    pub fn pause(&self) {
        self.controller.pause();
        if let Ok(mut p) = self.progress.write() {
            p.is_paused = true;
        }
        if let Some(ref coord) = self.coordinator {
            coord.set_paused(true);
        }
    }

    pub fn resume(&self) {
        self.controller.resume();
        if let Ok(mut p) = self.progress.write() {
            p.is_paused = false;
        }
        if let Some(ref coord) = self.coordinator {
            coord.set_paused(false);
        }
    }

    pub fn stop(&self) {
        self.controller.stop();
    }

    pub fn get_progress(&self) -> CrawlProgress {
        let mut progress = self.progress.read().map(|p| p.clone()).unwrap_or_default();
        progress.is_paused = self.controller.is_paused();
        progress
    }

    pub fn crawl_paths(&self, paths: &[PathBuf]) -> CrawlProgress {
        let _crawl_guard = match self.crawl_lock.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        tracing::info!("crawl_paths: started for paths: {:?}", paths);

        let is_coord_paused = self
            .coordinator
            .as_ref()
            .map(|c| c.is_paused())
            .unwrap_or(false);

        if is_coord_paused {
            self.controller.reset_stop();
            self.controller.pause();
        } else {
            self.controller.reset();
        }

        let start_time = Instant::now();

        {
            if let Ok(mut p) = self.progress.write() {
                p.is_running = true;
                p.is_paused = is_coord_paused;
                p.current_file = None;
                p.indexed_count = 0;
                p.skipped_count = 0;
                p.failed_count = 0;
                p.total_discovered = 0;
                p.elapsed_millis = 0;
            }
        }

        let blacklisted_roots: Vec<PathBuf> = self
            .pipeline
            .db()
            .get_blacklist_folders()
            .unwrap_or_default()
            .into_iter()
            .map(|b| PathBuf::from(b.path))
            .collect();

        let watched_roots: Vec<PathBuf> =
            if let Ok(folders) = self.pipeline.db().get_watched_folders() {
                let mut w_set: Vec<PathBuf> =
                    folders.into_iter().map(|f| PathBuf::from(f.path)).collect();
                for p in paths {
                    if !w_set.contains(p) {
                        w_set.push(p.clone());
                    }
                }
                w_set
            } else {
                paths.to_vec()
            };

        let mut files_processed = 0usize;

        for root in paths {
            if self.controller.is_stopped() {
                break;
            }

            if let Some(ref coord) = self.coordinator {
                if coord.is_cancelled(&root.to_string_lossy()) {
                    tracing::info!("crawl_paths: skipping cancelled root {:?}", root);
                    continue;
                }
                if coord.is_folder_paused(&root.to_string_lossy()) {
                    self.controller.pause();
                }
            }

            if !root.exists() {
                if let Ok(mut p) = self.progress.write() {
                    p.failed_count += 1;
                }
                if let Some(ref coord) = self.coordinator {
                    coord.complete_folder_crawl(&root.to_string_lossy(), false);
                }
                continue;
            }

            // ── PHASE 1: BACKGROUND ANALYSIS JOB ──────────────────────────────────
            let root_str = root.to_string_lossy().to_string();
            if let Some(ref coord) = self.coordinator {
                coord.start_folder_analysis(&root_str);
            }

            let analysis_res = crate::commands::onboarding::analyze_folder_path(
                &root_str,
                true,
                &self.pipeline.db(),
            );

            // Check if cancelled during analysis
            if let Some(ref coord) = self.coordinator {
                if coord.is_cancelled(&root_str) {
                    tracing::info!("crawl_paths: root {:?} cancelled during analysis", root);
                    continue;
                }
            }
            if self.controller.is_stopped() {
                break;
            }

            let total_expected = match analysis_res {
                Ok(report) => {
                    let count = report.total_indexable_files;
                    if let Some(ref coord) = self.coordinator {
                        coord.complete_folder_analysis(&root_str, report);
                    }
                    count
                }
                Err(err) => {
                    tracing::warn!(
                        "crawl_paths: folder analysis failed for {}: {}",
                        root_str,
                        err
                    );
                    0
                }
            };

            while self.controller.is_paused() {
                if self.controller.is_stopped() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if self.controller.is_stopped() {
                break;
            }

            // ── PHASE 2: STREAMING INDEXING WORKERS ────────────────────────────────
            let filter = self.filter.clone();
            let b_roots = blacklisted_roots.clone();
            let w_roots = watched_roots.clone();

            if let Some(ref coord) = self.coordinator {
                coord.start_folder_crawl_with_expected(&root_str, total_expected);
            }

            if let Ok(mut p) = self.progress.write() {
                p.total_discovered = total_expected;
            }

            let app_settings = self.pipeline.db().get_app_settings().unwrap_or_default();
            let num_workers = app_settings.indexing_threads.max(1);

            let (work_tx, work_rx) = std::sync::mpsc::sync_channel::<PathBuf>(512);
            let work_rx = Arc::new(std::sync::Mutex::new(work_rx));
            let (batch_tx, batch_rx) =
                std::sync::mpsc::sync_channel::<crate::db::IngestionBatchItem>(512);

            let pipeline_clone = self.pipeline.clone();
            let writer_handle = std::thread::spawn(move || {
                tracing::info!("crawl_paths: batch writer thread started");
                let mut batch = Vec::with_capacity(50);
                let mut last_flush = Instant::now();

                loop {
                    match batch_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(item) => {
                            batch.push(item);
                            if batch.len() >= 50 || last_flush.elapsed().as_millis() >= 100 {
                                let _ = pipeline_clone.db().persist_ingestion_batch(&batch);
                                for b_item in &batch {
                                    pipeline_clone.enqueue_for_background_embedding(&b_item.chunks);
                                }
                                batch.clear();
                                last_flush = Instant::now();
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                let _ = pipeline_clone.db().persist_ingestion_batch(&batch);
                                for b_item in &batch {
                                    pipeline_clone.enqueue_for_background_embedding(&b_item.chunks);
                                }
                                batch.clear();
                                last_flush = Instant::now();
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            if !batch.is_empty() {
                                let _ = pipeline_clone.db().persist_ingestion_batch(&batch);
                                for b_item in &batch {
                                    pipeline_clone.enqueue_for_background_embedding(&b_item.chunks);
                                }
                                batch.clear();
                            }
                            break;
                        }
                    }
                }
            });

            let mut worker_handles = Vec::with_capacity(num_workers);
            for _ in 0..num_workers {
                let work_rx = work_rx.clone();
                let batch_tx = batch_tx.clone();
                let pipeline = self.pipeline.clone();
                let controller = self.controller.clone();
                let progress = self.progress.clone();
                let coordinator = self.coordinator.clone();
                let db_path = self.db_path.clone();

                let handle = std::thread::spawn(move || {
                    tracing::info!("crawl_paths: worker thread started");
                    loop {
                        let path: PathBuf = {
                            let rx = match work_rx.lock() {
                                Ok(guard) => guard,
                                Err(_) => break,
                            };
                            match rx.recv() {
                                Ok(p) => p,
                                Err(_) => break,
                            }
                        };

                        if controller.is_stopped() {
                            break;
                        }
                        while controller.is_paused() {
                            if controller.is_stopped() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }

                        let path_string = path.to_string_lossy().to_string();
                        tracing::debug!("crawl_paths: worker processing file: {}", path_string);
                        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                        if let Some(ref coord) = coordinator {
                            coord.start_file_processing(&path_string, file_size);
                        }

                        let pipeline_c = pipeline.clone();
                        let path_c = path.clone();
                        let path_string_c = path_string.clone();
                        let (extract_tx, extract_rx) = std::sync::mpsc::channel();
                        let spawn_res = std::thread::Builder::new()
                            .name("serchx-extract-watchdog".to_string())
                            .spawn(move || {
                                let res = pipeline_c.extract_and_prepare_batch_item(&path_c);
                                let _ = extract_tx.send(res);
                            });

                        let extract_result = match spawn_res {
                            Ok(_) => match extract_rx.recv_timeout(PER_FILE_TIMEOUT) {
                                Ok(res) => res,
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    Err(IngestionError::Timeout(
                                        PER_FILE_TIMEOUT.as_secs(),
                                        path_string_c,
                                    ))
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    Err(IngestionError::Extraction(
                                        "File extraction thread terminated unexpectedly"
                                            .to_string(),
                                    ))
                                }
                            },
                            Err(e) => {
                                tracing::warn!("crawl_paths: could not spawn watchdog thread ({:?}), running inline", e);
                                pipeline.extract_and_prepare_batch_item(&path)
                            }
                        };

                        let file_name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_string.clone());

                        match extract_result {
                            Ok(Some(item)) => {
                                tracing::debug!(
                                    "crawl_paths: successfully extracted item for: {}",
                                    path_string
                                );
                                let _ = batch_tx.send(item);
                                if let Ok(mut p) = progress.write() {
                                    p.indexed_count += 1;
                                    p.current_file = Some(path_string.clone());
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::Indexed,
                                        file_size,
                                    );
                                }
                                let _ = pipeline.db().remove_dead_letter_by_path(&path_string);
                            }
                            Ok(None) => {
                                if let Ok(mut p) = progress.write() {
                                    p.skipped_count += 1;
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::Skipped,
                                        file_size,
                                    );
                                }
                            }
                            Err(IngestionError::Filtered(reason)) => {
                                tracing::info!(
                                    "crawl_paths: file {} filtered: {}",
                                    path_string,
                                    reason
                                );
                                if let Ok(mut p) = progress.write() {
                                    p.skipped_count += 1;
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::Skipped,
                                        file_size,
                                    );
                                }
                            }
                            Err(IngestionError::UnsupportedFormat(reason)) => {
                                tracing::info!(
                                    "crawl_paths: file {} unsupported: {}",
                                    path_string,
                                    reason
                                );
                                if let Ok(mut p) = progress.write() {
                                    p.skipped_count += 1;
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::Skipped,
                                        file_size,
                                    );
                                }
                            }
                            Err(IngestionError::Timeout(secs, ref details)) => {
                                tracing::warn!(
                                    "crawl_paths: file {} timed out after {}s: {}",
                                    path_string,
                                    secs,
                                    details
                                );
                                if let Ok(mut p) = progress.write() {
                                    p.failed_count += 1;
                                    p.timed_out_count += 1;
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::TimedOut,
                                        file_size,
                                    );
                                }
                                let dlq_item = crate::models::DeadLetterItem {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    path: path_string.clone(),
                                    file_name: file_name.clone(),
                                    file_size,
                                    error_type: "Timeout".to_string(),
                                    error_message: format!(
                                        "Processing timed out after {} seconds",
                                        secs
                                    ),
                                    failed_at: chrono::Utc::now().timestamp(),
                                    retry_count: 1,
                                    status: "pending".to_string(),
                                };
                                if let Err(e) =
                                    pipeline.db().insert_or_update_dead_letter(&dlq_item)
                                {
                                    tracing::error!(
                                        "Failed to record file in dead letter queue: {}",
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "crawl_paths: error processing {}: {:?}",
                                    path_string,
                                    e
                                );
                                if let Ok(mut p) = progress.write() {
                                    p.failed_count += 1;
                                }
                                if let Some(ref coord) = coordinator {
                                    coord.record_file_progress(
                                        &path_string,
                                        FileProcessStatus::Error,
                                        file_size,
                                    );
                                }
                                let err_type = match &e {
                                    IngestionError::Io(_) => "IO",
                                    IngestionError::Extraction(_) => "Extraction",
                                    IngestionError::Ocr(_) => "OCR",
                                    IngestionError::Db(_) => "Database",
                                    _ => "General",
                                };
                                let dlq_item = crate::models::DeadLetterItem {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    path: path_string.clone(),
                                    file_name: file_name.clone(),
                                    file_size,
                                    error_type: err_type.to_string(),
                                    error_message: e.to_string(),
                                    failed_at: chrono::Utc::now().timestamp(),
                                    retry_count: 1,
                                    status: "pending".to_string(),
                                };
                                if let Err(err) =
                                    pipeline.db().insert_or_update_dead_letter(&dlq_item)
                                {
                                    tracing::error!(
                                        "Failed to record file in dead letter queue: {}",
                                        err
                                    );
                                }
                            }
                        }

                        if let Ok(mut p) = progress.write() {
                            p.elapsed_millis = start_time.elapsed().as_millis() as u64;
                            if let Some(ref db_p) = db_path {
                                if let Ok(meta) = std::fs::metadata(db_p) {
                                    p.disk_usage_bytes = meta.len();
                                }
                            }
                        }
                    }
                });
                worker_handles.push(handle);
            }
            drop(batch_tx);

            let filter_walk = filter.clone();
            let walker = walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(move |e| {
                    if e.file_type().is_dir() {
                        let path = e.path();
                        // Skip the app's own data directory to prevent self-indexing loops
                        for excluded in &filter_walk.excluded_paths {
                            if crate::safety::filters::is_path_inside_or_equal(path, excluded) {
                                return false;
                            }
                        }
                        if e.depth() > 0 {
                            let name = e.file_name().to_string_lossy();
                            if name.starts_with('.') || filter_walk.is_ignored_directory_name(&name)
                            {
                                return false;
                            }
                        }
                        if crate::safety::filters::should_skip_directory(path, &w_roots, &b_roots) {
                            return false;
                        }
                    }
                    true
                });

            for entry_res in walker {
                if self.controller.is_stopped() {
                    tracing::info!(
                        "crawl_paths: crawler stopped by controller, breaking walker loop"
                    );
                    break;
                }

                while self.controller.is_paused() {
                    if self.controller.is_stopped() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                let entry = match entry_res {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::error!("crawl_paths: walker error: {:?}", e);
                        if let Ok(mut p) = self.progress.write() {
                            p.failed_count += 1;
                        }
                        continue;
                    }
                };

                let path = entry.path();
                if path.is_dir() {
                    continue;
                }

                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                if file_name.starts_with('.')
                    || file_name.ends_with('~')
                    || file_name.ends_with(".tmp")
                    || file_name.starts_with("~$")
                {
                    continue;
                }

                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext.is_empty()
                    || !filter.allowed_extensions.contains(&ext)
                    || filter.blacklisted_extensions.contains(&ext)
                {
                    if let Ok(mut p) = self.progress.write() {
                        p.skipped_count += 1;
                    }
                    continue;
                }

                let perm = crate::safety::filters::resolve_path_indexing_permission(
                    path,
                    &watched_roots,
                    &blacklisted_roots,
                );
                if perm != crate::safety::filters::PathPermissionResult::Allowed {
                    if let Ok(mut p) = self.progress.write() {
                        p.skipped_count += 1;
                    }
                    if let Some(ref coord) = self.coordinator {
                        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                        coord.record_file_progress(
                            &path.to_string_lossy(),
                            FileProcessStatus::Skipped,
                            file_size,
                        );
                    }
                    continue;
                }

                if let Ok(mut p) = self.progress.write() {
                    p.total_discovered += 1;
                }
                if let Some(ref coord) = self.coordinator {
                    coord.increment_discovered();
                }

                files_processed += 1;
                if files_processed % 50 == 0 {
                    let power_status = crate::safety::check_power_conditions();
                    if power_status.should_throttle {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }

                if work_tx.send(path.to_path_buf()).is_err() {
                    break;
                }
            }

            drop(work_tx);
            tracing::info!(
                "crawl_paths: finished discovering files for {:?}. Waiting for {} workers to join.",
                root,
                worker_handles.len()
            );
            for h in worker_handles {
                let _ = h.join();
            }
            tracing::info!("crawl_paths: workers joined. Waiting for writer thread.");
            let _ = writer_handle.join();
            tracing::info!(
                "crawl_paths: writer thread joined. Folder crawl complete for {:?}",
                root
            );

            let now = chrono::Utc::now().timestamp();
            let _ = self
                .pipeline
                .db()
                .update_watched_folder_last_scanned(&root.to_string_lossy(), now);
            if let Some(ref coord) = self.coordinator {
                coord.complete_folder_crawl(&root.to_string_lossy(), true);
            }
        }

        if let Some(ref coord) = self.coordinator {
            if !coord.has_pending_folders() {
                coord.finish_all_crawls();
            }
        }

        if let Ok(mut p) = self.progress.write() {
            p.is_running = false;
            p.current_file = None;
            p.elapsed_millis = start_time.elapsed().as_millis() as u64;
        }

        self.get_progress()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::db::DatabaseManager;
    use crate::ingest::extractors::ExtractorRegistry;
    use crate::ingest::ocr::AppleVisionOcr;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_crawler_engine_crawl_and_telemetry() {
        let temp_dir = tempdir().expect("temp dir");
        let root = temp_dir.path();

        // Create sample files
        let valid_doc = root.join("notes.txt");
        let mut f1 = File::create(&valid_doc).unwrap();
        writeln!(
            f1,
            "Important meeting notes about project scope and timelines."
        )
        .unwrap();

        let valid_md = root.join("README.md");
        let mut f2 = File::create(&valid_md).unwrap();
        writeln!(
            f2,
            "# Architecture Overview\nSystem components and details."
        )
        .unwrap();

        // Create a blacklisted dir
        let git_dir = root.join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        let git_file = git_dir.join("config");
        let mut f3 = File::create(&git_file).unwrap();
        writeln!(f3, "[core]\nrepositoryformatversion = 0").unwrap();

        // Setup pipeline & crawler
        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        let registry = Arc::new(ExtractorRegistry::new());
        let ocr = Arc::new(AppleVisionOcr::new());
        let filter = Arc::new(FileFilter::default());
        let pipeline = Arc::new(IngestionPipeline::with_filter(
            db,
            registry,
            ocr,
            filter.clone(),
        ));
        let controller = CrawlController::new();
        let engine = CrawlerEngine::new(pipeline, controller, filter, None);

        let progress = engine.crawl_paths(&[root.to_path_buf()]);

        // Verifications (STATUS-01):
        // 2 valid files indexed, .git directory skipped
        assert_eq!(progress.indexed_count, 2);
        assert_eq!(progress.failed_count, 0);
        assert!(!progress.is_running);
        assert!(progress.total_discovered >= 2);

        // Crawl again: existing files must be skipped because hash matches!
        let progress2 = engine.crawl_paths(&[root.to_path_buf()]);
        assert_eq!(progress2.indexed_count, 0);
        assert_eq!(progress2.skipped_count, 2);
    }

    #[test]
    fn test_crawler_engine_pause_and_resume() {
        let temp_dir = tempdir().expect("temp dir");
        let root = temp_dir.path();

        for i in 0..10 {
            let p = root.join(format!("doc_{}.txt", i));
            let mut f = File::create(p).unwrap();
            writeln!(f, "File content {}", i).unwrap();
        }

        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        let registry = Arc::new(ExtractorRegistry::new());
        let ocr = Arc::new(AppleVisionOcr::new());
        let filter = Arc::new(FileFilter::default());
        let pipeline = Arc::new(IngestionPipeline::with_filter(
            db,
            registry,
            ocr,
            filter.clone(),
        ));
        let controller = CrawlController::new();
        let engine = Arc::new(CrawlerEngine::new(pipeline, controller, filter, None));

        let engine_clone = engine.clone();
        let root_buf = root.to_path_buf();
        let handle = std::thread::spawn(move || {
            engine_clone.crawl_paths(&[root_buf]);
        });

        // Wait for crawl to start
        for _ in 0..50 {
            if engine.get_progress().is_running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // Trigger pause
        engine.pause();
        assert!(engine.get_progress().is_paused);

        // Trigger resume
        engine.resume();
        assert!(!engine.get_progress().is_paused);

        handle.join().unwrap();
        let final_progress = engine.get_progress();
        assert_eq!(final_progress.indexed_count, 10);
    }
}
