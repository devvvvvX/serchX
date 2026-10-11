use crate::db::DatabaseManager;
use crate::llm::LlmManager;
use crate::models::AppSettings;
use std::sync::Arc;
use tauri::{Emitter, State};

#[tauri::command]
pub fn get_settings(db: State<'_, Arc<DatabaseManager>>) -> Result<AppSettings, String> {
    db.get_app_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_settings(
    settings: AppSettings,
    db: State<'_, Arc<DatabaseManager>>,
    manager: State<'_, Arc<LlmManager>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // T-03-01: Settings bounds validation
    if settings.max_file_size_mb == 0 || settings.max_file_size_mb > 10_000 {
        return Err("max_file_size_mb must be between 1 and 10,000 MB".to_string());
    }
    if settings.max_indexed_files == 0 || settings.max_indexed_files > 1_000_000 {
        return Err("max_indexed_files must be between 1 and 1,000,000".to_string());
    }
    if settings.max_vector_chunks == 0 || settings.max_vector_chunks > 5_000_000 {
        return Err("max_vector_chunks must be between 1 and 5,000,000".to_string());
    }
    if settings.embedding_dimension == 0 || settings.embedding_dimension > 4096 {
        return Err("embedding_dimension must be between 1 and 4096".to_string());
    }
    if settings.rrf_k == 0 || settings.rrf_k > 1000 {
        return Err("rrf_k must be between 1 and 1000".to_string());
    }
    if settings.fts_weight < 0.0 || settings.vector_weight < 0.0 {
        return Err("Search weights cannot be negative".to_string());
    }

    db.save_app_settings(&settings).map_err(|e| e.to_string())?;
    manager.reset_active_provider().await;
    tracing::info!("update_settings: settings saved and LLM active provider cache invalidated");

    let _ = app.emit("settings-updated", &settings);

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let _ = app.global_shortcut().unregister_all();
        if let Ok(shortcut) = settings
            .global_hotkey
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
        {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                tracing::error!("Failed to register new global shortcut: {:?}", e);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn reset_settings(
    db: State<'_, Arc<DatabaseManager>>,
    manager: State<'_, Arc<LlmManager>>,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let defaults = AppSettings::default();
    db.save_app_settings(&defaults).map_err(|e| e.to_string())?;
    manager.reset_active_provider().await;

    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let _ = app.global_shortcut().unregister_all();
        if let Ok(shortcut) = defaults
            .global_hotkey
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
        {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                tracing::error!("Failed to register new global shortcut: {:?}", e);
            }
        }
    }

    let _ = app.emit("settings-updated", &defaults);

    Ok(defaults)
}
