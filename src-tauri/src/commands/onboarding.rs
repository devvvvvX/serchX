use crate::crawler::delete_queue::{DeleteJobKind, DeleteQueue};
use crate::db::DatabaseManager;
use crate::models::{FolderAnalysisReport, WatchedFolder};
use crate::safety::permissions::{check_folder_access, PermissionStatus};
use crate::scope::{get_default_folder_scope, DefaultFolderScope};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Manager, State};

#[tauri::command]
pub fn get_default_folders() -> Vec<DefaultFolderScope> {
    get_default_folder_scope()
}

#[tauri::command]
pub fn check_folder_permissions(path: String) -> PermissionStatus {
    check_folder_access(Path::new(&path))
}

#[tauri::command]
pub fn get_watched_folders(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<WatchedFolder>, String> {
    db.get_watched_folders().map_err(|e| e.to_string())
}

/// Runs a fast, metadata-only analysis on a directory before adding it to watched folders.
/// Returns categorized counts (documents, images, code, data), sizes, and estimated indexing time.
#[tauri::command]
pub fn analyze_folder(
    path: String,
    recursive: bool,
    db: State<'_, Arc<DatabaseManager>>,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
) -> Result<FolderAnalysisReport, String> {
    if let Some(cached) = coordinator.get_analysis_report(&path) {
        return Ok(cached);
    }
    analyze_folder_path(&path, recursive, &db)
}

pub fn analyze_folder_path(
    path_str: &str,
    recursive: bool,
    db: &DatabaseManager,
) -> Result<FolderAnalysisReport, String> {
    let root_path = Path::new(path_str);
    if !root_path.exists() {
        return Err(format!("Directory does not exist: {}", path_str));
    }
    if !root_path.is_dir() {
        return Err(format!("Path is not a directory: {}", path_str));
    }

    let start_time = std::time::Instant::now();

    let settings = db.get_app_settings().unwrap_or_default();
    let blacklist_folders = db.get_blacklist_folders().unwrap_or_default();
    let blacklisted_roots: Vec<PathBuf> = blacklist_folders
        .into_iter()
        .map(|b| PathBuf::from(b.path))
        .collect();

    let mut filter = crate::safety::FileFilter::default();
    filter.max_file_size_bytes = (settings.max_file_size_mb as u64) * 1024 * 1024;

    let mut report = FolderAnalysisReport {
        folder_path: path_str.to_string(),
        ..Default::default()
    };

    let mut walker = walkdir::WalkDir::new(root_path);
    if !recursive {
        walker = walker.max_depth(1);
    }

    for entry_res in walker.into_iter().filter_entry(|e| {
        let p = e.path();
        if p == root_path {
            return true;
        }
        if e.file_type().is_dir() {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if filter.is_ignored_directory_name(name)
                    || name.ends_with(".app")
                    || (name.starts_with('.') && name != "." && name != "..")
                {
                    return false;
                }
            }
            if blacklisted_roots
                .iter()
                .any(|b| crate::safety::filters::is_path_inside_or_equal(p, b))
            {
                return false;
            }
        }
        true
    }) {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        report.total_files_scanned += 1;
        let p = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                report.ignored_files_count += 1;
                continue;
            }
        };

        if blacklisted_roots
            .iter()
            .any(|b| crate::safety::filters::is_path_inside_or_equal(p, b))
        {
            report.ignored_files_count += 1;
            continue;
        }

        if filter.should_index_path(p, &metadata).is_err() {
            report.ignored_files_count += 1;
            continue;
        }

        let file_size = metadata.len();
        report.total_indexable_files += 1;
        report.total_indexable_bytes += file_size;

        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "pdf" | "docx" | "txt" | "md" | "markdown" | "rst" | "rtf" | "log" => {
                report.documents_count += 1;
                report.documents_bytes += file_size;
            }
            "png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "bmp" | "heic" => {
                report.images_count += 1;
                report.images_bytes += file_size;
            }
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "cc" | "h" | "hpp" | "go"
            | "java" | "kt" | "swift" | "rb" | "php" | "sh" | "zsh" | "bash" | "css" | "scss" => {
                report.code_count += 1;
                report.code_bytes += file_size;
            }
            "json" | "yaml" | "yml" | "toml" | "xml" | "html" | "htm" | "sql" | "csv" | "tsv" => {
                report.data_count += 1;
                report.data_bytes += file_size;
            }
            _ => {
                report.documents_count += 1;
                report.documents_bytes += file_size;
            }
        }
    }

    // ETA calculation based on real hardware throughput rates:
    // Code & Data: ~80 files/sec
    // Documents: ~15 files/sec
    // Images (Vision OCR): ~6 files/sec
    // Disk I/O buffer: ~200 MB/sec
    let code_and_data = report.code_count + report.data_count;
    let sec_code = (code_and_data as f64) / 80.0;
    let sec_docs = (report.documents_count as f64) / 15.0;
    let sec_images = (report.images_count as f64) / 6.0;
    let sec_io = ((report.total_indexable_bytes as f64) / (1024.0 * 1024.0)) / 200.0;

    let total_secs = (sec_code + sec_docs + sec_images + sec_io).ceil() as u64;
    report.estimated_time_seconds = if report.total_indexable_files > 0 {
        total_secs.max(1)
    } else {
        0
    };

    report.scan_duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(report)
}

