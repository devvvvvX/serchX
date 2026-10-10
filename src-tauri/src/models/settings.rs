use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingPreset {
    Balanced,
    Light,
    Power,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub preset: SettingPreset,
    pub semantic_search_enabled: bool,
    pub auto_pause_on_battery: bool,
    pub max_file_size_mb: u64,
    pub max_versions_per_artifact: usize,
    pub max_indexed_files: usize,
    pub max_vector_chunks: usize,
    pub thermal_throttle_delay_ms: u64,
    pub embedding_model: String,
    pub embedding_dimension: usize,
    pub fts_weight: f32,
    pub vector_weight: f32,
    pub rrf_k: usize,
    pub indexing_threads: usize,
    pub global_hotkey: String,
    pub llm_provider: String,
    pub active_llm_model: String,
    pub custom_llm_endpoint: String,
    pub llm_context_window: usize,
    pub llm_temperature: f32,
    pub llm_enabled: bool,
    #[serde(default = "default_true")]
    pub show_thinking_tokens: bool,
    #[serde(default = "default_context_length")]
    pub context_length: usize,
}

fn default_true() -> bool {
    true
}

fn default_context_length() -> usize {
    4096
}

impl AppSettings {
    pub fn is_valid_context_length(len: usize) -> bool {
        matches!(len, 2048 | 4096 | 8192 | 16384)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            preset: SettingPreset::Balanced,
            semantic_search_enabled: true,
            auto_pause_on_battery: false,
            max_file_size_mb: 50,
            max_versions_per_artifact: 3,
            max_indexed_files: 20_000,
            max_vector_chunks: 250_000,
            thermal_throttle_delay_ms: 200,
            embedding_model: "AllMiniLML6V2".to_string(),
            embedding_dimension: 384,
            fts_weight: 1.0,
            vector_weight: 1.0,
            rrf_k: 60,
            indexing_threads: 4,
            global_hotkey: "Control+Space".to_string(),
            llm_provider: "native".to_string(),
            active_llm_model: "qwen2.5-1.5b-instruct-q4_k_m".to_string(),
            custom_llm_endpoint: "http://127.0.0.1:11434".to_string(),
            llm_context_window: 8192,
            llm_temperature: 0.2,
            llm_enabled: true,
            show_thinking_tokens: true,
            context_length: 4096,
        }
    }
}
