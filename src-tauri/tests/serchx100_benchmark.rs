use chrono::{Datelike, Duration, TimeZone, Utc, Weekday};
use serchx_lib::benchmark::reporter::generate_markdown_report;
use serchx_lib::benchmark::telemetry::{get_peak_rss_bytes, get_sqlite_disk_bytes};
use serchx_lib::benchmark::{BenchmarkResult, CategoryMetrics, QueryEvaluation};
use serchx_lib::db::DatabaseManager;
use serchx_lib::diff::{DiffCache, DiffEngine};
use serchx_lib::models::{AppSettings, ArtifactVersion, DocumentChunk};
use serchx_lib::search::fts5::Fts5SearchEngine;
use serchx_lib::search::hybrid::HybridSearchEngine;
use serchx_lib::vector::embedder::{Embedder, MockEmbedder};
use serchx_lib::vector::index::{SqliteVectorIndex, VectorIndex};
use serchx_lib::vector::scheduler::ModelScheduler;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
struct BenchmarkQueryItem {
    id: String,
    query: String,
    category: String,
    expected_artifact: String,
    expected_version_index: Option<usize>,
    #[allow(dead_code)]
    description: String,
}

struct CorpusArtifactDef {
    filename: &'static str,
    folder: &'static str,
    versions: Vec<CorpusVersionDef>,
}

struct CorpusVersionDef {
    v_idx: usize,
    relative_days: i64,
    text: &'static str,
    is_pinned: bool,
}