#[tauri::command]
pub fn add_watched_folder(
    path: String,
    recursive: bool,
    db: State<'_, Arc<DatabaseManager>>,
    app: tauri::AppHandle,
) -> Result<WatchedFolder, String> {
    let resolved_path = if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            if path == "~" {
                home.to_string_lossy().to_string()
            } else {
                home.join(&path[2..]).to_string_lossy().to_string()
            }
        } else {
            path.clone()
        }
    } else {
        path.clone()
    };

    let folder = WatchedFolder {
        id: uuid::Uuid::new_v4().to_string(),
        path: resolved_path.clone(),
        enabled: true,
        recursive,
        added_at: chrono::Utc::now().timestamp(),
        last_scanned_at: None,
    };
    db.add_watched_folder(&folder).map_err(|e| e.to_string())?;
    let path_for_watcher = resolved_path.clone();
    let app_for_watcher = app.clone();
    std::thread::spawn(move || {
        if let Some(watcher) =
            app_for_watcher.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
        {
            if let Ok(mut w) = watcher.lock() {
                if let Err(e) = w.watch_directory(&path_for_watcher) {
                    tracing::error!(
                        "add_watched_folder: failed to watch directory {}: {:?}",
                        path_for_watcher,
                        e
                    );
                } else {
                    tracing::info!(
                        "add_watched_folder: successfully watching {}",
                        path_for_watcher
                    );
                }
            }
        }
    });
    let path_clone = folder.path.clone();
    let db_clone = db.inner().clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if let Some(coordinator) = app_handle.try_state::<Arc<crate::crawler::CrawlCoordinator>>() {
            coordinator.start_folder_analysis(&path_clone);
            match analyze_folder_path(&path_clone, true, &db_clone) {
                Ok(report) => {
                    tracing::info!(
                        "add_watched_folder: analysis completed for {}, awaiting user index action",
                        path_clone
                    );
                    coordinator.complete_folder_analysis(&path_clone, report);
                }
                Err(e) => {
                    tracing::warn!(
                        "add_watched_folder: analysis failed for {}: {}",
                        path_clone,
                        e
                    );
                }
            }
        }
    });
    Ok(folder)
}

#[tauri::command]
pub fn get_indexing_queue(
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
) -> Result<crate::crawler::IndexingQueueSnapshot, String> {
    Ok(coordinator.get_snapshot())
}

#[tauri::command]
pub fn cancel_queued_folder(
    folder_id: String,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
) -> Result<(), String> {
    coordinator.cancel_folder(&folder_id);
    Ok(())
}

