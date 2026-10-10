use chrono::{Duration, Utc};
use serchx_lib::db::{pin_version, prune_unpinned_versions, DatabaseManager, RetentionPolicy};
use serchx_lib::diff::{DiffCache, DiffEngine};
use serchx_lib::models::{AppSettings, ArtifactVersion, DocumentChunk};
use serchx_lib::search::fts5::Fts5SearchEngine;
use serchx_lib::search::hybrid::{HybridSearchEngine, MatchReason};
use serchx_lib::vector::embedder::{Embedder, MockEmbedder};
use serchx_lib::vector::index::SqliteVectorIndex;
use serchx_lib::vector::scheduler::ModelScheduler;
use std::sync::Arc;

#[tokio::test]
async fn test_match_reason_version_relationship() {
    let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
    let scheduler = Arc::new(ModelScheduler::with_factory(60, || {
        Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn Embedder>)
    }));
    let diff_engine = Arc::new(DiffEngine::new());
    let diff_cache = Arc::new(DiffCache::default());
    let engine = HybridSearchEngine::with_diff_support(
        db.clone(),
        fts,
        vector_index,
        scheduler,
        diff_engine,
        diff_cache,
    );

    let now = Utc::now();
    let artifact_path = "/Users/alice/Documents/pricing_contract.md";
    let artifact = db.upsert_artifact(artifact_path, "file").unwrap();

    let text_v1 = "Standard partnership contract terms.\nPricing clause: monthly rate $500.\nApproved by sales.\n";
    let text_v2 = "Standard partnership contract terms.\nPricing clause: monthly rate $800.\nUpdated pricing tier.\n";

    let v1 = ArtifactVersion {
        id: "ver-01".to_string(),
        artifact_id: artifact.id.clone(),
        path: artifact_path.to_string(),
        content_hash: "h-01".to_string(),
        text_hash: None,
        size_bytes: 512,
        modified_at: (now - Duration::days(10)).timestamp_millis(),
        previous_version_id: None,
        extracted_text: Some(text_v1.to_string()),
        is_pinned: false,
    };
    let v2 = ArtifactVersion {
        id: "ver-02".to_string(),
        artifact_id: artifact.id.clone(),
        path: artifact_path.to_string(),
        content_hash: "h-02".to_string(),
        text_hash: None,
        size_bytes: 512,
        modified_at: (now - Duration::days(2)).timestamp_millis(),
        previous_version_id: Some("ver-01".to_string()),
        extracted_text: Some(text_v2.to_string()),
        is_pinned: false,
    };

    db.record_version(&v1).unwrap();
    db.record_version(&v2).unwrap();

    let chunks = vec![
        DocumentChunk {
            id: "chk-01".to_string(),
            version_id: "ver-01".to_string(),
            chunk_index: 0,
            content: text_v1.to_string(),
        },
        DocumentChunk {
            id: "chk-02".to_string(),
            version_id: "ver-02".to_string(),
            chunk_index: 0,
            content: text_v2.to_string(),
        },
    ];
    db.insert_chunks(&chunks).unwrap();

    let settings = AppSettings::default();

    // Query for "before pricing changed"
    let results = engine
        .search("partnership contract before pricing changed", 5, &settings)
        .await
        .expect("search");

    assert!(!results.is_empty(), "Should return matching results");
    let top = &results[0];
    assert_eq!(top.version_id, "ver-01", "Should elevate version 1");

    // Check MatchReason::VersionRelationship
    let rel_reason = top.reasons.iter().find_map(|r| match r {
        MatchReason::VersionRelationship {
            version_number,
            total_versions,
            relation_summary,
            matched_diff_snippet,
            later_version_date,
            ..
        } => Some((*version_number, *total_versions, relation_summary.clone(), matched_diff_snippet.clone(), *later_version_date)),
        _ => None,
    });

    assert!(rel_reason.is_some(), "Result must have VersionRelationship reason");
    let (v_num, total_v, summary, snippet, later_date) = rel_reason.unwrap();
    assert_eq!(v_num, 1);
    assert_eq!(total_v, 2);
    assert!(summary.contains("pricing"), "Summary should mention pricing: {}", summary);
    assert!(snippet.is_some(), "Snippet should be present");
    let snip_str = snippet.unwrap();
    assert!(snip_str.contains("- "), "Snippet should format deleted line with - prefix: {}", snip_str);
    assert!(later_date.is_some(), "Later version date must be present");
}

