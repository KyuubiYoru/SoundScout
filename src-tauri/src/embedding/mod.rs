//! Text embeddings via local ONNX (`fastembed`); no Python sidecar.

mod session;

pub use session::{embed_batch, expected_dim, EmbedSession, TEXT_EMBEDDING_MODEL_ID};
