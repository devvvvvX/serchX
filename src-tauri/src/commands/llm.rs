use crate::db::DatabaseManager;
use crate::llm::{
    ContextBuilder, DownloadManager, DownloadProgress, HardwareProfile, LlmManager, LlmStatus,
    ModelCatalog, ModelCatalogItem,
};
use crate::search::HybridSearchEngine;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_hardware_profile() -> Result<HardwareProfile, String> {
    Ok(HardwareProfile::detect())
}

#[tauri::command]
pub async fn get_model_catalog(
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<ModelCatalogItem>, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let models_dir = ModelCatalog::default_models_dir();
    let mut models = ModelCatalog::discover_models(&models_dir, &settings.active_llm_model);

    // If LM Studio endpoint is configured, probe it to also discover any models served via API
    let lm_endpoint = if !settings.custom_llm_endpoint.is_empty() {
        settings.custom_llm_endpoint.clone()
    } else {
        "http://127.0.0.1:1234".to_string()
    };

    if let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(600))
        .build()
    {
        let models_url = format!("{}/v1/models", lm_endpoint.trim_end_matches('/'));
        if let Ok(res) = client.get(&models_url).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    for item in data {
                        if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                            let lower = id.to_lowercase();
                            if !lower.contains("embed") && !lower.contains("bge") {
                                if !models.iter().any(|m| m.id == id || m.filename.contains(id)) {
                                    models.push(ModelCatalogItem {
                                        id: id.to_string(),
                                        display_name: format!("LM Studio: {}", id),
                                        tier: crate::llm::hardware::ModelTier::Standard,
                                        parameter_size: "Remote / API".to_string(),
                                        quantization: "API".to_string(),
                                        download_size_bytes: 0,
                                        download_url: String::new(),
                                        sha256: String::new(),
                                        filename: id.to_string(),
                                        is_downloaded: true,
                                        is_active: id == settings.active_llm_model,
                                        is_custom: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // If provider is ollama or on default ollama port, also probe Ollama
        let ollama_endpoint =
            if settings.llm_provider == "ollama" && !settings.custom_llm_endpoint.is_empty() {
                settings.custom_llm_endpoint.clone()
            } else {
                "http://127.0.0.1:11434".to_string()
            };
        let tags_url = format!("{}/api/tags", ollama_endpoint.trim_end_matches('/'));
        if let Ok(res) = client.get(&tags_url).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(models_list) = json.get("models").and_then(|m| m.as_array()) {
                    for item in models_list {
                        if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                            let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                            if !models.iter().any(|m| m.id == name) {
                                models.push(ModelCatalogItem {
                                    id: name.to_string(),
                                    display_name: format!("Ollama: {}", name),
                                    tier: crate::llm::hardware::ModelTier::Standard,
                                    parameter_size: "Ollama".to_string(),
                                    quantization: "API".to_string(),
                                    download_size_bytes: size,
                                    download_url: String::new(),
                                    sha256: String::new(),
                                    filename: name.to_string(),
                                    is_downloaded: true,
                                    is_active: name == settings.active_llm_model,
                                    is_custom: true,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(models)
}

#[tauri::command]
pub async fn set_active_model(
    model_id: String,
    db: State<'_, Arc<DatabaseManager>>,
    manager: State<'_, Arc<LlmManager>>,
) -> Result<(), String> {
    let mut settings = db.get_app_settings().map_err(|e| e.to_string())?;
    settings.active_llm_model = model_id.clone();
    db.save_app_settings(&settings).map_err(|e| e.to_string())?;
    manager.reset_active_provider().await;
    tracing::info!(
        "set_active_model: switched active model to '{}' and cleared provider cache",
        model_id
    );
    Ok(())
}

fn emit_download_progress(app: &AppHandle, progress: &DownloadProgress) {
    let _ = app.emit("llm-download-progress", progress);
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.emit("llm-download-progress", progress);
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("llm-download-progress", progress);
    }
}

#[tauri::command]
pub fn get_model_download_progress(
    model_id: String,
    downloader: State<'_, Arc<DownloadManager>>,
) -> Result<Option<DownloadProgress>, String> {
    Ok(downloader.get_progress(&model_id))
}

#[tauri::command]
pub async fn start_model_download(
    model_id: String,
    app_handle: tauri::AppHandle,
    downloader: State<'_, Arc<DownloadManager>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let models_dir = ModelCatalog::default_models_dir();
    let models = ModelCatalog::discover_models(&models_dir, &settings.active_llm_model);
    let target_model = models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model {} not found in catalog", model_id))?;

    let app_clone = app_handle.clone();
    let error_app = app_handle.clone();
    let model_id_for_error = target_model.id.clone();
    let downloader_clone = downloader.inner().clone();

    tokio::spawn(async move {
        let app_for_cb = app_clone.clone();
        let result = downloader_clone
            .download_model(&target_model, &models_dir, move |progress| {
                emit_download_progress(&app_for_cb, &progress);
            })
            .await;

        match result {
            Ok(_) => {
                // Download completed — progress_callback already emitted "complete"
            }
            Err(ref msg) if msg == "Download paused" => {
                // Pause is handled: .part file is flushed, "paused" event was
                // already emitted by the download loop. Nothing more to do.
                tracing::info!("Model download paused for {}", model_id_for_error);
            }
            Err(ref msg) if msg == "Download cancelled" => {
                // Cancellation already emitted "cancelled" event.
                tracing::info!("Model download cancelled for {}", model_id_for_error);
            }
            Err(err) => {
                // Real error (network, IO, hash mismatch, etc.)
                tracing::error!("Model download failed for {}: {}", model_id_for_error, err);
                let err_progress = DownloadProgress {
                    model_id: model_id_for_error,
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    speed_mbps: 0.0,
                    eta_seconds: 0,
                    status: "error".to_string(),
                    error_message: Some(err),
                };
                emit_download_progress(&error_app, &err_progress);
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_model_download(
    model_id: String,
    downloader: State<'_, Arc<DownloadManager>>,
) -> Result<bool, String> {
    Ok(downloader.cancel(&model_id))
}

#[tauri::command]
pub fn pause_model_download(
    model_id: String,
    app_handle: tauri::AppHandle,
    downloader: State<'_, Arc<DownloadManager>>,
) -> Result<bool, String> {
    let paused = downloader.pause(&model_id);
    if paused {
        // Emit an immediate "paused" event so the frontend event listener
        // can toggle buttons without waiting for the next poll cycle.
        if let Some(p) = downloader.get_progress(&model_id) {
            emit_download_progress(&app_handle, &p);
        }
    }
    Ok(paused)
}

/// Resume a paused download. This starts a new download that picks up from the
/// existing .part file using HTTP Range headers — the previous HTTP connection
/// was closed during pause to avoid server-side idle timeouts.
#[tauri::command]
pub async fn resume_model_download(
    model_id: String,
    app_handle: tauri::AppHandle,
    downloader: State<'_, Arc<DownloadManager>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<(), String> {
    if !downloader.is_paused(&model_id) {
        // Also allow resume if a .part file exists on disk (e.g. after app restart)
        let models_dir = ModelCatalog::default_models_dir();
        let settings_check = db.get_app_settings().map_err(|e| e.to_string())?;
        let models_check =
            ModelCatalog::discover_models(&models_dir, &settings_check.active_llm_model);
        let has_part = models_check
            .iter()
            .find(|m| m.id == model_id)
            .map(|m| models_dir.join(format!("{}.part", m.filename)).exists())
            .unwrap_or(false);
        if !has_part {
            return Err("Model is not paused".to_string());
        }
    }

    // Re-use start_model_download logic — download_model internally checks for
    // an existing .part file and sends Range headers to resume.
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let models_dir = ModelCatalog::default_models_dir();
    let models = ModelCatalog::discover_models(&models_dir, &settings.active_llm_model);
    let target_model = models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model {} not found in catalog", model_id))?;

    let app_clone = app_handle.clone();
    let error_app = app_handle.clone();
    let model_id_for_error = target_model.id.clone();
    let downloader_clone = downloader.inner().clone();

    tokio::spawn(async move {
        let app_for_cb = app_clone.clone();
        let result = downloader_clone
            .download_model(&target_model, &models_dir, move |progress| {
                emit_download_progress(&app_for_cb, &progress);
            })
            .await;

        match result {
            Ok(_) => {}
            Err(ref msg) if msg == "Download paused" => {
                tracing::info!("Model download paused for {}", model_id_for_error);
            }
            Err(ref msg) if msg == "Download cancelled" => {
                tracing::info!("Model download cancelled for {}", model_id_for_error);
            }
            Err(err) => {
                tracing::error!("Model download failed for {}: {}", model_id_for_error, err);
                let err_progress = DownloadProgress {
                    model_id: model_id_for_error,
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    speed_mbps: 0.0,
                    eta_seconds: 0,
                    status: "error".to_string(),
                    error_message: Some(err),
                };
                emit_download_progress(&error_app, &err_progress);
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn delete_model(
    model_id: String,
    db: State<'_, Arc<DatabaseManager>>,
    llm_manager: State<'_, Arc<LlmManager>>,
    downloader: State<'_, Arc<DownloadManager>>,
) -> Result<(), String> {
    let mut settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let models_dir = ModelCatalog::default_models_dir();
    let models = ModelCatalog::discover_models(&models_dir, &settings.active_llm_model);
    let target_model = models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model {} not found", model_id))?;

    // 1. Cancel in-flight or paused download if any
    let _ = downloader.cancel(&target_model.id);

    // 2. Unload provider if active or matches
    if target_model.is_active || target_model.id == settings.active_llm_model {
        llm_manager.reset_active_provider().await;
    }

    // 3. Delete file and part file from disk
    DownloadManager::delete_model(&target_model.filename, &models_dir)?;

    // 4. If the active model was deleted, reassign active model to another downloaded model or default
    if target_model.is_active || target_model.id == settings.active_llm_model {
        let remaining_models = ModelCatalog::discover_models(&models_dir, "");
        if let Some(first_downloaded) = remaining_models.iter().find(|m| m.is_downloaded) {
            settings.active_llm_model = first_downloaded.id.clone();
        } else {
            settings.active_llm_model = "qwen2.5-1.5b-instruct-q4_k_m".to_string();
        }
        let _ = db.save_app_settings(&settings);
    }

    Ok(())
}

#[tauri::command]
pub fn reveal_models_dir_in_finder() -> Result<(), String> {
    let models_dir = ModelCatalog::default_models_dir();
    let _ = std::fs::create_dir_all(&models_dir);
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&models_dir).spawn();
    }
    Ok(())
}

#[tauri::command]
pub fn import_custom_model(
    source_path: String,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<ModelCatalogItem, String> {
    tracing::info!(
        "import_custom_model: invoked with source_path: '{}'",
        source_path
    );
    let raw = source_path.trim().trim_matches(|c| c == '"' || c == '\'');
    if raw.is_empty() {
        tracing::error!("import_custom_model: source_path is empty");
        return Err("Source path is empty".to_string());
    }

    // Expand ~ if present
    let path = if raw.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&raw[2..])
        } else {
            std::path::PathBuf::from(raw)
        }
    } else {
        std::path::PathBuf::from(raw)
    };

    if !path.exists() {
        tracing::error!(
            "import_custom_model: path does not exist: {}",
            path.display()
        );
        return Err(format!("Source path does not exist: {}", path.display()));
    }

    tracing::info!("import_custom_model: resolved path: {}", path.display());

    let target_file = if path.is_dir() {
        tracing::info!("import_custom_model: path is a directory, searching for .gguf files...");
        // Search directory for .gguf files
        let mut candidates = Vec::new();
        for entry in walkdir::WalkDir::new(&path)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("gguf") {
                        tracing::info!(
                            "import_custom_model: found candidate .gguf file: {}",
                            p.display()
                        );
                        candidates.push(p.to_path_buf());
                    }
                }
            }
        }

        if candidates.is_empty() {
            tracing::error!("import_custom_model: no .gguf files found in directory");
            return Err(format!(
                "No .gguf model files found in directory '{}'",
                path.display()
            ));
        }

        // Pick largest .gguf file (the primary weights)
        candidates.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
        let selected = candidates.pop().unwrap();
        tracing::info!(
            "import_custom_model: selected largest candidate: {}",
            selected.display()
        );
        selected
    } else if path.is_file() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("gguf") {
            tracing::error!("import_custom_model: file does not have .gguf extension");
            return Err("File must have .gguf extension".to_string());
        }
        tracing::info!("import_custom_model: path is a valid .gguf file");
        path
    } else {
        tracing::error!("import_custom_model: path is neither a file nor a directory");
        return Err("Invalid file or directory".to_string());
    };

    let file_name = target_file
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or("Invalid filename")?
        .to_string();

    tracing::info!("import_custom_model: target file_name: {}", file_name);

    let models_dir = ModelCatalog::default_models_dir();
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;
    let dest_path = models_dir.join(&file_name);

    tracing::info!(
        "import_custom_model: linking/copying to dest_path: {}",
        dest_path.display()
    );

    if let Ok(_meta) = dest_path.symlink_metadata() {
        if std::fs::read_link(&dest_path)
            .map(|p| p == target_file)
            .unwrap_or(false)
        {
            tracing::info!("import_custom_model: symlink already correctly points to target");
            // Already correctly linked to target
        } else {
            tracing::info!(
                "import_custom_model: existing file/symlink found but incorrect, replacing it"
            );
            let _ = std::fs::remove_file(&dest_path);
            #[cfg(unix)]
            let sym_res = std::os::unix::fs::symlink(&target_file, &dest_path);
            #[cfg(not(unix))]
            let sym_res = Err(std::io::Error::new(std::io::ErrorKind::Other, "non-unix"));

            if sym_res.is_err() {
                tracing::warn!("import_custom_model: symlink failed, falling back to copy");
                std::fs::copy(&target_file, &dest_path)
                    .map_err(|e| format!("Failed to copy model: {}", e))?;
            } else {
                tracing::info!("import_custom_model: symlink successfully created");
            }
        }
    } else {
        #[cfg(unix)]
        let sym_res = std::os::unix::fs::symlink(&target_file, &dest_path);
        #[cfg(not(unix))]
        let sym_res = Err(std::io::Error::new(std::io::ErrorKind::Other, "non-unix"));

        if sym_res.is_err() {
            tracing::warn!("import_custom_model: symlink failed, falling back to copy");
            std::fs::copy(&target_file, &dest_path)
                .map_err(|e| format!("Failed to copy model: {}", e))?;
        } else {
            tracing::info!("import_custom_model: symlink successfully created");
        }
    }

    tracing::info!("import_custom_model: model linked/copied. Fetching catalog...");
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let models = ModelCatalog::discover_models(&models_dir, &settings.active_llm_model);

    let result = models
        .into_iter()
        .find(|m| m.filename == file_name)
        .ok_or_else(|| {
            tracing::error!(
                "import_custom_model: failed to find imported model in catalog. filename was: {}",
                file_name
            );
            "Failed to discover imported model".to_string()
        });

    if result.is_ok() {
        tracing::info!("import_custom_model: successfully completed");
    }
    result
}

#[tauri::command]
pub fn pick_model_file_or_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    crate::commands::ui::set_dialog_active(true);
    let script = r#"
tell application (path to frontmost application as text)
    try
        set myItem to choose file of type {"gguf"} with prompt "Select GGUF model file"
        return POSIX path of myItem
    on error
        try
            set myFolder to choose folder with prompt "Or select folder containing GGUF model (e.g. LM Studio)"
            return POSIX path of myFolder
        on error
            return ""
        end try
    end try
end tell
"#;
    let output = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output();
    crate::commands::ui::set_dialog_active(false);

    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }

    match output {
        Ok(out) if out.status.success() => {
            let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path_str.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path_str))
            }
        }
        _ => Ok(None),
    }
}

#[tauri::command]
pub async fn cancel_synthesis(manager: State<'_, Arc<LlmManager>>) -> Result<bool, String> {
    Ok(manager.cancel_synthesis().await)
}

#[tauri::command]
pub async fn get_llm_status(
    manager: State<'_, Arc<LlmManager>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<LlmStatus, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    Ok(manager.get_status(&settings).await)
}

#[tauri::command]
pub async fn unload_llm(
    manager: State<'_, Arc<LlmManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    manager.force_unload().await;
    let _ = app_handle.emit("llm-status-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn load_active_model(
    manager: State<'_, Arc<LlmManager>>,
    db: State<'_, Arc<DatabaseManager>>,
    app_handle: AppHandle,
) -> Result<LlmStatus, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let status = manager
        .load_active_provider_with_app(&settings, Some(app_handle.clone()))
        .await?;
    let _ = app_handle.emit("llm-status-changed", &status);
    Ok(status)
}

use tauri::ipc::Channel;

#[tauri::command]
pub async fn synthesize_search_results(
    query: String,
    max_sources: Option<usize>,
    on_token: Channel<String>,
    hybrid_engine: State<'_, Arc<HybridSearchEngine>>,
    manager: State<'_, Arc<LlmManager>>,
    db: State<'_, Arc<DatabaseManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    if !settings.llm_enabled {
        return Err("Local LLM synthesis is currently disabled in Settings".to_string());
    }

    let search_results = hybrid_engine
        .search(&query, max_sources.unwrap_or(5), &settings)
        .await
        .map_err(|e| e.to_string())?;

    let builder = ContextBuilder::new(max_sources.unwrap_or(5), 1200);
    let evidence = builder.from_search_results(&search_results);

    // Broadcast sources so frontend can immediately render citations
    let _ = app_handle.emit("llm-sources", &evidence);

    let (system_prompt, user_prompt) = builder.build_prompt(&query, &evidence);

    manager
        .synthesize(
            user_prompt,
            system_prompt,
            settings,
            Some(on_token),
            Some(app_handle),
        )
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn summarize_document(
    artifact_id: String,
    on_token: Channel<String>,
    manager: State<'_, Arc<LlmManager>>,
    db: State<'_, Arc<DatabaseManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    if !settings.llm_enabled {
        return Err("Local LLM synthesis is currently disabled in Settings".to_string());
    }

    let versions = db
        .get_artifact_versions_ordered(&artifact_id)
        .map_err(|e| e.to_string())?;
    let latest_version = versions
        .first()
        .ok_or_else(|| "Document version not found".to_string())?;

    let text_content = if let Some(ref text) = latest_version.extracted_text {
        text.clone()
    } else {
        let chunks = db
            .get_chunks_for_version(&latest_version.id)
            .map_err(|e| e.to_string())?;
        chunks
            .into_iter()
            .map(|c| c.content)
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let file_name = std::path::Path::new(&latest_version.path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("Document");

    let builder = ContextBuilder::default();
    let (system_prompt, user_prompt) =
        builder.build_single_document_prompt(file_name, &latest_version.path, &text_content);

    manager
        .synthesize(
            user_prompt,
            system_prompt,
            settings,
            Some(on_token),
            Some(app_handle),
        )
        .await?;

    Ok(())
}

#[derive(serde::Serialize)]
pub struct DiscoveredEndpoint {
    pub provider: String,
    pub endpoint: String,
}

#[tauri::command]
pub async fn probe_local_endpoints() -> Result<Vec<DiscoveredEndpoint>, String> {
    let mut results = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .map_err(|e| e.to_string())?;

    // Try LM Studio (1234)
    let lm_url = "http://127.0.0.1:1234/v1/models";
    if let Ok(res) = client.get(lm_url).send().await {
        if res.status().is_success() {
            results.push(DiscoveredEndpoint {
                provider: "lm_studio".to_string(),
                endpoint: "http://127.0.0.1:1234".to_string(),
            });
        }
    }

    // Try Ollama (11434)
    let ollama_url = "http://127.0.0.1:11434/api/tags";
    if let Ok(res) = client.get(ollama_url).send().await {
        if res.status().is_success() {
            results.push(DiscoveredEndpoint {
                provider: "ollama".to_string(),
                endpoint: "http://127.0.0.1:11434".to_string(),
            });
        }
    }

    // Try generic OpenAI (e.g., vLLM or LocalAI on 8080)
    let openai_url = "http://127.0.0.1:8080/v1/models";
    if let Ok(res) = client.get(openai_url).send().await {
        if res.status().is_success() {
            results.push(DiscoveredEndpoint {
                provider: "openai_compat".to_string(),
                endpoint: "http://127.0.0.1:8080".to_string(),
            });
        }
    }

    Ok(results)
}
