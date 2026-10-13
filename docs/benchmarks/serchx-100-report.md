# serchX-100 Evaluation Benchmark Report

**Evaluated At:** 2026-10-13T18:25:11.498243+00:00

## Executive Summary

The `serchX-100` benchmark suite executes 100 realistic natural-language queries across 6 categories evaluating exact keyword precision, temporal recency, semantic hybrid retrieval, OCR screenshot text, vague conversational version recall, and person/source cues.

### Hardware & Resource Telemetry

| Metric | Value | Release Budget / Target |
|---|---|---|
| **p50 Latency** | 2.08 ms | < 50 ms |
| **p95 Latency** | 2.42 ms | < 200 ms |
| **Peak Resident RAM (RSS)** | 19.58 MB | < 500 MB (Apple Silicon) |
| **SQLite Storage Footprint** | 0.44 MB | Storage proportional to corpus |

### Category Retrieval Accuracy & Release Gates

| Category | Queries | Recall@1 | Recall@3 | Recall@5 | MRR | Gate Threshold | Status |
|---|---|---|---|---|---|---|---|
| **ExactMetadataPath** | 20 | 65.0% | 100.0% | 100.0% | 0.808 | Recall@3 ≥ 95% | ✅ PASSED |
| **TemporalRecency** | 20 | 95.0% | 100.0% | 100.0% | 0.975 | Recall@3 ≥ 85%, MRR ≥ 0.80 | ✅ PASSED |
| **VersionRecall** | 25 | 80.0% | 96.0% | 100.0% | 0.890 | Recall@3 ≥ 85%, MRR ≥ 0.80 | ✅ PASSED |
| **SemanticHybrid** | 20 | 45.0% | 95.0% | 95.0% | 0.700 | Recall@3 ≥ 80% | ✅ PASSED |
| **OcrScreenshot** | 10 | 40.0% | 100.0% | 100.0% | 0.700 | Recall@3 ≥ 80% | ✅ PASSED |
| **PersonSource** | 5 | 60.0% | 80.0% | 80.0% | 0.700 | Advisory evaluation | ℹ️ ADVISORY |

### Release Gate Assertions

1. **Exact / Metadata**: Recall@3 ≥ 95% (FTS5 BM25 indexed precision).
2. **Version Recall**: Recall@3 ≥ 85%, MRR ≥ 0.80 (Myers diff change predicate candidate injection).
3. **Temporal Recency**: Recall@3 ≥ 85%, MRR ≥ 0.80 (Date range filtering & recency decay).
4. **Semantic Hybrid**: Recall@3 ≥ 80% (Reciprocal Rank Fusion k=60 combining FTS5 + Vector kNN).
5. **OCR Screenshot**: Recall@3 ≥ 80% (Apple Vision OCR on-device text extraction).
6. **Person / Source**: Advisory evaluation for conversational author cues.
7. **Overall Latency**: p95 search latency strictly < 200 ms.
