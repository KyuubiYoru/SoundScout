# SoundVault — Architecture & Design Specification

**Version:** 0.4  
**Date:** 2026-03-25  
**Author:** Kyuubi + Claude  
**Status:** Final draft  
**Changelog:** v0.3 fixed second-audit issues. v0.4 final pass — dark-mode-only design system, design tokens, remaining consistency fixes.

---

## 1. Overview

SoundVault is a lightweight desktop application for indexing, searching, and previewing large audio asset libraries (100k+ files, 200GB+). Built for Linux-first with Tauri + Svelte, optimized for the Sonniss GDC audio bundle use case.

### 1.1 Problem Statement

The Sonniss GDC bundle ships ~200GB of royalty-free audio across thousands of nested folders. Files are organized by publisher and loosely by category, but there's no unified way to search, preview, or tag assets without manually browsing directories.

### 1.2 Goals

- Index 100k–500k audio files in under 10 minutes on NVMe SSD (under 30 minutes on spinning disk)
- Sub-100ms fuzzy search across filenames, paths, and user tags
- Instant audio preview with waveform display
- Minimal resource footprint (< 100MB RAM idle)
- Single binary, no external services

### 1.3 Non-Goals (v1)

- Cloud sync or multi-device
- Audio editing or processing
- DAW integration (future consideration)
- Mobile support

---

## 2. Tech Stack

| Layer        | Technology       | Rationale                                              |
|--------------|------------------|--------------------------------------------------------|
| Runtime      | Tauri 2.x        | Rust backend, small binary, native Linux support       |
| Frontend     | SvelteKit 2      | Default Tauri template, minimal JS bundle, reactive UI |
| Database     | SQLite 3 + FTS5  | Embedded, zero-config, proven at this scale            |
| Audio decode | Symphonia (Rust)  | Pure Rust, supports WAV/FLAC/MP3/OGG/AIFF             |
| Audio play   | Web Audio API    | Browser-native, low latency, waveform rendering        |
| Waveform     | Canvas 2D        | Fast rendering of pre-computed peak data               |

### 2.1 Why Not Electron?

Tauri uses the system webview (WebKitGTK on Linux) instead of bundling Chromium. For an app that needs to sit open alongside a DAW or game engine, the ~80MB vs ~300MB memory difference matters.

### 2.2 Why SQLite over a Vector DB?

For v1, pure text search covers 90%+ of use cases. Sonniss files are descriptively named (`Footstep_Concrete_Walk_01.wav`), and folder paths encode category info (`Impact Sounds/Metal/...`). FTS5 with trigram tokenizer handles substring matching inherently — any query like "footst" will match "Footstep" without needing prefix operators.

Vector search (CLAP embeddings) is planned for Phase 2 as an optional enhancement layer.

### 2.3 FTS5 Trigram Tokenizer Note

The trigram tokenizer breaks text into 3-character sequences, enabling substring matching out of the box. Unlike the default `unicode61` tokenizer, trigram does **not** support the `*` prefix operator. Queries are matched as-is — the search engine sanitizes input by removing FTS5 operators and passes bare terms. This gives "search as you type" behavior naturally.

**Minimum query length:** Trigram requires at least 3 characters to produce any trigrams. Queries of 1-2 characters will match nothing via FTS5. The search engine falls back to `LIKE '%query%'` on the filename column for short queries to maintain the "search as you type" experience.

---

## 3. Data Model

### 3.1 SQLite Schema

