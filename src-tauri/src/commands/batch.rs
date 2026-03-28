//! Bulk tag / favorite / rating updates.

use tauri::State;

use crate::commands::state::AppState;
use crate::db::queries;

#[tauri::command]
pub async fn bulk_add_tag(asset_ids: Vec<i64>, tag_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::bulk_add_tag(&conn, &asset_ids, tag_name.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bulk_set_favorite(
    asset_ids: Vec<i64>,
    favorite: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::bulk_set_favorite(&conn, &asset_ids, favorite).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bulk_set_rating(asset_ids: Vec<i64>, rating: u8, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::bulk_set_rating(&conn, &asset_ids, rating).map_err(|e| e.to_string())
}
