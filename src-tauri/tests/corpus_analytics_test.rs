use serchx_lib::db::DatabaseManager;
use serchx_lib::models::{ArtifactVersion, DocumentChunk};
use std::sync::Arc;

#[test]
fn test_corpus_analytics_categorization_and_metrics() {
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());

    // Insert artifacts across different categories
    // 1. Documents: 1 PDF (5000 bytes, 2 chunks) + 1 DOCX (3000 bytes, 1 chunk)
    // 2. Code: 1 Rust (1500 bytes, 1 chunk) + 1 Python (1000 bytes, 1 chunk)
    // 3. Images & OCR: 1 PNG (8000 bytes, 0 chunks)
    // 4. Data & Sheets: 1 CSV (2000 bytes, 1 chunk)

    let test_files = vec![
        ("/docs/report.pdf", "pdf", 5000, 2),
        ("/docs/specification.docx", "docx", 3000, 1),
        ("/code/main.rs", "rs", 1500, 1),
        ("/code/script.py", "py", 1000, 1),
        ("/images/diagram.png", "png", 8000, 0),
        ("/data/metrics.csv", "csv", 2000, 1),
    ];

    let now = chrono::Utc::now().timestamp();

    for (idx, (path, ext, size, chunk_count)) in test_files.iter().enumerate() {
        let artifact = db.upsert_artifact(path, ext).unwrap();
        let ver_id = format!("ver-{}", idx);

        db.record_version(&ArtifactVersion {
            id: ver_id.clone(),
            artifact_id: artifact.id,
            path: path.to_string(),
            content_hash: format!("hash-{}", idx),
            text_hash: Some(format!("thash-{}", idx)),
            size_bytes: *size,
            modified_at: now,
            previous_version_id: None,
            extracted_text: Some("Sample extracted text".to_string()),
            is_pinned: false,
        })
        .unwrap();

        let mut chunks = Vec::new();
        for c_idx in 0..*chunk_count {
            chunks.push(DocumentChunk {
                id: format!("chunk-{}-{}", idx, c_idx),
                version_id: ver_id.clone(),
                chunk_index: c_idx,
                content: format!("Chunk content for {} part {}", path, c_idx),
            });
        }
        if !chunks.is_empty() {
            db.insert_chunks(&chunks).unwrap();
        }
    }

    // Run analytics query
    let analytics = db.get_corpus_analytics().unwrap();

    // Verify overall totals
    assert_eq!(analytics.total_files, 6);
    assert_eq!(analytics.total_bytes, 5000 + 3000 + 1500 + 1000 + 8000 + 2000);
    assert_eq!(analytics.total_chunks, 2 + 1 + 1 + 1 + 0 + 1);

    // Verify categories
    let doc_cat = analytics.categories.iter().find(|c| c.category == "Documents").expect("Documents category");
    assert_eq!(doc_cat.file_count, 2);
    assert_eq!(doc_cat.total_bytes, 8000);
    assert_eq!(doc_cat.extensions.len(), 2);
    let pdf_stat = doc_cat.extensions.iter().find(|e| e.extension == "pdf").expect("pdf stat");
    assert_eq!(pdf_stat.file_count, 1);
    assert_eq!(pdf_stat.chunk_count, 2);
    assert_eq!(pdf_stat.total_bytes, 5000);

    let code_cat = analytics.categories.iter().find(|c| c.category == "Code").expect("Code category");
    assert_eq!(code_cat.file_count, 2);
    assert_eq!(code_cat.total_bytes, 2500);

    let img_cat = analytics.categories.iter().find(|c| c.category == "Images & OCR").expect("Images category");
    assert_eq!(img_cat.file_count, 1);
    assert_eq!(img_cat.total_bytes, 8000);

    let data_cat = analytics.categories.iter().find(|c| c.category == "Data & Sheets").expect("Data category");
    assert_eq!(data_cat.file_count, 1);
    assert_eq!(data_cat.total_bytes, 2000);

    // Storage breakdown
    let breakdown = db.get_storage_breakdown(None).unwrap();
    assert_eq!(breakdown.original_files_bytes, 20500);
    assert!(breakdown.sqlite_db_bytes > 0);
    assert!(breakdown.text_chunks_bytes > 0);
}

#[test]
fn test_system_telemetry_query() {
    let db = DatabaseManager::new_in_memory().unwrap();
    let telemetry = serchx_lib::commands::ui::collect_system_telemetry(&db);

    assert!(telemetry.memory_rss_mb > 0.0, "RSS memory must be positive");
    assert!(telemetry.db_size_mb >= 0.0, "Database size must be non-negative");
    assert!(telemetry.cpu_percent >= 0.0 && telemetry.cpu_percent <= 100.0, "CPU percent must be bounded");
}
