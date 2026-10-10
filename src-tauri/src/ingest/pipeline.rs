use crate::db::DatabaseManager;
use crate::ingest::extractors::ExtractorRegistry;
use crate::ingest::ocr::AppleVisionOcr;
use crate::ingest::IngestionError;
use crate::models::{ArtifactVersion, DocumentChunk, FileEvent};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub const MAX_CHUNKS_PER_FILE: usize = 300;
pub const MAX_VECTOR_CHUNKS_PER_DOC: usize = 64;

#[derive(Debug)]
pub struct IngestionResult {
    pub artifact_id: String,
    pub version_id: String,
    pub is_new_version: bool,
    pub chunks_count: usize,
}

pub struct IngestionPipeline {
    db: Arc<DatabaseManager>,
    extractors: Arc<ExtractorRegistry>,
    ocr: Arc<AppleVisionOcr>,
    filter: Arc<crate::safety::FileFilter>,
    vector_index: Option<Arc<dyn crate::vector::VectorIndex>>,
    embedder: Option<Arc<dyn crate::vector::Embedder>>,
    bg_embedding_tx: Option<std::sync::mpsc::SyncSender<Vec<DocumentChunk>>>,
}

impl IngestionPipeline {
    pub fn new(
        db: Arc<DatabaseManager>,
        extractors: Arc<ExtractorRegistry>,
        ocr: Arc<AppleVisionOcr>,
    ) -> Self {
        Self::with_filter(
            db,
            extractors,
            ocr,
            Arc::new(crate::safety::FileFilter::default()),
        )
    }

    pub fn with_filter(
        db: Arc<DatabaseManager>,
        extractors: Arc<ExtractorRegistry>,
        ocr: Arc<AppleVisionOcr>,
        filter: Arc<crate::safety::FileFilter>,
    ) -> Self {
        Self {
            db,
            extractors,
            ocr,
            filter,
            vector_index: None,
            embedder: None,
            bg_embedding_tx: None,
        }
    }

    pub fn with_vector_support(
        db: Arc<DatabaseManager>,
        extractors: Arc<ExtractorRegistry>,
        ocr: Arc<AppleVisionOcr>,
        filter: Arc<crate::safety::FileFilter>,
        vector_index: Option<Arc<dyn crate::vector::VectorIndex>>,
        embedder: Option<Arc<dyn crate::vector::Embedder>>,
    ) -> Self {
        let bg_embedding_tx = if let (Some(vindex), Some(emb)) = (&vector_index, &embedder) {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<DocumentChunk>>(512);
            let vindex_bg = vindex.clone();
            let emb_bg = emb.clone();
            let _ = std::thread::Builder::new()
                .name("serchx-bg-embedder".to_string())
                .spawn(move || {
                    while let Ok(chunks) = rx.recv() {
                        for chunk_batch in chunks.chunks(32) {
                            let texts: Vec<String> = chunk_batch.iter().map(|c| c.content.clone()).collect();
                            if let Ok(embeddings) = emb_bg.embed_batch(&texts) {
                                let batch_items: Vec<(&str, &[f32])> = chunk_batch
                                    .iter()
                                    .zip(embeddings.iter())
                                    .map(|(c, e)| (c.id.as_str(), e.as_slice()))
                                    .collect();
                                let _ = vindex_bg.insert_vectors_batch(&batch_items);
                            }
                        }
                    }
                });
            Some(tx)
        } else {
            None
        };

        Self {
            db,
            extractors,
            ocr,
            filter,
            vector_index,
            embedder,
            bg_embedding_tx,
        }
    }

    pub fn enqueue_for_background_embedding(&self, chunks: &[DocumentChunk]) {
        if let Some(ref bg_tx) = self.bg_embedding_tx {
            let vector_chunks: Vec<_> = chunks.iter().take(MAX_VECTOR_CHUNKS_PER_DOC).cloned().collect();
            let _ = bg_tx.try_send(vector_chunks);
        }
    }

    pub fn vector_index(&self) -> Option<&Arc<dyn crate::vector::VectorIndex>> {
        self.vector_index.as_ref()
    }

    pub fn embedder(&self) -> Option<&Arc<dyn crate::vector::Embedder>> {
        self.embedder.as_ref()
    }

    pub fn db(&self) -> &Arc<DatabaseManager> {
        &self.db
    }

    pub fn compute_sha256(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        if text.trim().is_empty() {
            return Vec::new();
        }

        let chars: Vec<char> = text.chars().collect();
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < chars.len() {
            let end = std::cmp::min(start + chunk_size, chars.len());
            let chunk_str: String = chars[start..end].iter().collect();
            chunks.push(chunk_str.trim().to_string());

            if end == chars.len() {
                break;
            }
            start += chunk_size.saturating_sub(overlap);
        }

        chunks
    }

