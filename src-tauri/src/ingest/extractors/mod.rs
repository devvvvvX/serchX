pub mod docx;
pub mod pdf;
pub mod text;

pub use docx::DocxExtractor;
pub use pdf::PdfExtractor;
pub use text::PlainTextExtractor;

use crate::ingest::IngestionError;
use std::path::Path;
use std::sync::Arc;

pub trait DocumentExtractor: Send + Sync {
    fn can_handle(&self, extension: &str) -> bool;
    fn extract_text(&self, path: &Path) -> Result<String, IngestionError>;
}

pub struct ExtractorRegistry {
    extractors: Vec<Arc<dyn DocumentExtractor>>,
}

impl Default for ExtractorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            extractors: Vec::new(),
        };
        registry.register(Arc::new(PlainTextExtractor::new()));
        registry.register(Arc::new(PdfExtractor::new()));
        registry.register(Arc::new(DocxExtractor::new()));
        registry
    }

    pub fn register(&mut self, extractor: Arc<dyn DocumentExtractor>) {
        self.extractors.push(extractor);
    }

    pub fn can_handle(&self, extension: &str) -> bool {
        self.extractors.iter().any(|e| e.can_handle(extension))
    }

    pub fn extract(&self, path: &Path) -> Result<String, IngestionError> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        for extractor in &self.extractors {
            if extractor.can_handle(extension) {
                return extractor.extract_text(path);
            }
        }

        Err(IngestionError::UnsupportedFormat(format!(
            "No extractor registered for file extension '{}' at {:?}",
            extension, path
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_extractor_registry() {
        let registry = ExtractorRegistry::new();
        assert!(registry.can_handle("txt"));
        assert!(registry.can_handle("md"));
        assert!(registry.can_handle("pdf"));
        assert!(registry.can_handle("docx"));
        assert!(!registry.can_handle("xyz123"));

        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        write!(temp_file, "Registry extraction test").unwrap();
        // Rename or use path with extension
        let temp_path = temp_file.path().with_extension("txt");
        std::fs::copy(temp_file.path(), &temp_path).unwrap();

        let text = registry.extract(&temp_path).unwrap();
        assert_eq!(text, "Registry extraction test");
        let _ = std::fs::remove_file(temp_path);
    }
}
