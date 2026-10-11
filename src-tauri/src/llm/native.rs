use super::provider::LlmProvider;
use async_trait::async_trait;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

static BACKEND: OnceLock<Result<Arc<LlamaBackend>, String>> = OnceLock::new();

pub fn get_llama_backend() -> Result<Arc<LlamaBackend>, Box<dyn std::error::Error + Send + Sync>> {
    BACKEND
        .get_or_init(|| {
            LlamaBackend::init()
                .map(Arc::new)
                .map_err(|e| format!("Failed to initialize LlamaBackend: {:?}", e))
        })
        .as_ref()
        .map(Arc::clone)
        .map_err(|e| e.clone().into())
}

struct LoadedSession {
    model: Arc<LlamaModel>,
    #[allow(dead_code)]
    loaded_at: std::time::Instant,
    memory_bytes: u64,
}

#[derive(Clone)]
pub struct NativeMetalRunner {
    pub model_path: PathBuf,
    pub model_name: String,
    pub context_size: usize,
    session: Arc<tokio::sync::Mutex<Option<LoadedSession>>>,
    loaded: Arc<AtomicBool>,
}

impl NativeMetalRunner {
    pub fn new(model_path: PathBuf, model_name: String) -> Self {
        Self::with_context_size(model_path, model_name, 2048)
    }

    pub fn with_context_size(model_path: PathBuf, model_name: String, context_size: usize) -> Self {
        Self {
            model_path,
            model_name,
            context_size,
            session: Arc::new(tokio::sync::Mutex::new(None)),
            loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Load the GGUF model from disk using llama-cpp-2 with Metal GPU acceleration (n_gpu_layers = 999)
    pub fn load_model(&self) -> Result<LlamaModel, Box<dyn std::error::Error + Send + Sync>> {
        if !self.model_path.exists() {
            return Err(format!("Model file does not exist at {:?}", self.model_path).into());
        }
        if !self.model_path.is_file() {
            return Err(format!("Model path is not a file: {:?}", self.model_path).into());
        }

        let backend = get_llama_backend()?;
        let params = LlamaModelParams::default().with_n_gpu_layers(999);
        let model = LlamaModel::load_from_file(&backend, &self.model_path, &params)
            .map_err(|e| format!("Failed to load GGUF model: {:?}", e))?;
        Ok(model)
    }
}

/// Format prompt using family-specific chat templates (Qwen, Llama 3, or fallback)
pub fn format_prompt(prompt: &str, system_prompt: &str, model_name: &str) -> String {
    let lower_name = model_name.to_lowercase();
    if lower_name.contains("qwen") || lower_name.contains("occ") {
        if system_prompt.trim().is_empty() {
            format!(
                "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                prompt
            )
        } else {
            format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                system_prompt, prompt
            )
        }
    } else if lower_name.contains("llama") {
        if system_prompt.trim().is_empty() {
            format!(
                "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                prompt
            )
        } else {
            format!(
                "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                system_prompt, prompt
            )
        }
    } else {
        if system_prompt.trim().is_empty() {
            prompt.to_string()
        } else {
            format!("{}\n\n{}", system_prompt, prompt)
        }
    }
}

/// Validate prompt tokens against context size headroom
pub fn validate_prompt_tokens(tokens_len: usize, context_size: usize) -> Result<(), String> {
    let limit = context_size.saturating_sub(64);
    if tokens_len > limit {
        Err(format!(
            "Prompt tokens ({}) exceed available context window limit ({})",
            tokens_len, limit
        ))
    } else {
        Ok(())
    }
}

#[async_trait]
impl LlmProvider for NativeMetalRunner {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider_type(&self) -> &str {
        "native"
    }

    fn endpoint(&self) -> Option<&str> {
        None
    }

    fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Relaxed)
    }

    async fn load(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.session.lock().await;
        if let Some(ref mut session) = *guard {
            session.loaded_at = std::time::Instant::now();
            self.loaded.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let model = Arc::new(self.load_model()?);
        let model_size = model.size();
        let file_bytes = std::fs::metadata(&self.model_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let base_bytes = if model_size > 0 {
            model_size
        } else {
            file_bytes
        };
        let kv_cache_bytes = (self.context_size as u64) * 128 * 1024;
        let memory_bytes = base_bytes + kv_cache_bytes;

        *guard = Some(LoadedSession {
            model,
            loaded_at: std::time::Instant::now(),
            memory_bytes,
        });
        self.loaded.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn unload(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.session.lock().await;
        *guard = None;
        self.loaded.store(false, Ordering::Relaxed);
        tracing::info!("native runner: unloaded model and released unified memory");
        Ok(())
    }

    fn memory_usage_bytes(&self) -> Option<u64> {
        if !self.is_loaded() {
            return None;
        }
        if let Ok(guard) = self.session.try_lock() {
            if let Some(ref session) = *guard {
                return Some(session.memory_bytes);
            }
        }
        let file_bytes = std::fs::metadata(&self.model_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let kv_cache_bytes = (self.context_size as u64) * 128 * 1024;
        Some(file_bytes + kv_cache_bytes)
    }

    fn model_footprint_bytes(&self) -> Option<u64> {
        let file_bytes = std::fs::metadata(&self.model_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if file_bytes > 0 {
            let kv_cache_bytes = (self.context_size as u64) * 128 * 1024;
            Some(file_bytes + kv_cache_bytes)
        } else {
            None
        }
    }

    async fn generate_stream(
        &self,
        prompt: &str,
        system_prompt: &str,
        temperature: f32,
        max_tokens: usize,
        cancel_token: CancellationToken,
        token_sender: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!(
            "native generate_stream: start with model '{}'",
            self.model_name
        );

        if cancel_token.is_cancelled() {
            tracing::info!("native generate_stream: already cancelled before start");
            return Ok(());
        }

        if !self.model_path.exists() {
            return Err(format!("Model file does not exist at {:?}", self.model_path).into());
        }

        self.load().await?;
        let model = {
            let guard = self.session.lock().await;
            guard
                .as_ref()
                .map(|s| s.model.clone())
                .ok_or_else(|| "Failed to obtain loaded session".to_string())?
        };
        let backend = get_llama_backend()?;

        let formatted = format_prompt(prompt, system_prompt, &self.model_name);
        let mut tokens = model.vocab().tokenize(formatted.as_bytes(), true, true);

        // Validate context window headroom
        let max_prompt_tokens = self.context_size.saturating_sub(64);
        if tokens.len() > max_prompt_tokens {
            tracing::warn!(
                "Prompt tokens ({}) exceed context headroom ({}), truncating",
                tokens.len(),
                max_prompt_tokens
            );
            let start = tokens.len() - max_prompt_tokens;
            tokens = tokens[start..].to_vec();
        }

        let context_size = self.context_size;
        let (blocking_tx, mut blocking_rx) = tokio::sync::mpsc::channel::<String>(32);
        let cancel_clone = cancel_token.clone();

        let blocking_handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
                .with_n_ctx(std::num::NonZeroU32::new(context_size as u32));
            let mut ctx = model
                .new_context(&backend, ctx_params)
                .map_err(|e| format!("Failed to create LlamaContext: {:?}", e))?;

            let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(512, 1);
            let n_prompt = tokens.len();

            // Ingest prompt tokens in batches of up to 512
            for (chunk_idx, chunk) in tokens.chunks(512).enumerate() {
                if cancel_clone.is_cancelled() {
                    return Ok(());
                }
                batch.clear();
                let start_pos = chunk_idx * 512;
                for (i, &token) in chunk.iter().enumerate() {
                    let pos = (start_pos + i) as i32;
                    let is_last = (start_pos + i) == (n_prompt - 1);
                    batch
                        .add(token, pos, &[0], is_last)
                        .map_err(|e| format!("Batch add failed: {:?}", e))?;
                }
                ctx.decode(&mut batch)
                    .map_err(|e| format!("Batch decode failed: {:?}", e))?;
            }

            let mut sampler = llama_cpp_2::sampling::LlamaSampler::chain_simple([
                llama_cpp_2::sampling::LlamaSampler::temp(temperature.max(0.1)),
                llama_cpp_2::sampling::LlamaSampler::dist(1234),
                llama_cpp_2::sampling::LlamaSampler::greedy(),
            ]);

            let mut decoder = encoding_rs::UTF_8.new_decoder();
            let mut n_cur = n_prompt as i32;
            let limit = ((n_prompt + max_tokens) as i32).min(context_size as i32);
            let vocab = model.vocab();

            while n_cur < limit {
                if cancel_clone.is_cancelled() {
                    tracing::info!("native generation loop: cancel_token flagged, halting");
                    break;
                }

                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                if vocab.is_eog(token) {
                    tracing::info!("native generation loop: encountered EOG token");
                    break;
                }

                let piece = vocab.token_to_piece(token, true, None);
                let mut piece_str = String::with_capacity(piece.len() + 8);
                let _ = decoder.decode_to_string(&piece, &mut piece_str, false);

                if !piece_str.is_empty() {
                    if blocking_tx.blocking_send(piece_str).is_err() {
                        tracing::warn!("native generation loop: blocking channel receiver dropped");
                        break;
                    }
                }

                if n_cur + 1 >= limit {
                    tracing::info!(
                        "native generation loop: reached context limit ({}), concluding generation gracefully",
                        limit
                    );
                    break;
                }

                batch.clear();
                batch
                    .add(token, n_cur, &[0], true)
                    .map_err(|e| format!("Token add failed: {:?}", e))?;
                if let Err(e) = ctx.decode(&mut batch) {
                    tracing::warn!(
                        "native generation loop: decode returned {:?}, concluding generation gracefully",
                        e
                    );
                    break;
                }

                n_cur += 1;
            }

            Ok(())
        });

        // Forward tokens from blocking channel to the async token_sender
        while let Some(piece) = blocking_rx.recv().await {
            if cancel_token.is_cancelled() {
                break;
            }
            if token_sender.send(piece).await.is_err() {
                break;
            }
        }

        match blocking_handle.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(format!("Blocking task panicked: {:?}", e).into()),
        }
    }
}
