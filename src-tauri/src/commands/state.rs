//! Shared mutable application state for commands.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use crate::audio::cache::AudioCache;
use crate::config::AppConfig;
use crate::db::connection::DbPool;
use crate::indexer::pipeline::CancelHandle;

/// Process-wide state behind Tauri `manage`.
pub struct AppState {
    pub pool: Arc<DbPool>,
    pub config: RwLock<AppConfig>,
    /// Shared with background scan tasks so they can clear the handle when finished.
    pub scan_handle: Arc<Mutex<Option<CancelHandle>>>,
    pub audio_cache: Arc<AudioCache>,
    /// Cancel flag for in-flight [`start_pcm_stream`](crate::commands::audio::start_pcm_stream).
    pub pcm_stream_cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    /// Last temp WAV written for [`copy_clip_wav_to_clipboard`](crate::commands::audio::copy_clip_wav_to_clipboard) (best-effort delete on next copy).
    pub last_clipboard_wav: Arc<Mutex<Option<PathBuf>>>,
}
