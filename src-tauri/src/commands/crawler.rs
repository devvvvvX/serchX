use crate::crawler::{CrawlProgress, CrawlerEngine};
use crate::db::{DatabaseManager, RetentionPolicy};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn start_crawl(
    paths: Vec<String>,
    crawler: State<'_, Arc<CrawlerEngine>>,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<CrawlProgress, String> {
    let engine = crawler.inner().clone();
    let path_bufs: Vec<PathBuf> = paths.clone().into_iter().map(PathBuf::from).collect();
    tracing::info!("start_crawl command invoked with paths: {:?}", paths);

    let watched_folders = db.get_watched_folders().unwrap_or_default();
    let mut to_enqueue = Vec::new();

    for p in &path_bufs {
        let p_norm = p.to_string_lossy().trim_end_matches('/').to_string();
        if let Some(existing) = watched_folders.iter().find(|f| f.path.trim_end_matches('/') == p_norm) {
            to_enqueue.push(existing.clone());
        } else {
            to_enqueue.push(crate::models::WatchedFolder {
                id: uuid::Uuid::new_v4().to_string(),
                path: p.to_string_lossy().to_string(),
                enabled: true,
                recursive: true,
                added_at: chrono::Utc::now().timestamp(),
                last_scanned_at: None,
            });
        }
    }

    if !to_enqueue.is_empty() {
        coordinator.enqueue_folders(&to_enqueue);
    }

    std::thread::spawn(move || {
        engine.crawl_paths(&path_bufs);
    });

    Ok(crawler.get_progress())
}

#[tauri::command]
pub fn pause_crawl(crawler: State<'_, Arc<CrawlerEngine>>) {
    crawler.pause();
}

#[tauri::command]
pub fn resume_crawl(crawler: State<'_, Arc<CrawlerEngine>>) {
    crawler.resume();
}

#[tauri::command]
pub fn pause_folder_indexing(
    folder_id: String,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
) {
    coordinator.pause_folder(&folder_id);
}

#[tauri::command]
pub fn resume_folder_indexing(
    folder_id: String,
    coordinator: State<'_, Arc<crate::crawler::CrawlCoordinator>>,
) {
    coordinator.resume_folder(&folder_id);
}

#[tauri::command]
pub fn get_crawl_progress(crawler: State<'_, Arc<CrawlerEngine>>) -> CrawlProgress {
    crawler.get_progress()
}

#[tauri::command]
pub fn prune_versions(
    max_versions: usize,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<usize, String> {
    let policy = RetentionPolicy {
        max_versions_per_artifact: max_versions,
        retention_days: None,
    };
    crate::db::prune_unpinned_versions(db.inner(), &policy).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pin_version(
    version_id: String,
    pinned: bool,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    crate::db::pin_version(db.inner(), &version_id, pinned).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_logs(
    tail_lines: Option<usize>,
    log_buffer: State<'_, crate::logging::LogRingBuffer>,
) -> Result<Vec<String>, String> {
    Ok(log_buffer.get_lines(tail_lines))
}

#[tauri::command]
pub fn export_app_logs(
    destination_path: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    use tauri::Manager;
    let app_data_dir = match app_handle.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => PathBuf::from(".serchx"),
    };
    let log_file = crate::logging::get_log_file_path(&app_data_dir);

    let dest_path = match destination_path {
        Some(p) => {
            let pb = PathBuf::from(&p);
            if let Some(parent) = pb.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            pb
        }
        None => {
            let filename = format!("serchx_logs_{}.log", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
            dirs::desktop_dir()
                .or_else(dirs::download_dir)
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(filename)
        }
    };

    if log_file.exists() {
        std::fs::copy(&log_file, &dest_path).map_err(|e| format!("Failed to copy log file: {}", e))?;
    } else if let Some(buf) = app_handle.try_state::<crate::logging::LogRingBuffer>() {
        let lines = buf.get_lines(None);
        let content = lines.join("\n");
        std::fs::write(&dest_path, content).map_err(|e| format!("Failed to write log buffer: {}", e))?;
    } else {
        std::fs::write(&dest_path, "").map_err(|e| format!("Failed to write log file: {}", e))?;
    }

    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn reveal_app_logs_in_finder(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let app_data_dir = match app_handle.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => PathBuf::from(".serchx"),
    };
    let logs_dir = app_data_dir.join("logs");
    let log_file = logs_dir.join("serchx.log");

    if !logs_dir.exists() {
        let _ = std::fs::create_dir_all(&logs_dir);
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if log_file.exists() {
            cmd.arg("-R").arg(&log_file);
        } else {
            cmd.arg(&logs_dir);
        }
        cmd.spawn().map_err(|e| format!("Failed to reveal logs in Finder: {}", e))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = log_file;
    }

    Ok(())
}

#[tauri::command]
pub fn get_dead_letter_queue(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<crate::models::DeadLetterItem>, String> {
    db.get_dead_letter_items().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_dead_letter_item(
    id: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    db.remove_dead_letter_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_dead_letter_queue(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    db.clear_dead_letter_queue().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn retry_dead_letter_item(
    path: String,
    crawler: State<'_, Arc<CrawlerEngine>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    let _ = db.remove_dead_letter_by_path(&path);
    let engine = crawler.inner().clone();
    std::thread::spawn(move || {
        engine.crawl_paths(&[p]);
    });
    Ok(())
}

