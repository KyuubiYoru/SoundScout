//! Text embeddings via local ONNX (`fastembed`); no Python sidecar.

mod session;

pub use session::{
    embed_batch, expected_dim, EmbedSession, EMBED_BATCH_SIZE, TEXT_EMBEDDING_MODEL_ID,
};