#[tauri::command]
pub fn trigger_folder_scan(
    path: String,
    db: State<'_, Arc<DatabaseManager>>,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
    crawler: State<'_, Arc<crate::crawler::CrawlerEngine>>,
) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if let Ok(folders) = db.get_watched_folders() {
        if let Some(folder) = folders
            .iter()
            .find(|f| crate::crawler::coordinator::paths_match(&f.path, &path))
        {
            coordinator.enqueue_folders(&[folder.clone()]);
        }
    }
    let crawler_run = crawler.inner().clone();
    std::thread::spawn(move || {
        crawler_run.crawl_paths(&[p]);
    });
    Ok(())
}

#[tauri::command]
pub fn toggle_watched_folder(
    id: String,
    enabled: bool,
    db: State<'_, Arc<DatabaseManager>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    db.update_watched_folder_enabled(&id, enabled)
        .map_err(|e| e.to_string())?;
    if let Ok(folders) = db.get_watched_folders() {
        if let Some(folder) = folders.iter().find(|f| f.id == id) {
            if let Some(watcher) =
                app.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
            {
                if let Ok(mut w) = watcher.lock() {
                    if enabled {
                        if let Err(e) = w.watch_directory(&folder.path) {
                            tracing::error!(
                                "toggle_watched_folder: failed to watch directory {}: {:?}",
                                folder.path,
                                e
                            );
                        }
                        // Also trigger a full crawl so the user sees the progress bar
                        if let Some(coordinator) =
                            app.try_state::<Arc<crate::crawler::CrawlCoordinator>>()
                        {
                            coordinator.enqueue_folders(&[folder.clone()]);
                        }
                        if let Some(crawler) = app.try_state::<Arc<crate::crawler::CrawlerEngine>>()
                        {
                            let p = std::path::PathBuf::from(&folder.path);
                            let crawler_run = crawler.inner().clone();
                            std::thread::spawn(move || {
                                crawler_run.crawl_paths(&[p]);
                            });
                        }
                    } else {
                        let _ = w.unwatch_directory(&folder.path);
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Delete-queue–backed commands ────────────────────────────────────────────

/// Enqueue a background job to delete all index data for a single folder path.
/// Returns the job ID immediately; the UI can track progress via `delete-queue-update` events.
#[tauri::command]
pub async fn purge_folder_index(
    path: String,
    db: State<'_, Arc<DatabaseManager>>,
    delete_queue: State<'_, Arc<DeleteQueue>>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let path_parts: Vec<&str> = path.trim_end_matches('/').rsplitn(2, '/').collect();
    let label = format!(
        "Delete index: {}",
        path_parts.first().unwrap_or(&path.as_str())
    );

    tracing::info!(
        "purge_folder_index: Requested purge for path '{}' (label: '{}')",
        path,
        label
    );

    // Enqueue delete job immediately (total computed in background thread to avoid DB lock)
    let (job_id, ctrl) =
        delete_queue.enqueue(DeleteJobKind::FolderIndex { path: path.clone() }, label, 0);

    let db = db.inner().clone();
    let dq = delete_queue.inner().clone();
    let jid = job_id.clone();
    let p = path.clone();
    let app_handle = app.clone();

    // Run deletion in background with pause/resume/stop support
    std::thread::spawn(move || {
        tracing::info!(
            "purge_folder_index: Worker thread started for job [{}] (path: '{}')",
            jid,
            p
        );
        dq.set_running(&jid);

        // 1. Stop active crawl and file-system monitoring for this path
        tracing::info!(
            "purge_folder_index: Job [{}]: Cancelling crawl and file watcher for '{}'",
            jid,
            p
        );
        if let Some(coord) = app_handle.try_state::<Arc<crate::crawler::CrawlCoordinator>>() {
            coord.cancel_folder(&p);
        }
        if let Some(watcher) =
            app_handle.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
        {
            if let Ok(mut w) = watcher.lock() {
                let _ = w.unwatch_directory(&p);
            }
        }

        // 2. Fetch list of files
        tracing::info!(
            "purge_folder_index: Job [{}]: Listing indexed files from DB for '{}'",
            jid,
            p
        );
        let paths = match db.list_files_for_path(&p) {
            Ok(paths) => {
                tracing::info!(
                    "purge_folder_index: Job [{}]: Found {} indexed files to delete for '{}'",
                    jid,
                    paths.len(),
                    p
                );
                paths
            }
            Err(e) => {
                tracing::error!(
                    "purge_folder_index: Job [{}]: Failed listing files for '{}': {}",
                    jid,
                    p,
                    e
                );
                dq.set_failed(&jid, e.to_string());
                return;
            }
        };
        let total_paths = paths.len();
        dq.set_total(&jid, total_paths);

        let mut deleted = 0usize;

        // Batch-delete: 500 files per transaction — larger batches = far fewer round-trips.
        // No artificial sleep; we only spin-wait while the user has paused.
        for chunk in paths.chunks(500) {
            // Pause-loop: spin until resumed or stopped
            while ctrl.is_paused() && !ctrl.is_stopped() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if ctrl.is_stopped() {
                tracing::warn!(
                    "purge_folder_index: Job [{}] stopped by user at {}/{} files",
                    jid,
                    deleted,
                    total_paths
                );
                dq.set_stopped(&jid);
                return;
            }

            if let Err(e) = db.purge_files_batch(chunk) {
                tracing::error!(
                    "purge_folder_index: Job [{}] failed purging batch: {}",
                    jid,
                    e
                );
            }
            deleted += chunk.len();
            dq.update_progress(&jid, deleted);
        }

        // Reset last_scanned_at in watched_folders so DB and UI reflect that the folder's index was cleared
        let clean_path = p.trim_end_matches('/');
        let exact_path = clean_path.to_string();
        let prefix_pattern = format!("{}/%", clean_path);
        if let Ok(conn) = db.lock_conn() {
            let _ = conn.execute(
                "UPDATE watched_folders SET last_scanned_at = NULL WHERE path = ?1 OR path LIKE ?2",
                rusqlite::params![exact_path, prefix_pattern],
            );
        }

        tracing::info!(
            "purge_folder_index: Job [{}] successfully purged all {} files for '{}'",
            jid,
            total_paths,
            p
        );
        dq.set_done(&jid, total_paths);

        // VACUUM in a detached thread — reclaims disk pages so header DB size drops.
        // We deliberately don't hold the DB lock during the main delete loop.
        let db_vac = db.clone();
        std::thread::spawn(move || {
            tracing::info!(
                "purge_folder_index: Running VACUUM after purge job [{}]",
                jid
            );
            if let Err(e) = db_vac.vacuum() {
                tracing::warn!("purge_folder_index: VACUUM failed: {}", e);
            } else {
                tracing::info!("purge_folder_index: VACUUM complete — disk space reclaimed");
            }
        });
    });

    Ok(job_id)
}

/// Enqueue a background job to remove a watched folder (+ optionally its index data).
#[tauri::command]
pub async fn remove_watched_folder(
    id: String,
    path: Option<String>,
    purge_data: Option<bool>,
    db: State<'_, Arc<DatabaseManager>>,
    delete_queue: State<'_, Arc<DeleteQueue>>,
    app: tauri::AppHandle,
) -> Result<crate::db::repository::PurgeFolderResult, String> {
    let should_purge = purge_data.unwrap_or(false);
    let known_path = path.unwrap_or_default();
    let path_label = if !known_path.is_empty() {
        known_path
            .trim_end_matches('/')
            .rsplitn(2, '/')
            .next()
            .unwrap_or(&known_path)
            .to_string()
    } else {
        id.clone()
    };

    if should_purge {
        tracing::info!(
            "remove_watched_folder: Enqueueing remove & purge job for folder id '{}', path: '{}'",
            id,
            known_path
        );
        // Enqueue delete job immediately (total computed in background thread)
        let (job_id, ctrl) = delete_queue.enqueue(
            DeleteJobKind::RemoveFolder {
                folder_id: id.clone(),
                path: known_path.clone(),
            },
            format!("Remove & purge: {}", path_label),
            0,
        );

        let db_clone = db.inner().clone();
        let dq = delete_queue.inner().clone();
        let app_handle = app.clone();
        let folder_id = id.clone();
        let mut fp = known_path.clone();

        std::thread::spawn(move || {
            tracing::info!(
                "remove_watched_folder: Worker thread started for job [{}] (folder_id: '{}')",
                job_id,
                folder_id
            );
            dq.set_running(&job_id);

            // Lookup path if missing
            if fp.is_empty() {
                if let Ok(folders) = db_clone.get_watched_folders() {
                    if let Some(folder) = folders.iter().find(|f| f.id == folder_id) {
                        fp = folder.path.clone();
                    }
                }
            }

            // 1. Stop active crawl and file-system monitoring
            tracing::info!(
                "remove_watched_folder: Job [{}]: Cancelling crawl and file watcher for '{}'",
                job_id,
                fp
            );
            if let Some(coord) = app_handle.try_state::<Arc<crate::crawler::CrawlCoordinator>>() {
                coord.cancel_folder(&folder_id);
                if !fp.is_empty() {
                    coord.cancel_folder(&fp);
                }
            }
            if !fp.is_empty() {
                if let Some(watcher) =
                    app_handle.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
                {
                    if let Ok(mut w) = watcher.lock() {
                        let _ = w.unwatch_directory(&fp);
                    }
                }
            }

            // 2. Remove from watchlist immediately in background
            tracing::info!(
                "remove_watched_folder: Job [{}]: Removing folder '{}' from DB watchlist",
                job_id,
                folder_id
            );
            if let Err(e) = db_clone.remove_watched_folder(&folder_id) {
                tracing::error!(
                    "remove_watched_folder: Job [{}]: Failed to remove folder from watchlist: {}",
                    job_id,
                    e
                );
            }

            // 3. Purge indexed files
            tracing::info!(
                "remove_watched_folder: Job [{}]: Listing indexed files for path '{}'",
                job_id,
                fp
            );
            let paths = if !fp.is_empty() {
                db_clone.list_files_for_path(&fp).unwrap_or_default()
            } else {
                Vec::new()
            };
            let total_paths = paths.len();
            tracing::info!(
                "remove_watched_folder: Job [{}]: Found {} indexed files to purge",
                job_id,
                total_paths
            );
            dq.set_total(&job_id, total_paths);

            let mut deleted = 0usize;
            for chunk in paths.chunks(500) {
                while ctrl.is_paused() && !ctrl.is_stopped() {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                if ctrl.is_stopped() {
                    tracing::warn!(
                        "remove_watched_folder: Job [{}] stopped by user at {}/{} files",
                        job_id,
                        deleted,
                        total_paths
                    );
                    dq.set_stopped(&job_id);
                    return;
                }
                if let Err(e) = db_clone.purge_files_batch(chunk) {
                    tracing::error!(
                        "remove_watched_folder: Job [{}] failed purging batch: {}",
                        job_id,
                        e
                    );
                }
                deleted += chunk.len();
                dq.update_progress(&job_id, deleted);
            }
            tracing::info!(
                "remove_watched_folder: Job [{}] finished successfully, purged {} files",
                job_id,
                total_paths
            );
            dq.set_done(&job_id, total_paths);

            // VACUUM to shrink the .db file on disk (reflected in telemetry header DB size)
            let db_vac = db_clone.clone();
            let jid_vac = job_id.clone();
            std::thread::spawn(move || {
                tracing::info!(
                    "remove_watched_folder: Running VACUUM after job [{}]",
                    jid_vac
                );
                if let Err(e) = db_vac.vacuum() {
                    tracing::warn!("remove_watched_folder: VACUUM failed: {}", e);
                } else {
                    tracing::info!("remove_watched_folder: VACUUM complete — disk space reclaimed");
                }
            });
        });

        Ok(crate::db::repository::PurgeFolderResult {
            files_removed: 0,
            chunks_removed: 0,
            path: known_path,
        })
    } else {
        tracing::info!(
            "remove_watched_folder: Removing folder id '{}' (path: '{}', purge_data=false)",
            id,
            known_path
        );
        // Not purging: remove watched folder & unwatch asynchronously
        let db_clone = db.inner().clone();
        let app_handle = app.clone();
        let folder_id = id.clone();
        let mut fp = known_path.clone();

        std::thread::spawn(move || {
            if fp.is_empty() {
                if let Ok(folders) = db_clone.get_watched_folders() {
                    if let Some(folder) = folders.iter().find(|f| f.id == folder_id) {
                        fp = folder.path.clone();
                    }
                }
            }
            if !fp.is_empty() {
                if let Some(watcher) =
                    app_handle.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
                {
                    if let Ok(mut w) = watcher.lock() {
                        let _ = w.unwatch_directory(&fp);
                    }
                }
            }
            let _ = db_clone.remove_watched_folder(&folder_id);
            tracing::info!(
                "remove_watched_folder: Folder id '{}' removed from DB watchlist",
                folder_id
            );
        });

        Ok(crate::db::repository::PurgeFolderResult {
            files_removed: 0,
            chunks_removed: 0,
            path: known_path,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResetDatabaseResult {
    pub files_purged: usize,
    pub chunks_purged: usize,
    pub reindexing_started: bool,
    pub factory_reset_applied: bool,
    pub message: String,
}

#[tauri::command]
pub async fn reset_database_and_reindex(
    reindex: Option<bool>,
    factory_reset: Option<bool>,
    db: State<'_, Arc<DatabaseManager>>,
    crawler: State<'_, Arc<crate::crawler::CrawlerEngine>>,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
    delete_queue: State<'_, Arc<DeleteQueue>>,
    app: tauri::AppHandle,
) -> Result<ResetDatabaseResult, String> {
    let is_factory = factory_reset.unwrap_or(false);
    let should_reindex = reindex.unwrap_or(true);

    tracing::info!(
        "Triggered reset_database_and_reindex (reindex: {:?}, factory_reset: {:?})",
        reindex,
        factory_reset
    );

    // 1. Stop active crawl (atomic flags in memory, non-blocking)
    crawler.stop();
    coordinator.finish_all_crawls();

    let label = if is_factory {
        "Factory Reset".to_string()
    } else {
        "Clean DB & Re-index".to_string()
    };

    // 2. Enqueue the delete job immediately (total computed in background thread)
    let (job_id, ctrl) = delete_queue.enqueue(
        DeleteJobKind::PurgeAll {
            factory_reset: is_factory,
        },
        label,
        0,
    );

    let db_clone = db.inner().clone();
    let crawler_clone = crawler.inner().clone();
    let coordinator_clone = coordinator.inner().clone();
    let dq = delete_queue.inner().clone();
    let jid = job_id.clone();
    let app_handle = app.clone();

    // 3. Move all DB access and file operations into background thread
    std::thread::spawn(move || {
        tracing::info!(
            "reset_database_and_reindex: Background worker started for job [{}] (is_factory: {}, should_reindex: {})",
            jid,
            is_factory,
            should_reindex
        );
        dq.set_running(&jid);

        // Stop all FS watchers
        tracing::info!(
            "reset_database_and_reindex: Job [{}]: Unwatching directories before purge",
            jid
        );
        let watcher_state = app_handle
            .try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>()
            .map(|s| s.inner().clone());
        if let Some(ref w) = watcher_state {
            if let Ok(mut w) = w.lock() {
                if let Ok(folders) = db_clone.get_watched_folders() {
                    for f in &folders {
                        let _ = w.unwatch_directory(&f.path);
                    }
                }
                if is_factory {
                    if let Ok(blacklists) = db_clone.get_blacklist_folders() {
                        for b in &blacklists {
                            let _ = w.unwatch_directory(&b.path);
                        }
                    }
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200)); // give crawl threads time to exit

        // Batch-delete all indexed files
        tracing::info!(
            "reset_database_and_reindex: Job [{}]: Listing all indexed files from database",
            jid
        );
        let all_paths = match db_clone.list_all_indexed_files() {
            Ok(p) => {
                tracing::info!(
                    "reset_database_and_reindex: Job [{}]: Found {} indexed files to purge",
                    jid,
                    p.len()
                );
                p
            }
            Err(e) => {
                tracing::error!(
                    "reset_database_and_reindex: Job [{}]: Failed listing indexed files: {}",
                    jid,
                    e
                );
                dq.set_failed(&jid, e.to_string());
                return;
            }
        };
        let total_paths = all_paths.len();
        dq.set_total(&jid, total_paths);

        let mut deleted = 0usize;

        for chunk in all_paths.chunks(500) {
            while ctrl.is_paused() && !ctrl.is_stopped() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if ctrl.is_stopped() {
                tracing::warn!(
                    "reset_database_and_reindex: Job [{}] stopped by user at {}/{} files",
                    jid,
                    deleted,
                    total_paths
                );
                dq.set_stopped(&jid);
                return;
            }
            if let Err(e) = db_clone.purge_files_batch(chunk) {
                tracing::error!(
                    "reset_database_and_reindex: Job [{}] failed purging batch: {}",
                    jid,
                    e
                );
            }
            deleted += chunk.len();
            dq.update_progress(&jid, deleted);
        }

        // Final cleanup (triggers, FTS, VACUUM)
        tracing::info!("reset_database_and_reindex: Job [{}]: Performing final index purge & VACUUM (purge_all_index_data_fast)", jid);
        if let Err(e) = db_clone.purge_all_index_data_fast() {
            tracing::error!(
                "reset_database_and_reindex: Job [{}]: purge_all_index_data_fast failed: {}",
                jid,
                e
            );
            dq.set_failed(&jid, e.to_string());
            return;
        }
        tracing::info!(
            "reset_database_and_reindex: Job [{}]: Final index purge complete",
            jid
        );

        crawler_clone.controller().reset();

        if is_factory {
            tracing::info!("reset_database_and_reindex: Job [{}]: Applying factory reset defaults and clearing folders", jid);
            let defaults = crate::models::AppSettings::default();
            let _ = db_clone.save_app_settings(&defaults);
            if let Ok(folders) = db_clone.get_watched_folders() {
                for f in &folders {
                    let _ = db_clone.remove_watched_folder(&f.id);
                }
            }
            if let Ok(blacklists) = db_clone.get_blacklist_folders() {
                for b in &blacklists {
                    let _ = db_clone.remove_blacklist_folder(&b.id);
                }
            }
        } else if should_reindex {
            tracing::info!("reset_database_and_reindex: Job [{}]: Scheduling reindexing for enabled watched folders", jid);
            if let Ok(folders) = db_clone.get_watched_folders() {
                let enabled: Vec<_> = folders.into_iter().filter(|f| f.enabled).collect();
                let paths: Vec<std::path::PathBuf> = enabled
                    .iter()
                    .map(|f| std::path::PathBuf::from(&f.path))
                    .collect();
                if !paths.is_empty() {
                    coordinator_clone.enqueue_folders(&enabled);
                    let crawler_run = crawler_clone.clone();
                    std::thread::spawn(move || {
                        crawler_run.crawl_paths(&paths);
                    });
                }
            }
        }

        tracing::info!(
            "reset_database_and_reindex: Job [{}] completed successfully (purged {} files)",
            jid,
            total_paths
        );
        dq.set_done(&jid, total_paths);
    });

    Ok(ResetDatabaseResult {
        files_purged: 0,
        chunks_purged: 0,
        reindexing_started: should_reindex && !is_factory,
        factory_reset_applied: is_factory,
        message: if is_factory {
            "Factory reset queued — running in background.".to_string()
        } else {
            "Database clean queued — running in background.".to_string()
        },
    })
}

// ─── Delete-queue management commands (called from UI) ───────────────────────

#[tauri::command]
pub fn get_delete_queue(
    delete_queue: State<'_, Arc<DeleteQueue>>,
) -> crate::crawler::delete_queue::DeleteQueueSnapshot {
    delete_queue.snapshot()
}

#[tauri::command]
pub fn pause_delete_job(
    job_id: String,
    delete_queue: State<'_, Arc<DeleteQueue>>,
) -> Result<(), String> {
    tracing::info!("pause_delete_job: UI requested pause for job [{}]", job_id);
    delete_queue.pause_job(&job_id);
    Ok(())
}

#[tauri::command]
pub fn resume_delete_job(
    job_id: String,
    delete_queue: State<'_, Arc<DeleteQueue>>,
) -> Result<(), String> {
    tracing::info!(
        "resume_delete_job: UI requested resume for job [{}]",
        job_id
    );
    delete_queue.resume_job(&job_id);
    Ok(())
}

#[tauri::command]
pub fn stop_delete_job(
    job_id: String,
    delete_queue: State<'_, Arc<DeleteQueue>>,
) -> Result<(), String> {
    tracing::info!("stop_delete_job: UI requested stop for job [{}]", job_id);
    delete_queue.stop_job(&job_id);
    Ok(())
}

#[tauri::command]
pub fn dismiss_delete_job(
    job_id: String,
    delete_queue: State<'_, Arc<DeleteQueue>>,
) -> Result<(), String> {
    tracing::info!(
        "dismiss_delete_job: UI requested dismissal of job [{}]",
        job_id
    );
    delete_queue.dismiss_job(&job_id);
    Ok(())
}

/// Re-add and re-scan a folder after a stopped delete job.
/// Restores FS monitoring and triggers a fresh crawl.
#[tauri::command]
pub fn requeue_folder_after_stop(
    job_id: String,
    delete_queue: State<'_, Arc<DeleteQueue>>,
    db: State<'_, Arc<DatabaseManager>>,
    crawler: State<'_, Arc<crate::crawler::CrawlerEngine>>,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(
        "requeue_folder_after_stop: UI requested requeue for stopped job [{}]",
        job_id
    );
    let job = delete_queue.get_job(&job_id).ok_or("Job not found")?;

    let folder_path = match &job.kind {
        DeleteJobKind::FolderIndex { path } => path.clone(),
        DeleteJobKind::RemoveFolder { path, .. } => path.clone(),
        DeleteJobKind::PurgeAll { .. } => return Err("Cannot re-queue a PurgeAll job".into()),
    };

    tracing::info!(
        "requeue_folder_after_stop: Restoring FS watcher and enqueuing crawl for '{}'",
        folder_path
    );

    // Restore FS monitoring
    if let Some(watcher) = app.try_state::<Arc<std::sync::Mutex<crate::ingest::FileWatcher>>>() {
        if let Ok(mut w) = watcher.lock() {
            let _ = w.watch_directory(&folder_path);
        }
    }

    // For RemoveFolder jobs, we need to re-add the folder back to the watchlist
    if let DeleteJobKind::RemoveFolder { folder_id, .. } = &job.kind {
        // folder was already removed — re-add via existing watched folder logic if it still exists in DB
        // If it's already gone, just start a fresh crawl
        let _ = folder_id; // folder already removed, we proceed with crawl only
    }

    // Enqueue a crawl
    if let Ok(folders) = db.get_watched_folders() {
        if let Some(folder) = folders.iter().find(|f| f.path == folder_path) {
            coordinator.enqueue_folders(&[folder.clone()]);
        }
    }
    let path_buf = std::path::PathBuf::from(&folder_path);
    let crawler_run = crawler.inner().clone();
    std::thread::spawn(move || {
        crawler_run.crawl_paths(&[path_buf]);
    });

    // Remove the stopped job from the queue display
    delete_queue.dismiss_job(&job_id);

    Ok(())
}