    pub fn ingest_file(&self, path: &Path) -> Result<IngestionResult, IngestionError> {
        let path_str = path
            .to_str()
            .ok_or_else(|| IngestionError::Extraction("Invalid UTF-8 path".to_string()))?;

        let metadata = fs::metadata(path)?;

        // Pre-read safety gate validation (FILTER-01, FILTER-02, FILTER-03, FILTER-04)
        self.filter.should_index_path(path, &metadata)?;

        let size_bytes = metadata.len() as i64;
        let modified_at = metadata
            .modified()
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
            .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

        // Fast-path: Check if latest version already has identical (size_bytes, modified_at)
        let prev_version = self.db.get_latest_version_by_path(path_str)?;
        if let Some(ref prev) = prev_version {
            if prev.size_bytes == size_bytes && prev.modified_at == modified_at {
                tracing::trace!("Fast-path hit for unchanged file: {}", path_str);
                return Ok(IngestionResult {
                    artifact_id: prev.artifact_id.clone(),
                    version_id: prev.id.clone(),
                    is_new_version: false,
                    chunks_count: 0,
                });
            }
        }

        // Streaming SHA-256 computation without whole-file memory read (HASH-01)
        let content_hash = crate::safety::compute_file_sha256(path)?;

        // Check if latest version already has the exact same content hash
        if let Some(ref prev) = prev_version {
            if prev.content_hash == content_hash {
                // Content has not changed, skip re-indexing
                return Ok(IngestionResult {
                    artifact_id: prev.artifact_id.clone(),
                    version_id: prev.id.clone(),
                    is_new_version: false,
                    chunks_count: 0,
                });
            }
        }

        // Determine file kind and extract text
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (kind, extracted_text) = if self.ocr.can_handle(&ext) {
            let text = self.ocr.recognize_text(path)?;
            ("image", text)
        } else if self.extractors.can_handle(&ext) {
            let text = self.extractors.extract(path)?;
            ("file", text)
        } else {
            return Err(IngestionError::UnsupportedFormat(format!(
                "No extractor or OCR available for extension '{}'",
                ext
            )));
        };

        let text_hash = Self::compute_sha256(extracted_text.as_bytes());

        // Upsert Artifact record
        let artifact = self.db.upsert_artifact(path_str, kind)?;

        // Create new ArtifactVersion
        let version_id = uuid::Uuid::new_v4().to_string();
        let prev_version_id = prev_version.as_ref().map(|p| p.id.clone());

        let new_version = ArtifactVersion {
            id: version_id.clone(),
            artifact_id: artifact.id.clone(),
            path: path_str.to_string(),
            content_hash,
            text_hash: Some(text_hash),
            size_bytes,
            modified_at,
            previous_version_id: prev_version_id,
            extracted_text: None, // Omitted to save DB space (chunks already store it)
            is_pinned: false,
        };
        self.db.record_version(&new_version)?;

        // Chunk passages and persist with runaway caps
        let text_chunks = Self::chunk_text(&extracted_text, 500, 50);
        let is_pinned = prev_version.as_ref().map(|v| v.is_pinned).unwrap_or(false);
        let max_chunks = if is_pinned {
            text_chunks.len()
        } else {
            text_chunks.len().min(MAX_CHUNKS_PER_FILE)
        };

        let mut document_chunks = Vec::new();
        for (i, content) in text_chunks.into_iter().take(max_chunks).enumerate() {
            document_chunks.push(DocumentChunk {
                id: uuid::Uuid::new_v4().to_string(),
                version_id: version_id.clone(),
                chunk_index: i as i32,
                content,
            });
        }
        let chunks_count = document_chunks.len();
        self.db.insert_chunks(&document_chunks)?;

        // Stage 2: Enqueue for deferred background vector embedding without blocking Stage 1 text searchability (D-06)
        self.enqueue_for_background_embedding(&document_chunks);

        // Record ingestion event
        let event = FileEvent {
            id: uuid::Uuid::new_v4().to_string(),
            artifact_id: Some(artifact.id.clone()),
            event_type: "ingested".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: Some(serde_json::json!({
                "path": path_str,
                "version_id": version_id,
                "chunks_count": chunks_count,
            })),
        };
        self.db.record_event(&event)?;

        Ok(IngestionResult {
            artifact_id: artifact.id,
            version_id,
            is_new_version: true,
            chunks_count,
        })
    }

