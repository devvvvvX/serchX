use super::provider::LlmProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct LmStudioClient {
    endpoint: String,
    model_name: String,
    loaded: Arc<AtomicBool>,
}

impl LmStudioClient {
    pub fn new(endpoint: String, model_name: String) -> Self {
        Self {
            endpoint,
            model_name,
            loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_localhost(url: &str) -> bool {
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                return host == "localhost"
                    || host == "127.0.0.1"
                    || host == "0.0.0.0"
                    || host == "::1"
                    || host == "[::1]";
            }
        }
        url.contains("localhost") || url.contains("127.0.0.1") || url.contains("0.0.0.0")
    }

    pub async fn check_availability(endpoint: &str) -> bool {
        if !Self::is_localhost(endpoint) {
            tracing::warn!("lm_studio: endpoint '{}' is not localhost", endpoint);
            return false;
        }

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("lm_studio: failed to create HTTP client: {:?}", e);
                return false;
            }
        };

        // Try LM Studio v1 models endpoint
        let models_url = format!("{}/v1/models", endpoint.trim_end_matches('/'));
        match client.get(&models_url).send().await {
            Ok(res) if res.status().is_success() => {
                tracing::info!("lm_studio: check_availability succeeded at {}", models_url);
                true
            }
            Ok(res) => {
                tracing::warn!(
                    "lm_studio: check_availability received status {} from {}",
                    res.status(),
                    models_url
                );
                false
            }
            Err(e) => {
                tracing::warn!(
                    "lm_studio: check_availability failed connecting to {}: {:?}",
                    models_url,
                    e
                );
                false
            }
        }
    }

    /// Query LM Studio's /v1/models endpoint to resolve the currently active chat model name.
    pub async fn resolve_active_model(endpoint: &str, preferred_model: &str) -> String {
        if !Self::is_localhost(endpoint) {
            return preferred_model.to_string();
        }

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            .build()
        {
            Ok(c) => c,
            Err(_) => return preferred_model.to_string(),
        };

        let models_url = format!("{}/v1/models", endpoint.trim_end_matches('/'));
        if let Ok(res) = client.get(&models_url).send().await {
            if let Ok(json) = res.json::<Value>().await {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    let mut candidate_models: Vec<String> = Vec::new();
                    for item in data {
                        if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                            let lower = id.to_lowercase();
                            if !lower.contains("embed") && !lower.contains("bge") {
                                candidate_models.push(id.to_string());
                            }
                        }
                    }

                    // 1. Exact match
                    if candidate_models.iter().any(|m| m == preferred_model) {
                        return preferred_model.to_string();
                    }

                    // 2. Substring match (stripping custom- prefix)
                    let clean_pref = preferred_model
                        .strip_prefix("custom-")
                        .unwrap_or(preferred_model)
                        .to_lowercase();
                    if let Some(matched) = candidate_models.iter().find(|m| {
                        let m_lower = m.to_lowercase();
                        clean_pref.contains(&m_lower) || m_lower.contains(&clean_pref)
                    }) {
                        return matched.clone();
                    }

                    // 3. Match by base name (e.g. "qwen3")
                    let stem = clean_pref
                        .split(|c| c == '-' || c == '_' || c == '.')
                        .next()
                        .unwrap_or(&clean_pref);
                    if stem.len() >= 3 {
                        if let Some(matched) = candidate_models
                            .iter()
                            .find(|m| m.to_lowercase().contains(stem))
                        {
                            return matched.clone();
                        }
                    }

                    // 4. Fallback to first available chat model in LM Studio
                    if let Some(first) = candidate_models.first() {
                        return first.clone();
                    }
                }
            }
        }
        preferred_model.to_string()
    }
}

#[async_trait]
impl LlmProvider for LmStudioClient {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider_type(&self) -> &str {
        "lm_studio"
    }

    fn endpoint(&self) -> Option<&str> {
        Some(&self.endpoint)
    }

    fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Relaxed)
    }

    async fn unload(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !Self::is_localhost(&self.endpoint) {
            return Err("Endpoint must be local".into());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()?;

        let unload_url = format!(
            "{}/api/v1/models/unload",
            self.endpoint.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model_name
        });

        let _ = client.post(&unload_url).json(&body).send().await;
        self.loaded.store(false, Ordering::Relaxed);
        Ok(())
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
        if !Self::is_localhost(&self.endpoint) {
            return Err(
                "External cloud endpoints strictly prohibited for privacy (zero egress)".into(),
            );
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;

        let gen_url = format!(
            "{}/v1/chat/completions",
            self.endpoint.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": prompt }
            ],
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": true
        });

        tracing::info!(
            "lm_studio generate_stream: sending request to {} with model '{}'",
            gen_url,
            self.model_name
        );
        let response = client.post(&gen_url).json(&body).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("lm_studio generate_stream: received success response");
                self.loaded.store(true, Ordering::Relaxed);
                let mut stream = resp.bytes_stream();
                let mut line_buffer = String::new();
                let mut in_reasoning = false;

                while let Some(chunk_res) = stream.next().await {
                    if cancel_token.is_cancelled() {
                        break;
                    }

                    let chunk = chunk_res?;
                    let text = String::from_utf8_lossy(&chunk);
                    line_buffer.push_str(&text);

                    while let Some(pos) = line_buffer.find('\n') {
                        let line = line_buffer[..pos].trim().to_string();
                        line_buffer = line_buffer[pos + 1..].to_string();

                        if line.is_empty() {
                            continue;
                        }
                        let raw_line = if let Some(stripped) = line.strip_prefix("data:") {
                            stripped.trim()
                        } else {
                            line.trim()
                        };

                        if raw_line == "[DONE]" {
                            if in_reasoning {
                                let _ = token_sender.send("\n</think>\n".to_string()).await;
                            }
                            tracing::info!(
                                "lm_studio generate_stream: stream finished ([DONE] received)"
                            );
                            return Ok(());
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(raw_line) {
                            if let Some(delta) = json
                                .get("choices")
                                .and_then(|c| c.as_array())
                                .and_then(|arr| arr.get(0))
                                .and_then(|c| c.get("delta"))
                            {
                                let reasoning_opt =
                                    delta.get("reasoning_content").and_then(|t| t.as_str());
                                let content_opt = delta.get("content").and_then(|t| t.as_str());

                                if let Some(reasoning) = reasoning_opt {
                                    if !reasoning.is_empty() {
                                        if !in_reasoning {
                                            in_reasoning = true;
                                            if token_sender
                                                .send("<think>\n".to_string())
                                                .await
                                                .is_err()
                                            {
                                                return Ok(());
                                            }
                                        }
                                        if token_sender.send(reasoning.to_string()).await.is_err() {
                                            tracing::warn!(
                                                "lm_studio generate_stream: token_sender closed"
                                            );
                                            return Ok(());
                                        }
                                    }
                                } else if let Some(content) = content_opt {
                                    if !content.is_empty() {
                                        if in_reasoning {
                                            in_reasoning = false;
                                            if token_sender
                                                .send("\n</think>\n".to_string())
                                                .await
                                                .is_err()
                                            {
                                                return Ok(());
                                            }
                                        }
                                        if token_sender.send(content.to_string()).await.is_err() {
                                            tracing::warn!(
                                                "lm_studio generate_stream: token_sender closed"
                                            );
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if in_reasoning {
                    let _ = token_sender.send("\n</think>\n".to_string()).await;
                }
                tracing::info!("lm_studio generate_stream: stream finished (EOF)");
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::error!("lm_studio generate_stream: non-success status: {}", status);
                Err(format!("LM Studio API Error: {}", status).into())
            }
            Err(e) => {
                tracing::error!("lm_studio generate_stream: request error: {}", e);
                Err(format!("Connection failed: {}", e).into())
            }
        }
    }
}
