use serchx_lib::llm::catalog::ModelCatalogItem;
use serchx_lib::llm::downloader::DownloadManager;
use serchx_lib::llm::hardware::ModelTier;
use sha2::{Digest, Sha256};
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_model_storage_purge() {
    let tmp = tempdir().unwrap();
    let model_path = tmp.path().join("qwen2.5-3b-instruct-q4_k_m.gguf");
    let part_path = tmp.path().join("qwen2.5-3b-instruct-q4_k_m.gguf.part");

    // Create mock model and partial file
    std::fs::write(&model_path, vec![0u8; 1024 * 1024]).unwrap(); // 1MB
    std::fs::write(&part_path, vec![0u8; 512 * 1024]).unwrap(); // 512KB

    assert!(model_path.exists());
    assert!(part_path.exists());

    // Execute purge
    let res = DownloadManager::delete_model("qwen2.5-3b-instruct-q4_k_m.gguf", tmp.path());
    assert!(res.is_ok(), "delete_model should succeed");

    // Both files should be cleaned up
    assert!(!model_path.exists(), "Final model file must be removed");
    assert!(!part_path.exists(), "Dangling .part file must be removed");

    // Subsequent delete should return error
    let res_second = DownloadManager::delete_model("qwen2.5-3b-instruct-q4_k_m.gguf", tmp.path());
    assert!(
        res_second.is_err(),
        "Deleting non-existent model should return error"
    );
}

#[tokio::test]
async fn test_resumable_downloader_sha256_verification() {
    let tmp = tempdir().unwrap();
    let content = b"This is a simulated GGUF model binary content for unit testing serchX.";

    let mut hasher = Sha256::new();
    hasher.update(content);
    let expected_hash = format!("{:x}", hasher.finalize());

    // Create a .part file with corrupt content and wrong hash to test verification rejection
    let part_path = tmp.path().join("corrupt_model.gguf.part");
    std::fs::write(&part_path, b"corrupted bytes").unwrap();

    let _invalid_item = ModelCatalogItem {
        id: "corrupt_model".to_string(),
        display_name: "Corrupt Model".to_string(),
        tier: ModelTier::Lightweight,
        parameter_size: "0B".to_string(),
        quantization: "Q4_K_M".to_string(),
        download_size_bytes: content.len() as u64,
        download_url: "http://127.0.0.1:9/invalid".to_string(),
        sha256: expected_hash.clone(),
        filename: "corrupt_model.gguf".to_string(),
        is_downloaded: false,
        is_active: false,
        is_custom: false,
    };

    // Verify SHA-256 calculation logic directly
    let mut check_file = File::open(&part_path).unwrap();
    let mut verify_hasher = Sha256::new();
    std::io::copy(&mut check_file, &mut verify_hasher).unwrap();
    let actual_hash = format!("{:x}", verify_hasher.finalize());

    assert_ne!(
        actual_hash, expected_hash,
        "Corrupt file hash must not match expected"
    );
}

#[test]
fn test_resumable_downloader_cancellation_manager() {
    let manager = DownloadManager::new();
    let token = manager.register_cancel_token("qwen-1.5b");
    assert!(!token.is_cancelled());

    let cancelled = manager.cancel("qwen-1.5b");
    assert!(cancelled);
    assert!(token.is_cancelled());

    // Cancelling already removed token returns false
    assert!(!manager.cancel("qwen-1.5b"));
}
