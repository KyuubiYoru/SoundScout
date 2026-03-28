//! Persistent app configuration (`~/.config/soundscout/config.toml`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::models::SearchMode;
use crate::error::SoundScoutError;

/// Top-level scan roots and general options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Absolute paths to index.
    pub scan_roots: Vec<PathBuf>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            scan_roots: Vec::new(),
        }
    }
}

/// Indexing behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexConfig {
    /// `0` means auto (`rayon` default pool).
    pub parallel_workers: usize,
    pub peak_resolution: usize,
    #[serde(rename = "skip_hidden_dirs")]
    pub skip_hidden: bool,
    /// Watch scan roots for changes and trigger a rescan (debounced).
    #[serde(default)]
    pub watch_scan_roots: bool,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            parallel_workers: 0,
            peak_resolution: 800,
            skip_hidden: true,
            watch_scan_roots: false,
        }
    }
}

/// Playback preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    pub buffer_cache_count: usize,
    #[serde(rename = "auto_play_on_select")]
    pub auto_play: bool,
    #[serde(default)]
    pub loop_playback: bool,
    /// Step for keyboard clip end nudges (`i` / `o`), in milliseconds.
    #[serde(default = "default_clip_notch_ms")]
    pub clip_notch_ms: u32,
}

fn default_clip_notch_ms() -> u32 {
    100
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            buffer_cache_count: 10,
            auto_play: true,
            loop_playback: false,
            clip_notch_ms: default_clip_notch_ms(),
        }
    }
}

/// Search defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub default_sort: String,
    pub results_per_page: u32,
    /// Ignored — ranking mode is chosen per query in the UI (`SearchQuery.search_mode`).
    #[serde(default)]
    pub semantic_search: bool,
    /// Default for new sessions when the front-end has no `localStorage` choice yet.
    #[serde(default)]
    pub default_search_mode: SearchMode,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_sort: "relevance".to_string(),
            results_per_page: 50,
            semantic_search: false,
            default_search_mode: SearchMode::default(),
        }
    }
}

/// Full application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub indexing: IndexConfig,
    pub playback: PlaybackConfig,
    pub search: SearchConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            indexing: IndexConfig::default(),
            playback: PlaybackConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load config from `path`, or return defaults if missing. Invalid TOML is an error.
    pub fn load(path: &Path) -> Result<Self, SoundScoutError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|e| SoundScoutError::Config(e.to_string()))?;
        toml::from_str(&raw).map_err(|e| SoundScoutError::Config(format!("invalid TOML: {e}")))
    }

    /// Write config to `path`, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), SoundScoutError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| SoundScoutError::Config(e.to_string()))?;
        }
        let s = toml::to_string_pretty(self)
            .map_err(|e| SoundScoutError::Config(format!("serialize: {e}")))?;
        fs::write(path, s).map_err(|e| SoundScoutError::Config(e.to_string()))?;
        Ok(())
    }
}

/// `~/.config/soundscout/`
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("soundscout")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_empty_scan_roots() {
        let c = AppConfig::default();
        assert!(c.general.scan_roots.is_empty());
    }

    #[test]
    fn default_config_has_800_peak_resolution() {
        let c = AppConfig::default();
        assert_eq!(c.indexing.peak_resolution, 800);
    }

    #[test]
    fn default_config_has_50_results_per_page() {
        let c = AppConfig::default();
        assert_eq!(c.search.results_per_page, 50);
    }

    #[test]
    fn default_config_watch_and_search_mode_lexical() {
        let c = AppConfig::default();
        assert!(!c.indexing.watch_scan_roots);
        assert!(!c.search.semantic_search);
        assert_eq!(c.search.default_search_mode, crate::db::models::SearchMode::Lexical);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("config.toml");
        let mut c = AppConfig::default();
        c.general.scan_roots.push(PathBuf::from("/tmp/audio"));
        c.save(&p).expect("save");
        let loaded = AppConfig::load(&p).expect("load");
        assert_eq!(loaded, c);
    }

    #[test]
    fn load_partial_toml_fills_defaults() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("c.toml");
        fs::write(&p, "[general]\nscan_roots = []\n").expect("write");
        let c = AppConfig::load(&p).expect("load");
        assert_eq!(c.indexing.peak_resolution, 800);
    }

    #[test]
    fn load_nonexistent_returns_defaults() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("nope.toml");
        let c = AppConfig::load(&p).expect("load");
        assert_eq!(c, AppConfig::default());
    }

    #[test]
    fn load_invalid_toml_returns_error() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("bad.toml");
        fs::write(&p, "[[[not toml").expect("write");
        assert!(AppConfig::load(&p).is_err());
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("a/b/c/config.toml");
        AppConfig::default().save(&p).expect("save");
        assert!(p.exists());
    }
}