```sql
-- Core asset table
CREATE TABLE assets (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,  -- absolute path on disk
    filename    TEXT NOT NULL,         -- basename without extension
    extension   TEXT NOT NULL,         -- wav, mp3, flac, ogg, aiff
    folder      TEXT NOT NULL,         -- parent directory path
    
    -- Audio metadata (extracted during indexing)
    duration_ms INTEGER,              -- duration in milliseconds
    sample_rate INTEGER,              -- e.g. 44100, 48000, 96000
    channels    INTEGER,              -- 1 = mono, 2 = stereo
    bit_depth   INTEGER,              -- 16, 24, 32
    file_size   INTEGER NOT NULL,     -- bytes
    
    -- Derived metadata
    category    TEXT,                  -- inferred from path segments
    publisher   TEXT,                  -- top-level folder (Sonniss publisher)
    
    -- User data
    favorite    INTEGER DEFAULT 0,    -- boolean
    rating      INTEGER DEFAULT 0,    -- 0-5 stars
    notes       TEXT,                 -- user notes
    play_count  INTEGER DEFAULT 0,
    
    -- Indexing metadata
    modified_at INTEGER NOT NULL,     -- file mtime (unix epoch)
    indexed_at  INTEGER NOT NULL,     -- when we last scanned this file
    peaks       BLOB                  -- pre-computed waveform peaks (binary f32 pairs)
);

-- Full-text search virtual table
CREATE VIRTUAL TABLE assets_fts USING fts5(
    filename,
    folder,
    category,
    publisher,
    notes,
    content='assets',
    content_rowid='id',
    tokenize='trigram'   -- enables substring matching (no prefix * operator)
);

-- User-defined tags (many-to-many)
CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE asset_tags (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
);

-- Playlists / collections
CREATE TABLE collections (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id      INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    sort_order    INTEGER DEFAULT 0,
    PRIMARY KEY (collection_id, asset_id)
);

-- FTS sync triggers
CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, folder, category, publisher, notes)
    VALUES (new.id, new.filename, new.folder, new.category, new.publisher, new.notes);
END;

CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, filename, folder, category, publisher, notes)
    VALUES ('delete', old.id, old.filename, old.folder, old.category, old.publisher, old.notes);
END;

CREATE TRIGGER assets_au AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, filename, folder, category, publisher, notes)
    VALUES ('delete', old.id, old.filename, old.folder, old.category, old.publisher, old.notes);
    INSERT INTO assets_fts(rowid, filename, folder, category, publisher, notes)
    VALUES (new.id, new.filename, new.folder, new.category, new.publisher, new.notes);
END;

-- Indexes for common queries
CREATE INDEX idx_assets_folder ON assets(folder);
CREATE INDEX idx_assets_duration ON assets(duration_ms);
CREATE INDEX idx_assets_sample_rate ON assets(sample_rate);
CREATE INDEX idx_assets_favorite ON assets(favorite) WHERE favorite = 1;
CREATE INDEX idx_assets_extension ON assets(extension);
```

### 3.2 Waveform Peak Storage

Peaks are stored as a binary BLOB of `f32` min/max pairs, not JSON text. At 200 points resolution:

- 200 points × 2 (min+max) × 4 bytes = 1,600 bytes per file
- 200k files × 1,600 bytes = **320 MB** total

This is ~4× more compact than JSON and eliminates parse overhead on load. The frontend receives raw bytes and creates a `Float32Array` view directly.

### 3.3 Path-Based Metadata Extraction

Sonniss bundles follow a predictable structure:

```
GDC Audio/
├── Publisher Name/
│   ├── Library Name/
│   │   ├── Category/
│   │   │   └── Subcategory/
│   │   │       └── SFX_Name_Variation_01.wav
```

The indexer extracts:
- **publisher** — first directory under the scan root
- **category** — intermediate path segments joined with ` > ` (e.g. `Impacts > Metal > Light`)
- **filename** — stem parsed for keywords by splitting on `_`, `-`, spaces

---

## 4. Architecture

### 4.1 Component Diagram

```
┌─────────────────────────────────────────────────┐
│                  Svelte Frontend                 │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │
│  │ Search   │  │ Browser  │  │ Audio Player  │  │
│  │ Bar      │  │ / List   │  │ + Waveform    │  │
│  └────┬─────┘  └────┬─────┘  └───────┬───────┘  │
│       │              │                │          │
│       └──────────────┴────────────────┘          │
│                      │ Tauri IPC (invoke)        │
├──────────────────────┼───────────────────────────┤
│                  Rust Backend                     │
│                      │                           │
│  ┌──────────┐  ┌─────┴─────┐  ┌───────────────┐ │
│  │ Indexer   │  │ Query     │  │ Audio Decoder │ │
│  │ Service   │  │ Engine    │  │ (Symphonia)   │ │
│  └────┬──────┘  └─────┬─────┘  └───────┬───────┘│
│       │               │                │        │
│       └───────────────┼────────────────┘        │
│                       │                          │
│              ┌────────┴────────┐                 │
│              │  SQLite + FTS5  │                 │
│              └─────────────────┘                 │
└──────────────────────────────────────────────────┘
```

### 4.2 Tauri Commands (IPC API)

