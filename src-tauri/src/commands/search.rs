//! Search and browse commands.

use tauri::State;

use crate::commands::state::AppState;
use crate::db::models::{Asset, FilterOptions, FolderNode, SearchQuery, SearchResults};
use crate::db::queries;
use crate::search::engine;

#[tauri::command]
pub async fn search(query: SearchQuery, state: State<'_, AppState>) -> Result<SearchResults, String> {
    use crate::db::models::SearchMode;
    let pool = state.pool.clone();
    let needs_embed = matches!(query.search_mode, SearchMode::Vector | SearchMode::Both)
        && !query.text.trim().is_empty();
    if needs_embed {
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| e.to_string())?;
            engine::execute_search(&conn, &query).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    } else {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        engine::execute_search(&conn, &query).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn get_filter_options(state: State<'_, AppState>) -> Result<FilterOptions, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::get_filter_options(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browse_folder(
    folder: String,
    limit: u32,
    offset: u32,
    state: State<'_, AppState>,
) -> Result<Vec<Asset>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::get_assets_by_folder(&conn, &folder, limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browse_folder_count(folder: String, state: State<'_, AppState>) -> Result<u64, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::count_assets_under_folder(&conn, &folder).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_folder_tree(state: State<'_, AppState>) -> Result<Vec<FolderNode>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    let flat = queries::get_folder_tree(&conn).map_err(|e| e.to_string())?;
    Ok(queries::build_folder_tree(&flat))
}
