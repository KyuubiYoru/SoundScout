//! Audio PCM + peaks commands.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use arboard::Clipboard;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::audio::clip_wav;
use crate::audio::loop_proc::{self, PostProcessConfig};
use crate::audio::pcm_stream::{self, PcmStreamFinished};
use crate::commands::state::AppState;
use crate::db::queries;
use crate::indexer::peaks;

/// Frontend listens for further chunks after [`start_pcm_stream`].
pub const EVT_PCM_STREAM_CHUNK: &str = "pcm-stream-chunk";
/// Emitted when decode finished (all chunk files were sent).
pub const EVT_PCM_STREAM_FINISHED: &str = "pcm-stream-finished";

/// Wait for first PCM chunk after starting background decode (slow disks / large files).
const PCM_STREAM_FIRST_CHUNK_TIMEOUT_SEC: u64 = 120;
const MILLISECONDS_PER_SECOND_F64: f64 = 1000.0;

static PCM_STREAM_SEQ: AtomicU64 = AtomicU64::new(1);

/// On Linux (X11/Wayland), clipboard payloads are served by the app that owns [`Clipboard`]; dropping it clears the clip for other apps. Keep one alive until the next copy.
static CLIPBOARD_FILE_KEEPALIVE: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();

fn file_clipboard_holder() -> &'static Mutex<Option<Clipboard>> {
    CLIPBOARD_FILE_KEEPALIVE.get_or_init(|| Mutex::new(None))
}

/// Small IPC payload: path to a temp file of raw little-endian interleaved `f32` PCM (avoids huge JSON `Vec<u8>`).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPcmFile {
    pub path: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Decode to PCM, write bytes to `temp_dir()/soundscout-playback-{id}.pcm`, return path for `convertFileSrc` + `fetch`.
#[tauri::command]
pub async fn get_audio_pcm_file(asset_id: i64, state: State<'_, AppState>) -> Result<AudioPcmFile, String> {
    let media_path = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        std::path::PathBuf::from(a.path)
    };

    let cache = Arc::clone(&state.audio_cache);
    tokio::task::spawn_blocking(move || {
        let pcm = cache
            .get_or_decode(asset_id, &media_path)
            .map_err(|e| e.to_string())?;
        let tmp = std::env::temp_dir().join(format!("soundscout-playback-{asset_id}.pcm"));
        let slice = bytemuck::cast_slice::<f32, u8>(pcm.samples.as_slice());
        std::fs::write(&tmp, slice).map_err(|e| e.to_string())?;
        Ok::<_, String>(AudioPcmFile {
            path: tmp.to_string_lossy().to_string(),
            sample_rate: pcm.sample_rate,
            channels: pcm.channels,
        })
    })
    .await
    .map_err(|e| format!("pcm file task: {e}"))?
}

/// Post-processed PCM (same as export pipeline) written to a temp `.pcm` for in-app preview.
#[tauri::command]
pub async fn get_processed_pcm_file(
    asset_id: i64,
    is_clip: bool,
    start_sec: f64,
    end_sec: f64,
    post_process: PostProcessConfig,
    state: State<'_, AppState>,
) -> Result<AudioPcmFile, String> {
    let media_path = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        std::path::PathBuf::from(a.path)
    };

    let cache = Arc::clone(&state.audio_cache);
    tokio::task::spawn_blocking(move || {
        let pcm = cache
            .get_or_decode(asset_id, &media_path)
            .map_err(|e| e.to_string())?;

        let ch = usize::from(pcm.channels);
        if ch == 0 || pcm.samples.is_empty() {
            return Err("no audio samples".to_string());
        }

        let interleaved: &[f32] = if is_clip {
            let (sf, ef) = clip_wav::clip_frame_range(&pcm, start_sec, end_sec)?;
            &pcm.samples[sf * ch..ef * ch]
        } else {
            &pcm.samples[..]
        };

        let result = loop_proc::apply(interleaved, pcm.sample_rate, pcm.channels, &post_process)?;
        let tmp = std::env::temp_dir().join(format!("soundscout-preview-{asset_id}.pcm"));
        let slice = bytemuck::cast_slice::<f32, u8>(result.samples.as_slice());
        std::fs::write(&tmp, slice).map_err(|e| e.to_string())?;
        Ok::<_, String>(AudioPcmFile {
            path: tmp.to_string_lossy().to_string(),
            sample_rate: pcm.sample_rate,
            channels: pcm.channels,
        })
    })
    .await
    .map_err(|e| format!("processed pcm task: {e}"))?
}

