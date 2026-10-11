use chrono::{Duration, Utc};
use serchx_lib::db::DatabaseManager;
use serchx_lib::models::{AppSettings, ArtifactVersion, DocumentChunk};
use serchx_lib::search::fts5::Fts5SearchEngine;
use serchx_lib::search::hybrid::{HybridSearchEngine, MatchReason};
use serchx_lib::vector::embedder::{Embedder, MockEmbedder};
use serchx_lib::vector::index::{SqliteVectorIndex, VectorIndex};
use serchx_lib::vector::overflow::CorpusCapManager;
use serchx_lib::vector::scheduler::ModelScheduler;
use std::sync::Arc;
use std::time::Instant;

fn setup_benchmark_corpus(
    total_chunks: usize,
) -> (
    Arc<DatabaseManager>,
    Arc<HybridSearchEngine>,
    Arc<SqliteVectorIndex>,
    Arc<ModelScheduler>,
) {
    let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
    let scheduler = Arc::new(ModelScheduler::with_factory(60, || {
        Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn Embedder>)
    }));
    let hybrid_engine = Arc::new(HybridSearchEngine::new(
        db.clone(),
        fts,
        vector_index.clone(),
        scheduler.clone(),
    ));

    let now = Utc::now();
    let num_artifacts = total_chunks / 10;

    let topics = [
        (
            "invoice consulting services quarterly payment",
            "pdf",
            "/Users/alice/Documents",
        ),
        (
            "quarterly financial tax statement budget sheet",
            "xlsx",
            "/Users/alice/Documents",
        ),
        (
            "docker container crash exception error logs",
            "txt",
            "/Users/alice/Downloads",
        ),
        (
            "user interface design mockups spotlight search",
            "png",
            "/Users/alice/Desktop",
        ),
        (
            "system architecture security encryption policy",
            "docx",
            "/Users/alice/Documents",
        ),
    ];

    for a_idx in 0..num_artifacts {
        let (topic, ext, folder) = topics[a_idx % topics.len()];
        let path = format!(
            "{}/doc_{}_{}.{}",
            folder,
            a_idx,
            topic.split_whitespace().next().unwrap(),
            ext
        );
        let artifact = db.upsert_artifact(&path, "file").unwrap();

        let v_id = format!("v-{}", a_idx);
        let mod_time = (now - Duration::days((a_idx % 30) as i64)).timestamp_millis();
        let version = ArtifactVersion {
            id: v_id.clone(),
            artifact_id: artifact.id,
            path: path.clone(),
            content_hash: format!("hash-{}", a_idx),
            text_hash: None,
            size_bytes: 2048,
            modified_at: mod_time,
            previous_version_id: None,
            extracted_text: Some(format!(
                "{} full text extracted for document {}",
                topic, a_idx
            )),
            is_pinned: a_idx % 20 == 0, // Every 20th artifact is pinned
        };
        db.record_version(&version).unwrap();

        let mut chunks = Vec::with_capacity(10);
        for c_idx in 0..10 {
            let chunk_id = format!("c-{}-{}", a_idx, c_idx);
            let content = format!(
                "Document {} chunk {}: {} with specific details and numbers #{}",
                a_idx,
                c_idx,
                topic,
                a_idx * 10 + c_idx
            );
            chunks.push(DocumentChunk {
                id: chunk_id,
                version_id: v_id.clone(),
                chunk_index: c_idx as i32,
                content,
            });
        }
        db.insert_chunks(&chunks).unwrap();

        // Vectorize chunks using MockEmbedder
        let embedder = MockEmbedder::new(384);
        for chunk in &chunks {
            let vec = embedder.embed_text(&chunk.content).unwrap();
            vector_index.insert_vector(&chunk.id, &vec).unwrap();
        }
    }

    (db, hybrid_engine, vector_index, scheduler)
}

