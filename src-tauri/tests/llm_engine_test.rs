use serchx_lib::llm::{LlmManager, LlmProvider, LmStudioClient, NativeMetalRunner, OllamaClient};
use serchx_lib::models::AppSettings;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_llm_provider_streaming() {
    let tmp = tempdir().unwrap();
    let runner = NativeMetalRunner::new(
        tmp.path().join("qwen-3b.gguf"),
        "qwen2.5-3b-instruct-q4_k_m".to_string(),
    );

    let prompt = r#"
You are serchX Evidence Synthesizer.
EVIDENCE:
[1] Title: "Tax Rates 2024.pdf" (Path: /docs/tax.pdf, Modified: 2024-01-01)
Excerpt: """Corporate tax rate for Delaware LLC is set to 8.7% flat."""
"#;

    // Test cancellation halts streaming immediately
    let (tx_cancel, mut rx_cancel) = mpsc::channel(64);
    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancelled

    let res_cancelled = runner
        .generate_stream(prompt, "", 0.2, 512, cancel_token, tx_cancel)
        .await;
    assert!(res_cancelled.is_ok(), "Cancelled stream should exit cleanly");
    assert!(rx_cancel.recv().await.is_none(), "Cancelled stream must emit zero tokens");

    // Test missing model weights fail safely without crashing
    let (tx, _rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let res_missing = runner
        .generate_stream(prompt, "", 0.2, 512, cancel, tx)
        .await;
    assert!(res_missing.is_err(), "Missing GGUF model must return Err");
}

#[tokio::test]
async fn test_llm_memory_lifecycle() {
    let tmp = tempdir().unwrap();
    // Configure manager with a 50ms idle TTL for rapid testing
    let manager = Arc::new(LlmManager::with_ttl(
        tmp.path().to_path_buf(),
        Duration::from_millis(50),
    ));

    let settings = AppSettings {
        llm_provider: "native".to_string(),
        active_llm_model: "qwen2.5-3b-instruct-q4_k_m".to_string(),
        ..AppSettings::default()
    };

    // Provider starts unloaded
    let status_before = manager.get_status(&settings).await;
    assert!(!status_before.is_loaded);
    assert!(status_before.ram_usage_bytes.is_none());

    // Explicit load fails safely when model file does not exist on disk
    let provider = manager.resolve_provider(&settings).await;
    let load_res = provider.load().await;
    assert!(load_res.is_err(), "Missing GGUF model file must return Err upon load");

    let status_mid = manager.get_status(&settings).await;
    assert!(!status_mid.is_loaded, "Missing model file must leave provider unloaded");
    assert!(status_mid.ram_usage_bytes.is_none());

    // Explicit unload
    let unload_res = provider.unload().await;
    assert!(unload_res.is_ok());
    let status_after = manager.get_status(&settings).await;
    assert!(!status_after.is_loaded, "Model weights must remain unloaded from RAM");
    assert!(status_after.ram_usage_bytes.is_none(), "Unloaded model must not report ram_usage_bytes");
}

#[tokio::test]
async fn test_idle_unload_lifecycle_timer() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(LlmManager::with_ttl(
        tmp.path().to_path_buf(),
        Duration::from_millis(50),
    ));

    let runner = Arc::new(NativeMetalRunner::with_context_size(
        PathBuf::from("/non/existent/model.gguf"),
        "test-model".to_string(),
        2048,
    ));

    // Initially no pending idle unload
    assert!(!manager.has_pending_idle_unload().await);

    // Scheduling idle unload starts the timer
    manager.schedule_idle_unload(runner.clone(), None).await;
    assert!(manager.has_pending_idle_unload().await);

    // Cancel idle unload clears the timer
    manager.cancel_idle_unload().await;
    assert!(!manager.has_pending_idle_unload().await);

    // Rescheduling and allowing it to expire
    manager.schedule_idle_unload(runner.clone(), None).await;
    assert!(manager.has_pending_idle_unload().await);
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(!manager.has_pending_idle_unload().await);

    // force_unload cancels any pending idle unload
    manager.schedule_idle_unload(runner.clone(), None).await;
    assert!(manager.has_pending_idle_unload().await);
    manager.force_unload().await;
    assert!(!manager.has_pending_idle_unload().await);
}

