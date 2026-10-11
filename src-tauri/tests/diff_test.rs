use serchx_lib::diff::{DiffCache, DiffEngine};

#[test]
fn test_diff_engine_insert_and_delete() {
    let engine = DiffEngine::new();
    let old_text = "Line 1\nPricing: Net 45\nLine 3\n";
    let new_text = "Line 1\nPricing: Net 30\nLine 3\n";

    let diff = engine.compute_diff(old_text, new_text, "v1", "v2");

    assert_eq!(diff.summary.removed_lines, 1);
    assert_eq!(diff.summary.added_lines, 1);
    assert_eq!(diff.summary.unchanged_lines, 2);

    assert!(diff.removed_terms.contains("45"));
    assert!(diff.removed_terms.contains("net"));
    assert!(diff.added_terms.contains("30"));
    assert!(diff.added_terms.contains("net"));
    assert!(!diff.is_truncated);
}

#[test]
fn test_diff_engine_whitespace_normalization() {
    let engine = DiffEngine::new();
    let old_text = "Hello   world \r\nSecond line \t here\r\n";
    let new_text = "Hello world\nSecond line here\n";

    let diff = engine.compute_diff(old_text, new_text, "v1", "v2");

    assert_eq!(diff.summary.removed_lines, 0);
    assert_eq!(diff.summary.added_lines, 0);
    assert_eq!(diff.summary.unchanged_lines, 2);
}

#[test]
fn test_diff_engine_size_guard() {
    let engine = DiffEngine::with_max_bytes(100);
    let large_old = "A".repeat(250);
    let large_new = "B".repeat(250);

    let diff = engine.compute_diff(&large_old, &large_new, "v1", "v2");

    assert!(diff.is_truncated);
}

#[test]
fn test_diff_cache_lru_eviction() {
    let cache = DiffCache::new(2);
    let engine = DiffEngine::new();

    let diff_1_2 = engine.compute_diff("A", "B", "v1", "v2");
    let diff_2_3 = engine.compute_diff("B", "C", "v2", "v3");
    let diff_3_4 = engine.compute_diff("C", "D", "v3", "v4");

    cache.put(diff_1_2);
    cache.put(diff_2_3);
    assert_eq!(cache.len(), 2);

    // Access v1 -> v2 so it becomes most recently used
    assert!(cache.get("v1", "v2").is_some());

    // Insert v3 -> v4; v2 -> v3 should be evicted
    cache.put(diff_3_4);
    assert_eq!(cache.len(), 2);

    assert!(
        cache.get("v1", "v2").is_some(),
        "v1->v2 should be preserved"
    );
    assert!(cache.get("v3", "v4").is_some(), "v3->v4 should be present");
    assert!(
        cache.get("v2", "v3").is_none(),
        "v2->v3 should have been evicted"
    );
}

#[test]
fn test_resolve_versions_for_path() {
    use serchx_lib::commands::version::resolve_versions_for_path;
    use serchx_lib::db::DatabaseManager;
    use serchx_lib::models::ArtifactVersion;

    let db = DatabaseManager::new_in_memory().expect("in-memory database");
    let test_path = "/Users/test/Documents/report.pdf";

    // When no artifact exists, returns empty list
    let empty = resolve_versions_for_path(&db, test_path).expect("lookup on empty");
    assert!(empty.is_empty());

    // Create artifact with 2 versions
    let art = db
        .upsert_artifact(test_path, "file")
        .expect("upsert artifact");
    let v1 = ArtifactVersion {
        id: "ver-alpha-1".to_string(),
        artifact_id: art.id.clone(),
        path: test_path.to_string(),
        content_hash: "hash-1".to_string(),
        text_hash: None,
        size_bytes: 100,
        modified_at: 1000,
        previous_version_id: None,
        extracted_text: Some("Initial text".to_string()),
        is_pinned: false,
    };
    let v2 = ArtifactVersion {
        id: "ver-alpha-2".to_string(),
        artifact_id: art.id.clone(),
        path: test_path.to_string(),
        content_hash: "hash-2".to_string(),
        text_hash: None,
        size_bytes: 120,
        modified_at: 2000,
        previous_version_id: Some("ver-alpha-1".to_string()),
        extracted_text: Some("Updated text".to_string()),
        is_pinned: true,
    };

    db.record_version(&v1).expect("record v1");
    db.record_version(&v2).expect("record v2");

    // Lookup by path
    let res = resolve_versions_for_path(&db, test_path).expect("lookup by path");
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].version_id, "ver-alpha-1");
    assert_eq!(res[0].version_number, 1);
    assert!(!res[0].is_pinned);

    assert_eq!(res[1].version_id, "ver-alpha-2");
    assert_eq!(res[1].version_number, 2);
    assert!(res[1].is_pinned);

    // Lookup by artifact_id fallback
    let by_art = resolve_versions_for_path(&db, &art.id).expect("lookup by artifact id");
    assert_eq!(by_art.len(), 2);
}

#[test]
fn test_get_compare_path() {
    use serchx_lib::commands::ui::get_compare_path;
    let path = get_compare_path().expect("get compare path");
    // Initially None or previously stored path
    let _ = path;
}