```rust
// === Indexing ===
#[tauri::command]
async fn start_scan(state: State<'_, AppState>, app: AppHandle) -> Result<(), String>;
// Fire-and-forget. Iterates all scan_roots from config.
// Emits "scan:progress" and "scan:complete" events via app.emit().

#[tauri::command]
async fn cancel_scan(state: State<'_, AppState>) -> Result<(), String>;

// === Search & Browse ===
#[tauri::command]
async fn search(query: SearchQuery, state: State<'_, AppState>) -> Result<SearchResults, String>;

#[tauri::command]
async fn browse_folder(folder: String, limit: u32, offset: u32, state: State<'_, AppState>) -> Result<Vec<Asset>, String>;

#[tauri::command]
async fn get_filter_options(state: State<'_, AppState>) -> Result<FilterOptions, String>;

#[tauri::command]
async fn get_folder_tree(state: State<'_, AppState>) -> Result<Vec<FolderNode>, String>;
// Returns nested tree structure built from flat folder paths.

// === Audio ===
#[tauri::command]
async fn get_audio_data(asset_id: i64, state: State<'_, AppState>) -> Result<tauri::ipc::Response, String>;
// Returns binary PCM data via Tauri's binary response channel (not JSON-serialized Vec<u8>).

#[tauri::command]
async fn get_peaks(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<f32>, String>;
// Reads peaks BLOB from DB, interprets as f32 slice.

// === User Data ===
#[tauri::command]
async fn toggle_favorite(asset_id: i64, state: State<'_, AppState>) -> Result<bool, String>;

#[tauri::command]
async fn set_rating(asset_id: i64, rating: u8, state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn add_tag(asset_id: i64, tag_name: String, state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn remove_tag(asset_id: i64, tag_id: i64, state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn get_tags_for_asset(asset_id: i64, state: State<'_, AppState>) -> Result<Vec<Tag>, String>;

#[tauri::command]
async fn get_all_tags(state: State<'_, AppState>) -> Result<Vec<TagWithCount>, String>;

// === Config ===
#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String>;

#[tauri::command]
async fn update_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn pick_directory(app: AppHandle) -> Result<Option<String>, String>;
// Opens native directory picker via tauri-plugin-dialog.
```

### 4.3 Search Query Model

```rust
struct SearchQuery {
    text: String,                    // FTS5 query (trigram, no prefix *)
    extensions: Option<Vec<String>>, // filter: ["wav", "flac"]
    duration_min: Option<i64>,       // filter: min ms
    duration_max: Option<i64>,       // filter: max ms
    sample_rates: Option<Vec<i32>>,  // filter: [44100, 48000]
    channels: Option<i32>,           // filter: 1 or 2
    favorites_only: bool,
    tags: Option<Vec<String>>,
    publisher: Option<String>,
    sort_by: SortField,              // relevance, name, duration, date
    sort_dir: SortDirection,
    offset: u32,
    limit: u32,                      // default 50, virtual scroll
}
```

### 4.4 Audio Data Transfer

Audio PCM data is sent from Rust to the frontend via Tauri's **binary IPC response** (`tauri::ipc::Response`), not JSON serialization. This avoids the ~4× size inflation of encoding `Vec<u8>` as a JSON number array. A 5-second stereo 48kHz file (~1.9MB PCM) transfers as ~1.9MB binary, not ~8MB JSON.

**Important:** The binary payload is raw interleaved f32 PCM with **no header**. The frontend must use `asset.sample_rate` and `asset.channels` from the Asset record (already available via the search results) to correctly interpret the bytes and create an `AudioBuffer`.

For files > 30 seconds, streaming via chunked responses is a Phase 2 optimization.

---

## 5. Indexing Pipeline

### 5.1 First-Run Scan

```
1. User adds scan roots via Settings UI (native directory picker)
2. start_scan iterates all configured scan_roots
3. Per root:
   a. Rust walkdir enumerates all files matching audio extensions
   b. Progress: emit file count to frontend via Tauri event
   c. For each file (parallelized across CPU cores via rayon):
      - stat() for file size and mtime
      - Open with Symphonia, read codec params (sample rate, channels, duration)
      - Compute waveform peaks (downsample to 200 points)
      - Extract publisher/category from path segments
      - Set indexed_at to current unix timestamp
      - Batch INSERT into SQLite (500 rows per transaction)
   d. Report completion + stats for this root
4. Emit scan:complete with aggregate ScanStats
```

