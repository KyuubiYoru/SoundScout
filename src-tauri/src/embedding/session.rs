//! Local text embeddings via ONNX (`fastembed` / `all-MiniLM-L6-v2`), no Python.

use std::sync::{Mutex, OnceLock};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::error::SoundScoutError;

/// Stored in `embeddings.model_id`. Distinct from the legacy Python pipeline so stale vectors are ignored until rebuild.
pub const TEXT_EMBEDDING_MODEL_ID: &str = "text_minilm_l6_v2_ort";

pub const EXPECTED_DIM: usize = 384;

pub fn expected_dim() -> usize {
    EXPECTED_DIM
}

fn embed_cache_dir() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("SOUNDSCOUT_EMBED_CACHE") {
        return std::path::PathBuf::from(p);
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("soundscout")
        .join("embed_models")
}

fn create_text_embedding() -> Result<TextEmbedding, SoundScoutError> {
    let cache = embed_cache_dir();
    std::fs::create_dir_all(&cache).map_err(|e| SoundScoutError::Embedding(e.to_string()))?;
    let opts = TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
        .with_cache_dir(cache)
        .with_show_download_progress(false);
    TextEmbedding::try_new(opts).map_err(|e| SoundScoutError::Embedding(e.to_string()))
}

fn global_model() -> &'static Mutex<Option<TextEmbedding>> {
    static M: OnceLock<Mutex<Option<TextEmbedding>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(None))
}

fn with_global_model<F, R>(f: F) -> Result<R, SoundScoutError>
where
    F: FnOnce(&mut TextEmbedding) -> Result<R, SoundScoutError>,
{
    let mut g = global_model()
        .lock()
        .map_err(|e| SoundScoutError::Embedding(format!("embed model lock: {e}")))?;
    if g.is_none() {
        *g = Some(create_text_embedding()?);
    }
    let model = g
        .as_mut()
        .ok_or_else(|| SoundScoutError::Embedding("embed model unavailable".into()))?;
    f(model)
}

fn validate_batch(vectors: &[Vec<f32>], n_texts: usize) -> Result<(), SoundScoutError> {
    if vectors.len() != n_texts {
        return Err(SoundScoutError::Embedding(format!(
            "expected {} vectors, got {}",
            n_texts,
            vectors.len()
        )));
    }
    for v in vectors {
        if v.len() != EXPECTED_DIM {
            return Err(SoundScoutError::Embedding(format!(
                "expected dim {EXPECTED_DIM}, got {}",
                v.len()
            )));
        }
    }
    Ok(())
}

/// Long-lived session (e.g. bulk rebuild): holds its own model instance.
pub struct EmbedSession {
    model: TextEmbedding,
}

impl EmbedSession {
    pub fn new() -> Result<Self, SoundScoutError> {
        Ok(Self {
            model: create_text_embedding()?,
        })
    }

    pub fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, SoundScoutError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let vectors = self
            .model
            .embed(texts.to_vec(), Some(32))
            .map_err(|e| SoundScoutError::Embedding(e.to_string()))?;
        validate_batch(&vectors, texts.len())?;
        Ok(vectors)
    }
}

/// Embed one batch using a shared loaded model (search / ad hoc).
pub fn embed_batch(texts: &[String]) -> Result<Vec<Vec<f32>>, SoundScoutError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    with_global_model(|model| {
        let vectors = model
            .embed(texts.to_vec(), Some(32))
            .map_err(|e| SoundScoutError::Embedding(e.to_string()))?;
        validate_batch(&vectors, texts.len())?;
        Ok(vectors)
    })
}
