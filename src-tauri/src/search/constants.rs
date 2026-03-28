//! Shared search tuning values (FTS vs `LIKE`, etc.).

/// Minimum non-empty query length (Unicode scalar count) to use FTS5 `MATCH` instead of `LIKE`.
pub const FTS_MIN_QUERY_CHARS: usize = 3;
