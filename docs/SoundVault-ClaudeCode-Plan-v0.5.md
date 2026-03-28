# SoundVault — Claude Code Implementation Plan

**Version:** 0.5  
**Date:** 2026-03-25  
**Tooling:** Claude Code (main orchestrator) + sub-agents via `task` tool  
**Companion to:** SoundVault Architecture Spec v0.4  
**Changelog:** v0.4 fixed second-audit. v0.5 final pass — dark-mode design tokens, keyboard shortcuts, parallel summary, remaining fixes.

---

## How to Use This Document

This plan is structured for **Claude Code** as the primary development tool. Each phase has a main prompt you paste into Claude Code. Within each phase, **parallelizable sub-tasks** are marked with `[PARALLEL]` — these are designed as self-contained units you can dispatch as sub-agents using Claude Code's `task` tool.

**Rules for AI-assisted development:**

1. **One file per agent.** Sub-agents must never edit the same file. Shared types go in a dedicated file that is written first by the orchestrator before spawning agents.
2. **Cargo.toml is NEVER edited by sub-agents.** All dependency additions happen in Phase 0 or in orchestrator merge steps.
3. **mod.rs files are NEVER edited by sub-agents.** All module declarations happen in Phase 0 or in orchestrator merge steps.
4. **Tests live next to code.** Every `.rs` file with logic has `#[cfg(test)] mod tests {}` at the bottom. Frontend tests go in `tests/` mirroring `src/lib/`.
5. **Compile-check after every agent completes.** Run `cargo check` / `npm run check` before moving to the next phase.
6. **No agent should use `unwrap()`.** All error handling uses `?` or explicit `.expect("invariant: ...")`.

---

## Phase 0 — Scaffold (Sequential, ~10 min)

> **This phase must complete before anything else. Run it yourself in Claude Code.**
> **CRITICAL: This phase installs ALL dependencies and declares ALL module structure.**
> **No subsequent agent should ever edit Cargo.toml or any mod.rs file.**

### Prompt for Claude Code:

```
Create a new Tauri 2.x project called "soundvault" with the SvelteKit TypeScript template.

Then apply ALL of the following configurations. This is the only phase where
Cargo.toml and mod.rs files are edited — no future agent may touch them.

=== 1. Rust dependencies (src-tauri/Cargo.toml) ===

Add workspace lints:
  [lints.rust]
  unsafe_code = "deny"
  [lints.clippy]
  all = "deny"
  pedantic = "warn"

Add ALL dependencies (every phase's needs, upfront):
  [dependencies]
  tauri = { version = "2", features = ["tray-icon"] }
  tauri-plugin-dialog = "2"
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  thiserror = "2"
  tracing = "0.1"
  tracing-subscriber = "0.3"
  toml = "0.8"
  dirs = "6"
  rusqlite = { version = "0.34", features = ["bundled", "column_decltype"] }
  r2d2 = "0.8"
  r2d2_sqlite = "0.27"
  walkdir = "2"
  symphonia = { version = "0.5", default-features = true }
  rayon = "1"
  lru = "0.12"

  [dev-dependencies]
  tempfile = "3"

NOTE on symphonia: version 0.5 with default-features = true enables all
built-in format readers and decoders (WAV, FLAC, MP3/MPEG, OGG/Vorbis, AIFF).
Do NOT specify individual feature flags like "wav" or "flac" — those are not
valid feature names in the symphonia meta-crate. Check docs.rs/symphonia if
the version has changed.

NOTE on r2d2_sqlite: version 0.27+ is required for rusqlite 0.34 compatibility.
Do NOT use r2d2_sqlite 0.25 — it depends on rusqlite 0.32 and will fail to compile.

=== 2. Tauri plugin registration ===

In src-tauri/src/main.rs or wherever the Tauri builder is configured,
register the dialog plugin:
  .plugin(tauri_plugin_dialog::init())

Create src-tauri/capabilities/default.json (Tauri 2 permission system):
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default"
  ]
}
Without this file, dialog:open will fail at runtime with a permission error.
Verify tauri.conf.json also lists "dialog" under plugins if needed.

=== 3. Rust module structure ===

Create ALL files below. Every file that is not a leaf module gets its
pub mod declarations NOW so that sub-agents never need to edit mod.rs.

src-tauri/src/
├── main.rs              (Tauri bootstrap)
├── lib.rs               (#![warn(missing_docs)], pub mod for all top-level modules)
├── error.rs             (SoundVaultError enum)
├── test_utils.rs        (#[cfg(test)] helper: write_test_wav)
├── config/
│   ├── mod.rs           (pub mod settings; pub use settings::AppConfig;)
│   └── settings.rs      (empty: // TODO Phase 1A)
├── db/
│   ├── mod.rs           (pub mod connection; pub mod migrations; pub mod models; pub mod queries;)
│   ├── connection.rs    (empty)
│   ├── migrations.rs    (empty)
│   ├── models.rs        (empty)
│   └── queries.rs       (empty)
├── indexer/
│   ├── mod.rs           (pub mod path_parser; pub mod scanner; pub mod metadata; pub mod peaks; pub mod pipeline;)
│   ├── path_parser.rs   (empty)
│   ├── scanner.rs       (empty)
│   ├── metadata.rs      (empty)
│   ├── peaks.rs         (empty)
│   └── pipeline.rs      (empty)
├── search/
│   ├── mod.rs           (pub mod query_builder; pub mod engine;)
│   ├── query_builder.rs (empty)
│   └── engine.rs        (empty)
├── audio/
│   ├── mod.rs           (pub mod decode_core; pub mod decoder; pub mod cache;)
│   ├── decode_core.rs   (empty)
│   ├── decoder.rs       (empty)
│   └── cache.rs         (empty)
└── commands/
    ├── mod.rs           (pub mod state; pub mod indexing; pub mod search; pub mod audio; pub mod user_data;)
    ├── state.rs         (empty)
    ├── indexing.rs       (empty)
    ├── search.rs        (empty)
    ├── audio.rs         (empty)
    └── user_data.rs     (empty)

lib.rs should contain:
  #![warn(missing_docs)]
  //! SoundVault — audio asset library manager.
  pub mod error;
  #[cfg(test)]
  pub mod test_utils;
  pub mod config;
  pub mod db;
  pub mod indexer;
  pub mod search;
  pub mod audio;
  pub mod commands;

Each empty file should contain a single line doc comment:
  //! TODO: Implemented in Phase N

=== 4. Error types ===

In src-tauri/src/error.rs, implement:

  use serde::Serialize;

  /// All error types for SoundVault.
  #[derive(Debug, thiserror::Error)]
  pub enum SoundVaultError {
      /// Database operation failed.
      #[error("Database error: {0}")]
      Database(#[from] rusqlite::Error),

      /// File system operation failed.
      #[error("IO error: {0}")]
      Io(#[from] std::io::Error),

      /// Audio file could not be decoded.
      #[error("Audio decode error: {0}")]
      AudioDecode(String),

      /// Configuration file could not be read or written.
      #[error("Config error: {0}")]
      Config(String),

      /// Scan was cancelled by user.
      #[error("Indexing cancelled")]
      Cancelled,

      /// A validation constraint was violated.
      #[error("Validation error: {0}")]
      Validation(String),

      /// Connection pool error (r2d2 timeout, exhaustion).
      #[error("Connection pool error: {0}")]
      Pool(String),
  }

  impl Serialize for SoundVaultError {
      fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
      where S: serde::Serializer {
          serializer.serialize_str(&self.to_string())
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn display_database_error() {
          let err = SoundVaultError::Database(
              rusqlite::Error::QueryReturnedNoRows
          );
          assert!(err.to_string().contains("Database error"));
      }

      #[test]
      fn display_config_error() {
          let err = SoundVaultError::Config("missing field".into());
          assert!(err.to_string().contains("missing field"));
      }

      #[test]
      fn display_cancelled() {
          let err = SoundVaultError::Cancelled;
          assert_eq!(err.to_string(), "Indexing cancelled");
      }

      #[test]
      fn io_error_converts() {
          let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
          let err: SoundVaultError = io_err.into();
          assert!(matches!(err, SoundVaultError::Io(_)));
      }

      #[test]
      fn serializes_to_string() {
          let err = SoundVaultError::Validation("bad input".into());
          let json = serde_json::to_string(&err).unwrap();
          assert!(json.contains("bad input"));
      }
  }

=== 5. Test utilities ===

In src-tauri/src/test_utils.rs, implement:

  //! Shared test helpers. Only compiled under #[cfg(test)].

  use std::io::Write;
  use std::path::Path;

  /// Writes a minimal valid WAV file for testing.
  /// Generates a sine wave at the given frequency.
  pub fn write_test_wav(
      path: &Path,
      sample_rate: u32,
      channels: u16,
      bits_per_sample: u16,
      num_samples: u32,
      frequency_hz: f32,
  ) -> std::io::Result<()> {
      // Build valid RIFF/WAV header + PCM data
      // 16-bit and 24-bit support
      // This function is used by multiple test modules across the project
      // ... (implement full WAV writer)
  }

  Add a basic test that generates a file and verifies it's valid.

=== 6. Frontend setup ===

In tsconfig.json: set "strict": true

In package.json, add/verify these dev dependencies:
  vitest, @testing-library/svelte, jsdom

Verify @tauri-apps/api is in dependencies (Tauri template usually includes it).

In vite.config.ts, add vitest configuration:
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts'],
  }

In package.json scripts:
  "test": "vitest run"
  "test:watch": "vitest"

Create these empty directories:
  src/lib/types/
  src/lib/stores/
  src/lib/ipc/
  src/lib/components/
  src/lib/utils/
  tests/
  tests/components/

=== 7. Verify ===

Run:
  cargo check   (must pass — all files exist, deps resolve)
  cargo test    (error.rs tests pass)
  npm run check (TypeScript clean)
  npm run test  (no tests yet, but vitest runs without error)
```

