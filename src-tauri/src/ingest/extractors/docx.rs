use super::DocumentExtractor;
use crate::ingest::IngestionError;
use std::fs;
use std::path::Path;

pub struct DocxExtractor;

impl Default for DocxExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocxExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentExtractor for DocxExtractor {
    fn can_handle(&self, extension: &str) -> bool {
        extension.eq_ignore_ascii_case("docx")
    }

    fn extract_text(&self, path: &Path) -> Result<String, IngestionError> {
        let bytes = fs::read(path)?;
        let docx = docx_rs::read_docx(&bytes).map_err(|e| {
            IngestionError::Extraction(format!("Failed to parse docx document: {:?}", e))
        })?;

        let mut extracted = String::new();
        for child in docx.document.children {
            match child {
                docx_rs::DocumentChild::Paragraph(p) => {
                    for p_child in p.children {
                        if let docx_rs::ParagraphChild::Run(run) = p_child {
                            for r_child in run.children {
                                if let docx_rs::RunChild::Text(t) = r_child {
                                    extracted.push_str(&t.text);
                                }
                            }
                        }
                    }
                    extracted.push('\n');
                }
                docx_rs::DocumentChild::Table(table) => {
                    for row in table.rows {
                        let docx_rs::TableChild::TableRow(tr) = row;
                        for cell in tr.cells {
                            let docx_rs::TableRowChild::TableCell(tc) = cell;
                            for tc_child in tc.children {
                                if let docx_rs::TableCellContent::Paragraph(p) = tc_child {
                                    for p_child in p.children {
                                        if let docx_rs::ParagraphChild::Run(run) = p_child {
                                            for r_child in run.children {
                                                if let docx_rs::RunChild::Text(t) = r_child {
                                                    extracted.push_str(&t.text);
                                                    extracted.push(' ');
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        extracted.push('\n');
                    }
                }
                _ => {}
            }
        }

        Ok(extracted.trim().to_string())
    }
}