### 5.2 Incremental Re-scan

```
1. Walk directory tree for each scan_root
2. Compare mtime against indexed_at
3. New files → INSERT with full metadata + peaks
4. Modified files → UPDATE metadata + peaks (not INSERT OR IGNORE)
5. Missing files → deleted from index during re-scan. User favorites/ratings on deleted files are lost. (A "soft delete" with a `missing` flag is a Phase 2 consideration.)
6. Handle removed scan_roots: files from removed roots are deleted on next re-scan
```

### 5.3 Performance Targets

| Metric               | Target          | Notes                          |
|----------------------|-----------------|--------------------------------|
| Files/second (index) | 500–1000        | Depends on disk I/O            |
| 200k files full scan | < 10 minutes    | NVMe SSD                       |
| 200k files full scan | < 30 minutes    | Spinning disk (conservative)   |
| 200k files re-scan   | < 60 seconds    | stat-only for unchanged files  |
| Search latency       | < 50ms          | FTS5 with trigram tokenizer    |
| DB size (no peaks)   | ~50–80 MB       | For 200k assets                |
| DB size (with peaks) | ~350–450 MB     | 200k assets × 1.6KB peaks     |

---

## 6. Frontend Design

### 6.1 Layout

```
┌──────────────────────────────────────────────────────┐
│  [Search Bar]  [Filters ▾]  [Sort ▾]     [Settings]  │
├──────────────┬───────────────────────────────────────┤
│              │                                        │
│  Folder      │  Results List (virtual scroll)         │
│  Tree        │  ┌──────────────────────────────────┐  │
│              │  │ ▶ Footstep_Concrete_01.wav       │  │
│  Publishers  │  │   Boom Library > Footsteps       │  │
│  ├─ Boom     │  │   0:02.3 · 48kHz · Stereo · WAV │  │
│  ├─ Sound    │  │   ★★★☆☆  #footstep #concrete    │  │
│  │  Ideas    │  ├──────────────────────────────────┤  │
│  ├─ ...      │  │ ▶ Metal_Impact_Heavy_05.wav      │  │
│              │  │   ...                             │  │
│  Tags        │  └──────────────────────────────────┘  │
│  Collections │                                        │
│              │                                        │
├──────────────┴───────────────────────────────────────┤
│  [Now Playing] ▶ ■  ════════════●══  0:01 / 0:03     │
│  ░░▓▓▓█▓▓▓▓▓▓▓▓▓▓█▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│  Footstep_Concrete_01.wav  ★ Tag  Add to Collection   │
└──────────────────────────────────────────────────────┘
```

### 6.2 First-Run Experience

On first launch with no configured scan roots:
1. App shows a centered welcome screen with "Add your audio library" prompt
2. Button opens native directory picker (via `tauri-plugin-dialog`)
3. Selected path is saved to config and scan begins automatically
4. Progress modal shows scan status with file count and progress bar
5. Search UI activates once first batch of files is indexed

### 6.3 Key UI Behaviors

- **Virtual scrolling** — only render visible rows (~30 at a time) for 100k+ result sets
- **Instant preview** — single click plays, spacebar toggles, arrow keys navigate
- **Keyboard-first** — `/` focuses search, `Esc` clears, `f` favorites, `1-5` rates
- **Waveform** — rendered from pre-computed peaks, click-to-seek
- **Drag and drop** — drag asset rows to filesystem (exports path for DAW import)
- **Dark theme** — dark-only for v1 (no light theme toggle)
- **Toast notifications** — error/success feedback for all operations (failed decode, scan errors, etc.)

### 6.5 Design System (Dark Mode Only)

SoundVault v1 is dark-mode only. No light theme toggle. All frontend agents must use these exact tokens.

