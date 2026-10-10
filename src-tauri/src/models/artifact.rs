use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    pub id: String,
    pub kind: String, // 'file', 'image', 'url'
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactVersion {
    pub id: String,
    pub artifact_id: String,
    pub path: String,
    pub content_hash: String,
    pub text_hash: Option<String>,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub previous_version_id: Option<String>,
    pub extracted_text: Option<String>,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentChunk {
    pub id: String,
    pub version_id: String,
    pub chunk_index: i32,
    pub content: String,
}
