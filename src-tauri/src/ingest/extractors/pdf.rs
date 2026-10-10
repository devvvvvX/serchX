use super::DocumentExtractor;
use crate::ingest::IngestionError;
use std::path::Path;

pub struct PdfExtractor;

impl PdfExtractor {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
pub fn extract_pdf_native_macos(path: &Path) -> Option<String> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Once;

    static INIT_PDFKIT: Once = Once::new();
    INIT_PDFKIT.call_once(|| unsafe {
        let bundle_path = NSString::alloc(nil).init_str("/System/Library/Frameworks/PDFKit.framework");
        let bundle: id = msg_send![class!(NSBundle), bundleWithPath: bundle_path];
        if !bundle.is_null() {
            let _: bool = msg_send![bundle, load];
        }
    });

    let path_str = path.to_str()?;
    unsafe {
        let ns_path = NSString::alloc(nil).init_str(path_str);
        let ns_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
        if ns_url.is_null() {
            return None;
        }

        let pdf_doc_class = match objc::runtime::Class::get("PDFDocument") {
            Some(c) => c,
            None => return None,
        };

        let doc: id = msg_send![pdf_doc_class, alloc];
        let doc: id = msg_send![doc, initWithURL: ns_url];
        if doc.is_null() {
            return None;
        }

        let doc_string: id = msg_send![doc, string];
        let text = if !doc_string.is_null() {
            let utf8_ptr: *const std::os::raw::c_char = msg_send![doc_string, UTF8String];
            if !utf8_ptr.is_null() {
                std::ffi::CStr::from_ptr(utf8_ptr).to_string_lossy().to_string()
            } else {
                String::new()
            }
        } else {
            // PDF has no text layer (e.g. scanned images)
            String::new()
        };

        let _: () = msg_send![doc, release];
        Some(text)
    }
}

impl DocumentExtractor for PdfExtractor {
    fn can_handle(&self, extension: &str) -> bool {
        extension.eq_ignore_ascii_case("pdf")
    }

    fn extract_text(&self, path: &Path) -> Result<String, IngestionError> {
        match pdf_extract::extract_text(path) {
            Ok(text) => Ok(text),
            Err(e) => {
                #[cfg(target_os = "macos")]
                {
                    tracing::warn!(
                        "pdf_extract failed for {:?} ({}), attempting native Apple PDFKit fallback",
                        path,
                        e
                    );
                    if let Some(text) = extract_pdf_native_macos(path) {
                        tracing::info!(
                            "Apple PDFKit fallback extracted {} characters for {:?}",
                            text.len(),
                            path
                        );
                        return Ok(text);
                    }
                }
                Err(IngestionError::Extraction(format!(
                    "Failed to extract text from PDF: {}",
                    e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_extractor_can_handle() {
        let extractor = PdfExtractor::new();
        assert!(extractor.can_handle("pdf"));
        assert!(extractor.can_handle("PDF"));
        assert!(!extractor.can_handle("docx"));
        assert!(!extractor.can_handle("txt"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_native_pdfkit_loader() {
        let dummy_path = Path::new("/nonexistent/dummy.pdf");
        // Should return None safely without panic
        assert!(extract_pdf_native_macos(dummy_path).is_none());
    }
}