```
Color Palette (CSS custom properties):
  --bg-base:       #0d0f11    /* app background */
  --bg-surface:    #151820    /* panels, cards, sidebar */
  --bg-elevated:   #1c2028    /* hover states, active rows, modals */
  --bg-input:      #1a1e26    /* search bar, filter inputs */
  --border:        #2a2f3a    /* subtle borders between sections */
  --border-focus:  #4a90d9    /* focused input borders */

  --text-primary:  #e2e4e8    /* filenames, headings */
  --text-secondary:#8b8f98    /* metadata, categories, timestamps */
  --text-muted:    #555962    /* placeholders, disabled */

  --accent:        #4a90d9    /* waveform played portion, active tab, focus rings */
  --accent-hover:  #5a9ee6    /* accent on hover */
  --favorite:      #e8b339    /* star / favorite icon active */
  --error:         #d94a4a    /* toast error border, failed states */
  --success:       #4ad97a    /* toast success border */

  --badge-wav:     #4a90d9    /* format pill: WAV */
  --badge-flac:    #4ad97a    /* format pill: FLAC */
  --badge-mp3:     #d9914a    /* format pill: MP3 */
  --badge-ogg:     #9a4ad9    /* format pill: OGG */
  --badge-aiff:    #d94a7a    /* format pill: AIFF */

Typography:
  --font-mono:     'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace
  --font-size-sm:  12px       /* metadata, badges */
  --font-size-base:13px       /* default body text */
  --font-size-lg:  15px       /* filenames in results */
  --font-size-xl:  18px       /* section headings */

Spacing:
  --spacing-xs:    4px
  --spacing-sm:    8px
  --spacing-md:    12px
  --spacing-lg:    16px
  --spacing-xl:    24px

  --row-height:    56px       /* asset row height */
  --sidebar-width: 260px      /* default sidebar */
  --player-height: 80px       /* bottom player bar */

Scrollbar (WebKitGTK):
  ::-webkit-scrollbar         { width: 8px; }
  ::-webkit-scrollbar-track   { background: var(--bg-base); }
  ::-webkit-scrollbar-thumb   { background: var(--border); border-radius: 4px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--text-muted); }
```

### 6.4 Svelte Component Tree

```
App.svelte
├── Sidebar.svelte
│   ├── FolderTree.svelte
│   ├── TagList.svelte
│   └── CollectionList.svelte
├── MainPanel.svelte
│   ├── SearchBar.svelte
│   ├── FilterBar.svelte
│   └── ResultsList.svelte        (virtual scroll)
│       └── AssetRow.svelte       (repeated)
├── PlayerBar.svelte
│   └── Waveform.svelte           (canvas)
├── SettingsPanel.svelte           (scan roots, preferences)
├── ProgressModal.svelte           (scan progress overlay)
└── ToastContainer.svelte          (error/success notifications)
```

---

## 7. Audio Playback

### 7.1 Strategy

Audio decoding happens in Rust (Symphonia), but playback uses the Web Audio API in the frontend for lowest latency and simplest waveform integration.

```
1. User clicks play on asset
2. Frontend invokes get_audio_data(asset_id)
3. Rust checks LRU cache → hit: return cached PCM. Miss: decode file, cache, return.
4. Binary PCM returned via Tauri binary IPC (not JSON)
5. Frontend creates AudioBuffer from raw bytes, connects to AudioContext
6. Peaks already loaded from DB (binary BLOB → Float32Array) for waveform rendering
```

### 7.2 Caching Strategy

Two-layer caching:
- **Rust-side LRU cache** (primary) — holds last 10 decoded `PcmData` in memory. Avoids re-reading + re-decoding files from disk. This is the important cache since decode is the expensive operation.
- **Frontend AudioBuffer reuse** (Phase 2) — the frontend can optionally hold onto `AudioBuffer` objects to avoid re-transferring over IPC. Lower priority since binary IPC is fast.

### 7.3 Shared Decode Core

Both waveform peak computation and playback decoding use the same Symphonia decode pipeline. A shared `decode_samples(path) -> Result<(Vec<f32>, u32, u16)>` function in `audio/decode_core.rs` is used by both `indexer/peaks.rs` and `audio/decoder.rs` to avoid duplicating the decode loop.

---

## 8. Phase 2: Semantic Search (CLAP Embeddings)

### 8.1 Concept

CLAP (Contrastive Language-Audio Pretraining) maps both text and audio into a shared embedding space. This enables searches like "rain on a tin roof" to match files regardless of filename.

### 8.2 Implementation Plan

