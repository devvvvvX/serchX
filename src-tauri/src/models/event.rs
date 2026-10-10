use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEventType {
    Created,
    Modified,
    Opened,
    Downloaded,
    Renamed,
    Deleted,
}

impl FileEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Opened => "opened",
            Self::Downloaded => "downloaded",
            Self::Renamed => "renamed",
            Self::Deleted => "deleted",
        }
    }
}

impl std::fmt::Display for FileEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEvent {
    pub id: String,
    pub artifact_id: Option<String>,
    pub event_type: String, // 'created', 'modified', 'opened', 'downloaded', 'renamed', 'deleted'
    pub timestamp: i64,
    pub metadata: Option<serde_json::Value>,
}

impl FileEvent {
    pub fn new(
        artifact_id: Option<String>,
        event_type: FileEventType,
        timestamp: i64,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            artifact_id,
            event_type: event_type.as_str().to_string(),
            timestamp,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: String, // 'person', 'vendor', 'project'
}

impl Entity {
    pub fn new(name: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            entity_type: entity_type.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
    pub weight: f64,
}

impl Relationship {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        relationship_type: impl Into<String>,
        weight: f64,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            target_id: target_id.into(),
            relationship_type: relationship_type.into(),
            weight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_event_types() {
        let ev = FileEvent::new(
            Some("art-1".to_string()),
            FileEventType::Downloaded,
            1718000000,
            None,
        );
        assert_eq!(ev.event_type, "downloaded");
    }
}
