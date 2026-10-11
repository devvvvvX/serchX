use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use tauri::{AppHandle, Emitter};

// ─── Job kinds ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteJobKind {
    /// Remove index data for a single folder path
    FolderIndex { path: String },
    /// Remove folder from watchlist + purge its index data
    RemoveFolder { folder_id: String, path: String },
    /// Wipe the whole database (clean DB / factory reset)
    PurgeAll { factory_reset: bool },
}

impl std::fmt::Display for DeleteJobKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeleteJobKind::FolderIndex { path } => write!(f, "FolderIndex('{}')", path),
            DeleteJobKind::RemoveFolder { folder_id, path } => {
                write!(f, "RemoveFolder(id='{}', path='{}')", folder_id, path)
            }
            DeleteJobKind::PurgeAll { factory_reset } => {
                write!(f, "PurgeAll(factory_reset={})", factory_reset)
            }
        }
    }
}

// ─── Job status ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteJobStatus {
    /// Waiting to start
    Pending,
    /// Actively deleting
    Running,
    /// User requested pause; job will pause between batch steps
    Paused,
    /// User stopped the job before it finished; data may be partially deleted
    Stopped,
    /// Job completed successfully
    Done,
    /// Job failed with an error
    Failed,
}

// ─── Job record ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteJob {
    pub id: String,
    pub kind: DeleteJobKind,
    pub status: DeleteJobStatus,
    /// Total items to delete (files)
    pub total: usize,
    /// Items deleted so far
    pub deleted: usize,
    /// Human-readable label shown in UI
    pub label: String,
    /// Unix timestamp when enqueued
    pub queued_at: i64,
    /// Error message if status == Failed
    pub error: Option<String>,
    /// After a stop: ask whether to re-monitor & reindex (for folder-scoped jobs)
    pub can_requeue: bool,
}

impl DeleteJob {
    pub fn progress_pct(&self) -> u8 {
        if self.total == 0 {
            return 0;
        }
        ((self.deleted as f64 / self.total as f64) * 100.0).round() as u8
    }
}

// ─── Queue snapshot (what gets emitted) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeleteQueueSnapshot {
    pub jobs: Vec<DeleteJob>,
    pub is_running: bool,
}

// ─── Per-job control flags ────────────────────────────────────────────────────

#[derive(Default)]
pub struct JobControl {
    pub pause: AtomicBool,
    pub stop: AtomicBool,
}

