use std::path::{Path, PathBuf};

use serde::Serialize;
use tracing::info;

const MODEL_REPO: &str = "Xenova/bge-small-zh-v1.5";
const ONNX_FILE: &str = "onnx/model_q4.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";
const MODEL_DIR_NAME: &str = "bge-small-zh-v1.5";
const DIMENSIONS: usize = 384;
const MAX_TOKEN_LENGTH: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding engine not loaded")]
    NotLoaded,
    #[error("model download failed: {0}")]
    Download(String),
    #[error("model load failed: {0}")]
    Load(String),
    #[error("inference failed: {0}")]
    Inference(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingResult {
    pub embedding: Vec<f32>,
    pub prompt_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingTestResult {
    pub model: String,
    pub dimensions: usize,
    pub duration_ms: u64,
}

pub struct LocalEmbeddingEngine {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    model_dir: PathBuf,
}

impl LocalEmbeddingEngine {
    pub fn load(model_dir: &Path) -> Result<Self, EmbeddingError> {
        let onnx_path = model_dir.join(ONNX_FILE);
        let tokenizer_path = model_dir.join(TOKENIZER_FILE);

        if !onnx_path.exists() {
            return Err(EmbeddingError::Load(format!(
                "ONNX model not found: {}",
                onnx_path.display()
            )));
        }
        if !tokenizer_path.exists() {
            return Err(EmbeddingError::Load(format!(
                "Tokenizer not found: {}",
                tokenizer_path.display()
            )));
        }

        let mut session_builder = ort::session::Session::builder()
            .map_err(|e| EmbeddingError::Load(format!("ONNX Runtime init failed: {e}")))?;
        let session = session_builder
            .commit_from_file(&onnx_path)
            .map_err(|e| EmbeddingError::Load(format!("Failed to load ONNX model: {e}")))?;

        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path.to_str().unwrap())
            .map_err(|e| EmbeddingError::Load(format!("Failed to load tokenizer: {e}")))?;

        info!(
            "Local embedding engine loaded from {}",
            model_dir.display()
        );

        Ok(Self {
            session,
            tokenizer,
            model_dir: model_dir.to_path_buf(),
        })
    }

    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    pub fn dimensions(&self) -> usize {
        DIMENSIONS
    }
}

/// Ensure model files exist. Downloads from HuggingFace if missing.
/// Returns the path to the model directory.
pub async fn ensure_model(app_data_dir: &Path) -> Result<PathBuf, EmbeddingError> {
    let model_dir = app_data_dir.join("models").join(MODEL_DIR_NAME);
    let onnx_path = model_dir.join(ONNX_FILE);
    let tokenizer_path = model_dir.join(TOKENIZER_FILE);

    if onnx_path.exists() && tokenizer_path.exists() {
        return Ok(model_dir);
    }

    info!(
        "Downloading embedding model from {} to {}",
        MODEL_REPO,
        model_dir.display()
    );

    std::fs::create_dir_all(&model_dir)
        .map_err(|e| EmbeddingError::Download(format!("Failed to create model dir: {e}")))?;

    let api = hf_hub::api::tokio::Api::new()
        .map_err(|e| EmbeddingError::Download(format!("Failed to init HF Hub: {e}")))?;
    let repo = api.model(MODEL_REPO.to_string());

    // Download ONNX model file
    let onnx_remote = repo
        .get(ONNX_FILE)
        .await
        .map_err(|e| EmbeddingError::Download(format!("Failed to download ONNX model: {e}")))?;
    if onnx_remote != onnx_path {
        if let Some(parent) = onnx_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                EmbeddingError::Download(format!("Failed to create onnx dir: {e}"))
            })?;
        }
        std::fs::copy(&onnx_remote, &onnx_path).map_err(|e| {
            EmbeddingError::Download(format!("Failed to copy ONNX model: {e}"))
        })?;
    }

    // Download tokenizer
    let tok_remote = repo
        .get(TOKENIZER_FILE)
        .await
        .map_err(|e| EmbeddingError::Download(format!("Failed to download tokenizer: {e}")))?;
    if tok_remote != tokenizer_path {
        std::fs::copy(&tok_remote, &tokenizer_path).map_err(|e| {
            EmbeddingError::Download(format!("Failed to copy tokenizer: {e}"))
        })?;
    }

    info!("Embedding model download complete");
    Ok(model_dir)
}

