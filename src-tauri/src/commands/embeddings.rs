//! Build / refresh text embeddings for all assets (local ONNX).

use std::time::Instant;

use tauri::{AppHandle, Emitter, State};

use crate::commands::state::AppState;
use crate::db::connection::DbPool;
use crate::db::models::{EmbedRebuildComplete, EmbedRebuildProgress};
use crate::db::queries;
use crate::embedding::{EmbedSession, TEXT_EMBEDDING_MODEL_ID};
use crate::search::text_doc;

const BATCH: usize = 32;

#[tauri::command]
pub async fn rebuild_text_embeddings(app: AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    let pool = std::sync::Arc::clone(&state.pool);
    let app_b = app.clone();
    tokio::task::spawn_blocking(move || rebuild_blocking(&pool, app_b))
        .await
        .map_err(|e| e.to_string())?
}

fn rebuild_blocking(pool: &DbPool, app: AppHandle) -> Result<u32, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let ids = queries::list_all_asset_ids(&conn).map_err(|e| e.to_string())?;
    let total = u32::try_from(ids.len()).unwrap_or(u32::MAX);

    let _ = app.emit(
        "embed:progress",
        &EmbedRebuildProgress {
            processed: 0,
            total,
            detail: "Loading embedding model…".to_string(),
        },
    );

    let mut session = EmbedSession::new().map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut done = 0u32;
    let mut examined = 0u32;

    let _ = app.emit(
        "embed:progress",
        &EmbedRebuildProgress {
            processed: 0,
            total,
            detail: "Embedding assets…".to_string(),
        },
    );

    for chunk in ids.chunks(BATCH) {
        let mut texts = Vec::new();
        let mut chunk_ids = Vec::new();
        for &id in chunk {
            examined += 1;
            let Some(asset) = queries::get_asset_by_id(&conn, id).map_err(|e| e.to_string())? else {
                continue;
            };
            let doc = text_doc::asset_document(&asset, &conn, id).map_err(|e| e.to_string())?;
            if doc.trim().is_empty() {
                continue;
            }
            texts.push(doc);
            chunk_ids.push(id);
        }

        if texts.is_empty() {
            let _ = app.emit(
                "embed:progress",
                &EmbedRebuildProgress {
                    processed: examined,
                    total,
                    detail: String::new(),
                },
            );
            continue;
        }

        let vecs = session.embed_batch(&texts).map_err(|e| e.to_string())?;
        for (id, v) in chunk_ids.iter().zip(vecs.iter()) {
            queries::upsert_text_embedding(&conn, *id, TEXT_EMBEDDING_MODEL_ID, v).map_err(|e| e.to_string())?;
            done += 1;
        }

        let _ = app.emit(
            "embed:progress",
            &EmbedRebuildProgress {
                processed: examined,
                total,
                detail: String::new(),
            },
        );
    }

    let complete = EmbedRebuildComplete {
        written: done,
        duration_secs: started.elapsed().as_secs_f64(),
    };
    let _ = app.emit("embed:complete", &complete);

    Ok(done)
}
