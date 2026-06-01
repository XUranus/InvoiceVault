use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, info};

use crate::app_core::{AppState, CleanupStorageResult, ExportLogsResult, PriceConfig,
    RecognitionQueueStatus, RegenerateEmbeddingsResult};
use crate::app_core::config::{load_config_raw, write_config};
use crate::app_core::constants::{
    DIR_MODELS, EMBEDDING_DOWNLOAD_TIMEOUT_SECS, EMBEDDING_MODEL_DIR, EMBEDDING_TEST_TIMEOUT_SECS,
};
use crate::chroma::ChromaConfig;
use crate::diag;
use crate::embedding::EmbeddingTestResult;
use crate::extractor::{BadgeConfig, DashboardStats, InvoiceBadgeSelection, LlmUsageStats};
use crate::llm::{
    analyze_error_with_llm, test_llm_connection as run_llm_connection_test,
    LlmConnectionTestResult, LlmProviderConfig,
};

#[derive(Serialize)]
pub struct LocalEmbeddingStatus {
    enabled: bool,
    model_present: bool,
    model_loaded: bool,
    model_dir: Option<String>,
    dimensions: Option<usize>,
}

pub fn embedding_model_presence(app_data_dir: &Path) -> (bool, Option<String>) {
    let model_dir = app_data_dir.join(DIR_MODELS).join(EMBEDDING_MODEL_DIR);
    let present = model_dir.join("onnx").join("model_q4.onnx").exists()
        && model_dir.join("tokenizer.json").exists();
    (
        present,
        present.then(|| model_dir.to_string_lossy().into_owned()),
    )
}

