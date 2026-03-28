//! Application error types.

use serde::Serialize;

/// All error types for SoundScout.
#[derive(Debug, thiserror::Error)]
pub enum SoundScoutError {
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

    /// Text embedding sidecar / ONNX pipeline failed.
    #[error("Text embedding error: {0}")]
    Embedding(String),
}

impl Serialize for SoundScoutError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_database_error() {
        let err = SoundScoutError::Database(rusqlite::Error::QueryReturnedNoRows);
        assert!(err.to_string().contains("Database error"));
    }

    #[test]
    fn display_config_error() {
        let err = SoundScoutError::Config("missing field".into());
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn display_cancelled() {
        let err = SoundScoutError::Cancelled;
        assert_eq!(err.to_string(), "Indexing cancelled");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: SoundScoutError = io_err.into();
        assert!(matches!(err, SoundScoutError::Io(_)));
    }

    #[test]
    fn serializes_to_string() {
        let err = SoundScoutError::Validation("bad input".into());
        let json = serde_json::to_string(&err).expect("serialize");
        assert!(json.contains("bad input"));
    }
}
