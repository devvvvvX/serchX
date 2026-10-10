pub mod reporter;
pub mod telemetry;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEvaluation {
    pub id: String,
    pub query: String,
    pub category: String,
    pub expected_artifact: String,
    pub expected_version_index: Option<usize>,
    pub rank_found: Option<usize>,
    pub latency_ms: f32,
    pub hit_top1: bool,
    pub hit_top3: bool,
    pub hit_top5: bool,
    pub reciprocal_rank: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMetrics {
    pub category: String,
    pub total_queries: usize,
    pub recall_at_1: f32,
    pub recall_at_3: f32,
    pub recall_at_5: f32,
    pub mrr: f32,
    pub latency_p50_ms: f32,
    pub latency_p95_ms: f32,
    pub gate_status: String, // "PASSED", "FAILED", or "ADVISORY"
    pub gate_threshold_desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub timestamp: String,
    pub overall_recall_at_3: f32,
    pub overall_mrr: f32,
    pub latency_p50_ms: f32,
    pub latency_p95_ms: f32,
    pub peak_rss_mb: f64,
    pub db_size_mb: f64,
    pub categories: Vec<CategoryMetrics>,
    pub evaluations: Vec<QueryEvaluation>,
}
