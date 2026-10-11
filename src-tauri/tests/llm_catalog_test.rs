use serchx_lib::llm::{HardwareProfile, ModelCatalog, ModelTier};
use serchx_lib::models::AppSettings;
use tempfile::tempdir;

#[test]
fn test_hardware_profiler() {
    let profile = HardwareProfile::detect();
    assert!(profile.total_memory_bytes > 0);
    assert!(profile.total_memory_gb > 0);
    assert!(!profile.chip_name.is_empty());

    // Verify mapping
    assert_eq!(
        HardwareProfile::recommended_tier_for_memory(8),
        ModelTier::Lightweight
    );
    assert_eq!(
        HardwareProfile::recommended_tier_for_memory(16),
        ModelTier::Standard
    );
    assert_eq!(
        HardwareProfile::recommended_tier_for_memory(32),
        ModelTier::Power
    );
}

#[test]
fn test_model_catalog() {
    let curated = ModelCatalog::curated_models();
    assert_eq!(curated.len(), 4);
    assert_eq!(curated[0].tier, ModelTier::Lightweight);
    assert_eq!(curated[1].tier, ModelTier::Lightweight);
    assert_eq!(curated[2].tier, ModelTier::Standard);
    assert_eq!(curated[3].tier, ModelTier::Power);

    let tmp = tempdir().unwrap();
    let models = ModelCatalog::discover_models(tmp.path(), "qwen2.5-1.5b-instruct-q4_k_m");
    assert!(models.len() >= 4);
    let active = models
        .iter()
        .find(|m| m.is_active)
        .expect("Should have active model");
    assert_eq!(active.id, "qwen2.5-1.5b-instruct-q4_k_m");
}

#[test]
fn test_app_settings_llm_defaults() {
    let settings = AppSettings::default();
    assert_eq!(settings.llm_provider, "native");
    assert_eq!(settings.active_llm_model, "qwen2.5-1.5b-instruct-q4_k_m");
    assert_eq!(settings.custom_llm_endpoint, "http://127.0.0.1:11434");
    assert_eq!(settings.llm_context_window, 4096);
    assert_eq!(settings.llm_temperature, 0.2);
    assert!(settings.llm_enabled);
}

#[test]
fn test_discover_models_with_symlinked_gguf() {
    let tmp_src = tempdir().unwrap();
    let tmp_models = tempdir().unwrap();

    let target_file = tmp_src.path().join("external_model.gguf");
    std::fs::write(&target_file, vec![0u8; 1024]).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_path = tmp_models.path().join("external_model.gguf");
        symlink(&target_file, &symlink_path).unwrap();

        let models = ModelCatalog::discover_models(tmp_models.path(), "custom-external_model");
        let custom = models.iter().find(|m| m.id == "custom-external_model");
        assert!(
            custom.is_some(),
            "Custom symlinked model should be discovered"
        );
        let custom = custom.unwrap();
        assert!(custom.is_custom);
        assert!(custom.is_downloaded);
        assert!(custom.is_active);
        assert_eq!(custom.download_size_bytes, 1024);
    }
}
