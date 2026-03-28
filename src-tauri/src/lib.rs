#![allow(missing_docs)]
//! SoundScout — search and export game audio SFX libraries.

pub mod audio;
pub mod commands;
pub mod config;
pub mod db;
pub mod embedding;
pub mod error;
pub mod indexer;
pub mod library_watch;
pub mod search;

#[cfg(test)]
pub mod test_utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config_dir = config::settings::config_dir();
    let _ = std::fs::create_dir_all(&config_dir);
    let config_path = config_dir.join("config.toml");
    let app_config = config::AppConfig::load(&config_path).unwrap_or_default();
    let db_path = config_dir.join("soundscout.db");
    let pool = match db::connection::DbPool::new(&db_path) {
        Ok(p) => std::sync::Arc::new(p),
        Err(e) => {
            tracing::error!("database open failed: {e}");
            std::process::exit(1);
        }
    };
    let cache_cap = app_config.playback.buffer_cache_count.max(1);
    let state = commands::state::AppState {
        pool,
        config: std::sync::RwLock::new(app_config),
        scan_handle: std::sync::Arc::new(std::sync::Mutex::new(None)),
        audio_cache: std::sync::Arc::new(audio::cache::AudioCache::new(cache_cap)),
        pcm_stream_cancel: std::sync::Arc::new(std::sync::Mutex::new(None)),
        last_clipboard_wav: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            let handle = app.handle().clone();
            let (do_watch, roots) = {
                let state = app.state::<commands::state::AppState>();
                let cfg = match state.config.read() {
                    Ok(c) => c.clone(),
                    Err(e) => {
                        tracing::error!(target: "soundscout", "config rwlock poisoned: {e}");
                        return Ok(());
                    }
                };
                let watch = cfg.indexing.watch_scan_roots && !cfg.general.scan_roots.is_empty();
                let roots = cfg.general.scan_roots.clone();
                (watch, roots)
            };
            if do_watch {
                let h = handle.clone();
                library_watch::spawn_watch(roots, move || {
                    library_watch::emit_library_changed(&h);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::indexing::start_scan,
            commands::indexing::cancel_scan,
            commands::search::search,
            commands::search::get_filter_options,
            commands::search::browse_folder,
            commands::search::get_folder_tree,
            commands::audio::get_audio_data,
            commands::audio::get_audio_pcm_file,
            commands::audio::get_processed_pcm_file,
            commands::audio::start_pcm_stream,
            commands::audio::cancel_pcm_stream,
            commands::audio::get_peaks,
            commands::audio::export_clip_wav,
            commands::audio::copy_clip_wav_to_clipboard,
            commands::user_data::toggle_favorite,
            commands::user_data::set_rating,
            commands::user_data::add_tag,
            commands::user_data::remove_tag,
            commands::user_data::get_tags,
            commands::user_data::get_all_tags,
            commands::user_data::get_config,
            commands::user_data::update_config,
            commands::user_data::pick_directory,
            commands::embeddings::rebuild_text_embeddings,
            commands::batch::bulk_add_tag,
            commands::batch::bulk_set_favorite,
            commands::batch::bulk_set_rating,
            commands::semantic::get_semantic_search_status,
            commands::semantic::get_similar_assets,
            commands::semantic::get_auto_category_suggestions,
            commands::library::export_database,
            commands::library::import_database,
            commands::library::wipe_library_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