    pub fn extract_and_prepare_batch_item(
        &self,
        path: &Path,
    ) -> Result<Option<crate::db::IngestionBatchItem>, IngestionError> {
        let path_str = path
            .to_str()
            .ok_or_else(|| IngestionError::Extraction("Invalid UTF-8 path".to_string()))?;

        let metadata = fs::metadata(path)?;

        // Pre-read safety gate validation
        self.filter.should_index_path(path, &metadata)?;

        let size_bytes = metadata.len() as i64;
        let modified_at = metadata
            .modified()
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
            .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

        // Fast-path: Check if latest version already has identical (size_bytes, modified_at)
        let prev_version = self.db.get_latest_version_by_path(path_str)?;
        if let Some(ref prev) = prev_version {
            if prev.size_bytes == size_bytes && prev.modified_at == modified_at {
                tracing::info!("Fast-path hit for unchanged file: {}", path_str);
                return Ok(None);
            }
        }

        // Streaming SHA-256 computation
        tracing::info!("Computing SHA-256 for {}", path_str);
        let content_hash = crate::safety::compute_file_sha256(path)?;
        tracing::info!("Computed SHA-256 for {}: {}", path_str, content_hash);

        // Check if latest version already has the exact same content hash
        if let Some(ref prev) = prev_version {
            if prev.content_hash == content_hash {
                return Ok(None);
            }
        }

        // Determine file kind and extract text
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (kind, extracted_text) = if self.ocr.can_handle(&ext) {
            let text = self.ocr.recognize_text(path)?;
            ("image", text)
        } else if self.extractors.can_handle(&ext) {
            let text = self.extractors.extract(path)?;
            ("file", text)
        } else {
            return Err(IngestionError::UnsupportedFormat(format!(
                "No extractor or OCR available for extension '{}'",
                ext
            )));
        };

        let text_hash = Self::compute_sha256(extracted_text.as_bytes());

        let artifact_id = prev_version
            .as_ref()
            .map(|p| p.artifact_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let version_id = uuid::Uuid::new_v4().to_string();
        let prev_version_id = prev_version.as_ref().map(|p| p.id.clone());

        let new_version = ArtifactVersion {
            id: version_id.clone(),
            artifact_id: artifact_id.clone(),
            path: path_str.to_string(),
            content_hash,
            text_hash: Some(text_hash),
            size_bytes,
            modified_at,
            previous_version_id: prev_version_id,
            extracted_text: None, // Omitted to save DB space (chunks already store it)
            is_pinned: false,
        };

        let text_chunks = Self::chunk_text(&extracted_text, 500, 50);
        let is_pinned = prev_version.as_ref().map(|v| v.is_pinned).unwrap_or(false);
        let max_chunks = if is_pinned {
            text_chunks.len()
        } else {
            text_chunks.len().min(MAX_CHUNKS_PER_FILE)
        };

        let mut document_chunks = Vec::new();
        for (i, content) in text_chunks.into_iter().take(max_chunks).enumerate() {
            document_chunks.push(DocumentChunk {
                id: uuid::Uuid::new_v4().to_string(),
                version_id: version_id.clone(),
                chunk_index: i as i32,
                content,
            });
        }

        let event = FileEvent {
            id: uuid::Uuid::new_v4().to_string(),
            artifact_id: Some(artifact_id.clone()),
            event_type: "ingested".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: Some(serde_json::json!({
                "path": path_str,
                "version_id": version_id,
                "chunks_count": document_chunks.len(),
            })),
        };

        Ok(Some(crate::db::IngestionBatchItem {
            artifact_path: path_str.to_string(),
            artifact_kind: kind.to_string(),
            version: new_version,
            chunks: document_chunks,
            event: Some(event),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_ingestion_and_versioning() {
        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        let extractors = Arc::new(ExtractorRegistry::new());
        let ocr = Arc::new(AppleVisionOcr::new());
        let pipeline = IngestionPipeline::new(db.clone(), extractors, ocr);

        // Create temporary test file
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("document.txt");
        std::fs::write(&path, "Initial document content for testing serchX.").unwrap();

        // 1. First Ingestion
        let res1 = pipeline.ingest_file(&path).unwrap();
        assert!(res1.is_new_version);
        assert!(res1.chunks_count > 0);

        // 2. Re-ingesting identical content without changes -> should skip!
        let res2 = pipeline.ingest_file(&path).unwrap();
        assert!(!res2.is_new_version);
        assert_eq!(res2.version_id, res1.version_id);

        // 3. Modifying file content -> should create new version
        std::fs::write(&path, "Updated document content with additional details.").unwrap();
        let res3 = pipeline.ingest_file(&path).unwrap();
        assert!(res3.is_new_version);
        assert_ne!(res3.version_id, res1.version_id);
    }

    #[test]
    fn test_pipeline_rejection_for_filtered_files() {
        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        let extractors = Arc::new(ExtractorRegistry::new());
        let ocr = Arc::new(AppleVisionOcr::new());
        let pipeline = IngestionPipeline::new(db.clone(), extractors, ocr);

        let temp_dir = tempfile::tempdir().unwrap();

        // 1. Blacklisted extension
        let dmg_path = temp_dir.path().join("installer.dmg");
        std::fs::write(&dmg_path, b"dummy dmg").unwrap();
        let err = pipeline.ingest_file(&dmg_path).unwrap_err();
        assert!(matches!(err, IngestionError::Filtered(crate::safety::FilterRejectionReason::BlacklistedExtension(_))));

        // 2. Disallowed extension
        let unknown_path = temp_dir.path().join("test.foobar");
        std::fs::write(&unknown_path, b"unknown format").unwrap();
        let err = pipeline.ingest_file(&unknown_path).unwrap_err();
        assert!(matches!(err, IngestionError::Filtered(crate::safety::FilterRejectionReason::DisallowedExtension(_))));
    }
}
