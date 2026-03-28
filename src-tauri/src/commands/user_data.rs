//! Favorites, ratings, tags, config, folder picker.
//!
//! Tag and metadata edits do not update dense text embeddings automatically; run **Rebuild text embeddings** when using vector/hybrid search.

use tauri::{AppHandle, State};

use crate::commands::state::AppState;
use crate::config::settings::{self, AppConfig};
use crate::db::models::Tag;
use crate::db::models::TagWithCount;
use crate::db::queries;

#[tauri::command]
pub async fn toggle_favorite(asset_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    let a = queries::get_asset_by_id(&conn, asset_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;
    let next = !a.favorite;
    queries::update_favorite(&conn, asset_id, next).map_err(|e| e.to_string())?;
    Ok(next)
}

#[tauri::command]
pub async fn set_rating(asset_id: i64, rating: u8, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::update_rating(&conn, asset_id, rating).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_tag(asset_id: i64, tag_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::add_tag(&conn, asset_id, &tag_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_tag(asset_id: i64, tag_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::remove_tag(&conn, asset_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_tags(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::get_tags_for_asset(&conn, asset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_tags(state: State<'_, AppState>) -> Result<Vec<TagWithCount>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    queries::get_all_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.read().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub async fn update_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    let path = settings::config_dir().join("config.toml");
    config.save(&path).map_err(|e| e.to_string())?;
    *state.config.write().map_err(|e| e.to_string())? = config;
    Ok(())
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = app
        .dialog()
        .file()
        .set_title("Select audio library folder")
        .blocking_pick_folder();
    Ok(dir.map(|p| p.to_string()))
}