#[tokio::test]
async fn test_grouped_search_results() {
    let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
    let scheduler = Arc::new(ModelScheduler::with_factory(60, || {
        Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn Embedder>)
    }));
    let diff_engine = Arc::new(DiffEngine::new());
    let diff_cache = Arc::new(DiffCache::default());
    let engine = HybridSearchEngine::with_diff_support(
        db.clone(),
        fts,
        vector_index,
        scheduler,
        diff_engine,
        diff_cache,
    );

    let now = Utc::now();
    let path = "/Users/alice/Documents/spec.md";
    let artifact = db.upsert_artifact(path, "file").unwrap();

    let v1 = ArtifactVersion {
        id: "spec-v1".to_string(),
        artifact_id: artifact.id.clone(),
        path: path.to_string(),
        content_hash: "hash-1".to_string(),
        text_hash: None,
        size_bytes: 100,
        modified_at: (now - Duration::days(20)).timestamp_millis(),
        previous_version_id: None,
        extracted_text: Some("System architecture initial blueprint specifications.".to_string()),
        is_pinned: false,
    };
    let v2 = ArtifactVersion {
        id: "spec-v2".to_string(),
        artifact_id: artifact.id.clone(),
        path: path.to_string(),
        content_hash: "hash-2".to_string(),
        text_hash: None,
        size_bytes: 120,
        modified_at: (now - Duration::days(10)).timestamp_millis(),
        previous_version_id: Some("spec-v1".to_string()),
        extracted_text: Some("System architecture updated blueprint specifications with auth.".to_string()),
        is_pinned: true,
    };
    let v3 = ArtifactVersion {
        id: "spec-v3".to_string(),
        artifact_id: artifact.id.clone(),
        path: path.to_string(),
        content_hash: "hash-3".to_string(),
        text_hash: None,
        size_bytes: 150,
        modified_at: (now - Duration::days(1)).timestamp_millis(),
        previous_version_id: Some("spec-v2".to_string()),
        extracted_text: Some("System architecture final blueprint specifications with cloud auth.".to_string()),
        is_pinned: false,
    };

    db.record_version(&v1).unwrap();
    db.record_version(&v2).unwrap();
    db.record_version(&v3).unwrap();

    let chunks = vec![
        DocumentChunk {
            id: "chk-s1".to_string(),
            version_id: "spec-v1".to_string(),
            chunk_index: 0,
            content: "System architecture initial blueprint specifications.".to_string(),
        },
        DocumentChunk {
            id: "chk-s2".to_string(),
            version_id: "spec-v2".to_string(),
            chunk_index: 0,
            content: "System architecture updated blueprint specifications with auth.".to_string(),
        },
        DocumentChunk {
            id: "chk-s3".to_string(),
            version_id: "spec-v3".to_string(),
            chunk_index: 0,
            content: "System architecture final blueprint specifications with cloud auth.".to_string(),
        },
    ];
    db.insert_chunks(&chunks).unwrap();

    let settings = AppSettings::default();
    let grouped = engine
        .search_grouped("System architecture specifications", 10, &settings)
        .await
        .expect("search grouped");

    assert_eq!(grouped.len(), 1, "Results should collapse into single grouped artifact card");
    let group = &grouped[0];
    assert_eq!(group.total_version_count, 3, "Total versions should be 3");
    assert_eq!(group.other_versions.len(), 2, "Other versions should contain remaining 2 versions");
    assert!(group.other_versions.iter().all(|ov| ov.version_id != group.primary_result.version_id));

    // Verify pinned status is reflected in other_versions
    let v2_summary = group.other_versions.iter().find(|ov| ov.version_id == "spec-v2");
    if let Some(s) = v2_summary {
        assert!(s.is_pinned, "spec-v2 is_pinned flag must be true");
    }
}

#[tokio::test]
async fn test_toggle_version_pin() {
    let db = DatabaseManager::new_in_memory().expect("in-memory db");
    let path = "/Users/alice/Documents/budget.xlsx";
    let art = db.upsert_artifact(path, "file").unwrap();

    let now_ms = Utc::now().timestamp_millis();
    let v_id = "v-pin-test";
    let ver = ArtifactVersion {
        id: v_id.to_string(),
        artifact_id: art.id.clone(),
        path: path.to_string(),
        content_hash: "hash-pin".to_string(),
        text_hash: None,
        size_bytes: 200,
        modified_at: now_ms,
        previous_version_id: None,
        extracted_text: Some("Financial budget 2026".to_string()),
        is_pinned: false,
    };
    db.record_version(&ver).unwrap();

    // Verify initial pin state is false
    let fetched = db.get_version_by_id(v_id).unwrap().unwrap();
    assert!(!fetched.is_pinned);

    // Toggle to true
    pin_version(&db, v_id, true).unwrap();
    let fetched_pinned = db.get_version_by_id(v_id).unwrap().unwrap();
    assert!(fetched_pinned.is_pinned);

    // Verify retention pruning respects pinned status
    // Create 5 newer versions to exceed retention cap of 3
    for i in 1..=5 {
        let n_id = format!("v-newer-{}", i);
        let n_ver = ArtifactVersion {
            id: n_id.clone(),
            artifact_id: art.id.clone(),
            path: path.to_string(),
            content_hash: format!("hash-{}", i),
            text_hash: None,
            size_bytes: 200,
            modified_at: now_ms + (i as i64 * 1000),
            previous_version_id: None,
            extracted_text: Some(format!("Financial budget version {}", i)),
            is_pinned: false,
        };
        db.record_version(&n_ver).unwrap();
    }

    let policy = RetentionPolicy {
        max_versions_per_artifact: 3,
        retention_days: Some(90),
    };
    let pruned = prune_unpinned_versions(&db, &policy).unwrap();
    assert!(pruned > 0);

    // The pinned v_id MUST still exist despite being the oldest
    let still_exists = db.get_version_by_id(v_id).unwrap();
    assert!(still_exists.is_some(), "Pinned version must not be pruned");

    // Toggle pin back to false
    pin_version(&db, v_id, false).unwrap();
    let fetched_unpinned = db.get_version_by_id(v_id).unwrap().unwrap();
    assert!(!fetched_unpinned.is_pinned);
}
