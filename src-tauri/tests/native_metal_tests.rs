use llama_cpp_2::llama_backend::LlamaBackend;
use serchx_lib::llm::native::{format_prompt, validate_prompt_tokens};
use serchx_lib::llm::{ContextBuilder, LlmProvider, NativeMetalRunner, StructuredEvidence};
use serchx_lib::models::AppSettings;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[test]
fn test_llama_backend_init() {
    let backend = LlamaBackend::init();
    assert!(backend.is_ok(), "LlamaBackend::init() should succeed");
}

#[tokio::test]
async fn test_native_metal_runner_model_load_nonexistent() {
    let runner = NativeMetalRunner::new(
        PathBuf::from("/non/existent/model.gguf"),
        "non-existent-model".to_string(),
    );
    let result = runner.load_model();
    assert!(
        result.is_err(),
        "Loading a non-existent model should fail with an error"
    );
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("does not exist"),
        "Error message should state file does not exist"
    );
}

#[test]
fn test_native_format_prompt() {
    // 1. Qwen model family
    let qwen_res = format_prompt(
        "What is the tax rate?",
        "You are an assistant.",
        "qwen2.5-3b-instruct-q4_k_m.gguf",
    );
    assert!(qwen_res.contains("<|im_start|>system\nYou are an assistant.<|im_end|>"));
    assert!(qwen_res.contains("<|im_start|>user\nWhat is the tax rate?<|im_end|>"));
    assert!(qwen_res.ends_with("<|im_start|>assistant\n"));

    // 2. Llama model family
    let llama_res = format_prompt(
        "Summarize this document.",
        "System context.",
        "llama-3.2-3b-instruct-q4_k_m.gguf",
    );
    assert!(llama_res.contains("<|begin_of_text|><|start_header_id|>system<|end_header_id|>"));
    assert!(llama_res.contains("<|start_header_id|>user<|end_header_id|>"));
    assert!(llama_res.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));

    // 3. Fallback generic model
    let generic_res = format_prompt("Hello world", "Be helpful", "custom-model.gguf");
    assert_eq!(generic_res, "Be helpful\n\nHello world");
}

#[test]
fn test_native_validate_prompt_tokens() {
    // Context size = 2048, headroom = 64 -> max tokens allowed = 1984
    let ok_res = validate_prompt_tokens(500, 2048);
    assert!(ok_res.is_ok());

    let border_res = validate_prompt_tokens(1984, 2048);
    assert!(border_res.is_ok());

    let overflow_res = validate_prompt_tokens(1985, 2048);
    assert!(overflow_res.is_err());
    let err_msg = overflow_res.unwrap_err();
    assert!(err_msg.contains("exceed available context window limit"));
}

#[tokio::test]
async fn test_native_token_stream_cancel() {
    let runner = NativeMetalRunner::new(
        PathBuf::from("/non/existent/model.gguf"),
        "qwen2.5-3b-instruct-q4_k_m".to_string(),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancelled

    let result = runner
        .generate_stream("test prompt", "", 0.2, 512, cancel_token, tx)
        .await;

    assert!(
        result.is_ok(),
        "Pre-cancelled generation should exit cleanly with Ok(())"
    );
    assert!(
        rx.recv().await.is_none(),
        "No tokens should be sent when pre-cancelled"
    );
}

#[tokio::test]
async fn test_native_cancellation_during_generation() {
    let runner = NativeMetalRunner::new(
        PathBuf::from("/non/existent/model.gguf"),
        "qwen2.5-3b-instruct-q4_k_m".to_string(),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let cancel_token = CancellationToken::new();

    let cancel_clone = cancel_token.clone();
    let cancel_handle = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        cancel_clone.cancel();
    });

    let start = std::time::Instant::now();
    let _ = runner
        .generate_stream(
            "What is the revenue?",
            "You are an assistant.",
            0.2,
            512,
            cancel_token,
            tx,
        )
        .await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "Generation halt must complete within 100ms, took {:?}",
        elapsed
    );
    let _ = cancel_handle.await;
    assert!(
        rx.recv().await.is_none(),
        "No tokens emitted on cancelled stream"
    );
}