**Done gate:** `cargo check` succeeds with zero warnings for empty files, `cargo test` passes error.rs tests, `npm run check` clean, app window opens.

---

## Phase 1 — Foundation Layer (3 parallel agents)

> **All three agents work on independent files with zero overlap.**
> **Pre-requisite:** Phase 0 complete, `cargo check` passes.
> **REMINDER: No agent edits Cargo.toml or any mod.rs file.**

### Agent 1A: Configuration System `[PARALLEL]`

**Files this agent owns (and ONLY these files):**
- `src-tauri/src/config/settings.rs`

**Sub-agent prompt:**

```
You are working in the soundvault Tauri project at src-tauri/.

All dependencies are already in Cargo.toml. Do NOT edit Cargo.toml.
The module is already declared in config/mod.rs. Do NOT edit mod.rs.

Create src-tauri/src/config/settings.rs with:

1. AppConfig struct (Serialize, Deserialize, Debug, Clone, PartialEq) containing:
   - general: GeneralConfig {
       scan_roots: Vec<PathBuf> (default empty)
       // NO theme field — v1 is dark-only
     }
   - indexing: IndexConfig {
       parallel_workers: usize (default 0 = auto),
       peak_resolution: usize (default 200),
       skip_hidden: bool (default true)
     }
   - playback: PlaybackConfig {
       buffer_cache_count: usize (default 10),
       auto_play: bool (default true)
     }
   - search: SearchConfig {
       default_sort: String (default "relevance"),
       results_per_page: u32 (default 50)
     }

   Use #[serde(default)] on all sub-structs so that missing fields in
   existing config files don't cause deserialization errors.

2. AppConfig::default() with documented defaults
3. AppConfig::load(path: &Path) -> Result<Self, SoundVaultError>
   - If file doesn't exist, return defaults
   - If file exists, parse TOML and merge with defaults for missing fields
   - If TOML is invalid, return Config error
4. AppConfig::save(&self, path: &Path) -> Result<(), SoundVaultError>
   - Create parent directories if needed
5. pub fn config_dir() -> PathBuf returning ~/.config/soundvault/

Unit tests:
- default_config_has_empty_scan_roots
- default_config_has_200_peak_resolution
- default_config_has_50_results_per_page
- save_then_load_roundtrips (use tempfile::TempDir)
- load_partial_toml_fills_defaults
- load_nonexistent_returns_defaults
- load_invalid_toml_returns_error
- save_creates_parent_directories

Do NOT touch any files outside config/settings.rs.
Run `cargo test --lib config` to verify.
```

---

### Agent 1B: Database Schema & Connection `[PARALLEL]`

**Files this agent owns (and ONLY these files):**
- `src-tauri/src/db/connection.rs`
- `src-tauri/src/db/migrations.rs`

**Sub-agent prompt:**

```
You are working in the soundvault Tauri project at src-tauri/.

All dependencies are already in Cargo.toml (rusqlite, r2d2, r2d2_sqlite).
All modules are already declared in db/mod.rs. Do NOT edit Cargo.toml or mod.rs.

Create src-tauri/src/db/connection.rs:

1. pub struct DbPool wrapping r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>
2. DbPool::new(path: &Path) -> Result<Self, SoundVaultError>
   - Creates file if not exists
   - Sets PRAGMA journal_mode=WAL
   - Sets PRAGMA foreign_keys=ON
   - Sets PRAGMA busy_timeout=5000
   - Runs migrations
   - Returns pool with max_size=4
3. DbPool::new_in_memory() -> Result<Self, SoundVaultError>
   - For tests. NOTE: WAL mode is not available for in-memory databases
     (SQLite returns "memory" instead of "wal"). This is expected.
4. DbPool::get(&self) -> Result<r2d2::PooledConnection<...>, SoundVaultError>
   - Map r2d2 errors via: .map_err(|e| SoundVaultError::Pool(e.to_string()))

Create src-tauri/src/db/migrations.rs:

1. pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), SoundVaultError>
   - Reads PRAGMA user_version
   - Applies each migration whose version > current
   - Updates user_version after each
2. Migrations are embedded as const &str, not read from files at runtime

Embed migration v001_initial:

CREATE TABLE IF NOT EXISTS assets (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    filename    TEXT NOT NULL,
    extension   TEXT NOT NULL,
    folder      TEXT NOT NULL,
    duration_ms INTEGER,
    sample_rate INTEGER,
    channels    INTEGER,
    bit_depth   INTEGER,
    file_size   INTEGER NOT NULL,
    category    TEXT,
    publisher   TEXT,
    favorite    INTEGER NOT NULL DEFAULT 0,
    rating      INTEGER NOT NULL DEFAULT 0 CHECK(rating >= 0 AND rating <= 5),
    notes       TEXT,
    play_count  INTEGER NOT NULL DEFAULT 0,
    modified_at INTEGER NOT NULL,
    indexed_at  INTEGER NOT NULL,
    peaks       BLOB
);

CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    filename, folder, category, publisher, notes,
    content='assets', content_rowid='id', tokenize='trigram'
);

-- FTS sync triggers (INSERT, DELETE, UPDATE)
-- Same as architecture spec.

CREATE INDEX IF NOT EXISTS idx_assets_folder ON assets(folder);
CREATE INDEX IF NOT EXISTS idx_assets_duration ON assets(duration_ms);
CREATE INDEX IF NOT EXISTS idx_assets_sample_rate ON assets(sample_rate);
CREATE INDEX IF NOT EXISTS idx_assets_favorite ON assets(favorite) WHERE favorite = 1;
CREATE INDEX IF NOT EXISTS idx_assets_extension ON assets(extension);

NOTE: peaks column is BLOB (binary f32 pairs), NOT TEXT/JSON.

Embed migration v002_tags_collections:

CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE IF NOT EXISTS asset_tags (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
);
CREATE TABLE IF NOT EXISTS collections (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id      INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    sort_order    INTEGER DEFAULT 0,
    PRIMARY KEY (collection_id, asset_id)
);

Unit tests in connection.rs:
- new_in_memory_creates_tables (query assets table)
- migrations_are_idempotent (run twice, no error)
- wal_mode_enabled_for_file_db (use tempfile, verify PRAGMA journal_mode = "wal")
- foreign_keys_are_enforced (PRAGMA foreign_keys = 1)
- user_version_matches_latest_migration (should be 2)

Unit tests in migrations.rs:
- v001_creates_assets_table
- v001_creates_fts_table
- v002_creates_tags_table
- v002_creates_collections_table
- migration_skips_already_applied

Do NOT touch any files outside db/connection.rs and db/migrations.rs.
Run `cargo test --lib db` to verify.
```

---

### Agent 1C: Shared Type Definitions + Frontend Utils `[PARALLEL]`