#[tokio::test]
async fn test_hybrid_search_2000_chunks_latency_p95() {
    let (_, engine, _, _) = setup_benchmark_corpus(2_000);
    let settings = AppSettings::default();

    let test_queries = [
        "invoice consulting services",
        "tax budget spreadsheet in Documents",
        "docker crash logs from yesterday",
        "spotlight design screenshot in Desktop",
        "security encryption policy",
        "quarterly payment invoice",
        "financial sheet 2024",
        "container exception error in Downloads",
        "system architecture policy in Documents",
        "consulting payment details",
    ];

    let mut latencies_ms: Vec<f64> = Vec::with_capacity(50);

    // Warm-up run
    let _ = engine.search(test_queries[0], 20, &settings).await;

    // Run 50 timed queries
    for i in 0..50 {
        let q = test_queries[i % test_queries.len()];
        let start = Instant::now();
        let results = engine.search(q, 20, &settings).await.expect("search");
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        latencies_ms.push(elapsed);
        assert!(!results.is_empty(), "Query '{}' should yield matches", q);
    }

    latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencies_ms[latencies_ms.len() / 2];
    let p95_idx = ((latencies_ms.len() as f64) * 0.95) as usize;
    let p95 = latencies_ms[p95_idx.min(latencies_ms.len() - 1)];

    println!(
        "\n================ HYBRID BENCHMARK (2,000 chunks) ================\n\
         p50 Latency: {:.2} ms\n\
         p95 Latency: {:.2} ms\n\
         Max Latency: {:.2} ms\n\
         Min Latency: {:.2} ms\n\
         ==================================================================",
        p50,
        p95,
        latencies_ms.last().unwrap(),
        latencies_ms.first().unwrap()
    );

    // RET-03: p95 must be under 200 ms within caps
    assert!(
        p95 < 200.0,
        "RET-03 violated: p95 latency must be under 200 ms, got {:.2} ms",
        p95
    );
}

#[tokio::test]
async fn test_corpus_overflow_and_fts_graceful_degradation() {
    let cap_manager = CorpusCapManager::new();
    let max_vectors = 500;

    // Verify cap logic
    assert!(cap_manager.can_embed_chunk(false, 499, max_vectors));
    assert!(!cap_manager.can_embed_chunk(false, 500, max_vectors));
    assert!(cap_manager.can_embed_chunk(true, 500, max_vectors)); // Pinned always allowed

    let warn_at_limit = cap_manager.should_warn_overflow(500, max_vectors);
    assert!(warn_at_limit.is_some());
    assert!(warn_at_limit.unwrap().contains("Vector corpus cap reached"));

    // Verify search graceful degradation: disable semantic search (simulating overflow mode)
    let (_, engine, _, _) = setup_benchmark_corpus(500);
    let degraded_settings = AppSettings {
        semantic_search_enabled: false,
        ..Default::default()
    };

    let results = engine
        .search("invoice consulting", 10, &degraded_settings)
        .await
        .expect("search degraded");

    assert!(!results.is_empty());
    // All results should have ExactKeyword match reasons, and no SemanticSimilarity
    for r in &results {
        assert!(r
            .reasons
            .iter()
            .any(|m| matches!(m, MatchReason::ExactKeyword { .. })));
        assert!(!r
            .reasons
            .iter()
            .any(|m| matches!(m, MatchReason::SemanticSimilarity { .. })));
    }
}

#[tokio::test]
async fn test_dynamic_settings_weight_adjustments() {
    let (db, _, vector_index, scheduler) = setup_benchmark_corpus(100);
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let engine = HybridSearchEngine::new(db.clone(), fts, vector_index, scheduler);

    // 1. Search with heavy FTS weight
    let fts_heavy = AppSettings {
        fts_weight: 5.0,
        vector_weight: 0.1,
        ..Default::default()
    };

    let fts_heavy_results = engine
        .search("budget spreadsheet in Documents", 10, &fts_heavy)
        .await
        .unwrap();

    // 2. Search with heavy Vector weight
    let vec_heavy = AppSettings {
        fts_weight: 0.1,
        vector_weight: 5.0,
        ..Default::default()
    };

    let vec_heavy_results = engine
        .search("budget spreadsheet in Documents", 10, &vec_heavy)
        .await
        .unwrap();

    assert!(!fts_heavy_results.is_empty());
    assert!(!vec_heavy_results.is_empty());

    // Both should yield results, but scores reflect weights
    println!(
        "FTS-heavy top score: {:.4}, Vector-heavy top score: {:.4}",
        fts_heavy_results[0].score, vec_heavy_results[0].score
    );
    assert!(fts_heavy_results[0].score > 0.0);
    assert!(vec_heavy_results[0].score > 0.0);
}
