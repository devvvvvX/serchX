pub mod cache;
pub mod engine;

pub use cache::DiffCache;
pub use engine::{DiffEngine, DiffHunk, DiffSummary, VersionDiff};
