//! 本地 Embedding 引擎模块：基于 ONNX Runtime 运行 BERT 模型生成文本向量。
//!
//! 支持模型自动下载、加载 ONNX 模型和 tokenizer，
//! 提供文本向量化和连接测试功能。

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::Serialize;
use tracing::info;

use crate::app_core::constants::{
    EMBEDDING_DIMENSIONS as DIMENSIONS, EMBEDDING_MAX_TOKENS as MAX_TOKEN_LENGTH,
    EMBEDDING_MODEL_DIR as MODEL_DIR_NAME, EMBEDDING_MODEL_REPO as MODEL_REPO,
    EMBEDDING_ONNX_PATH as ONNX_FILE, EMBEDDING_TOKENIZER_FILE as TOKENIZER_FILE,
};
#[cfg(target_os = "windows")]
const ONNX_RUNTIME_DLL: &str = "onnxruntime.dll";

static ORT_INIT_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

/// Embedding 模块错误类型。
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

/// Embedding 推理结果。
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingResult {
    pub embedding: Vec<f32>,
    pub prompt_tokens: i64,
    pub total_tokens: i64,
}

/// Embedding 连接测试结果。
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingTestResult {
    pub model: String,
    pub dimensions: usize,
    pub duration_ms: u64,
}

/// 本地 Embedding 引擎，封装 ONNX Runtime 会话和 tokenizer。
pub struct LocalEmbeddingEngine {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    model_dir: PathBuf,
}

impl LocalEmbeddingEngine {
    /// 从模型目录加载 ONNX 模型和 tokenizer。
    pub fn load(model_dir: &Path) -> Result<Self, EmbeddingError> {
        ensure_onnx_runtime_loaded().map_err(EmbeddingError::Load)?;

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

        let tokenizer =
            tokenizers::Tokenizer::from_file(tokenizer_path.to_str().ok_or_else(|| {
                EmbeddingError::Load(format!(
                    "Tokenizer path is not valid UTF-8: {}",
                    tokenizer_path.display()
                ))
            })?)
            .map_err(|e| EmbeddingError::Load(format!("Failed to load tokenizer: {e}")))?;

        info!("Local embedding engine loaded from {}", model_dir.display());

        Ok(Self {
            session,
            tokenizer,
            model_dir: model_dir.to_path_buf(),
        })
    }

    /// 返回模型文件所在目录。
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// 返回向量维度。
    pub fn dimensions(&self) -> usize {
        DIMENSIONS
    }
}

fn ensure_onnx_runtime_loaded() -> Result<(), String> {
    ORT_INIT_RESULT
        .get_or_init(|| {
            #[cfg(target_os = "windows")]
            {
                let dll_path = find_onnxruntime_dll().ok_or_else(|| {
                    "ONNX Runtime DLL not found. Put onnxruntime.dll in src-tauri/resources for dev/build, or set ORT_DYLIB_PATH to the full DLL path.".to_owned()
                })?;
                info!("Using ONNX Runtime DLL at {}", dll_path.display());
                ort::init_from(&dll_path)
                    .map_err(|e| {
                        format!(
                            "Failed to load ONNX Runtime from {}: {e}",
                            dll_path.display()
                        )
                    })?
                    .commit();
            }

            #[cfg(target_os = "linux")]
            {
                let so_path = find_onnxruntime_so().ok_or_else(|| {
                    "ONNX Runtime .so not found. Put libonnxruntime.so in src-tauri/resources for dev/build, or set ORT_DYLIB_PATH to the full .so path.".to_owned()
                })?;
                info!("Using ONNX Runtime .so at {}", so_path.display());
                ort::init_from(&so_path)
                    .map_err(|e| {
                        format!(
                            "Failed to load ONNX Runtime from {}: {e}",
                            so_path.display()
                        )
                    })?
                    .commit();
            }

            #[cfg(target_os = "macos")]
            {
                let dylib_path = find_onnxruntime_dylib().ok_or_else(|| {
                    "ONNX Runtime .dylib not found. Put libonnxruntime.dylib in src-tauri/resources for dev/build, or set ORT_DYLIB_PATH to the full .dylib path.".to_owned()
                })?;
                info!("Using ONNX Runtime .dylib at {}", dylib_path.display());
                ort::init_from(&dylib_path)
                    .map_err(|e| {
                        format!(
                            "Failed to load ONNX Runtime from {}: {e}",
                            dylib_path.display()
                        )
                    })?
                    .commit();
            }

            Ok(())
        })
        .as_ref()
        .map(|_| ())
        .map_err(Clone::clone)
}

