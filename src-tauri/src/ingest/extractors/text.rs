use super::DocumentExtractor;
use crate::ingest::IngestionError;
use std::fs;
use std::path::Path;

pub struct PlainTextExtractor;

impl PlainTextExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlainTextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for PlainTextExtractor {
    fn can_handle(&self, extension: &str) -> bool {
        matches!(
            extension.to_lowercase().as_str(),
            "txt"
                | "md"
                | "markdown"
                | "json"
                | "csv"
                | "log"
                | "yaml"
                | "yml"
                | "toml"
                | "xml"
                | "rs"
                | "ts"
                | "js"
                | "py"
                | "go"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "html"
                | "css"
                | "sh"
                | "zsh"
                | "bash"
                | "sql"
                | "env"
                | "conf"
                | "ini"
        )
    }

    fn extract_text(&self, path: &Path) -> Result<String, IngestionError> {
        let bytes = fs::read(path)?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_plain_text_extractor() {
        let extractor = PlainTextExtractor::new();
        assert!(extractor.can_handle("md"));
        assert!(extractor.can_handle("RS"));
        assert!(!extractor.can_handle("pdf"));

        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        write!(temp_file, "Hello, serchX text extraction!").unwrap();

        let extracted = extractor.extract_text(temp_file.path()).unwrap();
        assert_eq!(extracted, "Hello, serchX text extraction!");
    }
}
