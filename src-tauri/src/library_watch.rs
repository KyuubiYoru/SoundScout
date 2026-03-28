//! Debounced filesystem watch on scan roots — emits `library-files-changed` when files update.

use std::path::Path;
use std::time::Duration;

use notify_debouncer_mini::new_debouncer;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer};
use tauri::{Emitter, Runtime};

/// Spawn a background thread that watches `roots` and calls `emit_fn` after debounced FS activity.
pub fn spawn_watch<F>(roots: Vec<std::path::PathBuf>, emit_fn: F)
where
    F: Fn() + Send + Sync + 'static,
{
    if roots.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let mut debouncer: Debouncer<_> = match new_debouncer(Duration::from_secs(3), move |res: DebounceEventResult| {
            if let Ok(events) = res {
                if !events.is_empty() {
                    emit_fn();
                }
            }
        }) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(target: "soundscout::watch", "debouncer init failed: {e}");
                return;
            }
        };
        for r in &roots {
            if r.exists() {
                if let Err(e) = debouncer
                    .watcher()
                    .watch(Path::new(r), RecursiveMode::Recursive)
                {
                    tracing::warn!(target: "soundscout::watch", "watch {:?}: {e}", r);
                }
            }
        }
        tracing::info!(target: "soundscout::watch", "watching {} roots", roots.len());
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });
}

/// Typed payload for the frontend listener.
pub fn emit_library_changed<R: Runtime>(handle: &tauri::AppHandle<R>) {
    let _ = handle.emit("library-files-changed", serde_json::json!({ "reason": "fs" }));
}
