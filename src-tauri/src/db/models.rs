//! Row and IPC data models.

use serde::{Deserialize, Serialize};

/// One indexed audio file (no `peaks` — load via `get_peaks`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub folder: String,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bit_depth: Option<i32>,
    pub file_size: i64,
    pub category: Option<String>,
    pub publisher: Option<String>,
    pub favorite: bool,
    pub rating: u8,
    pub notes: Option<String>,
    pub play_count: i64,
}

/// Insert/update payload from the indexer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewAsset {
    pub path: String,
    pub filename: String,
    pub extension: String,
    pub folder: String,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bit_depth: Option<i32>,
    pub file_size: i64,
    pub category: Option<String>,
    pub publisher: Option<String>,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub peaks: Option<Vec<u8>>,
}

/// Sort column for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    Relevance,
    Name,
    Duration,
    Date,
}

impl Default for SortField {
    fn default() -> Self {
        Self::Relevance
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        Self::Desc
    }
}

/// How search ranks results: lexical FTS5 only, text-embedding similarity only, or combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    #[default]
    Lexical,
    Vector,
    Both,
}

/// Default page size for search queries (`limit` when not overridden).
pub const DEFAULT_SEARCH_LIMIT: u32 = 50;

/// Search filters and pagination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub search_mode: SearchMode,
    pub extensions: Option<Vec<String>>,
    pub duration_min: Option<i64>,
    pub duration_max: Option<i64>,
    pub sample_rates: Option<Vec<i32>>,
    pub channels: Option<i32>,
    #[serde(default)]
    pub favorites_only: bool,
    pub tags: Option<Vec<String>>,
    pub publisher: Option<String>,
    #[serde(default)]
    pub sort_by: SortField,
    #[serde(default)]
    pub sort_dir: SortDirection,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    DEFAULT_SEARCH_LIMIT
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            search_mode: SearchMode::default(),
            extensions: None,
            duration_min: None,
            duration_max: None,
            sample_rates: None,
            channels: None,
            favorites_only: false,
            tags: None,
            publisher: None,
            sort_by: SortField::default(),
            sort_dir: SortDirection::default(),
            offset: 0,
            limit: DEFAULT_SEARCH_LIMIT,
        }
    }
}

/// Paginated search response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub assets: Vec<Asset>,
    pub total: u64,
    pub offset: u32,
    /// Cosine similarity (−1…1) for vector-only search, or hybrid fusion score (0…1) for **Both** — same order as `assets`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relevance_scores: Option<Vec<f32>>,
}

/// Progress during a scan (emitted to the UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scanned: u64,
    pub total: u64,
    pub current_file: String,
    pub phase: ScanPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanPhase {
    Enumerating,
    Extracting,
    Indexing,
    Complete,
}

/// Summary after indexing a library root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub files_indexed: u64,
    pub files_skipped: u64,
    pub files_missing: u64,
    pub errors: u64,
    pub duration_secs: f64,
}

/// Progress while rebuilding dense text embeddings (emitted to the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedRebuildProgress {
    pub processed: u32,
    pub total: u32,
    pub detail: String,
}

/// Summary after an embedding rebuild finishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedRebuildComplete {
    pub written: u32,
    pub duration_secs: f64,
}

/// Codec-level metadata from Symphonia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMetadata {
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bit_depth: Option<i32>,
}

/// Path-derived publisher/category/filename parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathMetadata {
    pub filename: String,
    pub extension: String,
    pub folder: String,
    pub publisher: Option<String>,
    pub category: Option<String>,
}

/// Decoded PCM for playback (interleaved f32).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcmData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Distinct filter values derived from the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptions {
    pub extensions: Vec<String>,
    pub sample_rates: Vec<i32>,
    pub min_duration_ms: i64,
    pub max_duration_ms: i64,
    pub publishers: Vec<String>,
}

/// Nested folder tree node for the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub name: String,
    pub path: String,
    pub count: u64,
    pub children: Vec<FolderNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagWithCount {
    pub id: i64,
    pub name: String,
    pub count: u64,
}

/// Row counts for dense text embeddings (`embeddings`); vector search is available when `embedding_count > 0`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchStatus {
    pub embedding_count: i64,
    pub asset_count: i64,
    pub semantic_enabled: bool,
    pub clap_pipeline_ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_default_has_limit_50() {
        let q = SearchQuery::default();
        assert_eq!(q.limit, 50);
    }

    #[test]
    fn search_query_default_has_relevance_sort() {
        let q = SearchQuery::default();
        assert_eq!(q.sort_by, SortField::Relevance);
    }

    #[test]
    fn new_asset_serializes_to_json() {
        let a = NewAsset {
            path: "/a.wav".into(),
            filename: "a".into(),
            extension: "wav".into(),
            folder: "/".into(),
            duration_ms: Some(100),
            sample_rate: Some(44_100),
            channels: Some(2),
            bit_depth: Some(16),
            file_size: 10,
            category: None,
            publisher: None,
            modified_at: 0,
            indexed_at: 0,
            peaks: None,
        };
        let s = serde_json::to_string(&a).expect("json");
        let b: NewAsset = serde_json::from_str(&s).expect("de");
        assert_eq!(a, b);
    }

    #[test]
    fn scan_progress_serializes_correctly() {
        let p = ScanProgress {
            scanned: 1,
            total: 10,
            current_file: "x.wav".into(),
            phase: ScanPhase::Indexing,
        };
        let v = serde_json::to_value(&p).expect("v");
        assert_eq!(v["phase"], "indexing");
    }
}
