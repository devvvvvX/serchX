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
pub struct AnthropicCompatClient {
    endpoint: String,
    model_name: String,
    loaded: Arc<AtomicBool>,
}

impl AnthropicCompatClient {
    pub fn new(endpoint: String, model_name: String) -> Self {
        Self {
            endpoint,
            model_name,
            loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    fn is_localhost(url: &str) -> bool {
        url.contains("localhost") || url.contains("127.0.0.1") || url.contains("0.0.0.0")
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

        // Try hitting a generic endpoint
        let models_url = format!("{}/v1/models", endpoint.trim_end_matches('/'));
        match client.get(&models_url).send().await {
            Ok(res) => res.status().is_success() || res.status().as_u16() < 500,
            Err(_) => false,
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicCompatClient {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider_type(&self) -> &str {
        "anthropic_compat"
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
            return Err("External cloud endpoints strictly prohibited for privacy (zero egress)".into());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;

        let gen_url = format!("{}/v1/messages", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model_name,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": prompt }
            ],
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": true
        });

        tracing::info!("anthropic generate_stream: sending request to {}", gen_url);
        let response = client.post(&gen_url)
            .header("x-api-key", "dummy_local_key") // Dummy key for compatible servers
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("anthropic generate_stream: received success response");
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
                        let raw_line = if let Some(stripped) = line.strip_prefix("data:") {
                            stripped.trim()
                        } else {
                            line.trim()
                        };

                        if let Ok(json) = serde_json::from_str::<Value>(raw_line) {
                            let type_str = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if type_str == "content_block_delta" {
                                if let Some(token) = json.get("delta")
                                    .and_then(|d| d.get("text"))
                                    .and_then(|t| t.as_str()) 
                                {
                                    if !token.is_empty() {
                                        if token_sender.send(token.to_string()).await.is_err() {
                                            return Ok(());
                                        }
                                    }
                                }
                            } else if type_str == "message_stop" {
                                return Ok(());
                            }
                        }
                    }
                }
                tracing::info!("anthropic generate_stream: stream finished");
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::error!("anthropic generate_stream: non-success status: {}", status);
                Err(format!("Anthropic API Error: {}", status).into())
            }
            Err(e) => {
                tracing::error!("anthropic generate_stream: request error: {}", e);
                Err(format!("Connection failed: {}", e).into())
            }
        }
    }
}
