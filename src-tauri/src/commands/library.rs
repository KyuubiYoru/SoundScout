//! Export / import the SQLite library file.

use std::fs;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use tauri::State;
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind};

use crate::commands::state::AppState;
use crate::config::settings;
use crate::db::queries;

fn db_path() -> std::path::PathBuf {
    settings::config_dir().join("soundscout.db")
}

/// Copy the live DB to a path chosen in a save dialog. Returns destination path if saved.
#[tauri::command]
pub async fn export_database(app: AppHandle) -> Result<Option<String>, String> {
    let dest = app
        .dialog()
        .file()
        .set_title("Export SoundScout library")
        .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
        .blocking_save_file();
    let Some(dest) = dest else {
        return Ok(None);
    };
    let dest_pb = match dest {
        FilePath::Path(p) => p,
        FilePath::Url(u) => u.to_file_path().map_err(|_| "could not convert export URL to path".to_string())?,
    };
    let src = db_path();
    if !src.exists() {
        return Err("no database to export yet — index a library first".to_string());
    }
    fs::copy(&src, &dest_pb).map_err(|e| e.to_string())?;
    Ok(Some(dest_pb.to_string_lossy().into_owned()))
}

/// Replace the live DB from a file chosen in an open dialog, then restart the app.
#[tauri::command]
pub async fn import_database(app: AppHandle) -> Result<(), String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Import SoundScout library")
        .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
        .blocking_pick_file();
    let Some(picked) = picked else {
        return Ok(());
    };
    let src_pb = match picked {
        FilePath::Path(p) => p,
        FilePath::Url(u) => u.to_file_path().map_err(|_| "could not convert import URL to path".to_string())?,
    };
    let dest = db_path();
    let _ = fs::create_dir_all(settings::config_dir());
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if dest.exists() {
        let bak = settings::config_dir().join(format!("soundscout.db.bak.{ts}"));
        fs::copy(&dest, &bak).map_err(|e| format!("backup failed: {e}"))?;
    }
    fs::copy(&src_pb, &dest).map_err(|e| format!("import copy failed: {e}"))?;
    app.restart();
}

/// Drop all rows from the library tables (scan roots in config are unchanged; audio files on disk are untouched).
///
/// Shows a **native** confirmation first (`window.confirm` in the webview is not reliable on some platforms).
/// Returns `true` if data was cleared, `false` if the user cancelled.
#[tauri::command]
pub async fn wipe_library_database(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let app_dialog = app.clone();
    let confirmed = tokio::task::spawn_blocking(move || {
        app_dialog
            .dialog()
            .message(
                "This removes indexed files, tags, ratings, favorites, notes, peaks, and embeddings from SoundScout.\n\n\
                 Your audio files on disk are not deleted. Scan folders in settings are unchanged.\n\n\
                 This cannot be undone.",
            )
            .title("Clear library database?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancel)
            .blocking_show()
    })
    .await
    .map_err(|e| format!("dialog task: {e}"))?;

    if !confirmed {
        return Ok(false);
    }

    {
        let mut g = state.scan_handle.lock().map_err(|e| e.to_string())?;
        if let Some(h) = g.as_ref() {
            h.cancel();
        }
        *g = None;
    }
    if let Some(c) = state
        .pcm_stream_cancel
        .lock()
        .map_err(|e| e.to_string())?
        .take()
    {
        c.store(true, Ordering::Relaxed);
    }
    state.audio_cache.clear();
    let pool = Arc::clone(&state.pool);
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| e.to_string())?;
        queries::wipe_library_data(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("wipe task: {e}"))??;
    Ok(true)
}
