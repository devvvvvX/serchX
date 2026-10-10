use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CrawlProgress {
    pub indexed_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    #[serde(default)]
    pub timed_out_count: usize,
    pub total_discovered: usize,
    pub disk_usage_bytes: u64,
    pub current_file: Option<String>,
    pub is_paused: bool,
    pub is_running: bool,
    pub elapsed_millis: u64,
}
