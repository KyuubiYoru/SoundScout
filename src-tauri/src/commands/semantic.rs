//! Embedding index status and heuristic similarity (until audio-similarity pipelines land).

use tauri::State;

use crate::commands::state::AppState;
use crate::db::models::{Asset, SemanticSearchStatus};
use crate::db::queries;

#[tauri::command]
pub async fn get_semantic_search_status(state: State<'_, AppState>) -> Result<SemanticSearchStatus, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::semantic_status(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_similar_assets(
    asset_id: i64,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<Asset>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::get_similar_assets_heuristic(&conn, asset_id, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_auto_category_suggestions(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::suggest_categories_for_asset(&conn, asset_id).map_err(|e| e.to_string())
}