#[tokio::test]
async fn test_serchx100_benchmark_evaluation_and_report() {
    let db = Arc::new(DatabaseManager::new_in_memory().expect("in-memory db"));
    let fts = Arc::new(Fts5SearchEngine::new(db.clone()));
    let vector_index = Arc::new(SqliteVectorIndex::new(db.clone()));
    let embedder = Arc::new(MockEmbedder::new(384));
    let embedder_clone = embedder.clone();
    let scheduler = Arc::new(ModelScheduler::with_factory(60, move || {
        Ok(embedder_clone.clone() as Arc<dyn Embedder>)
    }));
    let diff_engine = Arc::new(DiffEngine::new());
    let diff_cache = Arc::new(DiffCache::default());
    let engine = Arc::new(HybridSearchEngine::with_diff_support(
        db.clone(),
        fts,
        vector_index.clone(),
        scheduler,
        diff_engine,
        diff_cache,
    ));

    let now = Utc::now();
    let today_start = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 12, 0, 0)
        .unwrap();

    // Helper to calculate exact days back for target weekdays
    let days_back_for_weekday = |target: Weekday| -> i64 {
        let current = now.weekday();
        let mut diff =
            (current.num_days_from_monday() as i64) - (target.num_days_from_monday() as i64);
        if diff <= 0 {
            diff += 7;
        }
        diff
    };

    let monday_offset = days_back_for_weekday(Weekday::Mon);
    let _tuesday_offset = days_back_for_weekday(Weekday::Tue);
    let wednesday_offset = days_back_for_weekday(Weekday::Wed);
    let thursday_offset = days_back_for_weekday(Weekday::Thu);
    let friday_offset = days_back_for_weekday(Weekday::Fri);

    // Build comprehensive corpus definition
    let corpus_defs: Vec<CorpusArtifactDef> = vec![
        // Exact metadata & paths
        CorpusArtifactDef {
            filename: "invoice_services.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Invoice for consulting services. Total balance due $1,200.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "meeting_notes.txt",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Executive team meeting notes discussion topics.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "product_specs.pdf",
            folder: "/Users/alice/Downloads",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Product specs document outline and system requirements.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "budget_sheet.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Department quarterly budget sheet financial metrics.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "system_diagram.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Architecture system diagram whiteboard sketch.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "security_audit.docx",
            folder: "/Users/alice/Downloads",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Security audit compliance report findings and remediations.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "sales_playbook.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Enterprise sales playbook strategy methodology by David.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "product_roadmap.pptx",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Product roadmap future milestones release schedule deck.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "engineering_guidelines.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Engineering guidelines coding standards practices from Elena.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "incident_summary.txt",
            folder: "/Users/alice/Downloads",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 20, text: "Production incident summary postmortem resolution steps.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "nda_agreement.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Mutual Confidentiality Undertaking nondisclosure agreement binding terms.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "q3_financial_plan.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Q3 Financial Plan quarterly forecast analysis prepared by Michael.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "zero_trust_spec.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Zero Trust Architecture security policy end to end mutual TLS encryption.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "customer_sla.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Customer Service Level Agreement guarantee 99.99% uptime credits.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "quarterly_tax.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Spreadsheet quarterly tax filing calculations deduction forms.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "vendor_contract.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Vendor PDF contract agreement Master Services Agreement terms signed.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "executive_update.pptx",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Presentation executive update slide deck board review.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "myers_diff.rs",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Code algorithm rust safe Myers diff shortest edit script.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "spotlight_mockup.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Image mockup search bar spotlight search UI frame design preview.", is_pinned: false }],
        },

        // Temporal recency documents
        CorpusArtifactDef {
            filename: "daily_invoice.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 0, text: "Daily invoice services payment processed receipt.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "daily_standup.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 0, text: "Daily standup notes today completed work items blockers.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "deploy_status.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 0, text: "Production deploy log today release 2.4.1 succeeded.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "triage_bugs.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 0, text: "Bug report today critical triage list priorities.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "board_minutes.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1, text: "Board meeting minutes yesterday resolutions approved.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "updated_budget.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1, text: "Updated budget sheet yesterday expense allocation revisions.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "v01_release_notes.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1, text: "Product release notes yesterday feature highlights fixes.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "perf_benchmark.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1, text: "System benchmark log yesterday latency and throughput results.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "client_proposal.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 4, text: "Enterprise client proposal draft last week scope statement.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "compliance_audit.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 5, text: "Security compliance audit findings last week checklist.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "pricing_model.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 6, text: "SaaS pricing calculator last week margin scenario models.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "user_feedback.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 5, text: "Customer user interview last week usability testing notes.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "sprint_retro.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: monday_offset, text: "Product design sprint last monday retrospective review from Sarah.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "architecture_talk.pptx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: wednesday_offset, text: "Engineering tech talk last wednesday distributed systems slides.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "outage_postmortem.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: friday_offset, text: "Infrastructure incident review last friday root cause analysis.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "quarterly_planning.pptx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: thursday_offset, text: "Quarterly planning deck last thursday roadmap priorities.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "taxes_2024.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 600, text: "Corporate tax filing 2024 returns schedule forms filed.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "annual_report_2023.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1000, text: "Company annual report 2023 letter to shareholders finances.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "keynote_2022.pptx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1400, text: "Annual conference talk 2022 keynote presentation deck.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "audit_archive_2021.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 1800, text: "Historical audit summary 2021 archive findings record.", is_pinned: false }],
        },

        // Version-aware documents with revision histories
        CorpusArtifactDef {
            filename: "acme_msa.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 30, text: "Acme contract Master Services Agreement preliminary draft terms.\nInitial version v1.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 20, text: "Acme contract Master Services Agreement.\nPricing clause: monthly net 45 payment terms.\nApproved by legal.", is_pinned: false },
                CorpusVersionDef { v_idx: 3, relative_days: 5, text: "Acme contract Master Services Agreement.\nPricing clause: monthly net 30 payment terms.\nSigned final contract sent by Rahul.", is_pinned: true },
            ],
        },
        CorpusArtifactDef {
            filename: "auth_spec.md",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 25, text: "Authentication spec draft.\nSession cookies stored in browser.\nBefore auth redesign.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 10, text: "Authentication spec version 2.\nOAuth2 PKCE authorization code grant token refresh flow.\nAfter auth redesign.", is_pinned: true },
            ],
        },
        CorpusArtifactDef {
            filename: "john_resume.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 35, text: "John Doe resume curriculum vitae.\nSoftware engineer at Tech Corp.\nResume before job update.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 8, text: "John Doe resume curriculum vitae.\nSenior architect lead at Cloud Scale Inc.\nResume after job update.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "infra_arch.docx",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 40, text: "Infrastructure architecture draft specifications.\nOn-premise datacenter bare metal servers before cloud migration.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 12, text: "Infrastructure architecture final specifications.\nAWS native cloud topology kubernetes cluster after cloud migration.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "seed_pitch.pptx",
            folder: "/Users/alice/Desktop",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 30, text: "Seed pitch deck investment presentation.\nValuation $5M pre money cap before valuation changed.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 7, text: "Seed pitch deck investment presentation.\nValuation $12M post money cap after valuation changed.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "privacy_policy.md",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 20, text: "User privacy policy draft guidelines before legal review.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 6, text: "User privacy policy final legal compliance GDPR clauses after legal review.", is_pinned: true },
            ],
        },
        CorpusArtifactDef {
            filename: "dept_budget.xlsx",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 28, text: "Department operational budget allocation $1,000,000 before cuts.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 14, text: "Department operational budget allocation $800,000 after cuts.", is_pinned: false },
                CorpusVersionDef { v_idx: 3, relative_days: 3, text: "Department operational budget v3 approved final allocation.", is_pinned: true },
            ],
        },
        CorpusArtifactDef {
            filename: "warranty_terms.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 22, text: "Hardware warranty terms conditions standard 1-year coverage before warranty update.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 5, text: "Hardware warranty terms conditions extended 3-year coverage after warranty update.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "client_quote.docx",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 18, text: "Client proposal quote $50,000 full rate before discount added.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 4, text: "Client proposal quote $42,500 partner rate after discount added.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "database_schema.sql",
            folder: "/Users/alice/Documents",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 24, text: "Database schema initial setup single-tenant before migration.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 7, text: "Database schema updated multi-tenant tenant_id after migration.", is_pinned: false },
            ],
        },
        CorpusArtifactDef {
            filename: "sync_notes.txt",
            folder: "/Users/alice/Desktop",
            versions: vec![
                CorpusVersionDef { v_idx: 1, relative_days: 15, text: "Weekly engineering sync notes agenda topics before sync.", is_pinned: false },
                CorpusVersionDef { v_idx: 2, relative_days: 5, text: "Weekly engineering sync notes action items decided after team sync.", is_pinned: false },
            ],
        },

        // Semantic hybrid technical documents
        CorpusArtifactDef {
            filename: "docker_errors.txt",
            folder: "/Users/alice/Downloads",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Docker container crash exception error logs: OOMKilled exit code 137 terminated.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "embedding_guide.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Machine learning embedding dimension vector dense space representations semantic search.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "k8s_troubleshooting.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Kubernetes pod crashloopbackoff memory limit container exceeded cgroup allocation.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "db_internals.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Database deadlock transaction rollback detected cycle aborting victim transaction.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "consensus_notes.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Distributed consensus raft leader election append entries heartbeat log replication.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "event_driven_arch.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Asynchronous event driven microservices messaging kafka consumer groups topic streams.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "ci_cd_workflow.yml",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Continuous integration pipeline automated deployment GitHub actions release artifacts.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "frontend_arch.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Frontend state management reactive signals fine-grained dependency tracking UI.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "perf_tuning.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Garbage collection memory leak heap profiler heapdump allocation flamegraph.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "web_security.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Content security policy cross site scripting mitigation CSP headers XSS defense.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "cloud_patterns.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Serverless lambda cold start optimization snapstart provisioned concurrency.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "api_design.docx",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "GraphQL schema resolver batching dataloader N+1 query batching optimization.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "ha_setup.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "High availability load balancer failover clustering keepalived VRRP virtual IP.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "search_algorithms.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Search relevance ranking reciprocal rank fusion combining lexical BM25 and vector scores.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "ocr_pipeline.pdf",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Optical character recognition text extraction confidence Apple Vision framework accuracy.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "transaction_levels.txt",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Atomic transactions ACID compliance isolation levels serializable read committed snapshot.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "concurrency_guide.md",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 10, text: "Concurrency control optimistic locking version check compare and swap ETag update.", is_pinned: false }],
        },

        // OCR Screenshot images
        CorpusArtifactDef {
            filename: "crash_dialog.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Screenshot error dialog exception: Unhandled fatal exception 0x80004005 in thread 4.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "bistro_receipt.png",
            folder: "/Users/alice/Documents",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Receipt business lunch expenses Cafe Bistro lunch sandwich coffee subtotal $42.50.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "whiteboard_session.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Whiteboard architecture diagram brainstorming session microservices database api.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "metrics_dashboard.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Dashboard analytics revenue charts ARR MRR churn customer retention graphs.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "terminal_err.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Terminal command output permission denied zsh: sudo error opening socket file.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "slide_takeaways.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Slide deck key takeaways summary executive slide bullet points takeaways.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "app_wireframe.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Wireframe mobile navigation tab bar design mockup home search icons.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "network_topology.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Diagram network topology router firewall VPC subnets route tables nat.", is_pinned: false }],
        },
        CorpusArtifactDef {
            filename: "cloud_security_info.png",
            folder: "/Users/alice/Desktop",
            versions: vec![CorpusVersionDef { v_idx: 1, relative_days: 8, text: "Infographic cloud infrastructure security policies encryption access compliance.", is_pinned: false }],
        },
    ];

    // Map artifact filename -> (artifact_id, Vec<version_id>)
    let mut artifact_map: HashMap<String, (String, Vec<String>)> = HashMap::new();

    // Populate database, FTS, vectors, and versions
    for art in &corpus_defs {
        let full_path = format!("{}/{}", art.folder, art.filename);
        let artifact = db.upsert_artifact(&full_path, "file").unwrap();
        let mut version_ids = Vec::new();

        let mut prev_id: Option<String> = None;
        for v in &art.versions {
            let ver_id = format!("{}-v{}", art.filename.replace('.', "_"), v.v_idx);
            version_ids.push(ver_id.clone());

            let mod_time = (today_start - Duration::days(v.relative_days)).timestamp_millis();
            let version = ArtifactVersion {
                id: ver_id.clone(),
                artifact_id: artifact.id.clone(),
                path: full_path.clone(),
                content_hash: format!("h-{}-{}", art.filename, v.v_idx),
                text_hash: None,
                size_bytes: v.text.len() as i64,
                modified_at: mod_time,
                previous_version_id: prev_id.clone(),
                extracted_text: Some(v.text.to_string()),
                is_pinned: v.is_pinned,
            };
            db.record_version(&version).unwrap();

            // Insert chunk
            let chunk_id = format!("c-{}-{}", ver_id, 0);
            let chunk = DocumentChunk {
                id: chunk_id.clone(),
                version_id: ver_id.clone(),
                chunk_index: 0,
                content: v.text.to_string(),
            };
            db.insert_chunks(&[chunk]).unwrap();

            // Insert vector
            let vec = embedder.embed_text(v.text).unwrap();
            vector_index.insert_vector(&chunk_id, &vec).unwrap();

            prev_id = Some(ver_id);
        }

        artifact_map.insert(art.filename.to_string(), (artifact.id.clone(), version_ids));
    }

    // Load queries from fixture JSON
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/serchx100_queries.json"
    );
    let fixture_content = fs::read_to_string(fixture_path).expect("read serchx100_queries.json");
    let queries: Vec<BenchmarkQueryItem> =
        serde_json::from_str(&fixture_content).expect("parse serchx100_queries.json");

    assert_eq!(
        queries.len(),
        100,
        "Benchmark must contain exactly 100 queries"
    );

    let settings = AppSettings::default();

    // Execute queries and record evaluations
    let mut evaluations: Vec<QueryEvaluation> = Vec::with_capacity(100);
    let mut latencies: Vec<f32> = Vec::with_capacity(100);

    for q in &queries {
        let t0 = Instant::now();
        let results = engine.search(&q.query, 10, &settings).await.unwrap();
        let elapsed_ms = t0.elapsed().as_secs_f32() * 1000.0;
        latencies.push(elapsed_ms);

        // Find rank of expected result
        let mut rank_found = None;

        let (exp_art_id, exp_versions) =
            artifact_map.get(&q.expected_artifact).unwrap_or_else(|| {
                panic!(
                    "Expected artifact '{}' not found in corpus",
                    q.expected_artifact
                )
            });

        for (idx, r) in results.iter().enumerate() {
            let is_artifact_match =
                r.artifact_id == *exp_art_id || r.path.ends_with(&q.expected_artifact);

            let is_match = if let Some(v_idx) = q.expected_version_index {
                // If version index specified, check if version matches
                if v_idx > 0 && v_idx <= exp_versions.len() {
                    let target_v_id = &exp_versions[v_idx - 1];
                    is_artifact_match && r.version_id == *target_v_id
                } else {
                    is_artifact_match
                }
            } else {
                is_artifact_match
            };

            if is_match {
                rank_found = Some(idx + 1);
                break;
            }
        }

        let rank = rank_found;
        let hit_top1 = rank == Some(1);
        let hit_top3 = rank.is_some_and(|r| r <= 3);
        let hit_top5 = rank.is_some_and(|r| r <= 5);
        let reciprocal_rank = rank.map_or(0.0, |r| 1.0 / (r as f32));

        evaluations.push(QueryEvaluation {
            id: q.id.clone(),
            query: q.query.clone(),
            category: q.category.clone(),
            expected_artifact: q.expected_artifact.clone(),
            expected_version_index: q.expected_version_index,
            rank_found: rank,
            latency_ms: elapsed_ms,
            hit_top1,
            hit_top3,
            hit_top5,
            reciprocal_rank,
        });
    }

    // Partition metrics by category
    let category_names = [
        "ExactMetadataPath",
        "TemporalRecency",
        "VersionRecall",
        "SemanticHybrid",
        "OcrScreenshot",
        "PersonSource",
    ];

    let mut category_metrics = Vec::new();

    for cat in &category_names {
        let cat_evals: Vec<&QueryEvaluation> =
            evaluations.iter().filter(|e| e.category == *cat).collect();

        let count = cat_evals.len();
        if count == 0 {
            continue;
        }

        let hits_1 = cat_evals.iter().filter(|e| e.hit_top1).count();
        let hits_3 = cat_evals.iter().filter(|e| e.hit_top3).count();
        let hits_5 = cat_evals.iter().filter(|e| e.hit_top5).count();
        let mrr_sum: f32 = cat_evals.iter().map(|e| e.reciprocal_rank).sum();

        let recall_at_1 = hits_1 as f32 / count as f32;
        let recall_at_3 = hits_3 as f32 / count as f32;
        let recall_at_5 = hits_5 as f32 / count as f32;
        let mrr = mrr_sum / count as f32;

        let mut cat_latencies: Vec<f32> = cat_evals.iter().map(|e| e.latency_ms).collect();
        cat_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50_idx = (count as f32 * 0.50).floor() as usize;
        let p95_idx = ((count as f32 * 0.95).floor() as usize).min(count - 1);
        let latency_p50_ms = cat_latencies[p50_idx];
        let latency_p95_ms = cat_latencies[p95_idx];

        let (gate_status, gate_threshold_desc) = match *cat {
            "ExactMetadataPath" => {
                let passed = recall_at_3 >= 0.95;
                (
                    if passed { "PASSED" } else { "FAILED" }.to_string(),
                    "Recall@3 ≥ 95%".to_string(),
                )
            }
            "TemporalRecency" => {
                let passed = recall_at_3 >= 0.85 && mrr >= 0.80;
                (
                    if passed { "PASSED" } else { "FAILED" }.to_string(),
                    "Recall@3 ≥ 85%, MRR ≥ 0.80".to_string(),
                )
            }
            "VersionRecall" => {
                let passed = recall_at_3 >= 0.85 && mrr >= 0.80;
                (
                    if passed { "PASSED" } else { "FAILED" }.to_string(),
                    "Recall@3 ≥ 85%, MRR ≥ 0.80".to_string(),
                )
            }
            "SemanticHybrid" => {
                let passed = recall_at_3 >= 0.80;
                (
                    if passed { "PASSED" } else { "FAILED" }.to_string(),
                    "Recall@3 ≥ 80%".to_string(),
                )
            }
            "OcrScreenshot" => {
                let passed = recall_at_3 >= 0.80;
                (
                    if passed { "PASSED" } else { "FAILED" }.to_string(),
                    "Recall@3 ≥ 80%".to_string(),
                )
            }
            "PersonSource" => ("ADVISORY".to_string(), "Advisory evaluation".to_string()),
            _ => ("PASSED".to_string(), "N/A".to_string()),
        };

        category_metrics.push(CategoryMetrics {
            category: cat.to_string(),
            total_queries: count,
            recall_at_1,
            recall_at_3,
            recall_at_5,
            mrr,
            latency_p50_ms,
            latency_p95_ms,
            gate_status,
            gate_threshold_desc,
        });
    }

    // Overall telemetry
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let overall_p50 = latencies[50];
    let overall_p95 = latencies[95];
    let peak_rss_mb = (get_peak_rss_bytes() as f64) / (1024.0 * 1024.0);

    let db_size_mb = {
        let conn = db.lock_conn().unwrap();
        (get_sqlite_disk_bytes(&conn) as f64) / (1024.0 * 1024.0)
    };

    let total_top3: usize = evaluations.iter().filter(|e| e.hit_top3).count();
    let total_mrr_sum: f32 = evaluations.iter().map(|e| e.reciprocal_rank).sum();
    let overall_recall_at_3 = total_top3 as f32 / 100.0;
    let overall_mrr = total_mrr_sum / 100.0;

    let benchmark_result = BenchmarkResult {
        timestamp: Utc::now().to_rfc3339(),
        overall_recall_at_3,
        overall_mrr,
        latency_p50_ms: overall_p50,
        latency_p95_ms: overall_p95,
        peak_rss_mb,
        db_size_mb,
        categories: category_metrics.clone(),
        evaluations: evaluations.clone(),
    };

    // Assert release gates
    for cat in &category_metrics {
        if cat.category == "PersonSource" {
            continue; // Advisory only
        }
        assert_eq!(
            cat.gate_status, "PASSED",
            "Category '{}' failed release gate! Recall@3: {:.2}, MRR: {:.2}",
            cat.category, cat.recall_at_3, cat.mrr
        );
    }

    // Assert overall latency gate
    assert!(
        overall_p95 < 200.0,
        "Overall p95 latency must be < 200 ms, got {:.2} ms",
        overall_p95
    );

    // Save JSON results to fixtures
    let results_json = serde_json::to_string_pretty(&benchmark_result).unwrap();
    let json_dest = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/serchx100_results.json"
    );
    fs::write(json_dest, &results_json).expect("write serchx100_results.json");

    // Generate markdown report
    let markdown_report = generate_markdown_report(&benchmark_result);
    let report_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/benchmarks");
    fs::create_dir_all(report_dir).expect("create docs/benchmarks dir");
    let report_dest = format!("{}/serchx-100-report.md", report_dir);
    fs::write(report_dest, &markdown_report).expect("write serchx-100-report.md");

    println!("serchX-100 benchmark complete: Overall Recall@3: {:.1}%, Overall MRR: {:.3}, p95 latency: {:.2}ms",
        overall_recall_at_3 * 100.0, overall_mrr, overall_p95);
}
