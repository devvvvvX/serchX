use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedFolder {
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub recursive: bool,
    pub added_at: i64,
    pub last_scanned_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlacklistFolder {
    pub id: String,
    pub path: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FolderAnalysisReport {
    pub folder_path: String,
    pub total_files_scanned: usize,
    pub total_indexable_files: usize,
    pub total_indexable_bytes: u64,
    pub ignored_files_count: usize,
    pub documents_count: usize,
    pub documents_bytes: u64,
    pub images_count: usize,
    pub images_bytes: u64,
    pub code_count: usize,
    pub code_bytes: u64,
    pub data_count: usize,
    pub data_bytes: u64,
    pub estimated_time_seconds: u64,
    pub scan_duration_ms: u64,
}
