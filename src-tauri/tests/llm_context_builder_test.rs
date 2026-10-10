use serchx_lib::llm::{ContextBuilder, StructuredEvidence};
use serchx_lib::search::hybrid::HybridSearchResult;

#[test]
fn test_context_builder_grounding() {
    let builder = ContextBuilder::new(3, 200);

    let search_results = vec![
        HybridSearchResult {
            chunk_id: "c1".to_string(),
            artifact_id: "a1".to_string(),
            version_id: "v1".to_string(),
            path: "/Users/alice/Documents/Q3_Financials.pdf".to_string(),
            score: 0.95,
            snippet: "Revenue grew 24% year-over-year reaching $4.2M.".to_string(),
            reasons: vec![],
            modified_at: 1720000000,
            is_pinned: false,
        },
        HybridSearchResult {
            chunk_id: "c2".to_string(),
            artifact_id: "a2".to_string(),
            version_id: "v2".to_string(),
            path: "/Users/alice/Documents/Roadmap_2025.docx".to_string(),
            score: 0.88,
            snippet: "Core focus for Q1 is enterprise security and SSO.".to_string(),
            reasons: vec![],
            modified_at: 1721000000,
            is_pinned: true,
        },
    ];

    let evidence = builder.from_search_results(&search_results);
    assert_eq!(evidence.len(), 2);

    // Verify 1-based indexing
    assert_eq!(evidence[0].index, 1);
    assert_eq!(evidence[0].document_title, "Q3_Financials.pdf");
    assert_eq!(evidence[1].index, 2);
    assert_eq!(evidence[1].document_title, "Roadmap_2025.docx");

    let (system_prompt, user_prompt) = builder.build_prompt("What was Q3 revenue?", &evidence);

    // Verify strict grounding rules in system prompt
    assert!(system_prompt.contains("Evidence Synthesizer"));
    assert!(system_prompt.contains("cite its source using inline bracketed numbers like [1] or [2]"));
    assert!(system_prompt.contains("I could not find enough evidence in your local files to answer this question with confidence."));

    // Verify user prompt format
    assert!(user_prompt.contains("[1] Title: \"Q3_Financials.pdf\""));
    assert!(user_prompt.contains("Excerpt: \"\"\"Revenue grew 24% year-over-year reaching $4.2M.\"\"\""));
    assert!(user_prompt.contains("[2] Title: \"Roadmap_2025.docx\""));
    assert!(user_prompt.contains("Excerpt: \"\"\"Core focus for Q1 is enterprise security and SSO.\"\"\""));
}

#[test]
fn test_context_builder_chunk_truncation() {
    let builder = ContextBuilder::new(5, 50); // limit to 50 chars

    let long_snippet = "This is a very long text snippet that will definitely exceed fifty characters in total length.";
    let search_results = vec![HybridSearchResult {
        chunk_id: "c1".to_string(),
        artifact_id: "a1".to_string(),
        version_id: "v1".to_string(),
        path: "/test/file.txt".to_string(),
        score: 0.9,
        snippet: long_snippet.to_string(),
        reasons: vec![],
        modified_at: 1700000000,
        is_pinned: false,
    }];

    let evidence = builder.from_search_results(&search_results);
    assert_eq!(evidence.len(), 1);
    assert!(evidence[0].text_snippet.len() <= 53); // 50 + "..."
    assert!(evidence[0].text_snippet.ends_with("..."));
}

#[test]
fn test_context_builder_empty_evidence() {
    let builder = ContextBuilder::default();
    let evidence: Vec<StructuredEvidence> = vec![];
    let (system_prompt, user_prompt) = builder.build_prompt("Unknown topic", &evidence);

    assert!(user_prompt.contains("(No matching local file evidence found.)"));
    assert!(system_prompt.contains("I could not find enough evidence in your local files to answer this question with confidence."));
}

#[test]
fn test_context_builder_single_document_prompt() {
    let builder = ContextBuilder::default();
    let (system_prompt, user_prompt) = builder.build_single_document_prompt(
        "Meeting_Notes.md",
        "/notes/Meeting_Notes.md",
        "Action items: 1. Deploy serchX v1.0",
    );

    assert!(system_prompt.contains("Document Summarizer"));
    assert!(user_prompt.contains("Meeting_Notes.md"));
    assert!(user_prompt.contains("Deploy serchX v1.0"));
}
