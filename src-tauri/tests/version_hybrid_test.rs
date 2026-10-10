use chrono::{Duration, Utc};
use serchx_lib::db::DatabaseManager;
use serchx_lib::models::{AppSettings, ArtifactVersion, DocumentChunk};
use serchx_lib::search::fts5::Fts5SearchEngine;
use serchx_lib::search::hybrid::HybridSearchEngine;
use serchx_lib::vector::embedder::{Embedder, MockEmbedder};
use serchx_lib::vector::index::SqliteVectorIndex;
use serchx_lib::vector::scheduler::ModelScheduler;
use std::sync::Arc;

#[tokio::test]
async fn test_directional_version_retrieval_and_injection() {
    let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
    let scheduler = Arc::new(ModelScheduler::with_factory(60, || {
        Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn Embedder>)
    }));
    let engine = HybridSearchEngine::new(db.clone(), fts, vector_index, scheduler);

    let now = Utc::now();
    let artifact_path = "/Users/alice/Documents/Acme_MSA.pdf";
    let artifact = db.upsert_artifact(artifact_path, "file").unwrap();

    let text_v1 = "Acme contract Master Services Agreement preliminary draft terms.\n";
    let text_v2 = "Acme contract Master Services Agreement.\nPricing clause: net 45 payment terms.\nApproved by legal.\n";
    let text_v3 = "Acme contract Master Services Agreement.\nPricing clause: net 30 payment terms.\nSigned final.\n";

    let v1 = ArtifactVersion {
        id: "v-01".to_string(),
        artifact_id: artifact.id.clone(),
        path: artifact_path.to_string(),
        content_hash: "hash-01".to_string(),
        text_hash: None,
        size_bytes: 1024,
        modified_at: (now - Duration::days(15)).timestamp_millis(),
        previous_version_id: None,
        extracted_text: Some(text_v1.to_string()),
        is_pinned: false,
    };
    let v2 = ArtifactVersion {
        id: "v-02".to_string(),
        artifact_id: artifact.id.clone(),
        path: artifact_path.to_string(),
        content_hash: "hash-02".to_string(),
        text_hash: None,
        size_bytes: 1024,
        modified_at: (now - Duration::days(10)).timestamp_millis(),
        previous_version_id: Some("v-01".to_string()),
        extracted_text: Some(text_v2.to_string()),
        is_pinned: false,
    };
    let v3 = ArtifactVersion {
        id: "v-03".to_string(),
        artifact_id: artifact.id.clone(),
        path: artifact_path.to_string(),
        content_hash: "hash-03".to_string(),
        text_hash: None,
        size_bytes: 1024,
        modified_at: (now - Duration::days(5)).timestamp_millis(),
        previous_version_id: Some("v-02".to_string()),
        extracted_text: Some(text_v3.to_string()),
        is_pinned: false,
    };

    db.record_version(&v1).unwrap();
    db.record_version(&v2).unwrap();
    db.record_version(&v3).unwrap();

    let chunks = vec![
        DocumentChunk {
            id: "c-01".to_string(),
            version_id: "v-01".to_string(),
            chunk_index: 0,
            content: text_v1.to_string(),
        },
        DocumentChunk {
            id: "c-02".to_string(),
            version_id: "v-02".to_string(),
            chunk_index: 0,
            content: text_v2.to_string(),
        },
        DocumentChunk {
            id: "c-03".to_string(),
            version_id: "v-03".to_string(),
            chunk_index: 0,
            content: text_v3.to_string(),
        },
    ];
    db.insert_chunks(&chunks).unwrap();



    let settings = AppSettings::default();

    // Test 1: Query for "before pricing changed" -> Should elevate V2 above V3
    let results_before = engine
        .search("Acme contract before pricing changed", 5, &settings)
        .await
        .unwrap();

    assert!(!results_before.is_empty(), "Should return matching results");
    let top_result = &results_before[0];
    assert_eq!(top_result.version_id, "v-02", "V2 must rank first for 'before pricing changed'");

    // Check evidence
    let has_version_rel = top_result.reasons.iter().any(|r| match r {
        serchx_lib::search::hybrid::MatchReason::VersionRelationship { relation_summary, .. } => {
            relation_summary.contains("pricing")
        }
        _ => false,
    });
    assert!(has_version_rel, "Top result must contain VersionRelationship match reason");

    // Test 2: Query for "after pricing changed" -> Should elevate V3 above V2
    let results_after = engine
        .search("Acme contract after pricing changed", 5, &settings)
        .await
        .unwrap();

    assert!(!results_after.is_empty());
    assert_eq!(results_after[0].version_id, "v-03", "V3 must rank first for 'after pricing changed'");

    // Test 3: Normal query without version cue preserves standard execution
    let results_normal = engine
        .search("Acme Master Services Agreement", 5, &settings)
        .await
        .unwrap();
    assert!(!results_normal.is_empty());
}
