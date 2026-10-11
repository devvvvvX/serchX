use crate::db::repository::{CorpusAnalytics, StorageBreakdown};
use crate::db::DatabaseManager;
use crate::models::BlacklistFolder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

static IS_DIALOG_ACTIVE: AtomicBool = AtomicBool::new(false);
static AUTO_HIDE_ENABLED: AtomicBool = AtomicBool::new(false);
static LAST_WINDOW_POS: AtomicI64 = AtomicI64::new(i64::MIN);
static LAST_COMPARE_PATH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn pack_pos(x: i32, y: i32) -> i64 {
    let ux = x as u32 as u64;
    let uy = y as u32 as u64;
    ((ux << 32) | uy) as i64
}

pub fn unpack_pos(val: i64) -> (i32, i32) {
    let u = val as u64;
    let x = (u >> 32) as u32 as i32;
    let y = (u & 0xFFFF_FFFF) as u32 as i32;
    (x, y)
}

pub fn record_window_position(x: i32, y: i32) {
    LAST_WINDOW_POS.store(pack_pos(x, y), Ordering::SeqCst);
}

pub fn get_recorded_window_position() -> Option<(i32, i32)> {
    let val = LAST_WINDOW_POS.load(Ordering::SeqCst);
    if val == i64::MIN {
        None
    } else {
        Some(unpack_pos(val))
    }
}

pub fn persist_window_position(db: &crate::db::DatabaseManager) -> Result<(), String> {
    if let Some((x, y)) = get_recorded_window_position() {
        db.save_window_position(x, y)
            .map_err(|e| format!("Failed to save window position: {}", e))?;
    }
    Ok(())
}

/// Check whether a native system modal dialog is currently active.
pub fn is_dialog_active() -> bool {
    IS_DIALOG_ACTIVE.load(Ordering::SeqCst)
}

/// Set whether a native system modal dialog is currently active.
pub fn set_dialog_active(active: bool) {
    IS_DIALOG_ACTIVE.store(active, Ordering::SeqCst);
}

/// Check whether window auto-hide on blur is enabled.
pub fn is_auto_hide_enabled() -> bool {
    AUTO_HIDE_ENABLED.load(Ordering::SeqCst)
}

/// Enable or disable window auto-hide on blur.
#[tauri::command]
pub fn set_auto_hide(enabled: bool) {
    AUTO_HIDE_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Enforce clamp bounds on window height (64px minimum, 520px maximum).
pub fn clamp_window_height(height: f64) -> f64 {
    height.clamp(64.0, 520.0)
}

/// Enforce clamp bounds on window width (640px minimum, 820px maximum).
pub fn clamp_window_width(width: f64) -> f64 {
    width.clamp(640.0, 820.0)
}

pub const PRIVACY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Open a target file using the system default application.
/// Validates path existence before invoking `/usr/bin/open` to prevent arbitrary command execution.
#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    Command::new("/usr/bin/open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to open file '{}': {}", path, e))?;

    Ok(())
}

/// Reveal a target file or folder in macOS Finder.
/// Validates path existence before invoking `/usr/bin/open -R`.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    Command::new("/usr/bin/open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to reveal in Finder '{}': {}", path, e))?;

    Ok(())
}

/// Open native macOS directory picker dialog using tauri-plugin-dialog.
/// Runs asynchronously off the main thread to prevent event loop deadlocks.
/// Sets `is_dialog_active = true` during execution to prevent window blur auto-hide.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    set_dialog_active(true);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Select folder to index in serchX")
        .pick_folder(move |folder| {
            let _ = tx.send(folder);
        });

    let picked = rx.await.unwrap_or(None);
    set_dialog_active(false);

    // Restore focus to the settings window if visible
    if let Some(window) = app.get_webview_window("settings") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.set_focus();
        }
    }

    Ok(picked.and_then(|fp| fp.as_path().map(|p| p.to_string_lossy().to_string())))
}

/// Open macOS Security & Privacy settings page for Full Disk Access.
#[tauri::command]
pub fn open_privacy_settings() -> Result<(), String> {
    set_dialog_active(true);
    let res = Command::new("/usr/bin/open")
        .arg(PRIVACY_SETTINGS_URL)
        .spawn()
        .map_err(|e| format!("Failed to open System Settings: {}", e));

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(2000));
        set_dialog_active(false);
    });

    res.map(|_| ())
}

/// Resize the main launcher window dynamically based on content height and optional width.
/// Clamps height between 64.0 and 520.0 logical pixels, and width between 640.0 and 820.0.
#[tauri::command]
pub fn resize_window(app: AppHandle, height: f64, width: Option<f64>) -> Result<(), String> {
    let clamped_height = clamp_window_height(height);
    let target_width = clamp_window_width(width.unwrap_or(680.0));
    if let Some(window) = app.get_webview_window("main") {
        let size = tauri::Size::Logical(tauri::LogicalSize {
            width: target_width,
            height: clamped_height,
        });
        window
            .set_size(size)
            .map_err(|e| format!("Failed to resize window: {}", e))?;
    }
    Ok(())
}