```
1. Add a CLAP model (laion/larger_clap_music_and_speech) via Python sidecar
   or compile to ONNX and run in Rust via ort
2. During indexing, generate 512-dim embedding per file
3. Store embeddings in a new table:
   CREATE TABLE embeddings (
       asset_id INTEGER PRIMARY KEY REFERENCES assets(id),
       vector   BLOB NOT NULL  -- 512 x f32 = 2048 bytes
   );
4. At query time, embed the search text, brute-force cosine similarity
   (100k vectors × 512 dims ≈ 50ms on modern CPU)
5. Combine FTS5 score + cosine similarity for ranked results
```

### 8.3 Storage Impact

- 512 floats × 4 bytes × 200k files = ~400 MB
- Acceptable for local storage, fits in RAM for brute-force search
- If scale exceeds this, migrate to sqlite-vss or usearch

---

## 9. Configuration

Stored in `~/.config/soundvault/config.toml`:

```toml
[general]
scan_roots = ["/path/to/GDC Audio", "/other/library"]
# theme is dark-only for v1 — no toggle

[indexing]
parallel_workers = 0              # 0 = auto (num_cpus)
peak_resolution = 200             # waveform points per file
skip_hidden_dirs = true

[playback]
buffer_cache_count = 10           # decoded files to keep in memory
auto_play_on_select = true

[search]
default_sort = "relevance"
results_per_page = 50

# Phase 2 fields (added later):
# [indexing] watch_for_changes = false
# [playback] loop_mode = "none"
# [search] semantic_search = false
```

---

## 10. Build & Distribution

### 10.1 Dependencies (Linux)

```bash
# Build dependencies
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev

# Runtime
# WebKitGTK (system), SQLite (bundled), ALSA/PulseAudio (system)
```

### 10.2 Build

```bash
cargo tauri build --bundles deb,appimage
```

Outputs:
- `.deb` package for Debian/Ubuntu
- `.AppImage` portable binary

### 10.3 Target Platforms

| Platform        | Priority | Status   |
|-----------------|----------|----------|
| Linux (x86_64)  | P0       | Primary  |
| Linux (AArch64) | P1       | Test     |
| Windows         | P2       | Future   |
| macOS           | P3       | Future   |

---

## 11. Development Phases

### Phase 1 — Core (MVP)

- [ ] Tauri project scaffold with Svelte
- [ ] Rust directory walker + audio metadata extraction
- [ ] SQLite schema + FTS5 indexing
- [ ] Search bar with substring matching (trigram)
- [ ] Virtual-scroll results list
- [ ] Audio playback with waveform display
- [ ] Folder tree sidebar
- [ ] Filter by duration, sample rate, channels, format
- [ ] Favorites and star ratings
- [ ] Settings panel with scan root management
- [ ] First-run directory picker
- [ ] Toast notification system

### Phase 2 — Organization

- [ ] User tags (create, assign, bulk tag)
- [ ] Collections / playlists
- [ ] Drag-and-drop export (file path to DAW)
- [ ] Keyboard shortcut system
- [ ] Incremental re-scan with file watcher (inotify)
- [ ] Frontend AudioBuffer caching

### Phase 3 — Intelligence

- [ ] CLAP embedding generation (Python sidecar or ONNX)
- [ ] Semantic text-to-audio search
- [ ] "Similar sounds" feature (nearest-neighbor in embedding space)
- [ ] Auto-categorization suggestions

### Phase 4 — Polish

- [ ] Batch operations (tag, rate, export)
- [ ] Advanced filters (BPM detection for loops, key detection)
- [ ] Spectrogram view
- [ ] Multiple library management
- [ ] Import/export database (share tags with team)

---

## 12. Resolved Design Decisions

1. **Waveform storage** → Binary BLOB in SQLite. 200 points × 2 × 4 bytes = 1.6KB/file. ~320MB for 200k files. Instant load, no JSON parse.

2. **Audio transport** → Tauri binary IPC response (`tauri::ipc::Response`). No JSON inflation. ~1.9MB for a 5s stereo 48kHz file.

3. **CLAP model** → Optional download on first use. Phase 2 feature. Keep v1 binary small.

4. **Multiple scan roots** → Supported from v1. Config stores `Vec<PathBuf>`. `start_scan` iterates all roots. Missing files from removed roots are deleted on next re-scan.

5. **Search tokenizer** → FTS5 trigram. No prefix `*` operator needed — trigram gives substring matching inherently.

6. **Incremental update** → INSERT for new files, UPDATE for modified files (not INSERT OR IGNORE which would silently drop changed metadata).
