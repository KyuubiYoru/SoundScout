//! Scan / index commands.

use std::sync::mpsc::channel;
use std::thread;

use tauri::{AppHandle, Emitter, State};

use crate::commands::state::AppState;
use crate::db::models::ScanStats;
use crate::indexer::pipeline::IndexPipeline;

#[tauri::command]
pub async fn start_scan(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let (roots, idx_cfg) = {
        let cfg = state.config.read().map_err(|e| e.to_string())?;
        (cfg.general.scan_roots.clone(), cfg.indexing.clone())
    };
    if roots.is_empty() {
        return Ok(());
    }

    {
        let g = state.scan_handle.lock().map_err(|e| e.to_string())?;
        if g.is_some() {
            return Ok(());
        }
    }

    let pool = std::sync::Arc::clone(&state.pool);
    let pipe = IndexPipeline::new(std::sync::Arc::clone(&pool), idx_cfg.clone());
    let cancel = pipe.cancel_handle();
    {
        let mut g = state.scan_handle.lock().map_err(|e| e.to_string())?;
        *g = Some(cancel);
    }

    let app_progress = app.clone();
    let app_done = app.clone();
    let scan_handle = std::sync::Arc::clone(&state.scan_handle);

    tokio::spawn(async move {
        let res = tokio::task::spawn_blocking(move || {
            let (tx, rx) = channel();
            thread::spawn({
                let app_p = app_progress.clone();
                move || {
                    while let Ok(p) = rx.recv() {
                        let _ = app_p.emit("scan:progress", &p);
                    }
                }
            });

            let mut last_stats = ScanStats {
                files_indexed: 0,
                files_skipped: 0,
                files_missing: 0,
                errors: 0,
                duration_secs: 0.0,
            };
            for root in roots {
                match pipe.run(&root, tx.clone()) {
                    Ok(s) => last_stats = s,
                    Err(e) => {
                        tracing::warn!("scan error: {e}");
                        last_stats.errors += 1;
                    }
                }
            }
            drop(tx);
            last_stats
        })
        .await;

        {
            let mut g = scan_handle.lock().expect("scan_handle");
            *g = None;
        }

        match res {
            Ok(stats) => {
                let _ = app_done.emit("scan:complete", &stats);
            }
            Err(e) => {
                tracing::warn!("scan join: {e}");
                let _ = app_done.emit(
                    "scan:complete",
                    &ScanStats {
                        files_indexed: 0,
                        files_skipped: 0,
                        files_missing: 0,
                        errors: 1,
                        duration_secs: 0.0,
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    let mut g = state.scan_handle.lock().map_err(|e| e.to_string())?;
    if let Some(h) = g.as_ref() {
        h.cancel();
    }
    *g = None;
    Ok(())
}