/// First chunk is returned here; later chunks use [`EVT_PCM_STREAM_CHUNK`] then [`EVT_PCM_STREAM_FINISHED`].
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcmStreamStart {
    pub stream_id: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_sec: f64,
    pub first_chunk_path: String,
    pub first_chunk_index: u32,
}

#[tauri::command]
pub async fn start_pcm_stream(
    asset_id: i64,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PcmStreamStart, String> {
    if let Some(prev) = state.pcm_stream_cancel.lock().unwrap().take() {
        prev.store(true, Ordering::Relaxed);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *state.pcm_stream_cancel.lock().unwrap() = Some(Arc::clone(&cancel));

    let (media_path, duration_ms) = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        (std::path::PathBuf::from(a.path), a.duration_ms)
    };

    let stream_id = PCM_STREAM_SEQ.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<pcm_stream::PcmStreamChunkMsg>();
    let path_for_task = media_path;
    let cancel_task = Arc::clone(&cancel);
    tokio::task::spawn_blocking(move || {
        let _ = pcm_stream::run_pcm_stream(path_for_task, stream_id, cancel_task, tx);
    });

    let first = tokio::time::timeout(
        std::time::Duration::from_secs(PCM_STREAM_FIRST_CHUNK_TIMEOUT_SEC),
        rx.recv(),
    )
        .await
        .map_err(|_| "pcm stream decode timeout".to_string())?
        .ok_or_else(|| "pcm stream produced no audio".to_string())?;

    let duration_sec = duration_ms
        .map(|ms| (ms as f64) / MILLISECONDS_PER_SECOND_F64)
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);

    let app_fwd = app.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let _ = app_fwd.emit(EVT_PCM_STREAM_CHUNK, &msg);
        }
        let _ = app_fwd.emit(
            EVT_PCM_STREAM_FINISHED,
            &PcmStreamFinished { stream_id },
        );
    });

    Ok(PcmStreamStart {
        stream_id: first.stream_id,
        sample_rate: first.sample_rate,
        channels: first.channels,
        duration_sec,
        first_chunk_path: first.path,
        first_chunk_index: first.chunk_index,
    })
}