**Files this agent owns (and ONLY these files):**
- `src-tauri/src/db/models.rs`
- `src/lib/types/asset.ts`
- `src/lib/types/search.ts`
- `src/lib/types/player.ts`
- `src/lib/types/index.ts`
- `src/lib/utils/format.ts`
- `tests/format.test.ts`

**Sub-agent prompt:**

```
You are working in the soundvault project. You will create BOTH Rust models
and their TypeScript mirrors.

All modules are already declared. Do NOT edit any mod.rs or Cargo.toml.

=== Rust models (src-tauri/src/db/models.rs) ===

All structs derive Debug, Clone, Serialize, Deserialize.

1. pub struct Asset — all fields from the schema:
   id: i64, path: String, filename: String, extension: String, folder: String,
   duration_ms: Option<i64>, sample_rate: Option<i32>, channels: Option<i32>,
   bit_depth: Option<i32>, file_size: i64, category: Option<String>,
   publisher: Option<String>, favorite: bool, rating: u8, notes: Option<String>,
   play_count: i64

   NOTE: peaks is NOT included in Asset — it's fetched separately via get_peaks command.

2. pub struct NewAsset — for inserting into DB:
   path: String, filename: String, extension: String, folder: String,
   duration_ms: Option<i64>, sample_rate: Option<i32>, channels: Option<i32>,
   bit_depth: Option<i32>, file_size: i64, category: Option<String>,
   publisher: Option<String>, modified_at: i64, indexed_at: i64,
   peaks: Option<Vec<u8>>    ← binary f32 pairs, NOT JSON

3. pub struct SearchQuery with Default:
   text: String, extensions: Option<Vec<String>>, duration_min: Option<i64>,
   duration_max: Option<i64>, sample_rates: Option<Vec<i32>>, channels: Option<i32>,
   favorites_only: bool (default false), tags: Option<Vec<String>>,
   publisher: Option<String>,
   sort_by: SortField (enum: Relevance/Name/Duration/Date, default Relevance),
   sort_dir: SortDirection (enum: Asc/Desc, default Desc),
   offset: u32 (default 0), limit: u32 (default 50)

4. pub struct SearchResults { pub assets: Vec<Asset>, pub total: u64, pub offset: u32 }

5. pub struct ScanProgress { pub scanned: u64, pub total: u64, pub current_file: String, pub phase: ScanPhase }
   pub enum ScanPhase { Enumerating, Extracting, Indexing, Complete }

6. pub struct ScanStats { pub files_indexed: u64, pub files_skipped: u64, pub files_missing: u64, pub errors: u64, pub duration_secs: f64 }

7. pub struct AudioMetadata { pub duration_ms: Option<i64>, pub sample_rate: Option<i32>, pub channels: Option<i32>, pub bit_depth: Option<i32> }

8. pub struct PathMetadata { pub filename: String, pub extension: String, pub folder: String, pub publisher: Option<String>, pub category: Option<String> }

9. pub struct PcmData { pub samples: Vec<f32>, pub sample_rate: u32, pub channels: u16 }

10. pub struct FilterOptions { pub extensions: Vec<String>, pub sample_rates: Vec<i32>, pub min_duration_ms: i64, pub max_duration_ms: i64, pub publishers: Vec<String> }

11. pub struct FolderNode { pub name: String, pub path: String, pub count: u64, pub children: Vec<FolderNode> }

12. pub struct Tag { pub id: i64, pub name: String }
    pub struct TagWithCount { pub id: i64, pub name: String, pub count: u64 }

Unit tests in models.rs:
- search_query_default_has_limit_50
- search_query_default_has_relevance_sort
- new_asset_serializes_to_json (verify round-trip)
- scan_progress_serializes_correctly

=== TypeScript types ===

src/lib/types/asset.ts:
  export interface Asset { ... } (mirror all fields, number for i64/i32/u8, string | null for Option<String>)
  export interface NewAsset { ... }
  export interface FolderNode { name: string; path: string; count: number; children: FolderNode[]; }
  export interface Tag { id: number; name: string; }
  export interface TagWithCount extends Tag { count: number; }

src/lib/types/search.ts:
  export interface SearchQuery { ... } with defaults as comments
  export interface SearchResults { ... }
  export interface FilterOptions { ... }
  export type SortField = 'relevance' | 'name' | 'duration' | 'date';
  export type SortDirection = 'asc' | 'desc';

src/lib/types/player.ts:
  export interface PcmData { samples: Float32Array; sampleRate: number; channels: number; }
  export interface ScanProgress { scanned: number; total: number; currentFile: string; phase: ScanPhase; }
  export type ScanPhase = 'enumerating' | 'extracting' | 'indexing' | 'complete';
  export interface ScanStats { ... }

src/lib/types/index.ts:
  Re-export everything from the above files.

=== Frontend utils ===

src/lib/utils/format.ts:
  export function formatDuration(ms: number | null | undefined): string
    - null/undefined → '—'
    - < 60s → '0:02.5'
    - >= 60s → '2:05.0'
    - >= 3600s → '1:01:01.0'
  export function formatFileSize(bytes: number): string
    - B / KB / MB / GB with 1 decimal
  export function formatSampleRate(rate: number): string
    - 44100 → '44.1 kHz', 48000 → '48 kHz'

tests/format.test.ts:
  Test all formatDuration cases (0, 500, 2500, 125000, 3661000, null, undefined)
  Test formatFileSize (500, 1536, 5242880, 1073741824)
  Test formatSampleRate (44100, 48000, 96000, 22050)

Do NOT edit mod.rs, Cargo.toml, or any other files.
Run `npx vitest run` to verify frontend tests.
Run `cargo test --lib db::models` to verify Rust tests.
```

---

### After Phase 1 — Orchestrator verification

```
Run `cargo check` and `cargo test` to verify everything compiles together.
All mod.rs files were set up in Phase 0, so no wiring needed.
Run `npm run test` to verify frontend tests pass.
```

---

## Phase 2 — Indexer Core (3 parallel agents)

> **Pre-requisite:** Phase 1 complete, `cargo check` + `cargo test` all green.
> **REMINDER: No agent edits Cargo.toml or any mod.rs file.**

### Agent 2A: Path Parser `[PARALLEL]`

**Files this agent owns:**
- `src-tauri/src/indexer/path_parser.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/indexer/.
All dependencies and module declarations are already set up. Do NOT edit
Cargo.toml or mod.rs.

Create path_parser.rs:

pub fn parse_path(root: &Path, file_path: &Path) -> PathMetadata

Import PathMetadata from crate::db::models.

Logic:
1. Strip the root prefix from file_path to get the relative path
2. Split into components
3. filename = file stem (without extension)
4. extension = file extension lowercase
5. folder = parent directory as string
6. publisher = first component after root (if exists)
7. category = components between publisher and filename, joined with " > " (if any)
   e.g. root=/audio, file=/audio/Boom/Impacts/Metal/hit.wav → category="Impacts > Metal"

Handle edge cases:
- File directly in root → publisher=None, category=None
- File one level deep → publisher=Some, category=None
- Non-UTF8 paths → use to_string_lossy()
- Files with multiple dots (e.g. "sfx.v2.wav") → extension is last segment only

Unit tests (exhaustive):
- extracts_publisher_from_first_segment
- extracts_category_from_intermediate_segments
- file_directly_in_root_has_no_publisher
- file_one_level_deep_has_publisher_no_category
- deeply_nested_path_joins_all_middle_segments
- filename_strips_extension
- extension_is_lowercase
- handles_non_ascii_paths (use "Ström Sounds/Björk.wav")
- handles_spaces_and_parens (use "Sound (Ideas)/SFX - Vol.1/bang!_01.wav")
- handles_multiple_dots_in_filename (use "ambience.v2.final.wav" → ext="wav")
- handles_no_extension (edge case, extension="" )

Do NOT modify any other files. Run `cargo test --lib indexer::path_parser`.
```

---

### Agent 2B: Directory Scanner `[PARALLEL]`