impl JobControl {
    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::Relaxed)
    }
    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn pause(&self) {
        self.pause.store(true, Ordering::Relaxed);
    }
    pub fn resume(&self) {
        self.pause.store(false, Ordering::Relaxed);
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// ─── Queue ───────────────────────────────────────────────────────────────────

pub struct DeleteQueue {
    jobs: Arc<RwLock<Vec<DeleteJob>>>,
    controls: Arc<Mutex<std::collections::HashMap<String, Arc<JobControl>>>>,
    app_handle: Arc<RwLock<Option<AppHandle>>>,
}

impl DeleteQueue {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(Vec::new())),
            controls: Arc::new(Mutex::new(std::collections::HashMap::new())),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut h) = self.app_handle.write() {
            *h = Some(handle);
        }
    }

    // ── Snapshot ──────────────────────────────────────────────────────────────

    pub fn snapshot(&self) -> DeleteQueueSnapshot {
        let jobs = self.jobs.read().map(|j| j.clone()).unwrap_or_default();
        let is_running = jobs.iter().any(|j| j.status == DeleteJobStatus::Running);
        DeleteQueueSnapshot { jobs, is_running }
    }

    fn emit(&self) {
        let snap = self.snapshot();
        if let Ok(h) = self.app_handle.read() {
            if let Some(ref app) = *h {
                let _ = app.emit("delete-queue-update", &snap);
            }
        }
    }

    // ── Enqueue ───────────────────────────────────────────────────────────────

    /// Add a job and return its ID and control handle.
    pub fn enqueue(
        &self,
        kind: DeleteJobKind,
        label: String,
        total: usize,
    ) -> (String, Arc<JobControl>) {
        let id = uuid::Uuid::new_v4().to_string();
        let ctrl = Arc::new(JobControl::default());

        let job = DeleteJob {
            id: id.clone(),
            kind: kind.clone(),
            status: DeleteJobStatus::Pending,
            total,
            deleted: 0,
            label: label.clone(),
            queued_at: chrono::Utc::now().timestamp(),
            error: None,
            can_requeue: matches!(
                kind,
                DeleteJobKind::FolderIndex { .. } | DeleteJobKind::RemoveFolder { .. }
            ),
        };

        tracing::info!(
            "DeleteQueue: Enqueued job [{}] '{}' (kind: {}, initial_total: {})",
            id,
            label,
            kind,
            total
        );

        if let Ok(mut jobs) = self.jobs.write() {
            jobs.push(job);
        }
        if let Ok(mut ctrls) = self.controls.lock() {
            ctrls.insert(id.clone(), ctrl.clone());
        }
        self.emit();
        (id, ctrl)
    }

    // ── Control ───────────────────────────────────────────────────────────────

    pub fn set_running(&self, id: &str) {
        tracing::info!("DeleteQueue: Job [{}] transitioned to RUNNING", id);
        self.update_status(id, DeleteJobStatus::Running);
    }

    pub fn set_paused(&self, id: &str) {
        tracing::info!("DeleteQueue: Job [{}] transitioned to PAUSED", id);
        self.update_status(id, DeleteJobStatus::Paused);
    }

    pub fn set_stopped(&self, id: &str) {
        tracing::warn!("DeleteQueue: Job [{}] transitioned to STOPPED", id);
        self.update_status(id, DeleteJobStatus::Stopped);
    }

    pub fn set_total(&self, id: &str, total: usize) {
        tracing::info!("DeleteQueue: Job [{}] total items updated to {}", id, total);
        if let Ok(mut jobs) = self.jobs.write() {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.total = total;
            }
        }
        self.emit();
    }

    pub fn set_done(&self, id: &str, deleted: usize) {
        tracing::info!(
            "DeleteQueue: Job [{}] transitioned to DONE (purged {} items)",
            id,
            deleted
        );
        if let Ok(mut jobs) = self.jobs.write() {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.status = DeleteJobStatus::Done;
                j.deleted = deleted;
                if j.total < deleted {
                    j.total = deleted;
                }
            }
        }
        if let Ok(mut ctrls) = self.controls.lock() {
            ctrls.remove(id);
        }
        self.emit();
    }

    pub fn set_failed(&self, id: &str, err: String) {
        tracing::error!("DeleteQueue: Job [{}] FAILED: {}", id, err);
        if let Ok(mut jobs) = self.jobs.write() {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.status = DeleteJobStatus::Failed;
                j.error = Some(err);
            }
        }
        if let Ok(mut ctrls) = self.controls.lock() {
            ctrls.remove(id);
        }
        self.emit();
    }

    pub fn update_progress(&self, id: &str, deleted: usize) {
        let mut total = 0;
        let mut label = String::new();
        if let Ok(mut jobs) = self.jobs.write() {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.deleted = deleted;
                total = j.total;
                label = j.label.clone();
            }
        }
        if total > 0 {
            let pct = ((deleted as f64 / total as f64) * 100.0).round() as usize;
            tracing::info!(
                "DeleteQueue: Job [{}] '{}' progress: {}/{} files ({}%)",
                id,
                label,
                deleted,
                total,
                pct
            );
        } else {
            tracing::info!(
                "DeleteQueue: Job [{}] '{}' progress: {} files deleted",
                id,
                label,
                deleted
            );
        }
        self.emit();
    }

    fn update_status(&self, id: &str, status: DeleteJobStatus) {
        if let Ok(mut jobs) = self.jobs.write() {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.status = status;
            }
        }
        self.emit();
    }

    // ── Control from UI commands ──────────────────────────────────────────────

    pub fn pause_job(&self, id: &str) -> bool {
        tracing::info!("DeleteQueue: pause_job requested for [{}]", id);
        if let Ok(ctrls) = self.controls.lock() {
            if let Some(ctrl) = ctrls.get(id) {
                ctrl.pause();
                drop(ctrls);
                self.set_paused(id);
                return true;
            }
        }
        tracing::warn!(
            "DeleteQueue: pause_job failed — job [{}] not found in active controls",
            id
        );
        false
    }

    pub fn resume_job(&self, id: &str) -> bool {
        tracing::info!("DeleteQueue: resume_job requested for [{}]", id);
        if let Ok(ctrls) = self.controls.lock() {
            if let Some(ctrl) = ctrls.get(id) {
                ctrl.resume();
                drop(ctrls);
                self.set_running(id);
                return true;
            }
        }
        tracing::warn!(
            "DeleteQueue: resume_job failed — job [{}] not found in active controls",
            id
        );
        false
    }

    pub fn stop_job(&self, id: &str) -> bool {
        tracing::warn!("DeleteQueue: stop_job requested for [{}]", id);
        if let Ok(ctrls) = self.controls.lock() {
            if let Some(ctrl) = ctrls.get(id) {
                ctrl.stop();
                drop(ctrls);
                self.set_stopped(id);
                return true;
            }
        }
        tracing::warn!(
            "DeleteQueue: stop_job failed — job [{}] not found in active controls",
            id
        );
        false
    }

    /// Remove a finished/stopped/failed job from the list
    pub fn dismiss_job(&self, id: &str) {
        tracing::info!("DeleteQueue: Dismissing job [{}]", id);
        if let Ok(mut jobs) = self.jobs.write() {
            jobs.retain(|j| j.id != id);
        }
        self.emit();
    }

    /// Return a clone of a job by ID (for requeue decisions)
    pub fn get_job(&self, id: &str) -> Option<DeleteJob> {
        self.jobs.read().ok()?.iter().find(|j| j.id == id).cloned()
    }
}

impl Default for DeleteQueue {
    fn default() -> Self {
        Self::new()
    }
}