#[tauri::command]
pub fn cancel_pcm_stream(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(c) = state.pcm_stream_cancel.lock().unwrap().take() {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Raw little-endian interleaved `f32` PCM (metadata comes from the [`Asset`](crate::db::models::Asset) row).
#[tauri::command]
pub async fn get_audio_data(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let path = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        std::path::PathBuf::from(a.path)
    };

    let cache = Arc::clone(&state.audio_cache);
    tokio::task::spawn_blocking(move || {
        let pcm = cache.get_or_decode(asset_id, &path).map_err(|e| e.to_string())?;
        Ok(bytemuck::cast_slice::<f32, u8>(pcm.samples.as_slice()).to_vec())
    })
    .await
    .map_err(|e| format!("decode task: {e}"))?
}

#[tauri::command]
pub async fn get_peaks(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<f32>, String> {
    let conn = state.pool.get().map_err(|e| e.to_string())?;
    let blob = queries::get_peaks(&conn, asset_id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    Ok(peaks::peaks_to_floats(&blob))
}

/// Save as IEEE float WAV via a save dialog. If `is_clip`, exports `[start_sec, end_sec]`; otherwise the full decoded file.
#[tauri::command]
pub async fn export_clip_wav(
    app: AppHandle,
    asset_id: i64,
    is_clip: bool,
    start_sec: f64,
    end_sec: f64,
    post_process: PostProcessConfig,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let (media_path, suggested_name, starting_dir) = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        let pb = std::path::PathBuf::from(&a.path);
        let dir = pb.parent().filter(|p| p.is_dir()).map(std::path::Path::to_path_buf);
        let sugg = if is_clip {
            clip_wav::export_clip_suggested_filename(&a.filename, start_sec, end_sec)
        } else {
            clip_wav::export_full_wav_suggested_filename(&a.filename)
        };
        (pb, sugg, dir)
    };

    let mut dialog = app
        .dialog()
        .file()
        .set_title("Export as WAV")
        .add_filter("WAV", &["wav"]);
    if let Some(dir) = starting_dir {
        dialog = dialog.set_directory(dir);
    }
    dialog = dialog.set_file_name(suggested_name);
    let dest = dialog.blocking_save_file();
    let Some(dest) = dest else {
        return Ok(None);
    };
    let dest_pb = match dest {
        FilePath::Path(p) => p,
        FilePath::Url(u) => u
            .to_file_path()
            .map_err(|_| "could not convert save URL to path".to_string())?,
    };

    let cache = Arc::clone(&state.audio_cache);
    let path_str = tokio::task::spawn_blocking(move || {
        let pcm = cache
            .get_or_decode(asset_id, &media_path)
            .map_err(|e| e.to_string())?;
        let pp = Some(&post_process);
        if is_clip {
            clip_wav::write_clip_wav(&pcm, start_sec, end_sec, &dest_pb, pp)?;
        } else {
            clip_wav::write_full_wav(&pcm, &dest_pb, pp)?;
        }
        Ok::<_, String>(dest_pb.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("export clip task: {e}"))??;

    Ok(Some(path_str))
}

/// Write WAV to a temp file and put it on the clipboard as a file. `is_clip` selects a time range vs full file.
/// Linux/Wayland: behavior depends on the compositor. The temp file must stay until paste; a previous temp file is removed when copying again.
#[tauri::command]
pub async fn copy_clip_wav_to_clipboard(
    asset_id: i64,
    is_clip: bool,
    start_sec: f64,
    end_sec: f64,
    post_process: PostProcessConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (media_path, source_filename) = {
        let conn = state.pool.get().map_err(|e| e.to_string())?;
        let a = queries::get_asset_by_id(&conn, asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "asset not found".to_string())?;
        (std::path::PathBuf::from(a.path), a.filename)
    };

    let cache = Arc::clone(&state.audio_cache);
    let last = Arc::clone(&state.last_clipboard_wav);
    tokio::task::spawn_blocking(move || {
        {
            let mut g = last.lock().map_err(|e| e.to_string())?;
            if let Some(prev) = g.take() {
                let _ = std::fs::remove_file(prev);
            }
        }

        let pcm = cache
            .get_or_decode(asset_id, &media_path)
            .map_err(|e| e.to_string())?;

        let p = clip_wav::clipboard_temp_wav_path(&source_filename, is_clip, start_sec, end_sec);
        let pp = Some(&post_process);
        if is_clip {
            clip_wav::write_clip_wav(&pcm, start_sec, end_sec, &p, pp)?;
        } else {
            clip_wav::write_full_wav(&pcm, &p, pp)?;
        }

        {
            let mut holder = file_clipboard_holder()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *holder = None;
        }

        let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
        cb.set()
            .file_list(std::slice::from_ref(&p))
            .map_err(|e| e.to_string())?;

        {
            let mut holder = file_clipboard_holder()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *holder = Some(cb);
        }

        let mut g = last.lock().map_err(|e| e.to_string())?;
        *g = Some(p);
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| format!("clipboard clip task: {e}"))??;

    Ok(())
}