/// Generate an embedding vector for the given text using the local ONNX model.
pub fn generate_embedding(
    engine: &mut LocalEmbeddingEngine,
    text: &str,
) -> Result<EmbeddingResult, EmbeddingError> {
    let encoding = engine
        .tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::Inference(format!("Tokenization failed: {e}")))?;

    let token_ids: Vec<i64> = encoding
        .get_ids()
        .iter()
        .take(MAX_TOKEN_LENGTH)
        .map(|&id| id as i64)
        .collect();
    let attention_mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .take(MAX_TOKEN_LENGTH)
        .map(|&m| m as i64)
        .collect();
    let seq_len = token_ids.len();

    // Build input tensors: shape [1, seq_len]
    let input_ids_tensor = ort::value::Tensor::from_array((
        vec![1i64, seq_len as i64],
        token_ids.into_boxed_slice(),
    ))
    .map_err(|e| EmbeddingError::Inference(format!("Failed to create input_ids tensor: {e}")))?;

    let attention_mask_tensor = ort::value::Tensor::from_array((
        vec![1i64, seq_len as i64],
        attention_mask.clone().into_boxed_slice(),
    ))
    .map_err(|e| {
        EmbeddingError::Inference(format!("Failed to create attention_mask tensor: {e}"))
    })?;

    let token_type_ids: Vec<i64> = vec![0i64; seq_len];
    let token_type_ids_tensor = ort::value::Tensor::from_array((
        vec![1i64, seq_len as i64],
        token_type_ids.into_boxed_slice(),
    ))
    .map_err(|e| {
        EmbeddingError::Inference(format!("Failed to create token_type_ids tensor: {e}"))
    })?;

    let outputs = engine
        .session
        .run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_ids_tensor,
        ])
        .map_err(|e| EmbeddingError::Inference(format!("ONNX inference failed: {e}")))?;

    // Extract last_hidden_state: shape [1, seq_len, hidden_dim]
    // BERT-based models output "last_hidden_state" as the first output
    let output = outputs
        .get("last_hidden_state")
        .ok_or(EmbeddingError::Inference(
            "Output 'last_hidden_state' not found".to_string(),
        ))?;

    let (shape, data) = output
        .try_extract_tensor::<f32>()
        .map_err(|e| EmbeddingError::Inference(format!("Failed to extract tensor: {e}")))?;

    // shape is [1, seq_len, hidden_dim] — get hidden_dim from last dimension
    let shape_vec: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    let hidden_dim = *shape_vec.last().unwrap_or(&DIMENSIONS);

    // Mean pooling with attention mask
    let mut pooled = vec![0.0f32; hidden_dim];
    let mut mask_sum = 0.0f32;

    for t in 0..seq_len {
        let mask = attention_mask[t] as f32;
        mask_sum += mask;
        for h in 0..hidden_dim {
            pooled[h] += data[t * hidden_dim + h] * mask;
        }
    }

    if mask_sum > 0.0 {
        for h in 0..hidden_dim {
            pooled[h] /= mask_sum;
        }
    }

    // L2 normalize
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in pooled.iter_mut() {
            *x /= norm;
        }
    }

    let prompt_tokens = seq_len as i64;
    Ok(EmbeddingResult {
        embedding: pooled,
        prompt_tokens,
        total_tokens: prompt_tokens,
    })
}

/// Test the local embedding engine by running a sample inference.
pub fn test_embedding_connection(
    engine: &mut LocalEmbeddingEngine,
) -> Result<EmbeddingTestResult, EmbeddingError> {
    let start = std::time::Instant::now();
    let result = generate_embedding(engine, "test connection")?;
    let dimensions = result.embedding.len();
    let duration_ms = start.elapsed().as_millis() as u64;
    Ok(EmbeddingTestResult {
        model: "bge-small-zh-v1.5".to_string(),
        dimensions,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_model_dir_constants_are_valid() {
        assert!(!ONNX_FILE.is_empty());
        assert!(!TOKENIZER_FILE.is_empty());
        assert!(DIMENSIONS > 0);
        assert!(MAX_TOKEN_LENGTH > 0);
    }
}
