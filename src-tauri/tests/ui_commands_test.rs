use serchx_lib::commands::ui::{
    clamp_window_height, clamp_window_width, is_dialog_active, open_file, reveal_in_finder,
    set_dialog_active, PRIVACY_SETTINGS_URL,
};
use std::fs;
use std::path::Path;

#[test]
fn test_clamp_window_height_bounds() {
    assert_eq!(clamp_window_height(0.0), 64.0);
    assert_eq!(clamp_window_height(32.0), 64.0);
    assert_eq!(clamp_window_height(64.0), 64.0);
    assert_eq!(clamp_window_height(250.0), 250.0);
    assert_eq!(clamp_window_height(520.0), 520.0);
    assert_eq!(clamp_window_height(600.0), 520.0);
    assert_eq!(clamp_window_height(1000.0), 520.0);
}

#[test]
fn test_clamp_window_width_bounds() {
    assert_eq!(clamp_window_width(0.0), 640.0);
    assert_eq!(clamp_window_width(500.0), 640.0);
    assert_eq!(clamp_window_width(640.0), 640.0);
    assert_eq!(clamp_window_width(680.0), 680.0);
    assert_eq!(clamp_window_width(820.0), 820.0);
    assert_eq!(clamp_window_width(900.0), 820.0);
}

#[test]
fn test_open_file_nonexistent_path_fails() {
    let non_existent = "/non/existent/path/to/serchx_test_file_never_exists.xyz";
    let res = open_file(non_existent.to_string());
    assert!(res.is_err(), "open_file on nonexistent path must return Err");
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("File does not exist"));
}

#[test]
fn test_reveal_in_finder_nonexistent_path_fails() {
    let non_existent = "/non/existent/path/to/serchx_test_file_never_exists_2.xyz";
    let res = reveal_in_finder(non_existent.to_string());
    assert!(
        res.is_err(),
        "reveal_in_finder on nonexistent path must return Err"
    );
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("File does not exist"));
}

#[test]
fn test_privacy_settings_url() {
    assert_eq!(
        PRIVACY_SETTINGS_URL,
        "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"
    );
}

#[test]
fn test_dialog_active_flag() {
    set_dialog_active(true);
    assert!(is_dialog_active());
    set_dialog_active(false);
    assert!(!is_dialog_active());
}

#[test]
fn test_auto_hide_flag() {
    serchx_lib::commands::ui::set_auto_hide(true);
    assert!(serchx_lib::commands::ui::is_auto_hide_enabled());
    serchx_lib::commands::ui::set_auto_hide(false);
    assert!(!serchx_lib::commands::ui::is_auto_hide_enabled());
}

#[test]
fn test_tauri_conf_spotlight_window_properties() {
    let conf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let conf_str = fs::read_to_string(&conf_path).expect("tauri.conf.json must exist and be readable");
    let conf_json: serde_json::Value =
        serde_json::from_str(&conf_str).expect("tauri.conf.json must be valid JSON");

    let windows = conf_json["app"]["windows"]
        .as_array()
        .expect("app.windows must be an array");
    assert!(!windows.is_empty(), "Must configure at least one window");

    let main_win = &windows[0];
    assert_eq!(main_win["width"], 680);
    assert_eq!(main_win["height"], 64);
    assert_eq!(main_win["resizable"], false);
    assert_eq!(main_win["decorations"], false);
    assert_eq!(main_win["transparent"], true);
    assert_eq!(main_win["alwaysOnTop"], true);
}

#[test]
fn test_window_position_packing_and_recording() {
    use serchx_lib::commands::ui::{pack_pos, unpack_pos, record_window_position, get_recorded_window_position};

    // Test positive coordinates
    let packed = pack_pos(450, 200);
    assert_eq!(unpack_pos(packed), (450, 200));

    // Test negative coordinates (multi-monitor setups)
    let packed_neg = pack_pos(-300, -150);
    assert_eq!(unpack_pos(packed_neg), (-300, -150));

    // Test recording and retrieving
    record_window_position(520, 180);
    assert_eq!(get_recorded_window_position(), Some((520, 180)));
}