**Files this agent owns:**
- `src-tauri/src/indexer/scanner.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/indexer/.
walkdir is already in Cargo.toml. Do NOT edit Cargo.toml or mod.rs.

Create scanner.rs:

pub struct ScannedFile {
    pub path: PathBuf,
    pub filename: String,
    pub extension: String,
    pub file_size: u64,
    pub modified_at: i64,  // unix timestamp
}

const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "mp3", "ogg", "aiff", "aif"];

pub fn scan_directory(root: &Path) -> Result<Vec<ScannedFile>, SoundVaultError>

Logic:
1. Use walkdir::WalkDir to recurse
2. Skip hidden directories (name starts with '.') using filter_entry
3. For each file, check extension (case-insensitive) against AUDIO_EXTENSIONS
4. Read metadata for file_size and modified time
5. Convert modified time to unix timestamp (i64)
6. Collect into Vec, sorted by path for deterministic output

Also implement:
pub fn scan_directory_with_progress(
    root: &Path,
    progress: &dyn Fn(u64),
) -> Result<Vec<ScannedFile>, SoundVaultError>

Unit tests using tempfile::TempDir (create real files on disk):
- finds_all_audio_files (create wav, flac, mp3 → finds 3)
- skips_non_audio_extensions (create .txt, .jpg → not in results)
- skips_hidden_directories (create .hidden/secret.wav → not found)
- returns_correct_file_size (write known bytes, verify size)
- returns_nonzero_modified_at
- empty_directory_returns_empty_vec
- nonexistent_root_returns_error
- recognizes_all_supported_extensions (one file per ext, finds 6)
- extension_matching_is_case_insensitive (create TEST.WAV, TEST.Flac → found)
- results_are_sorted_by_path

Do NOT modify any other files. Run `cargo test --lib indexer::scanner`.
```

---

### Agent 2C: Audio Metadata Extraction + Test Fixtures `[PARALLEL]`

**Files this agent owns:**
- `src-tauri/src/indexer/metadata.rs`
- `src-tauri/src/test_utils.rs` (EXPAND the existing stub from Phase 0)

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/.
symphonia is already in Cargo.toml with default-features = true.
Do NOT edit Cargo.toml or any mod.rs.

First, EXPAND src-tauri/src/test_utils.rs (it has a stub from Phase 0).

Implement the full write_test_wav function:
pub fn write_test_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,  // 16 or 24
    num_samples: u32,
    frequency_hz: f32,     // 0.0 for silence
) -> std::io::Result<()>

This builds a complete valid RIFF/WAV file from raw bytes (NOT using symphonia).
Must support 16-bit and 24-bit PCM, mono and stereo.
For stereo: duplicate mono signal to both channels.

Add tests in test_utils.rs:
- generates_valid_wav_file (write file, verify size matches expected)
- generates_silence_when_frequency_zero
- generates_stereo_file

Now create src-tauri/src/indexer/metadata.rs:

pub fn extract_metadata(path: &Path) -> Result<AudioMetadata, SoundVaultError>

Import AudioMetadata from crate::db::models.

Logic:
1. Open file with symphonia::default::get_probe()
2. Probe the format, get the default audio track
3. Read codec params: sample_rate, channels, bits_per_sample
4. Compute duration from n_frames and sample_rate, OR from the TimeBase if available
5. Return AudioMetadata with all fields as Option (graceful partial extraction)
6. On any symphonia error, return SoundVaultError::AudioDecode with description

Unit tests using write_test_wav from crate::test_utils:
- reads_wav_44100_mono_16bit (generate in test, verify all 4 fields)
- reads_wav_48000_stereo (verify channels=2)
- reads_wav_24bit (verify bit_depth=24)
- duration_is_approximately_correct (within ±50ms of expected 1s)
- corrupt_file_returns_error_not_panic (write random bytes to .wav)
- empty_file_returns_error
- truncated_header_no_panic (write only "RIFF\x00\x00\x00\x00WAVE")

Do NOT modify any other files except test_utils.rs and indexer/metadata.rs.
Run `cargo test --lib indexer::metadata` and `cargo test --lib test_utils`.
```

---

### After Phase 2 — Orchestrator verification

```
Run `cargo check` and `cargo test` — all indexer tests should pass.
All modules already declared in Phase 0, no wiring needed.
```

---

## Phase 3 — DB Queries, then Pipeline (SEQUENTIAL)

> **Pre-requisite:** Phase 2 merged and green.
> **IMPORTANT: Phase 3 is SEQUENTIAL, not parallel.**
> **3A must complete before 3B starts, because pipeline.rs calls query functions.**

### Step 3A: DB Query Functions

**Files this agent owns:**
- `src-tauri/src/db/queries.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/db/.
Do NOT edit Cargo.toml or mod.rs.

Create queries.rs with these functions. All take &rusqlite::Connection as first arg.
Import models from crate::db::models.

pub fn insert_asset_batch(conn: &Connection, assets: &[NewAsset]) -> Result<usize, SoundVaultError>
  - Uses a single transaction
  - Uses INSERT OR IGNORE to skip duplicates by path
  - Returns count of rows actually inserted
  - NOTE: peaks field is stored as BLOB via rusqlite's blob binding

pub fn update_asset_metadata(conn: &Connection, path: &str, asset: &NewAsset) -> Result<(), SoundVaultError>
  - UPDATE existing row by path — used for modified files during incremental re-scan
  - Updates: duration_ms, sample_rate, channels, bit_depth, file_size, category,
    publisher, modified_at, indexed_at, peaks

pub fn get_asset_by_id(conn: &Connection, id: i64) -> Result<Option<Asset>, SoundVaultError>

pub fn get_asset_by_path(conn: &Connection, path: &str) -> Result<Option<Asset>, SoundVaultError>

pub fn get_indexed_paths_with_mtime(conn: &Connection) -> Result<std::collections::HashMap<String, i64>, SoundVaultError>
  - Returns map of path → modified_at for all assets
  - Used by incremental re-scan to diff against filesystem

pub fn delete_assets_by_ids(conn: &Connection, ids: &[i64]) -> Result<usize, SoundVaultError>

pub fn update_favorite(conn: &Connection, id: i64, favorite: bool) -> Result<(), SoundVaultError>

pub fn update_rating(conn: &Connection, id: i64, rating: u8) -> Result<(), SoundVaultError>
  - Validate rating 0-5, return Validation error if out of range

pub fn update_peaks(conn: &Connection, id: i64, peaks: &[u8]) -> Result<(), SoundVaultError>

pub fn get_peaks(conn: &Connection, id: i64) -> Result<Option<Vec<u8>>, SoundVaultError>
  - Returns raw BLOB bytes

pub fn get_folder_tree(conn: &Connection) -> Result<Vec<(String, u64)>, SoundVaultError>
  - SELECT folder, COUNT(*) FROM assets GROUP BY folder ORDER BY folder

pub fn build_folder_tree(flat: &[(String, u64)]) -> Vec<FolderNode>
  - Converts flat (path, count) list into nested FolderNode tree
  - Import FolderNode from crate::db::models

pub fn get_publishers(conn: &Connection) -> Result<Vec<(String, u64)>, SoundVaultError>

pub fn get_filter_options(conn: &Connection) -> Result<FilterOptions, SoundVaultError>

pub fn get_assets_by_folder(conn: &Connection, folder: &str, limit: u32, offset: u32) -> Result<Vec<Asset>, SoundVaultError>

pub fn add_tag(conn: &Connection, asset_id: i64, tag_name: &str) -> Result<(), SoundVaultError>
  - INSERT OR IGNORE into tags table, then insert into asset_tags

pub fn remove_tag(conn: &Connection, asset_id: i64, tag_id: i64) -> Result<(), SoundVaultError>

pub fn get_tags_for_asset(conn: &Connection, asset_id: i64) -> Result<Vec<Tag>, SoundVaultError>

pub fn get_all_tags(conn: &Connection) -> Result<Vec<TagWithCount>, SoundVaultError>

Helper: implement a from_row helper for Asset (manual mapping from rusqlite::Row).

Unit tests using DbPool::new_in_memory():
- insert_and_retrieve_by_id
- insert_and_retrieve_by_path
- insert_batch_returns_correct_count
- insert_duplicate_path_is_ignored
- update_asset_metadata_changes_fields
- batch_insert_1000_under_500ms (performance)
- get_nonexistent_returns_none
- update_favorite_toggles
- update_rating_valid_range
- update_rating_rejects_over_5
- delete_by_ids_removes_correct_rows
- delete_by_ids_returns_count
- get_indexed_paths_returns_all_entries
- get_folder_tree_groups_correctly
- build_folder_tree_creates_nested_structure
- get_publishers_groups_correctly
- get_filter_options_reflects_data
- get_assets_by_folder_filters_correctly
- add_tag_creates_tag_and_association
- remove_tag_deletes_association
- get_tags_for_asset_returns_correct_tags
- get_all_tags_returns_counts

