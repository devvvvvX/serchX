pub mod extractors;
pub mod ocr;
pub mod pipeline;
pub mod watcher;

pub use pipeline::IngestionPipeline;
pub use watcher::{FileSystemWatcher, FileWatcher, WatchEvent, WatchEventKind};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IngestionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Extraction error: {0}")]
    Extraction(String),
    #[error("OCR error: {0}")]
    Ocr(String),
    #[error("Database error: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("Watcher error: {0}")]
    Watcher(String),
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("Filtered: {0}")]
    Filtered(#[from] crate::safety::FilterRejectionReason),
    #[error("Timed out after {0}s: {1}")]
    Timeout(u64, String),
}