#[tauri::command]
pub async fn get_dashboard_stats(
    app: AppHandle,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<DashboardStats, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .get_dashboard_stats(date_from, date_to)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn test_llm_connection(
    state: State<'_, AppState>,
    config: LlmProviderConfig,
) -> Result<LlmConnectionTestResult, String> {
    let audit_config = state.llm_audit_config();
    run_llm_connection_test(config, audit_config.as_ref())
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn analyze_email_error(
    state: State<'_, AppState>,
    error_message: String,
) -> Result<Option<String>, String> {
    let config = match state.get_llm_config() {
        Some(c) => c,
        None => return Ok(None),
    };
    match analyze_error_with_llm(config, &error_message).await {
        Ok(suggestion) => Ok(Some(suggestion)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn set_chroma_config(state: State<'_, AppState>, config: ChromaConfig) -> Result<(), String> {
    state
        .set_chroma_config(config)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_chroma_config(state: State<'_, AppState>) -> Result<ChromaConfig, String> {
    Ok(state.get_chroma_config())
}

#[tauri::command]
pub fn set_embedding_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state
        .set_embedding_enabled(enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn get_embedding_status(app: AppHandle) -> Result<LocalEmbeddingStatus, String> {
    debug!("[emb] get_embedding_status: start");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (enabled, model_loaded, model_dir, dimensions) = state.embedding_status();
        let (model_present, fallback_model_dir) = embedding_model_presence(state.app_data_dir());
        Ok(LocalEmbeddingStatus {
            enabled,
            model_present,
            model_loaded,
            model_dir: model_dir.or(fallback_model_dir),
            dimensions,
        })
    })
    .await
    .map_err(|err| err.to_string())?;
    debug!("[emb] get_embedding_status: done");
    result
}

#[tauri::command]
pub async fn download_embedding_model(
    state: State<'_, AppState>,
) -> Result<LocalEmbeddingStatus, String> {
    let app_data_dir = state.app_data_dir().to_path_buf();
    let model_dir = tokio::time::timeout(
        Duration::from_secs(EMBEDDING_DOWNLOAD_TIMEOUT_SECS),
        crate::embedding::ensure_model(&app_data_dir),
    )
    .await
    .map_err(|_| {
        "模型下载超时。请检查网络或代理；也可以手动下载 Xenova/bge-small-zh-v1.5 的 onnx/model_q4.onnx 和 tokenizer.json 到应用数据目录的 models/bge-small-zh-v1.5。".to_owned()
    })?
    .map_err(|e| e.to_string())?;
    state
        .set_embedding_enabled(true)
        .map_err(|e| e.to_string())?;
    let (enabled, model_loaded, model_dir_path, dimensions) = state.embedding_status();
    Ok(LocalEmbeddingStatus {
        enabled,
        model_present: true,
        model_loaded,
        model_dir: model_dir_path.or_else(|| Some(model_dir.to_string_lossy().into_owned())),
        dimensions,
    })
}

#[tauri::command]
pub fn set_badge_config(state: State<'_, AppState>, config: BadgeConfig) -> Result<(), String> {
    state
        .set_badge_config(config)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_badge_config(state: State<'_, AppState>) -> Result<BadgeConfig, String> {
    Ok(state.get_badge_config())
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    let theme_path = state.app_data_dir().join("theme.json");
    let theme = std::fs::read_to_string(&theme_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("theme").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| "light".to_owned());
    Ok(theme)
}

#[tauri::command]
pub fn set_theme(app: AppHandle, state: State<'_, AppState>, theme: String) -> Result<(), String> {
    if theme != "light" && theme != "dark" {
        return Err("theme must be 'light' or 'dark'".to_owned());
    }
    let theme_path = state.app_data_dir().join("theme.json");
    let json = serde_json::to_string_pretty(&serde_json::json!({ "theme": &theme }))
        .map_err(|e| e.to_string())?;
    std::fs::write(&theme_path, json).map_err(|e| e.to_string())?;
    app.emit("theme-change", serde_json::json!({ "theme": theme }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_invoice_badge(
    state: State<'_, AppState>,
    invoice_id: i64,
    group_name: String,
    value: Option<String>,
) -> Result<Vec<InvoiceBadgeSelection>, String> {
    state
        .set_invoice_badge(invoice_id, group_name, value)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn test_chroma_connection(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .test_chroma_connection()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn test_embedding_connection(app: AppHandle) -> Result<EmbeddingTestResult, String> {
    tokio::time::timeout(
        Duration::from_secs(EMBEDDING_TEST_TIMEOUT_SECS),
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            state
                .test_embedding_connection()
                .map_err(|err| err.to_string())
        }),
    )
    .await
    .map_err(|_| {
        "Embedding 测试超时（>120s）。ONNX Runtime 首次加载可能较慢，请重启应用后重试。".to_owned()
    })?
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn regenerate_all_embeddings(
    state: State<'_, AppState>,
) -> Result<RegenerateEmbeddingsResult, String> {
    state
        .regenerate_all_embeddings()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_llm_config(state: State<'_, AppState>, config: LlmProviderConfig) -> Result<(), String> {
    state.set_llm_config(config).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn get_llm_config(app: AppHandle) -> Result<Option<LlmProviderConfig>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        Ok(state.get_llm_config())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn set_llm_audit_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.set_llm_audit_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub fn get_llm_audit_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.get_llm_audit_enabled())
}

#[tauri::command]
pub async fn get_recognition_queue_status(app: AppHandle) -> Result<RecognitionQueueStatus, String> {
    debug!("[poll] get_recognition_queue_status: start");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db().lock().map_err(|e| format!("db lock: {e}"))?;
        let running: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM import_jobs WHERE status = 'recognizing'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(RecognitionQueueStatus {
            pending: 0,
            running,
            max_concurrent: 0,
        })
    })
    .await
    .map_err(|err| err.to_string())?;
    debug!("[poll] get_recognition_queue_status: done");
    result
}

#[tauri::command]
pub fn raw_file_has_invoices(state: State<'_, AppState>, raw_file_id: i64) -> Result<bool, String> {
    state
        .raw_file_has_invoices(raw_file_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_invoice_id_by_raw_file(
    state: State<'_, AppState>,
    raw_file_id: i64,
) -> Result<Option<i64>, String> {
    state
        .invoice_id_for_raw_file(raw_file_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_logs(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<ExportLogsResult, String> {
    state.export_logs(&output_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_backup(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<ExportLogsResult, String> {
    state.export_backup(&output_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_storage(state: State<'_, AppState>) -> Result<CleanupStorageResult, String> {
    state.cleanup_storage().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_all_events(state: State<'_, AppState>) -> Result<usize, String> {
    state.delete_all_events().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_import_job(state: State<'_, AppState>, job_id: i64) -> Result<(), String> {
    state.delete_import_job(job_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_llm_usage(
    state: State<'_, AppState>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<LlmUsageStats, String> {
    state
        .get_llm_usage(date_from, date_to)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_price_config(state: State<'_, AppState>) -> Result<PriceConfig, String> {
    Ok(state.get_price_config())
}

#[tauri::command]
pub fn set_price_config(
    state: State<'_, AppState>,
    config: PriceConfig,
) -> Result<(), String> {
    state.set_price_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_diagnostic_config(state: State<'_, AppState>) -> Result<diag::DiagnosticConfig, String> {
    let app_data_dir = state.app_data_dir();
    Ok(diag::load_config(app_data_dir))
}

#[tauri::command]
pub fn set_diagnostic_config(
    state: State<'_, AppState>,
    config: diag::DiagnosticConfig,
) -> Result<(), String> {
    let app_data_dir = state.app_data_dir();
    diag::save_config(app_data_dir, &config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_llm_diagnostic(state: State<'_, AppState>) -> Result<diag::DiagnosticResult, String> {
    let app_data_dir = state.app_data_dir();
    let diag_config = diag::load_config(app_data_dir);

    let llm_config = state
        .get_llm_config()
        .ok_or("LLM 未配置，请先在设置中填写 API Key。")?;

    let emb_test_result = state.test_embedding_connection().ok();
    let audit_config = state.llm_audit_config();

    Ok(diag::run_diagnostic(
        &diag_config,
        &llm_config,
        emb_test_result.as_ref(),
        audit_config.as_ref(),
    )
    .await)
}

#[tauri::command]
pub fn get_log_level(state: State<'_, AppState>) -> Result<String, String> {
    let app_data_dir = state.app_data_dir();
    let level = load_config_raw::<serde_json::Value>(app_data_dir, "log_config.json")
        .and_then(|v| v.get("level").and_then(|v| v.as_str().map(|s| s.to_owned())))
        .unwrap_or_else(|| "info".to_owned());
    Ok(level)
}

#[tauri::command]
pub fn set_log_level(state: State<'_, AppState>, level: String) -> Result<(), String> {
    // Validate level string
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&level.as_str()) {
        return Err(format!(
            "无效的日志级别: '{}'，可选值: {}",
            level,
            valid_levels.join(", ")
        ));
    }

    // Persist to disk
    let app_data_dir = state.app_data_dir();
    write_config(
        app_data_dir,
        "log_config.json",
        &serde_json::json!({ "level": &level }),
    );

    // Apply at runtime
    crate::apply_log_level(&level)?;

    info!("log level changed to: {}", level);
    Ok(())
}
