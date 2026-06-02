//! Application-wide constants for timeouts, paths, model parameters, and limits.
//!
//! Centralizing these values makes it easy to tune behavior without hunting
//! through multiple source files.

// ---------------------------------------------------------------------------
// Directory & file names (relative to app_data_dir)
// ---------------------------------------------------------------------------

/// Log file directory name.
pub const DIR_LOGS: &str = "logs";

/// Embedding model storage directory name.
pub const DIR_MODELS: &str = "models";

/// Diagnostic configuration file name.
pub const DIAGNOSTIC_CONFIG_FILE: &str = "diagnostic_config.json";

/// Single-instance lock file name.
pub const SINGLE_INSTANCE_LOCK_FILE: &str = "invoicevault.lock";

// ---------------------------------------------------------------------------
// Embedding model constants
// ---------------------------------------------------------------------------

/// HuggingFace model repository for the local embedding engine.
pub const EMBEDDING_MODEL_REPO: &str = "Xenova/bge-small-zh-v1.5";

/// Model directory name inside `DIR_MODELS`.
pub const EMBEDDING_MODEL_DIR: &str = "bge-small-zh-v1.5";

/// Path to the quantized ONNX model file, relative to the model directory.
pub const EMBEDDING_ONNX_PATH: &str = "onnx/model_q4.onnx";

/// Tokenizer file name in the model directory.
pub const EMBEDDING_TOKENIZER_FILE: &str = "tokenizer.json";

/// Embedding vector dimensions (BGE-small-zh-v1.5).
pub const EMBEDDING_DIMENSIONS: usize = 384;

/// Maximum token length for embedding input.
pub const EMBEDDING_MAX_TOKENS: usize = 512;

// ---------------------------------------------------------------------------
// Timeouts
// ---------------------------------------------------------------------------

/// Default LLM chat request timeout in seconds.
pub const LLM_DEFAULT_TIMEOUT_SECS: u64 = 30;

/// LLM recognition (invoice extraction) timeout in seconds.
pub const LLM_RECOGNITION_TIMEOUT_SECS: u64 = 90;

/// LLM connection-test timeout in seconds.
pub const LLM_CONNECT_TEST_TIMEOUT_SECS: u64 = 30;

/// Default Agent LLM request timeout in seconds.
pub const AGENT_DEFAULT_TIMEOUT_SECS: u64 = 60;

/// SCNet OCR request timeout in seconds.
pub const SCNET_OCR_TIMEOUT_SECS: u64 = 30;

/// TCP connect/read/write timeout for single-instance check in milliseconds.
pub const SINGLE_INSTANCE_TCP_TIMEOUT_MS: u64 = 700;

/// Embedding model download timeout in seconds.
pub const EMBEDDING_DOWNLOAD_TIMEOUT_SECS: u64 = 120;

/// Embedding connection test timeout in seconds.
pub const EMBEDDING_TEST_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// LLM inference parameters
// ---------------------------------------------------------------------------

/// Maximum retry attempts for LLM recognition.
pub const LLM_MAX_RETRIES: u32 = 3;

/// Maximum VLM (vision-language model) recognition attempts.
pub const LLM_VLM_MAX_ATTEMPTS: u32 = 3;

/// Confidence threshold below which VLM results are rejected.
pub const LLM_VLM_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Temperature schedule for VLM retry attempts.
pub const LLM_VLM_TEMPERATURES: [f32; 3] = [0.0, 0.3, 0.5];

/// Max tokens for invoice recognition responses.
pub const LLM_RECOGNITION_MAX_TOKENS: u16 = 16384;

/// Max tokens for the Agent system prompt path.
pub const AGENT_MAX_TOKENS: u16 = 8192;

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

/// Maximum tool-call loop iterations before the agent gives up.
pub const AGENT_MAX_ITERATIONS: usize = 20;

/// Number of recent messages loaded as conversation context.
pub const AGENT_HISTORY_LIMIT: usize = 20;

/// Default session title when none is provided.
pub const AGENT_DEFAULT_TITLE: &str = "新对话";

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

/// Default file stability wait time in milliseconds.
pub const WATCHER_DEFAULT_STABLE_WAIT_MS: u64 = 2000;

/// Interval between file stability checks in milliseconds.
pub const WATCHER_STABILITY_CHECK_INTERVAL_MS: u64 = 100;

/// Seconds in a day, used for max-age conversion.
pub const SECONDS_PER_DAY: u64 = 86400;

// ---------------------------------------------------------------------------
// File import
// ---------------------------------------------------------------------------

/// Accepted file extensions for invoice import.

// ---------------------------------------------------------------------------
// Status constants — use these instead of raw string literals
// ---------------------------------------------------------------------------

// Import job statuses
pub const STATUS_IMPORTED: &str = "imported";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_RECOGNIZING: &str = "recognizing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_PENDING: &str = "pending";

// Invoice statuses
pub const STATUS_CONFIRMED: &str = "confirmed";
pub const STATUS_ARCHIVED: &str = "archived";

// Duplicate statuses
pub const STATUS_UNIQUE: &str = "unique";
pub const STATUS_EXACT_DUPLICATE: &str = "exact_duplicate";
pub const STATUS_PROBABLE_DUPLICATE: &str = "probable_duplicate";
pub const STATUS_POSSIBLE_DUPLICATE: &str = "possible_duplicate";
pub const STATUS_NOT_DUPLICATE: &str = "not_duplicate";
pub const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "jpeg"];
