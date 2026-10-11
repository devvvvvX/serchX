use super::anthropic::AnthropicCompatClient;
use super::catalog::ModelCatalog;
use super::lm_studio::LmStudioClient;
use super::native::NativeMetalRunner;
use super::ollama::OllamaClient;
use super::openai::OpenAiCompatClient;
use super::provider::LlmProvider;
use crate::models::AppSettings;
use crate::safety::power::check_power_conditions;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatus {
    pub provider: String,
    pub model_name: String,
    pub endpoint: Option<String>,
    pub is_loaded: bool,
    pub is_synthesizing: bool,
    pub on_battery: bool,
    pub ram_usage_bytes: Option<u64>,
    #[serde(default)]
    pub model_size_bytes: Option<u64>,
}

pub struct LlmManager {
    active_provider: Arc<RwLock<Option<Arc<dyn LlmProvider>>>>,
    active_cancel_token: Arc<Mutex<Option<CancellationToken>>>,
    idle_unload_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    is_synthesizing: Arc<AtomicBool>,
    idle_ttl: Duration,
    models_dir: PathBuf,
}

impl Default for LlmManager {
    fn default() -> Self {
        Self::new(ModelCatalog::default_models_dir())
    }
}

impl LlmManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            active_provider: Arc::new(RwLock::new(None)),
            active_cancel_token: Arc::new(Mutex::new(None)),
            idle_unload_task: Arc::new(Mutex::new(None)),
            is_synthesizing: Arc::new(AtomicBool::new(false)),
            idle_ttl: Duration::from_secs(300), // 5-minute idle TTL (D-02)
            models_dir,
        }
    }

    pub fn with_ttl(models_dir: PathBuf, ttl: Duration) -> Self {
        Self {
            active_provider: Arc::new(RwLock::new(None)),
            active_cancel_token: Arc::new(Mutex::new(None)),
            idle_unload_task: Arc::new(Mutex::new(None)),
            is_synthesizing: Arc::new(AtomicBool::new(false)),
            idle_ttl: ttl,
            models_dir,
        }
    }

    pub async fn get_status(&self, settings: &AppSettings) -> LlmStatus {
        let is_synth = self.is_synthesizing.load(Ordering::Relaxed);
        let provider_guard = self.active_provider.read().await;
        let is_loaded = provider_guard
            .as_ref()
            .map(|p| p.is_loaded())
            .unwrap_or(false);
        let ram_usage_bytes = provider_guard.as_ref().and_then(|p| p.memory_usage_bytes());
        let mut model_size_bytes = provider_guard
            .as_ref()
            .and_then(|p| p.model_footprint_bytes());
        let (provider, model_name, endpoint) = if let Some(ref p) = *provider_guard {
            (
                p.provider_type().to_string(),
                p.model_name().to_string(),
                p.endpoint().map(|s| s.to_string()),
            )
        } else {
            let prov = settings.llm_provider.clone();
            if prov == "lm_studio" {
                let model = LmStudioClient::resolve_active_model(
                    &settings.custom_llm_endpoint,
                    &settings.active_llm_model,
                )
                .await;
                (
                    "lm_studio".to_string(),
                    model,
                    Some(settings.custom_llm_endpoint.clone()),
                )
            } else if prov == "native" || prov == "hybrid" || prov == "local" {
                let discovered =
                    ModelCatalog::discover_models(&self.models_dir, &settings.active_llm_model);
                let model_path = discovered
                    .iter()
                    .find(|m| {
                        m.id == settings.active_llm_model || m.filename == settings.active_llm_model
                    })
                    .map(|m| {
                        let p = std::path::Path::new(&m.filename);
                        if p.is_absolute() {
                            p.to_path_buf()
                        } else {
                            self.models_dir.join(&m.filename)
                        }
                    })
                    .unwrap_or_else(|| {
                        let p = std::path::Path::new(&settings.active_llm_model);
                        if p.is_absolute() {
                            p.to_path_buf()
                        } else if settings.active_llm_model.ends_with(".gguf") {
                            self.models_dir.join(&settings.active_llm_model)
                        } else {
                            self.models_dir
                                .join(format!("{}.gguf", settings.active_llm_model))
                        }
                    });
                if let Ok(m) = std::fs::metadata(&model_path) {
                    let ctx_len = if settings.context_length > 0 {
                        settings.context_length
                    } else {
                        2048
                    };
                    model_size_bytes = Some(m.len() + (ctx_len as u64) * 128 * 1024);
                }
                (prov, settings.active_llm_model.clone(), None)
            } else {
                (
                    prov,
                    settings.active_llm_model.clone(),
                    Some(settings.custom_llm_endpoint.clone()),
                )
            }
        };

        if model_size_bytes.is_none()
            && (provider == "native" || provider == "hybrid" || provider == "local")
        {
            let discovered =
                ModelCatalog::discover_models(&self.models_dir, &settings.active_llm_model);
            let model_path = discovered
                .iter()
                .find(|m| {
                    m.id == settings.active_llm_model || m.filename == settings.active_llm_model
                })
                .map(|m| {
                    let p = std::path::Path::new(&m.filename);
                    if p.is_absolute() {
                        p.to_path_buf()
                    } else {
                        self.models_dir.join(&m.filename)
                    }
                })
                .unwrap_or_else(|| {
                    let p = std::path::Path::new(&settings.active_llm_model);
                    if p.is_absolute() {
                        p.to_path_buf()
                    } else if settings.active_llm_model.ends_with(".gguf") {
                        self.models_dir.join(&settings.active_llm_model)
                    } else {
                        self.models_dir
                            .join(format!("{}.gguf", settings.active_llm_model))
                    }
                });
            if let Ok(m) = std::fs::metadata(&model_path) {
                let ctx_len = if settings.context_length > 0 {
                    settings.context_length
                } else {
                    2048
                };
                model_size_bytes = Some(m.len() + (ctx_len as u64) * 128 * 1024);
            }
        }

        let power = check_power_conditions();

        LlmStatus {
            provider,
            model_name,
            endpoint,
            is_loaded,
            is_synthesizing: is_synth,
            on_battery: power.on_battery || power.should_throttle,
            ram_usage_bytes,
            model_size_bytes,
        }
    }

    pub async fn load_active_provider(&self, settings: &AppSettings) -> Result<LlmStatus, String> {
        self.load_active_provider_with_app(settings, None).await
    }

    pub async fn load_active_provider_with_app(
        &self,
        settings: &AppSettings,
        app_handle: Option<AppHandle>,
    ) -> Result<LlmStatus, String> {
        let provider = self.resolve_provider(settings).await;
        provider.load().await.map_err(|e| e.to_string())?;
        self.schedule_idle_unload(provider, app_handle).await;
        Ok(self.get_status(settings).await)
    }

    pub async fn reset_active_provider(&self) {
        self.cancel_idle_unload().await;
        let mut provider_guard = self.active_provider.write().await;
        if let Some(old) = provider_guard.take() {
            tracing::info!(
                "reset_active_provider: unloading cached provider '{}' with model '{}'",
                old.provider_type(),
                old.model_name()
            );
            let _ = old.unload().await;
        }
    }

    pub async fn cancel_synthesis(&self) -> bool {
        let mut token_guard = self.active_cancel_token.lock().await;
        if let Some(token) = token_guard.take() {
            token.cancel();
            self.is_synthesizing.store(false, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub async fn cancel_idle_unload(&self) {
        let mut unload_guard = self.idle_unload_task.lock().await;
        if let Some(task) = unload_guard.take() {
            task.abort();
        }
    }

    pub async fn has_pending_idle_unload(&self) -> bool {
        let guard = self.idle_unload_task.lock().await;
        guard.as_ref().map(|t| !t.is_finished()).unwrap_or(false)
    }

    pub async fn schedule_idle_unload(
        &self,
        provider: Arc<dyn LlmProvider>,
        app_handle: Option<AppHandle>,
    ) {
        let mut unload_guard = self.idle_unload_task.lock().await;
        if let Some(task) = unload_guard.take() {
            task.abort();
        }

        let power = check_power_conditions();
        if power.on_battery || power.should_throttle {
            // Immediate unload when on battery or throttled
            let _ = provider.unload().await;
            if let Some(ref app) = app_handle {
                let _ = app.emit("llm-status-changed", ());
            }
        } else {
            // Idle TTL timer
            let provider_for_unload = provider.clone();
            let app_handle_for_unload = app_handle.clone();
            let ttl = self.idle_ttl;
            let task = tokio::spawn(async move {
                tokio::time::sleep(ttl).await;
                tracing::info!(
                    "idle unload timer expired ({}s): unloading model",
                    ttl.as_secs()
                );
                let _ = provider_for_unload.unload().await;
                if let Some(ref app) = app_handle_for_unload {
                    let _ = app.emit("llm-status-changed", ());
                }
            });
            *unload_guard = Some(task);
        }
    }

    pub async fn resolve_provider(&self, settings: &AppSettings) -> Arc<dyn LlmProvider> {
        let mut provider_guard = self.active_provider.write().await;

        let expected_provider_type = match settings.llm_provider.as_str() {
            "hybrid" | "native" | "local" => "native",
            other => other,
        };

        // Check if cached provider matches active model, provider type, and endpoint
        if let Some(ref current) = *provider_guard {
            let type_matches = current.provider_type() == expected_provider_type;
            let model_matches = current.model_name() == settings.active_llm_model
                || (current.provider_type() == "lm_studio" && !current.model_name().is_empty());
            let endpoint_matches = match current.endpoint() {
                Some(ep) => {
                    ep.trim_end_matches('/') == settings.custom_llm_endpoint.trim_end_matches('/')
                }
                None => true,
            };

            if type_matches && model_matches && endpoint_matches {
                tracing::info!(
                    "resolve_provider: Reusing cached provider '{}' with model '{}'",
                    current.provider_type(),
                    current.model_name()
                );
                return current.clone();
            } else {
                tracing::info!(
                    "resolve_provider: Cached provider mismatch (type: {} vs {}, model: {} vs {}, endpoint: {:?} vs {}). Invalidating cache.",
                    current.provider_type(),
                    expected_provider_type,
                    current.model_name(),
                    settings.active_llm_model,
                    current.endpoint(),
                    settings.custom_llm_endpoint
                );
            }
        }

        // Unload old provider if any
        if let Some(old) = provider_guard.take() {
            let _ = old.unload().await;
        }

        tracing::info!(
            "resolve_provider: Resolving new provider for llm_provider='{}' (target='{}'), model='{}', endpoint='{}'",
            settings.llm_provider,
            expected_provider_type,
            settings.active_llm_model,
            settings.custom_llm_endpoint
        );

        // Route to the appropriate client based on the selected provider
        if settings.llm_provider == "lm_studio" {
            if LmStudioClient::check_availability(&settings.custom_llm_endpoint).await {
                let resolved_model = LmStudioClient::resolve_active_model(
                    &settings.custom_llm_endpoint,
                    &settings.active_llm_model,
                )
                .await;
                tracing::info!(
                    "resolve_provider: Connected to LM Studio at {}. Using model: '{}'",
                    settings.custom_llm_endpoint,
                    resolved_model
                );
                let client = Arc::new(LmStudioClient::new(
                    settings.custom_llm_endpoint.clone(),
                    resolved_model,
                ));
                *provider_guard = Some(client.clone());
                return client;
            } else {
                tracing::warn!(
                    "resolve_provider: LM Studio at '{}' not reachable; falling back to native runner",
                    settings.custom_llm_endpoint
                );
            }
        } else if settings.llm_provider == "openai_compat" {
            if OpenAiCompatClient::check_availability(&settings.custom_llm_endpoint).await {
                tracing::info!(
                    "resolve_provider: Connected to OpenAI compatible server at {}. Using model: '{}'",
                    settings.custom_llm_endpoint,
                    settings.active_llm_model
                );
                let client = Arc::new(OpenAiCompatClient::new(
                    settings.custom_llm_endpoint.clone(),
                    settings.active_llm_model.clone(),
                ));
                *provider_guard = Some(client.clone());
                return client;
            } else {
                tracing::warn!(
                    "resolve_provider: OpenAI compatible server at '{}' not reachable; falling back to native runner",
                    settings.custom_llm_endpoint
                );
            }
        } else if settings.llm_provider == "anthropic_compat" {
            if AnthropicCompatClient::check_availability(&settings.custom_llm_endpoint).await {
                tracing::info!(
                    "resolve_provider: Connected to Anthropic compatible server at {}. Using model: '{}'",
                    settings.custom_llm_endpoint,
                    settings.active_llm_model
                );
                let client = Arc::new(AnthropicCompatClient::new(
                    settings.custom_llm_endpoint.clone(),
                    settings.active_llm_model.clone(),
                ));
                *provider_guard = Some(client.clone());
                return client;
            } else {
                tracing::warn!(
                    "resolve_provider: Anthropic compatible server at '{}' not reachable; falling back to native runner",
                    settings.custom_llm_endpoint
                );
            }
        } else if settings.llm_provider == "ollama" {
            if OllamaClient::check_availability(&settings.custom_llm_endpoint).await {
                tracing::info!(
                    "resolve_provider: Connected to Ollama at {}. Using model: '{}'",
                    settings.custom_llm_endpoint,
                    settings.active_llm_model
                );
                let client = Arc::new(OllamaClient::new(
                    settings.custom_llm_endpoint.clone(),
                    settings.active_llm_model.clone(),
                ));
                *provider_guard = Some(client.clone());
                return client;
            } else {
                tracing::warn!(
                    "resolve_provider: Ollama at '{}' not reachable; falling back to native runner",
                    settings.custom_llm_endpoint
                );
            }
        }

        // Native Metal runner fallback
        let discovered =
            ModelCatalog::discover_models(&self.models_dir, &settings.active_llm_model);
        let model_path = discovered
            .iter()
            .find(|m| m.id == settings.active_llm_model || m.filename == settings.active_llm_model)
            .map(|m| {
                let p = std::path::Path::new(&m.filename);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    self.models_dir.join(&m.filename)
                }
            })
            .unwrap_or_else(|| {
                let p = std::path::Path::new(&settings.active_llm_model);
                if p.is_absolute() {
                    p.to_path_buf()
                } else if settings.active_llm_model.ends_with(".gguf") {
                    self.models_dir.join(&settings.active_llm_model)
                } else {
                    self.models_dir
                        .join(format!("{}.gguf", settings.active_llm_model))
                }
            });

        tracing::info!(
            "resolve_provider: Using Native Metal runner with model '{}' (path: {:?})",
            settings.active_llm_model,
            model_path
        );

        let ctx_len = if settings.context_length > 0 {
            settings.context_length
        } else {
            2048
        };
        let native = Arc::new(NativeMetalRunner::with_context_size(
            model_path,
            settings.active_llm_model.clone(),
            ctx_len,
        ));
        *provider_guard = Some(native.clone());
        native
    }

    pub async fn synthesize(
        self: &Arc<Self>,
        prompt: String,
        system_prompt: String,
        settings: AppSettings,
        on_token: Option<Channel<String>>,
        app_handle: Option<AppHandle>,
    ) -> Result<String, String> {
        tracing::info!("synthesize: starting synthesis request");
        // Abort previous generation if any
        self.cancel_synthesis().await;

        // Cancel previous idle unload timer
        self.cancel_idle_unload().await;

        let cancel_token = CancellationToken::new();
        {
            let mut token_guard = self.active_cancel_token.lock().await;
            *token_guard = Some(cancel_token.clone());
        }

        self.is_synthesizing.store(true, Ordering::Relaxed);

        tracing::info!("synthesize: resolving provider");
        let provider = self.resolve_provider(&settings).await;
        if let Err(e) = provider.load().await {
            tracing::warn!("synthesize: provider load failed: {:?}", e);
            self.is_synthesizing.store(false, Ordering::Relaxed);
            let prov_type = provider.provider_type();
            if prov_type == "native" {
                return Err("NO_MODEL_AVAILABLE: No local language model is downloaded. Please download a model from the Neural Engine tab or configure LM Studio / Ollama in Settings.".to_string());
            } else if prov_type == "lm_studio" {
                return Err("LM_STUDIO_UNREACHABLE: LM Studio is not reachable at the configured endpoint. Please start LM Studio or download an on-device model from the Neural Engine tab.".to_string());
            } else if prov_type == "ollama" {
                return Err("OLLAMA_UNREACHABLE: Ollama is not reachable at the configured endpoint. Please start Ollama or download an on-device model from the Neural Engine tab.".to_string());
            } else {
                return Err(format!(
                    "NO_MODEL_AVAILABLE: Failed to load LLM provider ({}): {}",
                    prov_type, e
                ));
            }
        }

        let active_provider_type = provider.provider_type().to_string();
        let active_model_name = provider.model_name().to_string();

        tracing::info!(
            "synthesize: spawning inference task with provider='{}', model='{}'",
            active_provider_type,
            active_model_name
        );

        if let Some(ref app) = app_handle {
            #[derive(serde::Serialize, Clone)]
            struct LlmStartPayload {
                provider: String,
                model: String,
                endpoint: Option<String>,
                ram_usage_bytes: Option<u64>,
                model_size_bytes: Option<u64>,
            }
            let _ = app.emit(
                "llm-start",
                LlmStartPayload {
                    provider: active_provider_type.clone(),
                    model: active_model_name.clone(),
                    endpoint: provider.endpoint().map(|s| s.to_string()),
                    ram_usage_bytes: provider.memory_usage_bytes(),
                    model_size_bytes: provider.model_footprint_bytes(),
                },
            );
        }

        let (token_tx, mut token_rx) = mpsc::channel::<String>(128);

        // Spawn inference task
        let provider_clone = provider.clone();
        let cancel_clone = cancel_token.clone();
        let inference_handle = tokio::spawn(async move {
            provider_clone
                .generate_stream(
                    &prompt,
                    &system_prompt,
                    settings.llm_temperature,
                    settings.llm_context_window,
                    cancel_clone,
                    token_tx,
                )
                .await
        });

        let mut full_response = String::new();

        tracing::info!("synthesize: waiting for tokens");
        while let Some(token) = token_rx.recv().await {
            full_response.push_str(&token);
            tracing::info!("synthesize: received token: {:?}", token);
            if let Some(ref channel) = on_token {
                if let Err(e) = channel.send(token.clone()) {
                    tracing::warn!("synthesize: channel.send failed: {:?}", e);
                }
            }
            if let Some(ref app) = app_handle {
                if let Err(e) = app.emit("llm-token", token.clone()) {
                    tracing::error!("Failed to emit llm-token: {:?}", e);
                }
            }
        }

        tracing::info!("synthesize: waiting for inference handle to finish");
        let inference_result = inference_handle.await;
        tracing::info!("synthesize: inference finished with {:?}", inference_result);

        self.is_synthesizing.store(false, Ordering::Relaxed);

        let had_error = match inference_result {
            Ok(Err(ref e)) => {
                tracing::error!("synthesize: inference task error: {:?}", e);
                true
            }
            Err(ref e) => {
                tracing::error!("synthesize: inference task join error: {:?}", e);
                true
            }
            _ => false,
        };

        if had_error && full_response.is_empty() {
            self.schedule_idle_unload(provider.clone(), app_handle.clone())
                .await;
            if let Some(ref app) = app_handle {
                let _ = app.emit(
                    "llm-error",
                    "Model inference failed or timed out".to_string(),
                );
            }
            return Err("Model inference failed or timed out".to_string());
        }

        if let Some(ref app) = app_handle {
            #[derive(serde::Serialize, Clone)]
            struct LlmStatusPayload {
                is_loaded: bool,
                ram_usage_bytes: Option<u64>,
                model_size_bytes: Option<u64>,
                model: String,
                provider: String,
            }
            let _ = app.emit(
                "llm-status-changed",
                LlmStatusPayload {
                    is_loaded: provider.is_loaded(),
                    ram_usage_bytes: provider.memory_usage_bytes(),
                    model_size_bytes: provider.model_footprint_bytes(),
                    model: active_model_name.clone(),
                    provider: active_provider_type.clone(),
                },
            );
            let _ = app.emit("llm-done", ());
        }

        // Memory lifecycle handling (D-02, D-16)
        self.schedule_idle_unload(provider.clone(), app_handle.clone())
            .await;

        Ok(full_response)
    }

    pub async fn force_unload(&self) {
        self.cancel_idle_unload().await;
        let provider_guard = self.active_provider.read().await;
        if let Some(ref p) = *provider_guard {
            let _ = p.unload().await;
        }
    }
}
