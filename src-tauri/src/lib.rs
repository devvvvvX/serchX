pub mod benchmark;
pub mod commands;
pub mod crawler;
pub mod db;
pub mod diff;
pub mod ingest;
pub mod llm;
pub mod logging;
pub mod models;
pub mod safety;
pub mod scope;
pub mod search;
pub mod vector;

pub struct AppLogGuard(pub Option<tracing_appender::non_blocking::WorkerGuard>);

use crate::crawler::{CrawlController, CrawlerEngine};
use crate::db::DatabaseManager;
use crate::ingest::extractors::ExtractorRegistry;
use crate::ingest::ocr::AppleVisionOcr;
use crate::ingest::IngestionPipeline;
use crate::safety::FileFilter;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let last_blur_instant = Arc::new(Mutex::new(None::<Instant>));
    let last_blur_instant_tray = last_blur_instant.clone();
    let last_blur_instant_win = last_blur_instant.clone();

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_global_shortcut::ShortcutState;
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                if let Some((x, y)) =
                                    crate::commands::ui::get_recorded_window_position()
                                {
                                    let _ = window.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition { x, y },
                                    ));
                                }
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(),
        );
    }

    builder
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let app_data_dir = match app_handle.path().app_data_dir() {
                Ok(dir) if std::fs::create_dir_all(&dir).is_ok() => dir,
                _ => {
                    let fallback = std::path::PathBuf::from(".serchx");
                    let _ = std::fs::create_dir_all(&fallback);
                    fallback
                }
            };
            let db_path = app_data_dir.join("serchx.db");
            let log_buffer = crate::logging::LogRingBuffer::default();
            let log_guard = crate::logging::init_logging(&app_data_dir, log_buffer.clone());
            app.manage(log_buffer);
            app.manage(AppLogGuard(log_guard));
            let db = Arc::new(DatabaseManager::new(&db_path)?);

            let fts = Arc::new(crate::search::Fts5SearchEngine::new(db.clone()));
            let vector_index = Arc::new(crate::vector::SqliteVectorIndex::new(db.clone()));
            let models_cache_dir = app_data_dir.join("models").join("fastembed");
            let _ = std::fs::create_dir_all(&models_cache_dir);

            let embedder: Option<Arc<dyn crate::vector::Embedder>> =
                match crate::vector::FastEmbedder::try_new_with_cache_dir(Some(
                    models_cache_dir.clone(),
                )) {
                    Ok(e) => {
                        tracing::info!(
                            "FastEmbedder successfully initialized at {:?}",
                            models_cache_dir
                        );
                        Some(Arc::new(e) as Arc<dyn crate::vector::Embedder>)
                    }
                    Err(err) => {
                        tracing::error!(
                            "Failed to initialize FastEmbedder at {:?}: {:?}",
                            models_cache_dir,
                            err
                        );
                        None
                    }
                };

            let extractors = Arc::new(ExtractorRegistry::new());
            let ocr = Arc::new(AppleVisionOcr::new());
            let filter = Arc::new(FileFilter::with_excluded_paths(vec![app_data_dir.clone()]));
            let pipeline = Arc::new(IngestionPipeline::with_vector_support(
                db.clone(),
                extractors,
                ocr,
                filter.clone(),
                Some(vector_index.clone()),
                embedder,
            ));
            let controller = CrawlController::new();
            let coordinator = Arc::new(crate::crawler::CrawlCoordinator::new(Some(
                app_handle.clone(),
            )));
            let crawler = Arc::new(CrawlerEngine::with_coordinator(
                pipeline.clone(),
                controller,
                filter,
                Some(db_path),
                coordinator.clone(),
            ));

            let scheduler = Arc::new(crate::vector::ModelScheduler::with_cache_dir(
                60,
                Some(models_cache_dir.clone()),
            ));
            let diff_engine = Arc::new(crate::diff::DiffEngine::new());
            let diff_cache = Arc::new(crate::diff::DiffCache::default());
            let hybrid_engine = Arc::new(crate::search::HybridSearchEngine::with_diff_support(
                db.clone(),
                fts,
                vector_index,
                scheduler,
                diff_engine.clone(),
                diff_cache.clone(),
            ));

            // 1. Get watched folders (no longer auto-seeding defaults to respect user choice)
            let watched_folders = db.get_watched_folders().unwrap_or_default();

            // 2. Setup FileWatcher for enabled watched folders
            let mut file_watcher = crate::ingest::FileWatcher::default();
            let mut enabled_paths = Vec::new();
            for folder in &watched_folders {
                if folder.enabled {
                    let p = std::path::PathBuf::from(&folder.path);
                    if p.exists() {
                        match file_watcher.watch_directory(&p) {
                            Ok(_) => {
                                tracing::info!("Successfully started watching {:?}", p);
                                enabled_paths.push(p);
                            }
                            Err(e) => {
                                tracing::error!("Failed to watch directory {:?}: {:?}", p, e);
                                enabled_paths.push(p);
                            }
                        }
                    }
                }
            }

            // 3. Spawn background event worker for FileWatcher
            if let Some(rx) = file_watcher.take_receiver() {
                let pipeline_for_watcher = pipeline.clone();
                std::thread::spawn(move || {
                    while let Ok(event) = rx.recv() {
                        tracing::info!("FileWatcher received event: {:?}", event);
                        match event.kind {
                            crate::ingest::WatchEventKind::Created
                            | crate::ingest::WatchEventKind::Modified => {
                                if event.path.is_file() {
                                    tracing::info!("Ingesting new/modified file: {:?}", event.path);
                                    let _ = pipeline_for_watcher.ingest_file(&event.path);
                                }
                            }
                            crate::ingest::WatchEventKind::Deleted => {}
                        }
                    }
                });
            }

            let watcher_state = Arc::new(Mutex::new(file_watcher));
            app.manage(watcher_state);

            // Enqueue enabled folders in coordinator so queue reflects startup scan
            let enabled_watched_folders: Vec<crate::models::WatchedFolder> = watched_folders
                .iter()
                .filter(|f| f.enabled)
                .cloned()
                .collect();
            coordinator.enqueue_folders(&enabled_watched_folders);

            // 4. Spawn background startup crawl of enabled folders
            if !enabled_paths.is_empty() {
                let crawler_startup = crawler.clone();
                std::thread::spawn(move || {
                    crawler_startup.crawl_paths(&enabled_paths);
                });
            }

            app.manage(db.clone());
            app.manage(coordinator.clone());
            app.manage(crawler);
            app.manage(hybrid_engine);
            app.manage(diff_engine);
            app.manage(diff_cache);
            app.manage(Arc::new(crate::llm::DownloadManager::new()));
            app.manage(Arc::new(crate::llm::LlmManager::default()));
            // Delete queue — background-thread purge with pause/resume/stop
            let delete_queue = Arc::new(crate::crawler::delete_queue::DeleteQueue::new());
            delete_queue.set_app_handle(app.handle().clone());
            app.manage(delete_queue);
            tracing::info!("serchX core engine initialized on {:?}", app_data_dir);

            // Restore saved window position if exists and on an active screen
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some((x, y))) = db.get_window_position() {
                    let mut is_on_monitor = false;
                    if let Ok(monitors) = window.available_monitors() {
                        for m in monitors {
                            let m_pos = m.position();
                            let m_size = m.size();
                            if x >= m_pos.x - 500
                                && x < m_pos.x + m_size.width as i32
                                && y >= m_pos.y - 50
                                && y < m_pos.y + m_size.height as i32
                            {
                                is_on_monitor = true;
                                break;
                            }
                        }
                    }
                    if is_on_monitor {
                        crate::commands::ui::record_window_position(x, y);
                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition { x, y },
                        ));
                    } else {
                        let _ = window.center();
                        if let Ok(pos) = window.outer_position() {
                            crate::commands::ui::record_window_position(pos.x, pos.y);
                            let _ = db.save_window_position(pos.x, pos.y);
                        }
                    }
                }
            }

            // Set macOS default activation policy to Accessory (hide from Dock by default)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Setup macOS Menu Bar System Tray
            let quit_i =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit serchX", true, None::<&str>)?;
            let settings_i =
                tauri::menu::MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let tray_menu = tauri::menu::Menu::with_items(app, &[&settings_i, &quit_i])?;

            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
            let tray_blur = last_blur_instant_tray.clone();
            let _ = tauri::tray::TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("serchX")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "settings" => {
                        let _ = crate::commands::ui::open_settings_window(app.clone(), None);
                    }
                    _ => {}
                })
                .on_tray_icon_event(move |tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let debounced = {
                            if let Ok(mut lock) = tray_blur.lock() {
                                if let Some(instant) = *lock {
                                    if instant.elapsed() < std::time::Duration::from_millis(350) {
                                        *lock = None;
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        };

                        if debounced {
                            // Tray click caused or coincided with blur dismissal; remain hidden
                            return;
                        }

                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                if let Some((x, y)) =
                                    crate::commands::ui::get_recorded_window_position()
                                {
                                    let _ = window.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition { x, y },
                                    ));
                                }
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app);

            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let hotkey_str = db
                    .get_app_settings()
                    .map(|s| s.global_hotkey)
                    .unwrap_or_else(|_| "Option+Space".to_string());
                if let Ok(shortcut) = hotkey_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    if let Err(e) = app.global_shortcut().register(shortcut) {
                        tracing::error!("Failed to register global shortcut: {:?}", e);
                    }
                } else {
                    tracing::error!("Failed to parse global shortcut: {}", hotkey_str);
                }
            }

            // Safety fallback: Ensure window is revealed even if webview initialization is delayed.
            // Skip if onboarding window is visible (first-run setup in progress).
            let app_handle_fallback = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                // Don't forcibly show main while onboarding is active
                let onboarding_visible = app_handle_fallback
                    .get_webview_window("onboarding")
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);
                if onboarding_visible {
                    return;
                }
                if let Some(window) = app_handle_fallback.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(false) {
                        if let Some((x, y)) = crate::commands::ui::get_recorded_window_position() {
                            let _ = window.set_position(tauri::Position::Physical(
                                tauri::PhysicalPosition { x, y },
                            ));
                        }
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });

            Ok(())
        })
        .on_window_event(move |window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let label = window.label().to_string();
                if label == "settings" || label == "compare" || label == "onboarding" {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    {
                        let app = window.app_handle();
                        let is_visible = |w_label: &str| -> bool {
                            if w_label == label {
                                false
                            } else {
                                app.get_webview_window(w_label)
                                    .and_then(|w| w.is_visible().ok())
                                    .unwrap_or(false)
                            }
                        };
                        if !is_visible("settings")
                            && !is_visible("compare")
                            && !is_visible("onboarding")
                        {
                            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                        }
                    }
                }
            }
            tauri::WindowEvent::Moved(pos) => {
                if window.label() == "main" {
                    crate::commands::ui::record_window_position(pos.x, pos.y);
                }
            }
            tauri::WindowEvent::Focused(false) if window.label() == "main" => {
                if let Ok(mut lock) = last_blur_instant_win.lock() {
                    *lock = Some(Instant::now());
                }
                if let Ok(pos) = window.outer_position() {
                    crate::commands::ui::record_window_position(pos.x, pos.y);
                }
                if let Some(db) = window.app_handle().try_state::<Arc<DatabaseManager>>() {
                    let _ = crate::commands::ui::persist_window_position(&db);
                }
                if !crate::commands::ui::is_dialog_active()
                    && crate::commands::ui::is_auto_hide_enabled()
                {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::search_exact,
            commands::search_hybrid,
            commands::search_grouped,
            commands::get_version_diff,
            commands::get_versions_for_path,
            commands::get_compare_path,
            commands::toggle_version_pin,
            commands::get_default_folders,
            commands::check_folder_permissions,
            commands::get_watched_folders,
            commands::analyze_folder,
            commands::add_watched_folder,
            commands::toggle_watched_folder,
            commands::remove_watched_folder,
            commands::purge_folder_index,
            commands::reset_database_and_reindex,
            commands::start_crawl,
            commands::pause_crawl,
            commands::resume_crawl,
            commands::pause_folder_indexing,
            commands::resume_folder_indexing,
            commands::get_crawl_progress,
            commands::get_dead_letter_queue,
            commands::remove_dead_letter_item,
            commands::clear_dead_letter_queue,
            commands::retry_dead_letter_item,
            commands::prune_versions,
            commands::pin_version,
            commands::get_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::open_file,
            commands::reveal_in_finder,
            commands::pick_folder,
            commands::open_privacy_settings,
            commands::resize_window,
            commands::hide_window,
            commands::show_window,
            commands::set_auto_hide,
            commands::toggle_window,
            commands::start_dragging,
            commands::center_window,
            commands::set_dock_visible,
            commands::open_settings_window,
            commands::open_compare_window,
            commands::open_onboarding_window,
            commands::get_indexing_queue,
            commands::cancel_queued_folder,
            commands::trigger_folder_scan,
            commands::get_system_telemetry,
            commands::get_corpus_analytics,
            commands::get_storage_breakdown,
            commands::get_blacklist_folders,
            commands::add_blacklist_folder,
            commands::remove_blacklist_folder,
            commands::get_app_logs,
            commands::export_app_logs,
            commands::reveal_app_logs_in_finder,
            commands::quick_look_file,
            commands::get_hardware_profile,
            commands::get_model_catalog,
            commands::get_model_download_progress,
            commands::set_active_model,
            commands::start_model_download,
            commands::cancel_model_download,
            commands::pause_model_download,
            commands::resume_model_download,
            commands::delete_model,
            commands::reveal_models_dir_in_finder,
            commands::import_custom_model,
            commands::pick_model_file_or_folder,
            commands::cancel_synthesis,
            commands::get_llm_status,
            commands::unload_llm,
            commands::load_active_model,
            commands::synthesize_search_results,
            commands::probe_local_endpoints,
            commands::summarize_document,
            // Delete queue
            commands::get_delete_queue,
            commands::pause_delete_job,
            commands::resume_delete_job,
            commands::stop_delete_job,
            commands::dismiss_delete_job,
            commands::requeue_folder_after_stop,
        ])
        .build(tauri::generate_context!())
        .expect("error while building serchX application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    if let Some((x, y)) = crate::commands::ui::get_recorded_window_position() {
                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition { x, y },
                        ));
                    }
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