#[tokio::test]
async fn test_external_provider_ram_usage_is_none() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(LlmManager::new(tmp.path().to_path_buf()));

    let ollama_settings = AppSettings {
        llm_provider: "ollama".to_string(),
        custom_llm_endpoint: "http://127.0.0.1:11434".to_string(),
        active_llm_model: "llama3".to_string(),
        ..AppSettings::default()
    };

    let status = manager.get_status(&ollama_settings).await;
    assert_eq!(status.provider, "ollama");
    assert!(status.ram_usage_bytes.is_none(), "Reusing external Ollama model must not report RAM usage");

    let lm_studio_settings = AppSettings {
        llm_provider: "lm_studio".to_string(),
        custom_llm_endpoint: "http://127.0.0.1:1234".to_string(),
        active_llm_model: "qwen2.5".to_string(),
        ..AppSettings::default()
    };

    let status_lm = manager.get_status(&lm_studio_settings).await;
    assert_eq!(status_lm.provider, "lm_studio");
    assert!(status_lm.ram_usage_bytes.is_none(), "Reusing external LM Studio model must not report RAM usage");
}

#[test]
fn test_ollama_localhost_guard() {
    // Valid local endpoints
    assert!(OllamaClient::is_localhost("http://127.0.0.1:11434"));
    assert!(OllamaClient::is_localhost("http://localhost:11434"));
    assert!(OllamaClient::is_localhost("http://127.0.0.1:8080/v1"));
    assert!(OllamaClient::is_localhost("http://[::1]:11434"));

    // Prohibited external cloud endpoints (zero egress guarantee T-06-03)
    assert!(!OllamaClient::is_localhost("https://api.openai.com/v1"));
    assert!(!OllamaClient::is_localhost("https://api.anthropic.com/v1"));
    assert!(!OllamaClient::is_localhost("http://192.168.1.100:11434"));
    assert!(!OllamaClient::is_localhost("http://evil-server.com:11434"));
}

#[tokio::test]
async fn test_llm_provider_switching_and_cache_invalidation() {
    let tmp = tempdir().unwrap();
    let manager = Arc::new(LlmManager::new(tmp.path().to_path_buf()));

    // 1. Initial settings with native model 1
    let settings_1 = AppSettings {
        llm_provider: "native".to_string(),
        active_llm_model: "model-alpha".to_string(),
        ..AppSettings::default()
    };

    let p1 = manager.resolve_provider(&settings_1).await;
    assert_eq!(p1.provider_type(), "native");
    assert_eq!(p1.model_name(), "model-alpha");

    // 2. Request same model & provider -> reuses cache
    let p1_again = manager.resolve_provider(&settings_1).await;
    assert_eq!(p1_again.model_name(), "model-alpha");

    // 3. Switch active model to model 2 -> cache mismatch must resolve new model
    let settings_2 = AppSettings {
        llm_provider: "native".to_string(),
        active_llm_model: "model-beta".to_string(),
        ..AppSettings::default()
    };
    let p2 = manager.resolve_provider(&settings_2).await;
    assert_eq!(p2.provider_type(), "native");
    assert_eq!(p2.model_name(), "model-beta");

    // 4. Test explicit cache reset
    manager.reset_active_provider().await;
    let status_after_reset = manager.get_status(&settings_2).await;
    assert_eq!(status_after_reset.model_name, "model-beta");
}

#[tokio::test]
async fn test_lm_studio_resolve_model() {
    if LmStudioClient::check_availability("http://127.0.0.1:1234").await {
        let resolved = LmStudioClient::resolve_active_model(
            "http://127.0.0.1:1234",
            "custom-qwen3-4b-thinking-2507.Q4_K_M",
        ).await;
        println!("Resolved LM Studio model: {}", resolved);
        assert!(!resolved.is_empty());
        assert!(!resolved.to_lowercase().contains("embed"));
    }
}