#[test]
fn test_activation_policy_types() {
    #[cfg(target_os = "macos")]
    {
        let reg = tauri::ActivationPolicy::Regular;
        let acc = tauri::ActivationPolicy::Accessory;
        assert!(matches!(reg, tauri::ActivationPolicy::Regular));
        assert!(matches!(acc, tauri::ActivationPolicy::Accessory));
    }
}

#[test]
fn test_tauri_conf_settings_window_properties() {
    let conf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let conf_str = fs::read_to_string(&conf_path).expect("tauri.conf.json must exist and be readable");
    let conf_json: serde_json::Value =
        serde_json::from_str(&conf_str).expect("tauri.conf.json must be valid JSON");

    let windows = conf_json["app"]["windows"]
        .as_array()
        .expect("app.windows must be an array");
    let settings_win = windows
        .iter()
        .find(|w| w["label"] == "settings" || w["title"] == "serchX Settings")
        .expect("Dedicated settings window must be configured");

    assert_eq!(settings_win["label"], "settings");
    assert_eq!(settings_win["width"], 720);
    assert_eq!(settings_win["height"], 520);
    assert_eq!(settings_win["minWidth"], 640);
    assert_eq!(settings_win["minHeight"], 440);
    assert_eq!(settings_win["title"], "serchX Settings");
    assert_eq!(settings_win["decorations"], true);
    assert_eq!(settings_win["transparent"], false);
    assert_eq!(settings_win["resizable"], true);
    assert_eq!(settings_win["visible"], false);
    assert_eq!(settings_win["url"], "settings.html");
}

#[test]
fn test_blur_debounce_logic() {
    use serchx_lib::commands::ui::is_blur_debounced;
    use std::time::{Duration, Instant};

    // None should not debounce
    assert!(!is_blur_debounced(None, 350));

    // Instant within 350ms should debounce
    let recent = Some(Instant::now());
    assert!(is_blur_debounced(recent, 350));

    // Instant older than 350ms should not debounce
    let past = Some(Instant::now().checked_sub(Duration::from_millis(400)).unwrap());
    assert!(!is_blur_debounced(past, 350));
}

#[test]
fn test_get_indexing_queue_returns_snapshot() {
    let coordinator = serchx_lib::crawler::CrawlCoordinator::new(None);
    let snapshot = coordinator.get_snapshot();
    assert!(!snapshot.is_active);
    assert_eq!(snapshot.total_queue_depth, 0);
    assert!(snapshot.recent_events.is_empty());
}

#[test]
fn test_get_corpus_analytics_format() {
    let db = serchx_lib::db::DatabaseManager::new_in_memory().unwrap();
    let analytics = db.get_corpus_analytics().unwrap();
    assert_eq!(analytics.total_files, 0);
    assert_eq!(analytics.total_bytes, 0);
    assert_eq!(analytics.total_chunks, 0);

    let storage = db.get_storage_breakdown(None).unwrap();
    assert_eq!(storage.original_files_bytes, 0);
    assert!(storage.sqlite_db_bytes > 0);
}

#[test]
fn test_get_system_telemetry_rss_and_cpu() {
    let db = serchx_lib::db::DatabaseManager::new_in_memory().unwrap();
    let telemetry = serchx_lib::commands::ui::collect_system_telemetry(&db);
    assert!(telemetry.memory_rss_mb > 0.0);
    assert!(telemetry.cpu_percent >= 0.0 && telemetry.cpu_percent <= 100.0);
    assert!(telemetry.db_size_mb >= 0.0);
}

#[test]
fn test_blacklist_folder_ipc_flow() {
    let db = serchx_lib::db::DatabaseManager::new_in_memory().unwrap();
    let added = db.add_blacklist_folder("/Users/test/Secret").unwrap();
    assert_eq!(added.path, "/Users/test/Secret");

    let list = db.get_blacklist_folders().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].path, "/Users/test/Secret");

    db.remove_blacklist_folder(&added.id).unwrap();
    let list_after = db.get_blacklist_folders().unwrap();
    assert!(list_after.is_empty());
}


