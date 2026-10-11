use crate::db::DatabaseManager;
use crate::search::{
    Fts5SearchEngine, GroupedSearchResult, HybridSearchEngine, HybridSearchResult, SearchResult,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn search_exact(
    query: String,
    limit: Option<usize>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<SearchResult>, String> {
    let engine = Fts5SearchEngine::new(db.inner().clone());
    engine
        .search(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_hybrid(
    query: String,
    limit: Option<usize>,
    engine: State<'_, Arc<HybridSearchEngine>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<HybridSearchResult>, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    engine
        .search(&query, limit.unwrap_or(20), &settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_grouped(
    query: String,
    limit: Option<usize>,
    engine: State<'_, Arc<HybridSearchEngine>>,
    db: State<'_, Arc<DatabaseManager>>,
) -> Result<Vec<GroupedSearchResult>, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    engine
        .search_grouped(&query, limit.unwrap_or(20), &settings)
        .await
        .map_err(|e| e.to_string())
}
