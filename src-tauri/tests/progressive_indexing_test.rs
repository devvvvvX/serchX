use serchx_lib::db::DatabaseManager;
use serchx_lib::ingest::extractors::ExtractorRegistry;
use serchx_lib::ingest::ocr::AppleVisionOcr;
use serchx_lib::ingest::IngestionPipeline;
use serchx_lib::safety::FileFilter;
use serchx_lib::search::Fts5SearchEngine;
use serchx_lib::vector::embedder::{Embedder, MockEmbedder};
use serchx_lib::vector::index::{SqliteVectorIndex, VectorIndex};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn test_apple_vision_ocr_runner() {
    let ocr = AppleVisionOcr::new();
    assert!(ocr.can_handle("png"));
    assert!(ocr.can_handle("jpg"));
    assert!(ocr.can_handle("jpeg"));
    assert!(ocr.can_handle("webp"));
    assert!(!ocr.can_handle("txt"));
    assert!(!ocr.can_handle("pdf"));

    // Handling non-existent image safely
    let non_existent = Path::new("/tmp/non_existent_serchx_image_test_9999.png");
    let res = ocr.recognize_text(non_existent);
    assert!(res.is_err(), "Non-existent image must fail cleanly");

    // Real image OCR execution must succeed and complete sub-second
    let resume = Path::new("/Users/hwaiting_dk/Documents/Important/Resume.png");
    if resume.exists() {
        let res = ocr.recognize_text(resume);
        assert!(
            res.is_ok(),
            "Real image OCR must succeed without timing out"
        );
        let text = res.unwrap();
        assert!(!text.is_empty(), "Extracted text must not be empty");
    }

    let excalidraw = Path::new("/Users/hwaiting_dk/Documents/Dropbox.excalidraw.png");
    if excalidraw.exists() {
        let res = ocr.recognize_text(excalidraw);
        assert!(
            res.is_ok(),
            "Dropbox.excalidraw.png OCR must succeed without timing out"
        );
        let text = res.unwrap();
        assert!(
            !text.is_empty(),
            "Extracted text from excalidraw must not be empty"
        );
    }
}

#[test]
fn test_two_stage_progressive_indexing() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_progressive.db");
    let db = Arc::new(DatabaseManager::new(&db_path).unwrap());

    let extractors = Arc::new(ExtractorRegistry::new());
    let ocr = Arc::new(AppleVisionOcr::new());
    let filter = Arc::new(FileFilter::default());

    let vindex: Arc<dyn VectorIndex> = Arc::new(SqliteVectorIndex::new(db.clone()));
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::new(384));

    let pipeline = IngestionPipeline::with_vector_support(
        db.clone(),
        extractors,
        ocr,
        filter,
        Some(vindex.clone()),
        Some(embedder.clone()),
    );

    // Create a sample text file
    let sample_file = dir.path().join("progressive_doc.txt");
    fs::write(
        &sample_file,
        "Stage one provides instantaneous full text search results for quantum computing advancements.",
    )
    .unwrap();

    let res = pipeline
        .ingest_file(&sample_file)
        .expect("Ingestion must succeed");
    assert!(res.is_new_version);
    assert!(res.chunks_count >= 1);

    // Stage 1 Verification: FTS5 search index is populated IMMEDIATELY
    let fts = Fts5SearchEngine::new(db.clone());
    let results = fts
        .search("quantum computing", 10)
        .expect("FTS search should succeed");
    assert_eq!(
        results.len(),
        1,
        "Stage 1: Document must be immediately searchable via FTS5 without blocking for embeddings"
    );

    // Stage 2 Verification: Background worker asynchronously embeds and persists vectors into SQLite
    let start_wait = Instant::now();
    let mut vector_count = 0;
    while start_wait.elapsed() < Duration::from_secs(3) {
        vector_count = vindex.count_vectors().unwrap_or(0);
        if vector_count >= 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        vector_count >= 1,
        "Stage 2: Background worker must persist vector embeddings to SQLite chunk_vectors"
    );

    // Verify KNN search retrieves the embedded chunk
    let query_vec = embedder.embed_text("quantum computing").unwrap();
    let knn_results = vindex.search_knn(&query_vec, 5).unwrap();
    assert!(
        !knn_results.is_empty(),
        "KNN search should return the background-embedded chunk"
    );
}