Do NOT modify any other files. Run `cargo test --lib db::queries`.
```

---

### Step 3B: Waveform Peaks + Pipeline Orchestrator (after 3A)

**Files this agent owns:**
- `src-tauri/src/indexer/peaks.rs`
- `src-tauri/src/indexer/pipeline.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/indexer/.
rayon is already in Cargo.toml. Do NOT edit Cargo.toml or mod.rs.

IMPORTANT: This step depends on db::queries being implemented (Step 3A).
If db::queries is not complete, this step will not compile.

--- Part 1: peaks.rs ---

pub fn compute_peaks(path: &Path, resolution: usize) -> Result<Vec<u8>, SoundVaultError>

Logic:
1. Open and fully decode audio file using crate::audio::decode_core::decode_samples
   (NOTE: decode_core may not exist yet — if so, implement decode inline here
   using symphonia directly. We'll refactor to shared code in Phase 4.)
2. If stereo, mixdown to mono: (L + R) / 2.0
3. Divide samples into `resolution` chunks
4. For each chunk, compute (min, max) pair as f32
5. Serialize as raw bytes: for each pair, write min as f32 le bytes, then max as f32 le bytes
6. Return Vec<u8> of length resolution * 2 * 4 (binary f32 pairs)
7. All values normalized to [-1.0, 1.0]

Also provide:
pub fn peaks_to_floats(blob: &[u8]) -> Vec<f32>
  - Converts BLOB back to Vec<f32> for sending to frontend

Unit tests (use crate::test_utils::write_test_wav to generate fixtures):
- peak_byte_count_matches_resolution (request 100 → get 800 bytes = 100*2*4)
- peaks_to_floats_roundtrip (compute peaks, convert to floats, verify range)
- peaks_are_in_valid_range (all floats between -1.0 and 1.0)
- silence_produces_near_zero_peaks (all abs < 0.01)
- sine_wave_has_significant_peaks (max > 0.5)
- resolution_of_one_returns_8_bytes
- corrupt_file_returns_error

--- Part 2: pipeline.rs ---

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct CancelHandle(Arc<AtomicBool>);
impl CancelHandle {
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

pub struct IndexPipeline {
    pool: DbPool,
    config: IndexConfig,   // from crate::config::settings
    cancelled: Arc<AtomicBool>,
}

impl IndexPipeline {
    pub fn new(pool: DbPool, config: IndexConfig) -> Self
    pub fn cancel_handle(&self) -> CancelHandle

    pub fn run(
        &self,
        root: &Path,
        progress: std::sync::mpsc::Sender<ScanProgress>,
    ) -> Result<ScanStats, SoundVaultError>
}

Run logic:
1. Call scanner::scan_directory(root) to enumerate files
2. Send ScanProgress with phase=Enumerating, total=file_count
3. Load existing indexed paths via db::queries::get_indexed_paths_with_mtime
4. Partition files into: new (not in DB), modified (mtime changed), unchanged (skip)
5. Detect missing files (in DB but not on disk for this root)
6. Use rayon::par_iter on new+modified files:
   a. Check cancelled flag at start of each file
   b. extract metadata via indexer::metadata::extract_metadata
   c. extract path metadata via indexer::path_parser::parse_path
   d. compute peaks via indexer::peaks::compute_peaks (config.peak_resolution)
   e. Set indexed_at = current unix timestamp (SystemTime::now())
   f. Build NewAsset with all fields, collect results into Vec
   g. Send ScanProgress updates via channel
7. For NEW files: batch insert via db::queries::insert_asset_batch (chunks of 500)
8. For MODIFIED files: update via db::queries::update_asset_metadata (one per file)
9. Delete missing files from DB via db::queries::delete_assets_by_ids
   (Immediate delete — user favorites/ratings on deleted files are lost.
   Soft-delete with a `missing` flag is a Phase 2 consideration.)
10. Return ScanStats

If cancelled flag is set, return SoundVaultError::Cancelled after current batch.

IMPORTANT: When sending progress via the channel, handle disconnected receivers
gracefully. Use `let _ = progress.send(...)` — if the receiver has dropped
(e.g. frontend navigated away), log a tracing::warn but continue indexing.
Do NOT let SendError propagate into rayon workers as this causes panics.

Unit tests (use crate::test_utils::write_test_wav for test files):
- full_scan_indexes_all_files (3 files → 3 in DB)
- incremental_scan_skips_unchanged (scan twice, second has 0 indexed)
- scan_detects_new_files_on_rescan
- modified_file_gets_updated_metadata (change file, rescan, verify updated)
- cancellation_stops_scan_early (cancel after 10ms on 100 files)
- corrupt_files_counted_as_errors (add garbage .wav, verify stats.errors=1)
- progress_events_are_emitted (collect from receiver, verify non-empty)
- missing_files_are_detected (delete file after first scan, rescan)

Do NOT modify any other files. Run `cargo test --lib indexer::peaks indexer::pipeline`.
```

---

## Phase 4 — Search Engine + Audio Layer (2 parallel agents)

> **Pre-requisite:** Phase 3 complete and green.

### Agent 4A: Search Engine `[PARALLEL]`

**Files this agent owns:**
- `src-tauri/src/search/query_builder.rs`
- `src-tauri/src/search/engine.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/search/.
Do NOT edit Cargo.toml or mod.rs.

Create query_builder.rs:

CRITICAL: The FTS5 trigram tokenizer does NOT support the * prefix operator.
Do NOT append * to search tokens. Trigram gives substring matching inherently.

pub fn sanitize_fts_query(input: &str) -> String
  - Remove FTS5 operators: (, ), :, ^, *, ", AND, OR, NOT (as standalone words)
  - Trim whitespace
  - Split on whitespace, keep tokens as-is (NO * suffix — trigram handles substring)
  - Join with spaces (FTS5 implicit AND with trigram)
  - Empty input → return empty string

pub fn build_search_sql(query: &SearchQuery) -> (String, Vec<rusqlite::types::Value>)
  - If query.text is non-empty after sanitization AND length >= 3:
    JOIN with assets_fts, use MATCH (trigram needs 3+ chars)
  - If query.text is 1-2 characters: use LIKE '%query%' on filename column
    instead of FTS5 (trigram produces zero trigrams for < 3 chars)
  - If query.text is empty: query assets table directly
  - Add WHERE clauses for each active filter
  - Use parameterized queries (?) for all values
  - ORDER BY: Relevance uses fts5 rank function, others use column sort
  - Add LIMIT ? OFFSET ?
  - Return the SQL string and parameter vector

Also build a count query variant for total results.

Create engine.rs:

pub fn execute_search(conn: &Connection, query: &SearchQuery) -> Result<SearchResults, SoundVaultError>

Unit tests in query_builder.rs:
- sanitize_removes_fts_operators (parens, colons, carets, asterisks, quotes)
- sanitize_does_not_add_prefix_star (trigram handles this)
- sanitize_empty_returns_empty
- sanitize_handles_multiple_spaces
- sanitize_removes_standalone_AND_OR_NOT
- build_sql_text_3plus_chars_uses_fts
- build_sql_text_1_2_chars_uses_like_fallback
- build_sql_empty_text_skips_fts
- build_sql_with_extension_filter
- build_sql_with_duration_range
- build_sql_with_favorites
- build_sql_pagination

Integration tests in engine.rs (use DbPool::new_in_memory + insert test data):
- search_finds_by_filename
- search_finds_by_substring (trigram: "footst" matches "Footstep")
- search_short_query_uses_like_fallback (1-2 char query still returns results)
- search_finds_by_category
- search_finds_by_publisher
- search_with_extension_filter_narrows
- search_with_duration_filter
- search_favorites_only
- search_returns_correct_total
- search_pagination_works (offset=10, limit=5 on 20 results)
- search_empty_query_returns_all
- search_no_results_returns_empty
- search_performance_10k_under_50ms

Do NOT modify any other files. Run `cargo test --lib search`.
```

---

### Agent 4B: Shared Decode Core + Audio Decoder + Cache `[PARALLEL]`

**Files this agent owns:**
- `src-tauri/src/audio/decode_core.rs`
- `src-tauri/src/audio/decoder.rs`
- `src-tauri/src/audio/cache.rs`

**Sub-agent prompt:**

```
You are working in soundvault at src-tauri/src/audio/.
symphonia and lru are already in Cargo.toml. Do NOT edit Cargo.toml or mod.rs.

=== decode_core.rs ===

This is the shared Symphonia decode loop used by both peaks.rs and decoder.rs.

pub fn decode_samples(path: &Path) -> Result<(Vec<f32>, u32, u16), SoundVaultError>
  - Opens file with symphonia probe
  - Gets default audio track
  - Creates decoder for that track
  - Loops: decode packets, collect all samples into Vec<f32>
  - Normalizes samples to [-1.0, 1.0] range
  - Returns (samples, sample_rate, channels)
  - If stereo, samples are interleaved: [L0, R0, L1, R1, ...]
  - On error: return SoundVaultError::AudioDecode with description

Unit tests:
- decodes_wav_mono (verify sample_rate, channels=1, non-empty)
- decodes_wav_stereo (verify channels=2, samples.len() = frames * 2)
- samples_in_valid_range (all between -1.0 and 1.0)
- corrupt_file_returns_error
- empty_file_returns_error

=== decoder.rs ===

pub fn decode_to_pcm(path: &Path) -> Result<PcmData, SoundVaultError>
  - Calls decode_core::decode_samples
  - Wraps result in PcmData { samples, sample_rate, channels }
  - Import PcmData from crate::db::models

Unit tests:
- decode_returns_pcm_data (verify struct fields)
- pcm_data_has_correct_duration (samples.len() / sample_rate / channels ≈ expected)

=== cache.rs ===

Use the `lru` crate (already in Cargo.toml) for O(1) get/put.

use std::sync::{Arc, Mutex};

pub struct AudioCache {
    inner: Mutex<lru::LruCache<i64, Arc<PcmData>>>,
}

impl AudioCache {
    pub fn new(capacity: usize) -> Self
    pub fn get(&self, asset_id: i64) -> Option<Arc<PcmData>>
    pub fn insert(&self, asset_id: i64, data: PcmData) -> Arc<PcmData>
    pub fn get_or_decode(&self, asset_id: i64, path: &Path) -> Result<Arc<PcmData>, SoundVaultError>
      - Check cache first, decode if miss, insert and return
    pub fn len(&self) -> usize
    pub fn clear(&self)
}

Unit tests:
- cache_hit_returns_same_arc (Arc::ptr_eq)
- cache_miss_triggers_decode (use write_test_wav fixture)
- cache_evicts_oldest_when_full (capacity=2, insert 3, first gone)
- cache_len_tracks_entries
- cache_clear_empties
- get_moves_entry_to_most_recent (insert A, B, get A, insert C → B evicted not A)

Do NOT modify any other files. Run `cargo test --lib audio`.
```

---

### After Phase 4 — Orchestrator step

```
IMPORTANT: Now refactor indexer/peaks.rs to use audio::decode_core::decode_samples
instead of its inline Symphonia decode. This eliminates the duplicated decode logic.

Specifically:
1. In peaks.rs, replace the inline Symphonia decode logic (probe → format → decoder
   loop → sample collection) with a single call:
     let (samples, sample_rate, channels) = crate::audio::decode_core::decode_samples(path)?;
2. Remove all direct symphonia imports from peaks.rs (symphonia::core::*, etc.)
3. The stereo mixdown and peak computation logic stays in peaks.rs — only the
   raw decode is extracted.
4. Run `cargo test --lib indexer::peaks` to verify peaks still work with the new code path.

Run `cargo test` — all tests green.
```

---

## Phase 5 — Tauri Commands (Sequential)

> **This phase wires Rust logic to the IPC layer. Must be sequential because all
> commands register in the same Tauri builder and touch main.rs.**

### Prompt for Claude Code:

```
Create the Tauri command handlers that bridge the frontend to the backend.
These are thin wrappers — no business logic, just call into the library modules.

Do NOT edit Cargo.toml (tauri-plugin-dialog already added in Phase 0).

Create src-tauri/src/commands/state.rs:
  use std::sync::{Mutex, RwLock};
  use crate::audio::cache::AudioCache;
  use crate::config::AppConfig;
  use crate::db::connection::DbPool;
  use crate::indexer::pipeline::CancelHandle;

  pub struct AppState {
      pub pool: DbPool,
      pub config: RwLock<AppConfig>,
      pub scan_handle: Mutex<Option<CancelHandle>>,
      pub audio_cache: AudioCache,
  }

Create src-tauri/src/commands/indexing.rs:
  #[tauri::command]
  pub async fn start_scan(state: State<'_, AppState>, app: AppHandle) -> Result<(), String>
    - Reads scan_roots from config
    - For each root, spawns IndexPipeline on tokio::task::spawn_blocking
    - Emits "scan:progress" events to frontend via app.emit
    - Stores CancelHandle in state.scan_handle
    - On completion, emits "scan:complete"
    - Maps errors via .map_err(|e| e.to_string())

  #[tauri::command]
  pub async fn cancel_scan(state: State<'_, AppState>) -> Result<(), String>

Create src-tauri/src/commands/search.rs:
  #[tauri::command]
  pub async fn search(query: SearchQuery, state: State<'_, AppState>) -> Result<SearchResults, String>

  #[tauri::command]
  pub async fn get_filter_options(state: State<'_, AppState>) -> Result<FilterOptions, String>

  #[tauri::command]
  pub async fn browse_folder(folder: String, limit: u32, offset: u32, state: State<'_, AppState>) -> Result<Vec<Asset>, String>
    - Calls db::queries::get_assets_by_folder

  #[tauri::command]
  pub async fn get_folder_tree(state: State<'_, AppState>) -> Result<Vec<FolderNode>, String>
    - Calls db::queries::get_folder_tree then db::queries::build_folder_tree

Create src-tauri/src/commands/audio.rs:
  #[tauri::command]
  pub async fn get_audio_data(asset_id: i64, state: State<'_, AppState>) -> Result<tauri::ipc::Response, String>
    - Looks up asset path from DB
    - Uses audio_cache.get_or_decode
    - Converts PcmData samples to raw f32 le bytes
    - Returns as tauri::ipc::Response::new(bytes) for binary transfer (NOT JSON)

  #[tauri::command]
  pub async fn get_peaks(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<f32>, String>
    - Reads peaks BLOB from DB via db::queries::get_peaks
    - Converts via indexer::peaks::peaks_to_floats

Create src-tauri/src/commands/user_data.rs:
  #[tauri::command] pub async fn toggle_favorite(asset_id: i64, state: ...) -> Result<bool, String>
  #[tauri::command] pub async fn set_rating(asset_id: i64, rating: u8, state: ...) -> Result<(), String>
  #[tauri::command] pub async fn add_tag(asset_id: i64, tag_name: String, state: ...) -> Result<(), String>
  #[tauri::command] pub async fn remove_tag(asset_id: i64, tag_id: i64, state: ...) -> Result<(), String>
  #[tauri::command] pub async fn get_tags(asset_id: i64, state: ...) -> Result<Vec<Tag>, String>
  #[tauri::command] pub async fn get_all_tags(state: ...) -> Result<Vec<TagWithCount>, String>
  #[tauri::command] pub async fn get_config(state: ...) -> Result<AppConfig, String>
  #[tauri::command] pub async fn update_config(config: AppConfig, state: ...) -> Result<(), String>
  #[tauri::command] pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String>
    - Uses tauri_plugin_dialog::DialogExt to open native folder picker

Update src-tauri/src/main.rs:
  - Initialize tracing subscriber
  - Load config from config_dir()
  - Create DbPool::new(config_dir/soundvault.db)
  - Create AppState
  - Register ALL commands with tauri::Builder
  - .plugin(tauri_plugin_dialog::init())
  - .manage(app_state)
  - .invoke_handler(tauri::generate_handler![...all commands...])

Verify: `cargo build` succeeds.
```

---

## Phase 6 — Frontend (4 parallel agents + pre-step)

> **Pre-requisite:** Phase 5 complete, `cargo build` succeeds.
> **All agents work on separate component files. Shared stores are created first.**

### Pre-step: Create stores and IPC layer (sequential, do this first)

```
Create these files before spawning frontend agents:

src/lib/ipc/index.ts:
  Typed wrappers for ALL Tauri commands using invoke<T>() from @tauri-apps/api/core.
  Every function has explicit return type. Example:
    export async function search(query: SearchQuery): Promise<SearchResults> { ... }
    export async function getAudioData(assetId: number): Promise<ArrayBuffer> { ... }
      NOTE: get_audio_data returns binary via Tauri IPC, use invoke with responseType
    export async function startScan(): Promise<void> { ... }
    export async function pickDirectory(): Promise<string | null> { ... }
  etc.

src/lib/stores/searchStore.ts:
  Writable store with: { query: SearchQuery, results: Asset[], total: number, loading: boolean }
  Methods: search(text), setFilter(key, value), nextPage(), reset()
  Debounces search calls by 250ms INSIDE the store (components call immediately)
  Calls ipc.search(query)

src/lib/stores/playerStore.ts:
  Writable store with: { currentAsset: Asset | null, isPlaying: boolean, currentTime: number, duration: number, peaks: number[] }
  Methods: playAsset(asset), pause(), resume(), seek(position), stop()
  playAsset(asset) must:
    1. Call ipc.getAudioData(asset.id) to get raw PCM ArrayBuffer
    2. Call ipc.getPeaks(asset.id) to get waveform data
    3. Pass asset.sample_rate and asset.channels to AudioPlayer.load()
       (the binary PCM has NO header — metadata comes from the Asset record)

src/lib/stores/settingsStore.ts:
  Writable store with AppConfig
  Methods: load(), save(), addScanRoot(path), removeScanRoot(path)

src/lib/stores/toastStore.ts:
  Writable store with: { toasts: Toast[] }
  Toast = { id: string, message: string, type: 'error' | 'success' | 'info', timeout: number }
  Methods: show(message, type), dismiss(id)
  Auto-dismiss after timeout

Run `npm run check` to verify all stores compile.
```

---

### Agent 6A: Search Bar + Filter Bar + Toast `[PARALLEL]`

**Files this agent owns:**
- `src/lib/components/SearchBar.svelte`
- `src/lib/components/FilterBar.svelte`
- `src/lib/components/ToastContainer.svelte`
- `tests/components/SearchBar.test.ts`

**Sub-agent prompt:**

```
Create SearchBar.svelte:
- Input element with type="search", role="searchbox"
- Calls searchStore.search() on every input event (store handles debounce)
- Clear button (X) appears when text is non-empty
- Escape key clears input and blurs
- "/" key focuses input when not already focused (global listener)
- Style: use CSS variables from app.css — bg: var(--bg-input), border: var(--border),
  border on focus: var(--border-focus), text: var(--text-primary),
  placeholder: var(--text-muted), font: var(--font-mono). Full width, no rounded corners.

Create FilterBar.svelte:
- Row of filter controls below search bar
- Duration range: two number inputs (min/max seconds)
- Format: checkboxes for WAV, FLAC, MP3, OGG, AIFF
- Channels: radio buttons for Any / Mono / Stereo
- Favorites toggle: star button (var(--favorite) when active)
- Each control calls searchStore.setFilter(key, value)
- Collapsible (hidden by default, toggle button "Filters")
- Style: bg: var(--bg-surface), labels: var(--text-secondary), inputs: var(--bg-input)

Create ToastContainer.svelte:
- Fixed position bottom-right, z-index above everything
- Renders toasts from toastStore
- Each toast: bg: var(--bg-elevated), left border 3px solid (error: var(--error),
  success: var(--success), info: var(--info)), text: var(--text-primary)
- Click to dismiss, auto-dismiss via timeout
- Animated enter/exit (slide-in from right)

Tests (tests/components/SearchBar.test.ts):
- renders_search_input
- clears_on_escape
- shows_clear_button_when_has_text
- hides_clear_button_when_empty

Use @testing-library/svelte and vitest. Mock Tauri invoke.
Do NOT edit any store files or other component files.
```

---

### Agent 6B: Results List + Asset Row `[PARALLEL]`

**Files this agent owns:**
- `src/lib/components/ResultsList.svelte`
- `src/lib/components/AssetRow.svelte`
- `src/lib/utils/virtualScroll.ts`

**Sub-agent prompt:**

```
Create src/lib/utils/virtualScroll.ts:
- Export a function or Svelte action for virtual scrolling
- Given: items array, container height, row height (fixed 56px, matches --row-height token)
- Computes: visible range with 5-row buffer above/below
- Returns: visibleItems, totalHeight, offsetY
- Updates on scroll event

Create ResultsList.svelte:
- Scrollable container (flex: 1)
- Uses virtualScroll for visible-only rendering
- Shows "No results" when empty
- Shows result count: "1,234 results"
- Subscribes to searchStore

Create AssetRow.svelte:
- Props: asset (Asset type)
- Layout: [Play ▶] [Filename] [Category] [Duration] [Format badge] [★]
- Play → playerStore.playAsset(asset)
- Star → toggles favorite via IPC, catches errors → toastStore.show()
- Format badge: colored pill using CSS vars (--badge-wav, --badge-flac, --badge-mp3,
  --badge-ogg, --badge-aiff). Pill has 10% opacity bg with full-opacity text.
- Row height: var(--row-height) = 56px
- Filename: var(--text-primary), var(--font-size-lg), font-weight 500
- Category + metadata: var(--text-secondary), var(--font-size-sm)
- Hover: bg var(--bg-elevated)
- Active/playing: left border 2px solid var(--accent)
- Star inactive: var(--text-muted), active: var(--favorite)

Do NOT edit any store or other component files.
```

---

### Agent 6C: Player Bar + Waveform `[PARALLEL]`

**Files this agent owns:**
- `src/lib/components/PlayerBar.svelte`
- `src/lib/components/Waveform.svelte`
- `src/lib/utils/audioPlayer.ts`
- `tests/audioPlayer.test.ts`

**Sub-agent prompt:**

```
Create src/lib/utils/audioPlayer.ts:

export class AudioPlayer {
  private context: AudioContext | null = null;
  private sourceNode: AudioBufferSourceNode | null = null;
  private buffer: AudioBuffer | null = null;
  private startTime = 0;
  private pauseOffset = 0;
  isPlaying = false;
  duration = 0;

  get currentTime(): number — context.currentTime - startTime + pauseOffset

  load(arrayBuffer: ArrayBuffer, sampleRate: number, channels: number): void
    - Create AudioContext if needed
    - Create AudioBuffer from raw f32 bytes (binary, not JSON)
    - IMPORTANT: The ArrayBuffer from get_audio_data is raw interleaved f32 PCM
      with NO header. The sampleRate and channels parameters come from the Asset
      record (asset.sample_rate, asset.channels), NOT from the binary data itself.
      The playerStore.playAsset(asset) method must pass these values through.
    - Set duration

  play(): void — create source, connect, start at pauseOffset
  pause(): void — stop source, record pauseOffset
  seek(position: number): void — set pauseOffset, restart if playing
  stop(): void — stop source, reset pauseOffset
  destroy(): void — close AudioContext
}

Create Waveform.svelte:
- Props: peaks (number[]), currentTime, duration
- Canvas 2D: vertical bars (positive above center, negative below)
- Unplayed portion: var(--border) color (#2a2f3a)
- Played portion: var(--accent) color (#4a90d9)
- Playback position: 1px vertical line in var(--text-primary)
- Canvas bg: transparent (inherits from parent)
- Click → emit seek event (0-1 position)
- Responsive: ResizeObserver → redraw

Create PlayerBar.svelte:
- Fixed bottom bar, height: var(--player-height) = 80px
- Background: var(--bg-surface), top border: 1px solid var(--border)
- [Play/Pause] | [Waveform] | [Time] [Filename]
- Transport buttons: var(--text-secondary), hover: var(--text-primary)
- Time display: var(--text-muted), monospace
- Filename: var(--text-primary)
- Subscribes to playerStore
- Space = toggle, Left/Right = seek ±5s
- "No audio selected" in var(--text-muted) when empty

Tests (tests/audioPlayer.test.ts): Mock AudioContext, test:
- starts_in_stopped_state
- load_creates_buffer
- play_sets_is_playing
- pause_sets_not_playing
- seek_updates_offset
- stop_resets_to_zero

Do NOT edit store files or other components.
```

---

### Agent 6D: Sidebar + Folder Tree `[PARALLEL]`

**Files this agent owns:**
- `src/lib/components/Sidebar.svelte`
- `src/lib/components/FolderTree.svelte`
- `src/lib/components/TagList.svelte`
- `src/lib/components/CollectionList.svelte`

**Sub-agent prompt:**

```
Create FolderTree.svelte:
- Props: tree (FolderNode[]), onSelect callback
- Recursive tree with expand/collapse arrows
- File count badge per node
- Click → onSelect(path)

Create TagList.svelte:
- Props: tags (TagWithCount[]), onSelect callback
- Clickable tag pills with count

Create CollectionList.svelte:
- Props: collections[], onSelect callback
- List with folder icons

Create Sidebar.svelte:
- Three tabs: Folders / Tags / Collections
- Tab bar: bg var(--bg-surface), active tab underline var(--accent),
  inactive text var(--text-muted), active text var(--text-primary)
- Resizable width (drag handle, 200-400px range, default var(--sidebar-width))
- Background: var(--bg-surface), right border: 1px solid var(--border)
- Loads folder tree on mount via IPC

Do NOT edit store files or other components.
```

---

### Agent 6E: Settings Panel + Progress Modal `[PARALLEL]`

**Files this agent owns:**
- `src/lib/components/SettingsPanel.svelte`
- `src/lib/components/ProgressModal.svelte`

**Sub-agent prompt:**

```
Create SettingsPanel.svelte:
- Modal or slide-out panel, dark background (--bg-surface)
- Scan roots list with:
  - "Add folder" button → calls ipc.pickDirectory(), adds to list
  - "Remove" button per root
  - "Scan now" button → calls ipc.startScan()
- Peak resolution setting (slider: 100-500, default 200)
- Save button → calls ipc.updateConfig()
- v1 is dark-only — NO theme toggle

Create ProgressModal.svelte:
- Overlay: bg rgba(0,0,0,0.7), centered card bg var(--bg-surface),
  border: 1px solid var(--border), rounded corners 8px
- Listens to Tauri events using the EVENT api (NOT invoke):
    import { listen } from '@tauri-apps/api/event';
  This is a DIFFERENT import than invoke from @tauri-apps/api/core.
- On mount:
    const unlisten = await listen('scan:progress', (event) => {
      // event.payload is ScanProgress
    });
  Call unlisten() on component destroy (onDestroy or return from onMount).
- Also listen to 'scan:complete' event for ScanStats.
- Shows: progress bar, file count (scanned/total), current file name, phase label
- Cancel button → calls ipc.cancelScan()
- Auto-closes on scan:complete event
- Shows ScanStats summary briefly before closing

Do NOT edit store files or other components.
```

---

### After Phase 6 — Orchestrator assembly

```
Create src/routes/+page.svelte:
- Layout: Sidebar (left) | MainPanel (center) | PlayerBar (bottom)
- MainPanel contains SearchBar → FilterBar → ResultsList
- Settings gear icon → opens SettingsPanel
- Wire: sidebar folder select → searchStore.setFilter('folder', path)
- Wire: asset row play → playerStore.playAsset
- Load config on mount, check if scan_roots empty → show SettingsPanel
- ToastContainer rendered at root level

KEYBOARD SHORTCUT COORDINATION:
Add a global keydown handler in +page.svelte:
  - Only dispatch shortcuts when NO input/textarea element is focused
    (check document.activeElement?.tagName !== 'INPUT' && !== 'TEXTAREA')
  - "/" → focus search bar
  - "Escape" → blur current input, or close open modal
  - "Space" → toggle play/pause (when search bar NOT focused)
  - "f" → toggle favorite on selected/playing asset
  - "1"-"5" → set rating on selected/playing asset
  - ArrowLeft/ArrowRight → seek ±5s
  - ArrowUp/ArrowDown → navigate results list
  Each component also has local handlers (SearchBar handles its own Escape),
  but the global handler in +page.svelte coordinates to prevent conflicts.

Create src/app.css with the FULL design token system (dark-only, v1):

  @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600;700&display=swap');

  :root {
    /* Backgrounds */
    --bg-base:       #0d0f11;
    --bg-surface:    #151820;
    --bg-elevated:   #1c2028;
    --bg-input:      #1a1e26;

    /* Borders */
    --border:        #2a2f3a;
    --border-focus:  #4a90d9;

    /* Text */
    --text-primary:  #e2e4e8;
    --text-secondary:#8b8f98;
    --text-muted:    #555962;

    /* Accents */
    --accent:        #4a90d9;
    --accent-hover:  #5a9ee6;
    --favorite:      #e8b339;
    --error:         #d94a4a;
    --success:       #4ad97a;
    --info:          #4a90d9;

    /* Format badges */
    --badge-wav:     #4a90d9;
    --badge-flac:    #4ad97a;
    --badge-mp3:     #d9914a;
    --badge-ogg:     #9a4ad9;
    --badge-aiff:    #d94a7a;

    /* Typography */
    --font-mono:     'JetBrains Mono', 'Fira Code', monospace;
    --font-size-sm:  12px;
    --font-size-base:13px;
    --font-size-lg:  15px;
    --font-size-xl:  18px;

    /* Spacing */
    --spacing-xs:    4px;
    --spacing-sm:    8px;
    --spacing-md:    12px;
    --spacing-lg:    16px;
    --spacing-xl:    24px;

    /* Layout */
    --row-height:    56px;
    --sidebar-width: 260px;
    --player-height: 80px;
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  html, body {
    height: 100%;
    background: var(--bg-base);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: var(--font-size-base);
    overflow: hidden;
    user-select: none;
  }

  ::-webkit-scrollbar { width: 8px; }
  ::-webkit-scrollbar-track { background: var(--bg-base); }
  ::-webkit-scrollbar-thumb { background: var(--border); border-radius: 4px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--text-muted); }

  input, button { font-family: var(--font-mono); }

CRITICAL: Every frontend agent must use these CSS variables — no hardcoded
colors. Reference "var(--bg-surface)" not "#151820". This ensures visual
consistency across all components built by parallel agents.

Run `npm run check`, `npm run build`, `npm run test`.
```

---

## Phase 7 — Integration + First Run (Sequential)

```
1. Run `cargo tauri dev` to launch the app
2. Test first-run flow:
   - App opens → SettingsPanel shown (no scan roots)
   - Click "Add folder" → native picker opens
   - Select a small folder with audio files
   - Click "Scan now" → ProgressModal shows scan progress
   - Scan completes → results populate
   - Search works → type substring, results filter
   - Click result → audio plays with waveform
   - Favorite an asset → star toggles, persists after restart
   - Errors show as toast notifications
3. Fix any wiring issues
4. Test with ~1000 files from the Sonniss library
5. Profile memory: should be < 100MB idle
6. Profile search: should be < 50ms on 10k+ indexed files
```

---

## Parallel Execution Summary

```
Phase 0 ──────────────────── (sequential, ~10 min)
         │                    ALL deps, ALL mod.rs, test_utils stub
         │
Phase 1 ─┼─ 1A Config ──────┐
         ├─ 1B Database ─────┤ (3 parallel agents)
         └─ 1C Types/Utils ──┘
                              │ verify: cargo check + cargo test
Phase 2 ─┼─ 2A Path Parser ─┐
         ├─ 2B Scanner ──────┤ (3 parallel agents)
         └─ 2C Metadata ─────┘
                              │ verify: cargo check + cargo test
Phase 3 ── 3A DB Queries ────┐
                              │ (SEQUENTIAL — 3B depends on 3A)
         ── 3B Peaks+Pipeline─┘
                              │ verify: cargo test
Phase 4 ─┼─ 4A Search ──────┐
         └─ 4B Audio Layer ──┘ (2 parallel agents)
                              │ verify + refactor peaks→decode_core
Phase 5 ──────────────────── (sequential, Tauri commands + main.rs)
         │
Phase 6 ── Pre-step: stores ─┐ (sequential)
         ┌────────────────────┘
         ├─ 6A Search UI ───┐
         ├─ 6B Results List ─┤
         ├─ 6C Player ───────┤ (5 parallel agents)
         ├─ 6D Sidebar ──────┤
         └─ 6E Settings+Prog─┘
                              │ assemble +page.svelte + app.css + keyboard handler
Phase 7 ──────────────────── (sequential, integration test)
```

**Total agent dispatches:** 13 parallel + 5 sequential = 18 work units
**Critical path:** 8 sequential gates (agents run parallel within each phase)
**Estimated wall-clock with Claude Code:** 2-4 hours for full MVP
