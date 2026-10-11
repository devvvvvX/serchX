use crate::crawler::CrawlController;
use crate::models::WatchedFolder;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueFolderStatus {
    Analyzing,
    Ready,
    Queued,
    Scanning,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileProcessStatus {
    Indexed,
    Skipped,
    Error,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedFolderItem {
    pub id: String,
    pub path: String,
    pub status: QueueFolderStatus,
    pub total_files_discovered: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    #[serde(default)]
    pub total_files_expected: usize,
    pub queued_at: i64,
    /// Per-folder pause flag (UI intent; actual engine pause is global)
    #[serde(default)]
    pub is_paused: bool,
    /// ETA in seconds remaining, based on live throughput
    pub eta_seconds: Option<u64>,
    /// Computed files-per-second throughput for ETA
    #[serde(default)]
    pub throughput_fps: f64,
    /// Unix timestamp when this folder's crawl started
    pub folder_started_at: Option<i64>,
    /// Pre-indexing analysis report (breakdown by file type and category)
    #[serde(default)]
    pub analysis_report: Option<crate::models::FolderAnalysisReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedFileRecord {
    pub path: String,
    pub extension: String,
    pub status: FileProcessStatus,
    pub size_bytes: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexingQueueSnapshot {
    pub is_active: bool,
    pub is_paused: bool,
    pub active_folder: Option<QueuedFolderItem>,
    pub pending_queue: Vec<QueuedFolderItem>,
    pub current_file: Option<String>,
    #[serde(default)]
    pub current_file_size: u64,
    pub total_queue_depth: usize,
    pub recent_events: Vec<ProcessedFileRecord>,
    pub elapsed_millis: u64,
    /// Total files processed across all folders in this session
    pub session_files_processed: usize,
}

struct CoordinatorInner {
    active_folder: Option<QueuedFolderItem>,
    pending_queue: VecDeque<QueuedFolderItem>,
    recent_events: VecDeque<ProcessedFileRecord>,
    current_file: Option<String>,
    current_file_size: u64,
    is_paused: bool,
    start_time: Option<Instant>,
    folder_start_time: Option<Instant>,
    last_emit: Instant,
    session_files_processed: usize,
    cancelled_folders: HashSet<String>,
}

pub struct CrawlCoordinator {
    inner: Arc<RwLock<CoordinatorInner>>,
    controller: Arc<RwLock<Option<CrawlController>>>,
    app_handle: Arc<RwLock<Option<AppHandle>>>,
    emit_lock: Arc<Mutex<()>>,
}

fn normalize_path_str(p: &str) -> &str {
    let trimmed = p.trim();
    if trimmed.len() > 1 {
        trimmed.trim_end_matches('/')
    } else {
        trimmed
    }
}

pub fn paths_match(a: &str, b: &str) -> bool {
    let na = normalize_path_str(a);
    let nb = normalize_path_str(b);
    if na == nb {
        return true;
    }
    if let (Ok(pa), Ok(pb)) = (std::fs::canonicalize(na), std::fs::canonicalize(nb)) {
        if pa == pb {
            return true;
        }
    }
    false
}

impl CrawlCoordinator {
    pub fn new(app_handle: Option<AppHandle>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(CoordinatorInner {
                active_folder: None,
                pending_queue: VecDeque::new(),
                recent_events: VecDeque::with_capacity(50),
                current_file: None,
                current_file_size: 0,
                is_paused: false,
                start_time: None,
                folder_start_time: None,
                last_emit: Instant::now() - std::time::Duration::from_secs(10),
                session_files_processed: 0,
                cancelled_folders: HashSet::new(),
            })),
            controller: Arc::new(RwLock::new(None)),
            app_handle: Arc::new(RwLock::new(app_handle)),
            emit_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut h) = self.app_handle.write() {
            *h = Some(handle);
        }
    }

    pub fn set_controller(&self, controller: CrawlController) {
        if let Ok(mut c) = self.controller.write() {
            *c = Some(controller);
        }
    }

    pub fn enqueue_folders(&self, folders: &[WatchedFolder]) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let now = chrono::Utc::now().timestamp();
        for f in folders {
            let already_pending = inner
                .pending_queue
                .iter()
                .any(|q| q.id == f.id || paths_match(&q.path, &f.path));
            let is_active = inner
                .active_folder
                .as_ref()
                .map(|a| a.id == f.id || paths_match(&a.path, &f.path))
                .unwrap_or(false);
            if !already_pending && !is_active {
                inner.pending_queue.push_back(QueuedFolderItem {
                    id: f.id.clone(),
                    path: f.path.clone(),
                    status: QueueFolderStatus::Queued,
                    total_files_discovered: 0,
                    files_indexed: 0,
                    files_skipped: 0,
                    files_failed: 0,
                    total_files_expected: 0,
                    queued_at: now,
                    is_paused: false,
                    eta_seconds: None,
                    throughput_fps: 0.0,
                    folder_started_at: None,
                    analysis_report: None,
                });
            }
        }
        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn start_folder_analysis(&self, folder_path: &str) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let now_ts = chrono::Utc::now().timestamp();
        if inner.start_time.is_none() {
            inner.start_time = Some(Instant::now());
        }
        inner.folder_start_time = Some(Instant::now());

        let mut item = if let Some(idx) = inner
            .pending_queue
            .iter()
            .position(|q| paths_match(&q.path, folder_path))
        {
            tracing::info!("CrawlCoordinator::start_folder_analysis: found '{}' in pending_queue (idx {}), promoting to active", folder_path, idx);
            inner.pending_queue.remove(idx).unwrap()
        } else if let Some(active) = inner.active_folder.take() {
            if paths_match(&active.path, folder_path) {
                tracing::info!("CrawlCoordinator::start_folder_analysis: reusing matching active_folder for '{}'", folder_path);
                active
            } else {
                tracing::info!(
                    "CrawlCoordinator::start_folder_analysis: preserving existing active_folder '{}' into pending_queue for new folder '{}'",
                    active.path, folder_path
                );
                inner.pending_queue.push_back(active);
                QueuedFolderItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: folder_path.to_string(),
                    status: QueueFolderStatus::Analyzing,
                    total_files_discovered: 0,
                    files_indexed: 0,
                    files_skipped: 0,
                    files_failed: 0,
                    total_files_expected: 0,
                    queued_at: now_ts,
                    is_paused: false,
                    eta_seconds: None,
                    throughput_fps: 0.0,
                    folder_started_at: Some(now_ts),
                    analysis_report: None,
                }
            }
        } else {
            tracing::info!("CrawlCoordinator::start_folder_analysis: no existing slot found, creating new item for '{}'", folder_path);
            QueuedFolderItem {
                id: uuid::Uuid::new_v4().to_string(),
                path: folder_path.to_string(),
                status: QueueFolderStatus::Analyzing,
                total_files_discovered: 0,
                files_indexed: 0,
                files_skipped: 0,
                files_failed: 0,
                total_files_expected: 0,
                queued_at: now_ts,
                is_paused: false,
                eta_seconds: None,
                throughput_fps: 0.0,
                folder_started_at: Some(now_ts),
                analysis_report: None,
            }
        };

        item.path = folder_path.to_string();
        if item.id.is_empty() {
            item.id = uuid::Uuid::new_v4().to_string();
        }

        let is_already_paused = inner.is_paused || item.is_paused;
        item.status = QueueFolderStatus::Analyzing;
        item.is_paused = is_already_paused;
        item.folder_started_at = Some(now_ts);
        inner.active_folder = Some(item);

        tracing::info!(
            "CrawlCoordinator::start_folder_analysis: active_folder set to Analyzing for '{}'",
            folder_path
        );
        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn complete_folder_analysis(
        &self,
        folder_path: &str,
        report: crate::models::FolderAnalysisReport,
    ) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let mut matched = false;

        if let Some(ref mut active) = inner.active_folder {
            if paths_match(&active.path, folder_path) {
                tracing::info!(
                    "CrawlCoordinator::complete_folder_analysis: matched active_folder for '{}' \
                     — transitioning Analyzing → Ready ({} indexable files, ~{}s ETA)",
                    folder_path,
                    report.total_indexable_files,
                    report.estimated_time_seconds
                );
                active.status = QueueFolderStatus::Ready;
                active.total_files_expected = report.total_indexable_files;
                active.eta_seconds = Some(report.estimated_time_seconds);
                active.analysis_report = Some(report.clone());
                matched = true;
            } else {
                tracing::warn!(
                    "CrawlCoordinator::complete_folder_analysis: active_folder path '{}' \
                     does NOT match target '{}' — will check pending_queue",
                    active.path,
                    folder_path
                );
            }
        } else {
            tracing::warn!(
                "CrawlCoordinator::complete_folder_analysis: no active_folder present when completing analysis for '{}' — will check pending_queue",
                folder_path
            );
        }

        for item in inner.pending_queue.iter_mut() {
            if paths_match(&item.path, folder_path) {
                tracing::info!(
                    "CrawlCoordinator::complete_folder_analysis: matched pending_queue item for '{}' \
                     — transitioning Analyzing → Ready",
                    folder_path
                );
                item.status = QueueFolderStatus::Ready;
                item.total_files_expected = report.total_indexable_files;
                item.eta_seconds = Some(report.estimated_time_seconds);
                item.analysis_report = Some(report.clone());
                matched = true;
            }
        }

        if !matched {
            tracing::error!(
                "CrawlCoordinator::complete_folder_analysis: '{}' not found in active_folder or pending_queue — \
                 status will remain Analyzing and UI will be stuck! (active={:?}, queue_len={})",
                folder_path,
                inner.active_folder.as_ref().map(|a| a.path.as_str()),
                inner.pending_queue.len()
            );
        }

        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn get_analysis_report(
        &self,
        folder_path: &str,
    ) -> Option<crate::models::FolderAnalysisReport> {
        let inner = self.inner.read().ok()?;
        if let Some(ref active) = inner.active_folder {
            if paths_match(&active.path, folder_path) {
                return active.analysis_report.clone();
            }
        }
        for item in &inner.pending_queue {
            if paths_match(&item.path, folder_path) {
                return item.analysis_report.clone();
            }
        }
        None
    }

    pub fn start_folder_crawl(&self, folder_path: &str) {
        self.start_folder_crawl_with_expected(folder_path, 0);
    }

    pub fn start_folder_crawl_streaming(&self, folder_path: &str) {
        self.start_folder_crawl_with_expected(folder_path, 0);
    }

    pub fn increment_discovered(&self) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if let Some(ref mut active) = inner.active_folder {
            active.total_files_discovered += 1;
        }
    }

    pub fn start_folder_crawl_with_expected(&self, folder_path: &str, total_files_expected: usize) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let now_ts = chrono::Utc::now().timestamp();

        if inner.start_time.is_none() {
            inner.start_time = Some(Instant::now());
        }
        inner.folder_start_time = Some(Instant::now());

        // If this folder was active (e.g. from start_folder_analysis), reuse it
        let mut item = if let Some(active) = inner.active_folder.take() {
            if paths_match(&active.path, folder_path) {
                active
            } else {
                inner.pending_queue.push_back(active);
                if let Some(idx) = inner
                    .pending_queue
                    .iter()
                    .position(|q| paths_match(&q.path, folder_path))
                {
                    inner.pending_queue.remove(idx).unwrap()
                } else {
                    QueuedFolderItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: folder_path.to_string(),
                        status: QueueFolderStatus::Scanning,
                        total_files_discovered: 0,
                        files_indexed: 0,
                        files_skipped: 0,
                        files_failed: 0,
                        total_files_expected,
                        queued_at: now_ts,
                        is_paused: false,
                        eta_seconds: None,
                        throughput_fps: 0.0,
                        folder_started_at: Some(now_ts),
                        analysis_report: None,
                    }
                }
            }
        } else if let Some(idx) = inner
            .pending_queue
            .iter()
            .position(|q| paths_match(&q.path, folder_path))
        {
            inner.pending_queue.remove(idx).unwrap()
        } else {
            QueuedFolderItem {
                id: uuid::Uuid::new_v4().to_string(),
                path: folder_path.to_string(),
                status: QueueFolderStatus::Scanning,
                total_files_discovered: 0,
                files_indexed: 0,
                files_skipped: 0,
                files_failed: 0,
                total_files_expected,
                queued_at: now_ts,
                is_paused: false,
                eta_seconds: None,
                throughput_fps: 0.0,
                folder_started_at: Some(now_ts),
                analysis_report: None,
            }
        };

        let is_already_paused = inner.is_paused || item.is_paused;
        item.status = QueueFolderStatus::Scanning;
        if total_files_expected > 0 {
            item.total_files_expected = total_files_expected;
        }
        if item.eta_seconds.is_none() && item.analysis_report.is_some() {
            item.eta_seconds = item
                .analysis_report
                .as_ref()
                .map(|r| r.estimated_time_seconds);
        }
        item.is_paused = is_already_paused;
        item.folder_started_at = Some(now_ts);
        inner.active_folder = Some(item);

        if is_already_paused {
            if let Ok(c_guard) = self.controller.read() {
                if let Some(ref ctrl) = *c_guard {
                    ctrl.pause();
                }
            }
        }

        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn start_file_processing(&self, path: &str, file_size: u64) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        inner.current_file = Some(path.to_string());
        inner.current_file_size = file_size;
        drop(inner);
        self.emit_snapshot_throttled(false);
    }

    pub fn record_file_progress(&self, path: &str, status: FileProcessStatus, size_bytes: u64) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        inner.current_file = Some(path.to_string());
        inner.current_file_size = size_bytes;
        inner.session_files_processed += 1;

        // Pre-compute folder_elapsed before mutably borrowing active_folder (borrow splitter)
        let folder_elapsed_opt = inner.folder_start_time.map(|t| t.elapsed().as_secs_f64());

        if let Some(ref mut active) = inner.active_folder {
            match status {
                FileProcessStatus::Indexed => active.files_indexed += 1,
                FileProcessStatus::Skipped => active.files_skipped += 1,
                FileProcessStatus::Error | FileProcessStatus::TimedOut => active.files_failed += 1,
            }

            // Compute live ETA from folder_start_time throughput
            if let Some(folder_elapsed) = folder_elapsed_opt {
                let files_processed =
                    active.files_indexed + active.files_skipped + active.files_failed;
                if folder_elapsed > 0.5 && files_processed > 0 {
                    let fps = files_processed as f64 / folder_elapsed;
                    active.throughput_fps = fps;
                    let total = active
                        .total_files_expected
                        .max(active.total_files_discovered)
                        .max(files_processed);
                    let remaining = total.saturating_sub(files_processed);
                    if remaining > 0 && fps > 0.0 {
                        active.eta_seconds = Some((remaining as f64 / fps).ceil() as u64);
                    } else {
                        active.eta_seconds = None;
                    }
                }
            }
        }

        let extension = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let record = ProcessedFileRecord {
            path: path.to_string(),
            extension,
            status,
            size_bytes,
            timestamp: chrono::Utc::now().timestamp(),
        };

        if inner.recent_events.len() >= 50 {
            inner.recent_events.pop_front();
        }
        inner.recent_events.push_back(record);

        drop(inner);
        self.emit_snapshot_throttled(false);
    }

    pub fn complete_folder_crawl(&self, folder_path: &str, success: bool) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if let Some(mut active) = inner.active_folder.take() {
            active.status = if success {
                QueueFolderStatus::Completed
            } else {
                QueueFolderStatus::Failed
            };
        }
        // Also remove any matching item in pending_queue if still present
        if let Some(idx) = inner
            .pending_queue
            .iter()
            .position(|q| paths_match(&q.path, folder_path))
        {
            inner.pending_queue.remove(idx);
        }

        inner.current_file = None;
        inner.current_file_size = 0;

        if inner.pending_queue.is_empty() {
            inner.start_time = None;
        }

        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn finish_all_crawls(&self) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        inner.active_folder = None;
        inner.pending_queue.clear();
        inner.current_file = None;
        inner.current_file_size = 0;
        inner.start_time = None;
        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn cancel_folder(&self, folder_id: &str) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        inner.cancelled_folders.insert(folder_id.to_string());

        // Remove from pending queue if present
        if let Some(idx) = inner
            .pending_queue
            .iter()
            .position(|q| q.id == folder_id || paths_match(&q.path, folder_id))
        {
            let q = inner.pending_queue.remove(idx).unwrap();
            inner.cancelled_folders.insert(q.path);
        }

        // If it's the currently active folder, stop the crawl controller
        let active_match = inner.active_folder.as_ref().and_then(|active| {
            if active.id == folder_id || paths_match(&active.path, folder_id) {
                Some(active.path.clone())
            } else {
                None
            }
        });

        if let Some(active_path) = active_match {
            inner.cancelled_folders.insert(active_path);
            if let Ok(c_guard) = self.controller.read() {
                if let Some(ref ctrl) = *c_guard {
                    ctrl.stop();
                }
            }
            inner.active_folder = None;
            inner.current_file = None;
        }

        drop(inner);
        self.emit_snapshot_throttled(true);
    }

    pub fn set_paused(&self, paused: bool) {
        if let Ok(mut inner) = self.inner.write() {
            inner.is_paused = paused;
            // Propagate global pause state to active folder
            if let Some(ref mut active) = inner.active_folder {
                active.is_paused = paused;
            }
            // Also propagate to all items in pending queue
            for q in inner.pending_queue.iter_mut() {
                q.is_paused = paused;
            }
        }
        self.emit_snapshot_throttled(true);
    }

    pub fn is_paused(&self) -> bool {
        self.inner.read().map(|i| i.is_paused).unwrap_or(false)
    }

    pub fn has_pending_folders(&self) -> bool {
        self.inner
            .read()
            .map(|i| !i.pending_queue.is_empty())
            .unwrap_or(false)
    }

    pub fn is_folder_paused(&self, folder_path: &str) -> bool {
        if let Ok(inner) = self.inner.read() {
            if inner.is_paused {
                return true;
            }
            if let Some(ref active) = inner.active_folder {
                if paths_match(&active.path, folder_path) {
                    return active.is_paused;
                }
            }
            for q in &inner.pending_queue {
                if paths_match(&q.path, folder_path) {
                    return q.is_paused;
                }
            }
        }
        false
    }

    pub fn is_cancelled(&self, folder_path: &str) -> bool {
        if let Ok(mut inner) = self.inner.write() {
            if inner.cancelled_folders.remove(folder_path) {
                return true;
            }
            let mut to_remove = None;
            for c in inner.cancelled_folders.iter() {
                if paths_match(c, folder_path) {
                    to_remove = Some(c.clone());
                    break;
                }
            }
            if let Some(key) = to_remove {
                inner.cancelled_folders.remove(&key);
                return true;
            }
        }
        false
    }

    /// Pause indexing for a specific folder by ID.
    /// If the folder is currently active, also pauses the crawl engine.
    /// If the folder is queued, marks it as pre-paused so it will start paused.
    pub fn pause_folder(&self, folder_id: &str) {
        let is_active = {
            if let Ok(mut inner) = self.inner.write() {
                let active = inner.active_folder.as_mut().and_then(|a| {
                    if a.id == folder_id || paths_match(&a.path, folder_id) {
                        Some(a)
                    } else {
                        None
                    }
                });
                if let Some(a) = active {
                    a.is_paused = true;
                    true
                } else {
                    // Try pending queue
                    for q in inner.pending_queue.iter_mut() {
                        if q.id == folder_id || paths_match(&q.path, folder_id) {
                            q.is_paused = true;
                            break;
                        }
                    }
                    false
                }
            } else {
                false
            }
        };
        if is_active {
            // Pause the crawl engine for active folder
            if let Ok(c_guard) = self.controller.read() {
                if let Some(ref ctrl) = *c_guard {
                    ctrl.pause();
                }
            }
            if let Ok(mut inner) = self.inner.write() {
                inner.is_paused = true;
            }
        }
        self.emit_snapshot_throttled(true);
    }

    /// Resume indexing for a specific folder by ID.
    /// If the folder is currently active (paused), resumes the crawl engine.
    pub fn resume_folder(&self, folder_id: &str) {
        let is_active = {
            if let Ok(mut inner) = self.inner.write() {
                let active = inner.active_folder.as_mut().and_then(|a| {
                    if a.id == folder_id || paths_match(&a.path, folder_id) {
                        Some(a)
                    } else {
                        None
                    }
                });
                if let Some(a) = active {
                    a.is_paused = false;
                    true
                } else {
                    for q in inner.pending_queue.iter_mut() {
                        if q.id == folder_id || paths_match(&q.path, folder_id) {
                            q.is_paused = false;
                            break;
                        }
                    }
                    false
                }
            } else {
                false
            }
        };
        if is_active {
            if let Ok(c_guard) = self.controller.read() {
                if let Some(ref ctrl) = *c_guard {
                    ctrl.resume();
                }
            }
            if let Ok(mut inner) = self.inner.write() {
                inner.is_paused = false;
            }
        }
        self.emit_snapshot_throttled(true);
    }

    pub fn get_snapshot(&self) -> IndexingQueueSnapshot {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(_) => return IndexingQueueSnapshot::default(),
        };

        let elapsed_millis = inner
            .start_time
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let total_queue_depth =
            inner.pending_queue.len() + if inner.active_folder.is_some() { 1 } else { 0 };

        let is_scanning = inner
            .active_folder
            .as_ref()
            .map(|a| a.status == QueueFolderStatus::Scanning)
            .unwrap_or(false);
        let is_analyzing = inner
            .active_folder
            .as_ref()
            .map(|a| a.status == QueueFolderStatus::Analyzing)
            .unwrap_or(false)
            || inner
                .pending_queue
                .iter()
                .any(|q| q.status == QueueFolderStatus::Analyzing);
        let has_queued = inner
            .pending_queue
            .iter()
            .any(|q| q.status == QueueFolderStatus::Queued);

        IndexingQueueSnapshot {
            is_active: is_scanning || is_analyzing || has_queued,
            is_paused: inner.is_paused,
            active_folder: inner.active_folder.clone(),
            pending_queue: inner.pending_queue.iter().cloned().collect(),
            current_file: inner.current_file.clone(),
            current_file_size: inner.current_file_size,
            total_queue_depth,
            recent_events: inner.recent_events.iter().cloned().collect(),
            elapsed_millis,
            session_files_processed: inner.session_files_processed,
        }
    }

    fn emit_snapshot_throttled(&self, force: bool) {
        let _lock = match self.emit_lock.lock() {
            Ok(l) => l,
            Err(_) => return,
        };

        let should_emit = {
            if let Ok(mut inner) = self.inner.write() {
                if force || inner.last_emit.elapsed().as_millis() >= 150 {
                    inner.last_emit = Instant::now();
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_emit {
            let snapshot = self.get_snapshot();
            if let Ok(handle_guard) = self.app_handle.read() {
                if let Some(ref app) = *handle_guard {
                    let _ = app.emit("indexing-queue-update", &snapshot);
                }
            }
        }
    }
}
