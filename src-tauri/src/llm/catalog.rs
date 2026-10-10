use super::hardware::ModelTier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCatalogItem {
    pub id: String,
    pub display_name: String,
    pub tier: ModelTier,
    pub parameter_size: String,
    pub quantization: String,
    pub download_size_bytes: u64,
    pub download_url: String,
    pub sha256: String,
    pub filename: String,
    pub is_downloaded: bool,
    pub is_active: bool,
    pub is_custom: bool,
}

pub struct ModelCatalog;

impl ModelCatalog {
    pub fn default_models_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("serchX")
            .join("models")
    }

    pub fn curated_models() -> Vec<ModelCatalogItem> {
        vec![
            ModelCatalogItem {
                id: "qwen2.5-0.5b-instruct-q4_k_m".to_string(),
                display_name: "Qwen 2.5 0.5B Instruct".to_string(),
                tier: ModelTier::Lightweight,
                parameter_size: "0.5B".to_string(),
                quantization: "Q4_K_M".to_string(),
                download_size_bytes: 491_400_032,
                download_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
                sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db".to_string(),
                filename: "qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
                is_downloaded: false,
                is_active: false,
                is_custom: false,
            },
            ModelCatalogItem {
                id: "qwen2.5-1.5b-instruct-q4_k_m".to_string(),
                display_name: "Qwen 2.5 1.5B Instruct".to_string(),
                tier: ModelTier::Lightweight,
                parameter_size: "1.5B".to_string(),
                quantization: "Q4_K_M".to_string(),
                download_size_bytes: 1_117_320_736,
                download_url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
                sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e".to_string(),
                filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
                is_downloaded: false,
                is_active: false,
                is_custom: false,
            },
            ModelCatalogItem {
                id: "qwen2.5-3b-instruct-q4_k_m".to_string(),
                display_name: "Qwen 2.5 3B Instruct".to_string(),
                tier: ModelTier::Standard,
                parameter_size: "3B".to_string(),
                quantization: "Q4_K_M".to_string(),
                download_size_bytes: 2_104_932_768,
                download_url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf".to_string(),
                sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d".to_string(),
                filename: "qwen2.5-3b-instruct-q4_k_m.gguf".to_string(),
                is_downloaded: false,
                is_active: false,
                is_custom: false,
            },
            ModelCatalogItem {
                id: "qwen3.8-4b-distill-q4_k_m".to_string(),
                display_name: "Qwen 3.8 4B Distill".to_string(),
                tier: ModelTier::Power,
                parameter_size: "4B".to_string(),
                quantization: "Q4_K_M".to_string(),
                download_size_bytes: 2_783_446_304,
                download_url: "https://huggingface.co/empero-ai/Qwen3.8-4B-Distill-GGUF/resolve/main/Qwen3.8-4B-Q4_K_M.gguf".to_string(),
                sha256: "dec96e8cf2e11b613bb46513dec485377f9ca5a351e71712ee0e244f287c6790".to_string(),
                filename: "qwen3.8-4b-distill-q4_k_m.gguf".to_string(),
                is_downloaded: false,
                is_active: false,
                is_custom: false,
            },
        ]
    }

    fn parse_quantization(filename: &str) -> String {
        let upper = filename.to_uppercase();
        for q in &[
            "Q4_K_M", "Q4_K_S", "Q5_K_M", "Q5_K_S", "Q6_K", "Q8_0", "Q4_0", "Q4_1",
            "Q2_K", "Q3_K_M", "Q3_K_L", "Q3_K_S", "Q5_0", "Q5_1", "FP16", "BF16",
        ] {
            if upper.contains(q) {
                return q.to_string();
            }
        }
        "GGUF".to_string()
    }

    fn parse_parameter_size(name: &str) -> String {
        let upper = name.to_uppercase();
        for p in &[
            "0.5B", "1.5B", "3B", "4B", "7B", "8B", "9B", "14B", "32B", "70B", "72B",
        ] {
            if upper.contains(p) {
                return p.to_string();
            }
        }
        "Custom".to_string()
    }

    fn scan_ggufs_in_dir(
        dir: &Path,
        is_lmstudio: bool,
        models: &mut Vec<ModelCatalogItem>,
        active_model_id: &str,
    ) {
        if !dir.exists() {
            return;
        }

        for entry in walkdir::WalkDir::new(dir)
            .follow_links(true)
            .max_depth(6)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("gguf") {
                        if let Ok(meta) = path.metadata() {
                            if meta.len() > 0 {
                                let file_name = path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                let path_str = path.to_string_lossy().to_string();

                                // Skip if filename already registered
                                if models.iter().any(|m| m.filename == file_name || m.filename == path_str) {
                                    continue;
                                }

                                let clean_stem = file_name
                                    .trim_end_matches(".gguf")
                                    .trim_end_matches(".GGUF");
                                let quant = Self::parse_quantization(&file_name);
                                let param_size = Self::parse_parameter_size(&path_str);
                                let display_name = if is_lmstudio {
                                    format!("LM Studio: {}", clean_stem)
                                } else {
                                    format!("Custom: {}", clean_stem)
                                };
                                let id = if is_lmstudio {
                                    format!("lmstudio-{}", clean_stem.to_lowercase().replace(' ', "-"))
                                } else {
                                    format!("custom-{}", clean_stem.to_lowercase().replace(' ', "-"))
                                };

                                let is_active = id == active_model_id
                                    || file_name == active_model_id
                                    || path_str == active_model_id
                                    || clean_stem == active_model_id;

                                models.push(ModelCatalogItem {
                                    id,
                                    display_name,
                                    tier: ModelTier::Standard,
                                    parameter_size: param_size,
                                    quantization: quant,
                                    download_size_bytes: meta.len(),
                                    download_url: String::new(),
                                    sha256: String::new(),
                                    filename: path_str,
                                    is_downloaded: true,
                                    is_active,
                                    is_custom: true,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn discover_models(models_dir: &Path, active_model_id: &str) -> Vec<ModelCatalogItem> {
        let mut models = Self::curated_models();
        let _ = std::fs::create_dir_all(models_dir);

        // Update curated models status
        for item in &mut models {
            let path = models_dir.join(&item.filename);
            item.is_downloaded = path.exists() && path.metadata().map(|m| m.len() > 10_000_000).unwrap_or(false);
            item.is_active = item.id == active_model_id || item.filename == active_model_id;
        }

        // Check for custom GGUFs in serchX models directory
        Self::scan_ggufs_in_dir(models_dir, false, &mut models, active_model_id);

        // Check for LM Studio models in ~/.lmstudio/models and ~/.cache/lm-studio/models
        if let Some(home) = dirs::home_dir() {
            let lmstudio_dir = home.join(".lmstudio").join("models");
            Self::scan_ggufs_in_dir(&lmstudio_dir, true, &mut models, active_model_id);

            let lmstudio_cache_dir = home.join(".cache").join("lm-studio").join("models");
            Self::scan_ggufs_in_dir(&lmstudio_cache_dir, true, &mut models, active_model_id);
        }

        // Check for LM Studio models in Application Support
        if let Some(data_dir) = dirs::data_dir() {
            let lmstudio_app_dir = data_dir.join("LM Studio").join("models");
            Self::scan_ggufs_in_dir(&lmstudio_app_dir, true, &mut models, active_model_id);
        }

        // If no model is active and at least one is downloaded, default active to the first downloaded
        if !models.iter().any(|m| m.is_active) {
            if let Some(first_downloaded) = models.iter_mut().find(|m| m.is_downloaded) {
                first_downloaded.is_active = true;
            } else if let Some(default_item) = models.iter_mut().find(|m| m.id == "qwen2.5-1.5b-instruct-q4_k_m") {
                default_item.is_active = true;
            }
        }

        models
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_model_catalog() {
        let curated = ModelCatalog::curated_models();
        assert_eq!(curated.len(), 4);
        assert!(curated[0].download_url.starts_with("https://huggingface.co"));

        let tmp = tempdir().unwrap();
        let models = ModelCatalog::discover_models(tmp.path(), "qwen2.5-1.5b-instruct-q4_k_m");
        assert!(models.len() >= 4);
        assert!(models.iter().any(|m| m.is_active && m.id == "qwen2.5-1.5b-instruct-q4_k_m"));
    }
}