/// Check if a blur event occurred within the specified threshold in milliseconds.
pub fn is_blur_debounced(last_blur: Option<std::time::Instant>, threshold_ms: u64) -> bool {
    if let Some(instant) = last_blur {
        instant.elapsed() < std::time::Duration::from_millis(threshold_ms)
    } else {
        false
    }
}

/// Open, unminimize, show and focus the dedicated settings window.
#[tauri::command]
pub fn open_settings_window(app: AppHandle, tab: Option<String>) -> Result<(), String> {
    use tauri::Emitter;
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        window
            .show()
            .map_err(|e| format!("Failed to show settings window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus settings window: {}", e))?;

        if let Some(ref target_tab) = tab {
            let _ = window.emit("navigate-settings-tab", target_tab);
        }
    }
    Ok(())
}

/// Open, unminimize, show and focus the dedicated compare window.
#[tauri::command]
pub fn open_compare_window(app: AppHandle, path: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    if let Some(ref p) = path {
        if let Ok(mut lock) = LAST_COMPARE_PATH.lock() {
            *lock = Some(p.clone());
        }
    }
    if let Some(window) = app.get_webview_window("compare") {
        let _ = window.unminimize();
        window
            .show()
            .map_err(|e| format!("Failed to show compare window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus compare window: {}", e))?;
        if let Some(p) = path {
            use tauri::Emitter;
            let _ = window.emit("load-diff-path", p);
        }
    }
    Ok(())
}

/// Retrieve the active file path to compare.
#[tauri::command]
pub fn get_compare_path() -> Result<Option<String>, String> {
    let lock = LAST_COMPARE_PATH.lock().map_err(|e| e.to_string())?;
    Ok(lock.clone())
}

/// Open, unminimize, show and focus the onboarding window.
/// Also hides the main launcher window to prevent flash-of-search-bar.
#[tauri::command]
pub fn open_onboarding_window(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    // Hide the main window to prevent it from flashing behind the onboarding window
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.hide();
    }
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.unminimize();
        // Reload the page so stale cached content in dev mode is always refreshed
        let _ = window.eval("location.reload()");
        window
            .show()
            .map_err(|e| format!("Failed to show onboarding window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus onboarding window: {}", e))?;
    }
    Ok(())
}

/// Show and focus the main launcher window.
#[tauri::command]
pub fn show_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        if let Some((x, y)) = get_recorded_window_position() {
            let _ =
                window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
        window
            .show()
            .map_err(|e| format!("Failed to show window: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus window: {}", e))?;
    }
    Ok(())
}

/// Hide the main launcher window.
#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(pos) = window.outer_position() {
            record_window_position(pos.x, pos.y);
            if let Some(db) = app.try_state::<Arc<crate::db::DatabaseManager>>() {
                let _ = persist_window_position(&db);
            }
        }
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}

/// Toggle visibility of the main launcher window.
#[tauri::command]
pub fn toggle_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_vis = window.is_visible().unwrap_or(false);
        if is_vis {
            if let Ok(pos) = window.outer_position() {
                record_window_position(pos.x, pos.y);
                if let Some(db) = app.try_state::<Arc<crate::db::DatabaseManager>>() {
                    let _ = persist_window_position(&db);
                }
            }
            let _ = window.hide();
        } else {
            let _ = window.unminimize();
            if let Some((x, y)) = get_recorded_window_position() {
                let _ = window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    Ok(())
}

/// Initiate native window drag on the target window or currently focused window.
#[tauri::command]
pub fn start_dragging(app: AppHandle, window_label: Option<String>) -> Result<(), String> {
    let target_window = if let Some(ref label) = window_label {
        app.get_webview_window(label)
    } else {
        let windows = app.webview_windows();
        windows
            .values()
            .find(|w| w.is_focused().unwrap_or(false))
            .cloned()
            .or_else(|| app.get_webview_window("main"))
    };

    if let Some(window) = target_window {
        window
            .start_dragging()
            .map_err(|e| format!("Failed to start dragging: {}", e))?;
    }
    Ok(())
}

/// Center the main launcher window and update stored position in SQLite.
#[tauri::command]
pub fn center_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .center()
            .map_err(|e| format!("Failed to center window: {}", e))?;
        if let Ok(pos) = window.outer_position() {
            record_window_position(pos.x, pos.y);
            if let Some(db) = app.try_state::<Arc<crate::db::DatabaseManager>>() {
                let _ = persist_window_position(&db);
            }
        }
    }
    Ok(())
}

