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
pub struct OpenAiCompatClient {
    endpoint: String,
    model_name: String,
    loaded: Arc<AtomicBool>,
}

impl OpenAiCompatClient {
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

        let models_url = format!("{}/v1/models", endpoint.trim_end_matches('/'));
        if let Ok(res) = client.get(&models_url).send().await {
            if res.status().is_success() {
                return true;
            }
        }
        false
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatClient {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn provider_type(&self) -> &str {
        "openai_compat"
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
        // No explicit unload endpoint for generic OpenAI compat
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

        tracing::info!("openai generate_stream: sending request to {}", gen_url);
        let response = client.post(&gen_url).json(&body).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("openai generate_stream: received success response");
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
                            return Ok(());
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(raw_line) {
                            if let Some(delta) = json
                                .get("choices")
                                .and_then(|c| c.as_array())
                                .and_then(|arr| arr.first())
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
                tracing::info!("openai generate_stream: stream finished");
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::error!("openai generate_stream: non-success status: {}", status);
                Err(format!("OpenAI API Error: {}", status).into())
            }
            Err(e) => {
                tracing::error!("openai generate_stream: request error: {}", e);
                Err(format!("Connection failed: {}", e).into())
            }
        }
    }
}