#[test]
fn test_native_grounded_synthesis_citations() {
    let builder = ContextBuilder::new(5, 1200);
    let evidence = vec![
        StructuredEvidence {
            index: 1,
            artifact_id: "art-1".to_string(),
            document_title: "Quarterly_Financials_Q3.pdf".to_string(),
            file_path: "/Users/alice/Documents/Quarterly_Financials_Q3.pdf".to_string(),
            modified_at: Some("2024-09-30 14:00".to_string()),
            version_id: "v1".to_string(),
            text_snippet: "Operating margin increased by 4.2% year-over-year reaching $18.4M."
                .to_string(),
            relevance_score: 0.94,
        },
        StructuredEvidence {
            index: 2,
            artifact_id: "art-2".to_string(),
            document_title: "Product_Roadmap_2025.docx".to_string(),
            file_path: "/Users/alice/Documents/Product_Roadmap_2025.docx".to_string(),
            modified_at: Some("2024-10-01 09:30".to_string()),
            version_id: "v2".to_string(),
            text_snippet:
                "Deployment of native Metal inference engine scheduled for Phase 06.1 release."
                    .to_string(),
            relevance_score: 0.88,
        },
    ];

    let (system_prompt, user_prompt) = builder.build_prompt(
        "What are the financial and technical milestones?",
        &evidence,
    );

    // Verify grounded prompt formatting
    assert!(system_prompt.contains("inline citation") && system_prompt.contains("[1] or [2]"));
    assert!(user_prompt.contains("Quarterly_Financials_Q3.pdf"));
    assert!(user_prompt.contains("Product_Roadmap_2025.docx"));
    assert!(user_prompt.contains("Operating margin increased by 4.2%"));
    assert!(user_prompt.contains("native Metal inference engine scheduled"));
    assert!(user_prompt.contains("Answer the user's query using the evidence below."));

    // Verify formatting with Qwen template
    let qwen_prompt = format_prompt(
        &user_prompt,
        &system_prompt,
        "qwen2.5-3b-instruct-q4_k_m.gguf",
    );
    assert!(qwen_prompt.starts_with("<|im_start|>system\n"));
    assert!(qwen_prompt.contains("<|im_start|>user\n"));
    assert!(qwen_prompt.contains("USER_QUERY:"));
    assert!(qwen_prompt.ends_with("<|im_start|>assistant\n"));

    // Verify formatting with OCC-RAG template
    let occ_prompt = format_prompt(&user_prompt, &system_prompt, "occ-rag-0.6b-q4_k_m.gguf");
    assert!(occ_prompt.starts_with("<|im_start|>system\n"));
    assert!(occ_prompt.contains("<|im_start|>user\n"));
    assert!(occ_prompt.contains("USER_QUERY:"));
    assert!(occ_prompt.ends_with("<|im_start|>assistant\n"));

    // Verify formatting with Llama template
    let llama_prompt = format_prompt(
        &user_prompt,
        &system_prompt,
        "llama-3.2-3b-instruct-q4_k_m.gguf",
    );
    assert!(
        llama_prompt.starts_with("<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n")
    );
    assert!(llama_prompt.contains("<|start_header_id|>user<|end_header_id|>\n\n"));
    assert!(llama_prompt.contains("USER_QUERY:"));
    assert!(llama_prompt.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));

    // Validate tokens fit within standard context length
    let estimated_tokens = qwen_prompt.len() / 4;
    assert!(validate_prompt_tokens(estimated_tokens, 2048).is_ok());
    assert!(validate_prompt_tokens(estimated_tokens, 4096).is_ok());
}

#[test]
fn test_native_refusal_when_no_evidence() {
    let builder = ContextBuilder::new(5, 1200);
    let (system_prompt, user_prompt) =
        builder.build_prompt("Where are my secret offshore bank accounts?", &[]);

    // Empty evidence prompt
    assert!(user_prompt.contains("No matching local-file evidence is available"));

    // System prompt refusal contract
    assert!(system_prompt
        .contains("If the evidence is insufficient to answer the question, output exactly:"));
    assert!(system_prompt.contains("\"I could not find enough evidence in your local files to answer this question with confidence.\""));

    // Formatted model prompt retains refusal instructions
    let formatted = format_prompt(
        &user_prompt,
        &system_prompt,
        "qwen2.5-3b-instruct-q4_k_m.gguf",
    );
    assert!(formatted.contains("I could not find enough evidence in your local files to answer this question with confidence."));
    assert!(formatted.contains("No matching local-file evidence is available"));
    assert!(validate_prompt_tokens(formatted.len() / 4, 2048).is_ok());
}

#[test]
fn test_context_length_persistence() {
    // 1. Default serialization
    let default_settings = AppSettings::default();
    assert_eq!(default_settings.context_length, 4096);

    // 2. Custom value round-trip
    let json = serde_json::to_string(&default_settings).unwrap();
    assert!(json.contains("\"context_length\":4096"));

    let mut custom_settings: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(custom_settings.context_length, 4096);

    custom_settings.context_length = 2048;
    let custom_json = serde_json::to_string(&custom_settings).unwrap();
    let deserialized: AppSettings = serde_json::from_str(&custom_json).unwrap();
    assert_eq!(deserialized.context_length, 2048);

    // 3. Validation helper
    assert!(AppSettings::is_valid_context_length(2048));
    assert!(AppSettings::is_valid_context_length(4096));
    assert!(AppSettings::is_valid_context_length(8192));
    assert!(AppSettings::is_valid_context_length(16384));
    assert!(!AppSettings::is_valid_context_length(1024));
    assert!(!AppSettings::is_valid_context_length(32768));
}

#[tokio::test]
async fn test_native_memory_lifecycle_ttl() {
    let runner = NativeMetalRunner::with_context_size(
        PathBuf::from("/non/existent/model.gguf"),
        "test-model".to_string(),
        2048,
    );

    // Initially unloaded
    assert!(!runner.is_loaded());
    assert!(runner.memory_usage_bytes().is_none());

    // When unloaded again, remains unloaded
    let unload_res = runner.unload().await;
    assert!(unload_res.is_ok());
    assert!(!runner.is_loaded());
    assert!(runner.memory_usage_bytes().is_none());
}