/// Set whether the application appears in the macOS Dock or runs as a pure menu-bar accessory.
#[tauri::command]
pub fn set_dock_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTelemetry {
    pub memory_rss_mb: f64,
    #[serde(default)]
    pub metal_memory_mb: Option<f64>,
    #[serde(default)]
    pub metal_model_footprint_mb: Option<f64>,
    pub cpu_percent: f32,
    pub db_size_mb: f64,
    pub uptime_seconds: u64,
}

fn sample_process_cpu() -> f32 {
    let current_cpu = unsafe {
        let mut rusage = std::mem::zeroed::<libc::rusage>();
        if libc::getrusage(libc::RUSAGE_SELF, &mut rusage) == 0 {
            let u = rusage.ru_utime.tv_sec as f64 + (rusage.ru_utime.tv_usec as f64) / 1_000_000.0;
            let s = rusage.ru_stime.tv_sec as f64 + (rusage.ru_stime.tv_usec as f64) / 1_000_000.0;
            u + s
        } else {
            0.0
        }
    };

    let now = std::time::Instant::now();
    static LAST_CPU: std::sync::Mutex<Option<(std::time::Instant, f64)>> =
        std::sync::Mutex::new(None);
    if let Ok(mut guard) = LAST_CPU.lock() {
        if let Some((last_time, last_cpu)) = *guard {
            let elapsed_wall = now.duration_since(last_time).as_secs_f64();
            let elapsed_cpu = current_cpu - last_cpu;
            *guard = Some((now, current_cpu));
            if elapsed_wall > 0.05 {
                let pct = (elapsed_cpu / elapsed_wall * 100.0) as f32;
                return pct.clamp(0.0, 100.0);
            }
        } else {
            *guard = Some((now, current_cpu));
        }
    }
    0.0
}

pub fn collect_system_telemetry(db: &DatabaseManager) -> SystemTelemetry {
    let memory_rss_mb =
        (crate::benchmark::telemetry::get_current_rss_bytes() as f64) / (1024.0 * 1024.0);
    let db_bytes = db.get_sqlite_size_bytes();
    let db_size_mb = (db_bytes as f64) / (1024.0 * 1024.0);

    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let uptime = START_TIME
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs();
    let cpu_percent = sample_process_cpu();

    SystemTelemetry {
        memory_rss_mb: (memory_rss_mb * 100.0).round() / 100.0,
        metal_memory_mb: None,
        metal_model_footprint_mb: None,
        cpu_percent: (cpu_percent * 10.0).round() / 10.0,
        db_size_mb: (db_size_mb * 100.0).round() / 100.0,
        uptime_seconds: uptime,
    }
}

#[tauri::command]
pub async fn get_system_telemetry(
    db: State<'_, Arc<DatabaseManager>>,
    llm: State<'_, Arc<crate::llm::LlmManager>>,
) -> Result<SystemTelemetry, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let status = llm.get_status(&settings).await;
    let mut telemetry = collect_system_telemetry(&db);
    if status.provider == "native" || status.provider == "hybrid" {
        telemetry.metal_memory_mb = Some(
            status
                .ram_usage_bytes
                .map(|b| ((b as f64) / (1024.0 * 1024.0) * 10.0).round() / 10.0)
                .unwrap_or(0.0),
        );
        telemetry.metal_model_footprint_mb = status
            .model_size_bytes
            .map(|b| ((b as f64) / (1024.0 * 1024.0) * 10.0).round() / 10.0);
    } else {
        telemetry.metal_memory_mb = None;
        telemetry.metal_model_footprint_mb = None;
    }
    Ok(telemetry)
}

#[tauri::command]
pub async fn get_corpus_analytics(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<CorpusAnalytics, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || db.get_corpus_analytics().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_storage_breakdown(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<StorageBreakdown, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || db.get_storage_breakdown(None).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_blacklist_folders(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<BlacklistFolder>, String> {
    db.get_blacklist_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_blacklist_folder(
    path: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<BlacklistFolder, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Directory does not exist: {}", path));
    }
    db.add_blacklist_folder(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_blacklist_folder(
    id: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    db.remove_blacklist_folder(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quick_look_file(path: String, app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        set_dialog_active(true);

        // Temporarily disable always-on-top so Quick Look isn't hidden behind the search bar
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_always_on_top(false);
        }

        match tokio::process::Command::new("qlmanage")
            .arg("-p")
            .arg(&path)
            .spawn()
        {
            Ok(mut child) => {
                let _ = child.wait().await;
            }
            Err(e) => {
                set_dialog_active(false);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_always_on_top(true);
                }
                return Err(format!("Failed to launch Quick Look: {}", e));
            }
        }
        set_dialog_active(false);

        // Restore focus and always-on-top to our main window explicitly
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_always_on_top(true);
            let _ = window.set_focus();
        }
    }
    Ok(())
}
