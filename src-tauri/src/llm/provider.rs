use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate_stream(
        &self,
        prompt: &str,
        system_prompt: &str,
        temperature: f32,
        max_tokens: usize,
        cancel_token: CancellationToken,
        token_sender: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn model_name(&self) -> &str;
    fn provider_type(&self) -> &str;
    fn endpoint(&self) -> Option<&str> {
        None
    }
    fn is_loaded(&self) -> bool;
    async fn load(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    async fn unload(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn memory_usage_bytes(&self) -> Option<u64> {
        None
    }
    fn model_footprint_bytes(&self) -> Option<u64> {
        None
    }
}
