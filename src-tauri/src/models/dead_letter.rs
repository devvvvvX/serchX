use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterItem {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub error_type: String,
    pub error_message: String,
    pub failed_at: i64,
    pub retry_count: u32,
    pub status: String,
}