#[cfg(target_os = "windows")]
fn find_onnxruntime_dll() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH").map(PathBuf::from) {
        if path.exists() {
            return Some(path);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![
        manifest_dir
            .join("resources")
            .join("win-x86_64")
            .join(ONNX_RUNTIME_DLL),
        manifest_dir.join("resources").join(ONNX_RUNTIME_DLL),
        manifest_dir
            .join("target")
            .join("debug")
            .join(ONNX_RUNTIME_DLL),
        manifest_dir
            .join("target")
            .join("release")
            .join(ONNX_RUNTIME_DLL),
    ];

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        candidates.push(
            local_app_data
                .join("Programs")
                .join("onnxruntime")
                .join("lib")
                .join(ONNX_RUNTIME_DLL),
        );
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(ONNX_RUNTIME_DLL));
            candidates.push(dir.join("win-x86_64").join(ONNX_RUNTIME_DLL));
            candidates.push(dir.join("resources").join(ONNX_RUNTIME_DLL));
            candidates.push(
                dir.join("resources")
                    .join("win-x86_64")
                    .join(ONNX_RUNTIME_DLL),
            );
            candidates.push(
                dir.join("resources")
                    .join("resources")
                    .join("win-x86_64")
                    .join(ONNX_RUNTIME_DLL),
            );
        }
    }

    candidates.into_iter().find(|path| path.exists())
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const ONNX_RUNTIME_LINUX_SO: &str = "libonnxruntime.so";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const ONNX_RUNTIME_MACOS_DYLIB: &str = "libonnxruntime.dylib";

#[cfg(target_os = "linux")]
fn find_onnxruntime_so() -> Option<PathBuf> {
    find_onnxruntime_lib(ONNX_RUNTIME_LINUX_SO)
}

#[cfg(target_os = "macos")]
fn find_onnxruntime_dylib() -> Option<PathBuf> {
    find_onnxruntime_lib(ONNX_RUNTIME_MACOS_DYLIB)
}

#[cfg(not(target_os = "windows"))]
fn find_onnxruntime_lib(lib_name: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH").map(PathBuf::from) {
        if path.exists() {
            return Some(path);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![
        manifest_dir.join("resources").join(lib_name),
        manifest_dir.join("target").join("debug").join(lib_name),
        manifest_dir.join("target").join("release").join(lib_name),
        PathBuf::from("/usr/lib").join(lib_name),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(lib_name));
            candidates.push(dir.join("resources").join(lib_name));
        }
    }

    candidates.into_iter().find(|path| path.exists())
}

/// 确保 Embedding 模型文件存在，缺失时从 HuggingFace 自动下载。
/// 返回模型目录路径。
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
            std::fs::create_dir_all(parent)
                .map_err(|e| EmbeddingError::Download(format!("Failed to create onnx dir: {e}")))?;
        }
        std::fs::copy(&onnx_remote, &onnx_path)
            .map_err(|e| EmbeddingError::Download(format!("Failed to copy ONNX model: {e}")))?;
    }

    // Download tokenizer
    let tok_remote = repo
        .get(TOKENIZER_FILE)
        .await
        .map_err(|e| EmbeddingError::Download(format!("Failed to download tokenizer: {e}")))?;
    if tok_remote != tokenizer_path {
        std::fs::copy(&tok_remote, &tokenizer_path)
            .map_err(|e| EmbeddingError::Download(format!("Failed to copy tokenizer: {e}")))?;
    }

    info!("Embedding model download complete");
    Ok(model_dir)
}

/// 使用本地 ONNX 模型为文本生成 embedding 向量。
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
    let input_ids_tensor =
        ort::value::Tensor::from_array((vec![1i64, seq_len as i64], token_ids.into_boxed_slice()))
            .map_err(|e| {
                EmbeddingError::Inference(format!("Failed to create input_ids tensor: {e}"))
            })?;

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

/// 测试本地 Embedding 引擎，执行一次样例推理。
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
