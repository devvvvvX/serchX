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
pub struct OllamaClient {
    endpoint: String,
    model_name: String,
    loaded: Arc<AtomicBool>,
}

impl OllamaClient {
    pub fn new(endpoint: String, model_name: String) -> Self {
        Self {
            endpoint,
            model_name,
            loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn check_availability(endpoint: &str) -> bool {
        if !Self::is_localhost(endpoint) {
            return false;
        }

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        // Try Ollama native tags endpoint
        let tags_url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
        if let Ok(res) = client.get(&tags_url).send().await {
            if res.status().is_success() {
                return true;
            }
        }
        false
    }

    /// Zero-telemetry guard: strictly verifies endpoint is bound to localhost
    pub fn is_localhost(endpoint: &str) -> bool {
        if let Ok(url) = reqwest::Url::parse(endpoint) {
            if let Some(host) = url.host_str() {
                return host == "localhost"
                    || host == "127.0.0.1"
                    || host == "::1"
                    || host == "[::1]";
            }
        }
        false
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider_type(&self) -> &str {
        "ollama"
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

        let unload_url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model_name,
            "keep_alive": 0
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

        let gen_url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model_name,
            "prompt": prompt,
            "system": system_prompt,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens
            },
            "stream": true
        });

        tracing::info!("ollama generate_stream: sending request to {}", gen_url);
        let response = client.post(&gen_url).json(&body).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("ollama generate_stream: received success response");
                self.loaded.store(true, Ordering::Relaxed);
                let mut stream = resp.bytes_stream();
                let mut line_buffer = String::new();

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

                        let raw_line = line.strip_prefix("data: ").unwrap_or(&line);
                        if raw_line.trim() == "[DONE]" {
                            return Ok(());
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(raw_line) {
                            if let Some(token) = json.get("response").and_then(|r| r.as_str()) {
                                if !token.is_empty() {
                                    if token_sender.send(token.to_string()).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            if json.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                                return Ok(());
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => {
                // Try fallback to OpenAI compatible /v1/chat/completions (LM Studio, Ollama /v1)
                let chat_url = format!(
                    "{}/v1/chat/completions",
                    self.endpoint.trim_end_matches('/')
                );
                let chat_body = serde_json::json!({
                    "model": self.model_name,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": prompt}
                    ],
                    "temperature": temperature,
                    "max_tokens": max_tokens,
                    "stream": true
                });

                let chat_resp = client.post(&chat_url).json(&chat_body).send().await?;
                if !chat_resp.status().is_success() {
                    return Err(format!(
                        "Local inference server returned HTTP {}",
                        chat_resp.status()
                    )
                    .into());
                }

                self.loaded.store(true, Ordering::Relaxed);
                let mut stream = chat_resp.bytes_stream();
                let mut line_buffer = String::new();

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

                        if let Some(data) = line.strip_prefix("data: ") {
                            let data = data.trim();
                            if data == "[DONE]" {
                                return Ok(());
                            }
                            if let Ok(json) = serde_json::from_str::<Value>(data) {
                                if let Some(delta) = json["choices"][0]["delta"]["content"].as_str()
                                {
                                    if !delta.is_empty() {
                                        if token_sender.send(delta.to_string()).await.is_err() {
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
