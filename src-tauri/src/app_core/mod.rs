//! 应用核心模块，提供全局状态管理、发票 CRUD、导入导出、
//! 目录监控、邮件同步、Agent 会话和存储清理等功能。

use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{Datelike, Local};
use tracing::{error, info, warn};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

mod archive;
pub mod constants;
pub mod config;
mod fs_utils;
mod paths;
mod template_adapter;

pub use archive::{add_dir_to_zip, add_dir_to_zip_with_skip, stream_file_to_zip};
use config::write_config;
pub use config::{load_config_raw, sanitize_badge_config, PriceConfig};
use fs_utils::{remove_empty_dirs, sanitize_filename, walk_dir};
use paths::{create_app_paths, display_path, open_path_with_system, AppPaths};

use crate::{
    agent::{
        self, AgentArtifact, AgentAttachment, AgentError, AgentMessageRow, AgentResponse,
        AgentSession, AgentTask, ConfirmRequest, ToolExecResult,
    },
    chroma::{self, ChromaConfig, ChromaError},
    dedupe::{
        check_invoice_duplicates as run_dedupe_check, resolve_duplicate as run_dedupe_resolve,
        DedupeCheckResult, DedupeError, ResolveDuplicateRequest, ResolveDuplicateResult,
    },
    document::{
        prepare_image_for_recognition, render_pdf_pages, DocumentError, PreparedImage,
        RenderedPdfPage,
    },
    email_manager::{
        AddEmailSourceRequest, EmailError, EmailManager, EmailSource, EmailSyncResult,
        EmailTestResult, UpdateEmailSourceRequest,
    },
    embedding::{
        generate_embedding, test_embedding_connection as run_embedding_test, EmbeddingError,
        EmbeddingTestResult, LocalEmbeddingEngine,
    },
    event::{self, EventError, EventListResult},
    exporter::{
        export_column_catalog, export_invoices, export_pdf_report, preview_export, ExportError,
        ExportInvoicesRequest, ExportPreviewRequest, ExportResult, PdfReportRequest,
        PdfReportResult,
    },
    extractor::invoice_to_embedding_text,
    extractor::{
        batch_delete_invoices, batch_update_invoices, count_unviewed_invoices, get_dashboard_stats,
        get_invoice_detail, list_invoices, mark_invoice_viewed, merge_invoices,
        save_invoice_extraction, search_invoices, set_invoice_badge, update_invoice,
        update_invoice_items, BadgeConfig, BatchUpdateRequest, DashboardStats, ExtractorError,
        InvoiceBadgeSelection, InvoiceDetail, InvoiceItemRow, InvoiceSearchParams,
        InvoiceSearchResult, InvoiceSummary, MergeInvoicesResult, SaveInvoiceExtractionRequest,
        UpdateInvoiceItemsRequest, UpdateInvoiceRequest, UpdateInvoiceResult,
    },
    importer::{
        delete_import_job, import_files, list_import_jobs, recover_interrupted_import_jobs,
        update_import_job_status, update_import_job_status_by_raw_file, ImportError,
        ImportJobListResult, ImportJobSummary,
    },
    llm::{LlmAuditConfig, LlmProviderConfig},
    storage::{run_migrations, StorageError},
    watcher::{
        AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus, WatcherError, WatcherManager,
    },
};

use constants::{DIR_LOGS, DIR_MODELS, EMBEDDING_MODEL_DIR};
const EMBEDDING_MODEL_NAME: &str = EMBEDDING_MODEL_DIR;

/// 应用统一错误类型，聚合各子模块的错误。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 无法获取应用数据目录
    #[error("failed to resolve application data directory")]
    MissingAppDataDir,
    /// I/O 操作错误
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// 数据库/存储错误
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    /// 文件导入错误
    #[error("import error: {0}")]
    Import(#[from] ImportError),
    /// 发票提取错误
    #[error("extractor error: {0}")]
    Extractor(#[from] ExtractorError),
    /// 重复检测错误
    #[error("dedupe error: {0}")]
    Dedupe(#[from] DedupeError),
    /// 导出错误
    #[error("export error: {0}")]
    Export(#[from] ExportError),
    /// 文档处理错误
    #[error("document error: {0}")]
    Document(#[from] DocumentError),
    /// 目录监控错误
    #[error("watcher error: {0}")]
    Watcher(#[from] WatcherError),
    /// 邮件同步错误
    #[error("email error: {0}")]
    Email(#[from] EmailError),
    /// ChromaDB 向量数据库错误
    #[error("chromadb error: {0}")]
    Chroma(#[from] ChromaError),
    /// 本地 Embedding 引擎错误
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    /// Agent 会话错误
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
    /// 事件记录错误
    #[error("event error: {0}")]
    Event(#[from] EventError),
    /// 非法操作
    #[error("{0}")]
    InvalidOperation(String),
}

/// 应用健康状态，用于前端诊断面板展示。
#[derive(Debug, Clone, Serialize)]
pub struct AppHealth {
    pub app_data_dir: String,
    pub database_path: String,
    pub migration_version: i64,
}

/// 待识别的原始文件元数据。
#[derive(Debug, Clone, Serialize)]
pub struct RawFileForRecognition {
    pub id: i64,
    pub original_name: String,
    pub mime_type: String,
    pub storage_path: PathBuf,
}

/// 识别队列状态，显示待处理和正在处理的任务数量。
#[derive(Debug, Clone, Serialize)]
pub struct RecognitionQueueStatus {
    pub pending: i64,
    pub running: i64,
    pub max_concurrent: usize,
}

/// 日志导出结果。
#[derive(Debug, Clone, Serialize)]
pub struct ExportLogsResult {
    pub file_path: String,
    pub byte_size: u64,
}

/// 存储清理结果，统计删除的文件和释放的空间。
#[derive(Debug, Clone, Serialize)]
pub struct CleanupStorageResult {
    pub files_removed: usize,
    pub db_records_removed: usize,
    pub bytes_freed: u64,
}

/// 全量重新生成 Embedding 的结果统计。
#[derive(Debug, Clone, Serialize)]
pub struct RegenerateEmbeddingsResult {
    pub total_invoices: usize,
    pub success_count: usize,
    pub failure_count: usize,
}

/// 电子表格文件检查结果，用于 Agent 读取附件内容。
#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetInspection {
    pub attachment_id: i64,
    pub file_name: String,
    pub file_type: String,
    pub sheets: Vec<SpreadsheetSheet>,
}

/// 工作表信息，包含表头和样本数据行。
#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetSheet {
    pub name: String,
    pub header_row: usize,
    pub columns: Vec<SpreadsheetColumn>,
    pub sample_rows: Vec<Vec<String>>,
}

/// 电子表格列定义。
#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetColumn {
    pub index: usize,
    pub label: String,
}

/// 将识别错误信息转为用户友好的中文提示。
pub fn import_failure_message(error: &str) -> String {
    if error.contains("文件不含有发票") || error.contains("non-invoice") {
        "文件不含有发票".to_owned()
    } else if error.contains("置信度过低")
        || error.contains("did not include a JSON object")
        || error.contains("invalid extraction JSON")
        || error.contains("provider response did not include assistant content")
    {
        "识别失败：可能是图片分辨率不清晰、内容不完整，或文件不含有可识别的发票。".to_owned()
    } else {
        format!("识别失败：{error}")
    }
}

/// 应用全局状态，持有数据库连接、配置、各管理器实例和缓存。
///
/// 通过 Tauri 的状态管理机制注入到所有命令处理器中。
pub struct AppState {
    app_handle: AppHandle,
    paths: AppPaths,
    db: Arc<Mutex<Connection>>,
    watcher_manager: WatcherManager,
    email_manager: EmailManager,
    chroma_config: Mutex<ChromaConfig>,
    local_embedding: Arc<Mutex<Option<LocalEmbeddingEngine>>>,
    embedding_enabled: Mutex<bool>,
    badge_config: Arc<Mutex<BadgeConfig>>,
    price_config: Arc<Mutex<PriceConfig>>,
    llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
    llm_audit_enabled: Arc<Mutex<bool>>,
    importing_paths: Arc<Mutex<HashSet<String>>>,
    dashboard_cache: moka::sync::Cache<String, crate::extractor::DashboardStats>,
    llm_usage_cache: moka::sync::Cache<String, crate::extractor::LlmUsageStats>,
    pending_dropped_files: Arc<Mutex<Vec<String>>>,
}

fn cache_key(date_from: &Option<String>, date_to: &Option<String>) -> String {
    format!(
        "{}|{}",
        date_from.as_deref().unwrap_or(""),
        date_to.as_deref().unwrap_or("")
    )
}

impl AppState {
    /// 初始化应用状态，包括数据库迁移、配置加载和管理器创建。
    pub fn initialize(app: &AppHandle) -> Result<Self, AppError> {
        info!("[init] AppState::initialize: start");
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::MissingAppDataDir)?;
        let paths = create_app_paths(&app_data_dir)?;
        info!("[init] opening database");
        let mut db = Connection::open(&paths.database_path)?;
        run_migrations(&mut db)?;
        let recovered_jobs = recover_interrupted_import_jobs(&db)?;
        if recovered_jobs > 0 {
            warn!("Recovered {recovered_jobs} interrupted import jobs");
        }
        let db = Arc::new(Mutex::new(db));
        info!("[init] database ready");

        let chroma_config = ChromaConfig::default();

        // Load persisted configs
        let embedding_enabled: bool =
            load_config_raw::<serde_json::Value>(&app_data_dir, "embedding_enabled.json")
                .and_then(|v| v.get("enabled").and_then(|v| v.as_bool()))
                .unwrap_or(true);

        let llm_config: Arc<Mutex<Option<LlmProviderConfig>>> = {
            let saved = load_config_raw::<LlmProviderConfig>(&app_data_dir, "llm_config.json");
            Arc::new(Mutex::new(saved))
        };

        let llm_audit_enabled: Arc<Mutex<bool>> = {
            let saved = load_config_raw::<serde_json::Value>(&app_data_dir, "audit_config.json")
                .and_then(|v| v.get("enabled").and_then(|v| v.as_bool()))
                .unwrap_or(true);
            Arc::new(Mutex::new(saved))
        };

        let badge_config =
            load_config_raw::<BadgeConfig>(&app_data_dir, "badge_config.json").unwrap_or_default();

        let price_config =
            load_config_raw::<PriceConfig>(&app_data_dir, "price_config.json").unwrap_or_default();

        info!("[init] creating WatcherManager");
        let watcher_manager = WatcherManager::new(
            Arc::clone(&db),
            paths.raw_dir.clone(),
            paths.thumbnails_dir.clone(),
            paths.llm_audit_dir.clone(),
            Arc::clone(&llm_config),
            Arc::clone(&llm_audit_enabled),
            app.clone(),
        );

        info!("[init] creating EmailManager");
        let email_manager = EmailManager::new(
            Arc::clone(&db),
            paths.raw_dir.clone(),
            paths.thumbnails_dir.clone(),
            Arc::clone(&llm_config),
            Arc::clone(&llm_audit_enabled),
        );
        info!("[init] managers created");

        let state = Self {
            app_handle: app.clone(),
            paths,
            db,
            watcher_manager,
            email_manager,
            chroma_config: Mutex::new(chroma_config),
            local_embedding: Arc::new(Mutex::new(None)),
            embedding_enabled: Mutex::new(embedding_enabled),
            badge_config: Arc::new(Mutex::new(badge_config)),
            price_config: Arc::new(Mutex::new(price_config)),
            llm_config,
            llm_audit_enabled,
            importing_paths: Arc::new(Mutex::new(HashSet::new())),
            pending_dropped_files: Arc::new(Mutex::new(Vec::new())),
            dashboard_cache: moka::sync::Cache::builder()
                .max_capacity(8)
                .time_to_live(std::time::Duration::from_secs(60))
                .build(),
            llm_usage_cache: moka::sync::Cache::builder()
                .max_capacity(8)
                .time_to_live(std::time::Duration::from_secs(60))
                .build(),
        };

        if embedding_enabled {
            let model_dir = state
                .paths
                .app_data_dir
                .join(DIR_MODELS)
                .join(EMBEDDING_MODEL_NAME);
            if !model_dir.join("onnx").join("model_q4.onnx").exists()
                || !model_dir.join("tokenizer.json").exists()
            {
                info!(
                    "Embedding model not found at {}, will download on first use",
                    model_dir.display()
                );
            }
        }

        info!("AppState initialized");
        Ok(state)
    }

    /// 尝试加载本地 Embedding 引擎（ONNX Runtime），未就绪时返回 `false`。
    pub fn load_embedding_engine_if_available(&self) -> Result<bool, AppError> {
        if !*self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner()) {
            return Ok(false);
        }

        {
            let engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
            if engine_guard.is_some() {
                return Ok(true);
            }
        }

        let model_dir = self
            .paths
            .app_data_dir
            .join(DIR_MODELS)
            .join(EMBEDDING_MODEL_NAME);
        let onnx_path = model_dir.join("onnx").join("model_q4.onnx");
        let tok_path = model_dir.join("tokenizer.json");
        if !onnx_path.exists() || !tok_path.exists() {
            return Ok(false);
        }

        let engine = LocalEmbeddingEngine::load(&model_dir)?;
        let mut engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
        if engine_guard.is_some() {
            return Ok(true);
        }
        *engine_guard = Some(engine);
        info!("Local embedding engine loaded from {}", model_dir.display());
        Ok(true)
    }

    /// 返回应用健康状态，包括数据目录、数据库路径和迁移版本。
    pub fn health(&self) -> Result<AppHealth, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let migration_version = db.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?;

        Ok(AppHealth {
            app_data_dir: display_path(&self.paths.app_data_dir),
            database_path: display_path(&self.paths.database_path),
            migration_version,
        })
    }

    /// 恢复已启用的目录监控任务。
    pub fn resume_watchers(&self) -> Result<(), AppError> {
        self.watcher_manager
            .resume_enabled()
            .map_err(AppError::from)
    }

    /// 获取应用数据目录路径。
    pub fn app_data_dir(&self) -> &Path {
        &self.paths.app_data_dir
    }

    /// 获取数据库连接的共享引用。
    pub fn db(&self) -> &Arc<Mutex<Connection>> {
        &self.db
    }

    /// 存储拖拽到窗口的文件路径，供前端轮询获取。
    pub fn push_dropped_files(&self, paths: Vec<String>) {
        let mut guard = self
            .pending_dropped_files
            .lock()
            .expect("pending_dropped_files mutex poisoned");
        guard.extend(paths);
    }

    /// 取出并清空已存储的拖拽文件路径。
    pub fn take_dropped_files(&self) -> Vec<String> {
        let mut guard = self
            .pending_dropped_files
            .lock()
            .expect("pending_dropped_files mutex poisoned");
        std::mem::take(&mut *guard)
    }

    /// 导入文件列表，自动去重并异步触发发票识别。
    pub fn import_files(
        &self,
        paths: Vec<String>,
        app: &AppHandle,
    ) -> Result<Vec<ImportJobSummary>, AppError> {
        // Guard: skip paths already being processed in a concurrent call
        let paths: Vec<String> = {
            let mut guard = self
                .importing_paths
                .lock()
                .expect("importing_paths mutex poisoned");
            paths
                .into_iter()
                .filter(|p| guard.insert(p.clone()))
                .collect()
        };
        if paths.is_empty() {
            return Ok(vec![]);
        }

        let result = self.import_files_inner(paths.clone(), app);

        // Remove guard entries regardless of success/failure
        {
            let mut guard = self
                .importing_paths
                .lock()
                .expect("importing_paths mutex poisoned");
            for p in &paths {
                guard.remove(p);
            }
        }

        if let Err(err) = &result {
            error!(error = %err, files = ?paths, "Import request failed");
        }

        result
    }

    fn import_files_inner(
        &self,
        paths: Vec<String>,
        app: &AppHandle,
    ) -> Result<Vec<ImportJobSummary>, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let source_paths: Vec<String> = paths.iter().map(|p| p.clone()).collect();
        info!(count = paths.len(), files = ?paths, "Importing files");
        let jobs = import_files(&mut db, &self.paths.raw_dir, paths, "manual")?;
        for job in jobs.iter().filter(|job| job.status == "failed") {
            warn!(
                source_path = %job.source_path,
                error = ?job.error_message,
                "Import job failed"
            );
        }
        let total = jobs.len();
        let success = jobs.iter().filter(|j| j.status == "imported").count();
        let dups = jobs.iter().filter(|j| j.status == "duplicate").count();
        let failed = jobs.iter().filter(|j| j.status == "failed").count();
        let raw_file_ids: Vec<i64> = jobs
            .iter()
            .filter(|j| j.status == "imported")
            .filter_map(|j| j.raw_file_id)
            .collect();
        if failed > 0 {
            warn!("Import completed with {failed} failures, {success} success, {dups} duplicates");
        } else {
            info!("Import completed: {success} success, {dups} duplicates");
        }
        if let Err(e) = event::record_import_event(
            &db,
            total,
            success,
            dups,
            failed,
            &source_paths,
            &raw_file_ids,
        ) {
            warn!("Failed to record import event: {e}");
        }

        // Auto-trigger recognition for successfully imported files
        let config = self.llm_config.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(cfg) = config {
            if !cfg.api_key.is_empty() {
                for job in &jobs {
                    if job.status == "imported" {
                        if let Some(raw_file_id) = job.raw_file_id {
                            self.spawn_recognition_task(
                                job.id,
                                raw_file_id,
                                cfg.clone(),
                                self.llm_audit_config(),
                                app.clone(),
                            );
                        }
                    }
                }
            }
        }

        Ok(jobs)
    }

    fn spawn_recognition_task(
        &self,
        job_id: i64,
        raw_file_id: i64,
        config: LlmProviderConfig,
        audit: Option<LlmAuditConfig>,
        app: AppHandle,
    ) {
        let db = Arc::clone(&self.db);
        let thumbnails_dir = self.paths.thumbnails_dir.clone();
        let chroma_enabled = self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).enabled;
        let embedding_on = *self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner());
        let embedding_engine = Arc::clone(&self.local_embedding);

        tauri::async_runtime::spawn(async move {
            if let Ok(db) = db.lock() {
                if let Err(e) = update_import_job_status(&db, job_id, None, "recognizing", None) {
                    warn!("Failed to update import job {job_id} status to recognizing: {e}");
                }
            }

            let result: Result<(), String> = async {
                let raw_file = {
                    let conn = db.lock().map_err(|e| e.to_string())?;
                    conn.query_row(
                        "SELECT id, original_name, COALESCE(mime_type, ''), storage_path
                        FROM raw_files WHERE id = ?1",
                        [raw_file_id],
                        |row| {
                            Ok(RawFileForRecognition {
                                id: row.get(0)?,
                                original_name: row.get(1)?,
                                mime_type: row.get(2)?,
                                storage_path: PathBuf::from(row.get::<_, String>(3)?),
                            })
                        },
                    )
                    .map_err(|e| e.to_string())?
                };

                let recognition_inputs = if raw_file.mime_type == "application/pdf" {
                    let pages = crate::document::render_pdf_pages(
                        &raw_file.storage_path,
                        &thumbnails_dir,
                        raw_file.id,
                    )
                    .map_err(|e| e.to_string())?;
                    pages
                        .into_iter()
                        .map(|page| {
                            let prepared = crate::document::prepare_image_for_recognition(
                                &page.image_path,
                                &thumbnails_dir,
                                raw_file.id,
                                Some(page.page_number),
                            )
                            .map_err(|e| e.to_string())?;
                            Ok((
                                Some(page.page_number.to_string()),
                                prepared.image_path,
                                prepared.thumbnail_path,
                                prepared.mime_type,
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?
                } else {
                    let prepared = crate::document::prepare_image_for_recognition(
                        &raw_file.storage_path,
                        &thumbnails_dir,
                        raw_file.id,
                        None,
                    )
                    .map_err(|e| e.to_string())?;
                    vec![(
                        None,
                        prepared.image_path,
                        prepared.thumbnail_path,
                        prepared.mime_type,
                    )]
                };

                for (source_page_range, image_path, _thumbnail_path, mime_type) in
                    &recognition_inputs
                {
                    let recognition = crate::llm::recognize_invoice_with_retries(
                        config.clone(),
                        image_path,
                        mime_type,
                        audit.as_ref(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    // Save extraction and record events, then drop the lock
                    // before embedding inference to avoid holding the lock during ONNX.
                    let (invoice_id, _seller_name) = {
                        let mut conn = db.lock().map_err(|e| e.to_string())?;
                        let invoice = crate::extractor::save_invoice_extraction(
                            &mut conn,
                            crate::extractor::SaveInvoiceExtractionRequest {
                                raw_file_id,
                                source_page_range: source_page_range.clone(),
                                provider_name: Some(config.base_url.clone()),
                                model: Some(recognition.model.clone()),
                                response_json: recognition.response_json,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                        let title = invoice.seller_name.clone().unwrap_or_else(|| "未知".into());
                        if let Err(e) = event::record_recognition_event(
                            &conn,
                            invoice.id,
                            &title,
                            true,
                            recognition.duration_ms,
                            &recognition.model,
                            1,
                        ) {
                            warn!("Failed to record recognition event for invoice {}: {e}", invoice.id);
                        }
                        if let Err(e) = crate::extractor::insert_usage_log(
                            &conn,
                            "llm_recognition",
                            &recognition.model,
                            recognition.prompt_tokens,
                            recognition.completion_tokens,
                            recognition.total_tokens,
                        ) {
                            warn!("Failed to insert LLM usage log: {e}");
                        }
                        (invoice.id, title)
                    }; // db lock dropped here

                    // Best-effort embedding generation (no db lock held)
                    if chroma_enabled && embedding_on {
                        if let Some(ref mut engine) = *embedding_engine.lock().unwrap_or_else(|e| e.into_inner()) {
                            let thumb_dir = thumbnails_dir.clone();
                            let detail = {
                                let conn = db.lock().unwrap_or_else(|e| e.into_inner());
                                get_invoice_detail(&conn, &thumb_dir, invoice_id).ok()
                            };
                            if let Some(detail) = detail {
                                spawn_embedding_for_invoice(Arc::clone(&db), engine, invoice_id, &detail);
                            }
                        }
                    }
                }
                Ok(())
            }
            .await;

            if let Err(e) = result {
                error!("Auto recognition raw error: {e}");
                let message = import_failure_message(&e);
                error!("Auto recognition failed: {message}");
                if let Ok(db) = db.lock() {
                    // Fetch file name for event details
                    let file_name: Option<String> = db
                        .query_row(
                            "SELECT original_name FROM raw_files WHERE id = ?1",
                            [raw_file_id],
                            |row| row.get(0),
                        )
                        .ok();

                    // Mark as failed, detach from raw_file
                    if let Err(e) = db.execute(
                        "UPDATE import_jobs SET status = 'failed', error_message = ?1, raw_file_id = NULL WHERE id = ?2",
                        rusqlite::params![message, job_id],
                    ) {
                        error!("Failed to mark import job {job_id} as failed: {e}");
                    }

                    // Record failure event with file name
                    let event_title = match &file_name {
                        Some(name) => format!("自动识别失败: {name}"),
                        None => "自动识别失败".to_owned(),
                    };
                    if let Err(e) = event::create_event(
                        &db,
                        "recognition",
                        &event_title,
                        &message,
                        "failed",
                        None,
                        None,
                        None,
                    ) {
                        warn!("Failed to record recognition failure event: {e}");
                    }

                    // Delete the stored file from disk immediately
                    let storage_path: Option<String> = db
                        .query_row(
                            "SELECT storage_path FROM raw_files WHERE id = ?1",
                            [raw_file_id],
                            |row| row.get(0),
                        )
                        .ok();
                    if let Some(path) = storage_path {
                        if let Err(e) = std::fs::remove_file(&path) {
                            warn!("Failed to remove raw file {path}: {e}");
                        }
                    }
                    // Clean up all generated thumbnail directories
                    for subdir in &["previews", "normalized", "pdf-pages"] {
                        let dir_path = thumbnails_dir.join(subdir).join(raw_file_id.to_string());
                        if dir_path.exists() {
                            if let Err(e) = std::fs::remove_dir_all(&dir_path) {
                                warn!("Failed to remove {subdir} dir {}: {e}", dir_path.display());
                            }
                        }
                    }
                    // Delete dependent DB records, then the raw_file itself
                    if let Err(e) = db.execute(
                        "DELETE FROM extraction_runs WHERE raw_file_id = ?1",
                        [raw_file_id],
                    ) {
                        error!("Failed to delete extraction_runs for raw_file {raw_file_id}: {e}");
                    }
                    if let Err(e) =
                        db.execute("DELETE FROM invoices WHERE raw_file_id = ?1", [raw_file_id])
                    {
                        error!("Failed to delete invoices for raw_file {raw_file_id}: {e}");
                    }
                    if let Err(e) = db.execute("DELETE FROM raw_files WHERE id = ?1", [raw_file_id])
                    {
                        error!("Failed to delete raw_file {raw_file_id}: {e}");
                    }
                }
            } else if let Ok(db) = db.lock() {
                if let Err(e) = update_import_job_status(&db, job_id, None, "imported", None) {
                    warn!("Failed to update import job {job_id} status to imported: {e}");
                }
            }

            if let Err(e) = app.emit("recognition-complete", ()) {
                warn!("Failed to emit recognition-complete event: {e}");
            }
        });
    }

    /// 分页查询导入任务列表。
    pub fn list_import_jobs(
        &self,
        page: i64,
        page_size: i64,
    ) -> Result<ImportJobListResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(list_import_jobs(&db, page, page_size)?)
    }

    /// 删除指定导入任务记录。
    pub fn delete_import_job(&self, job_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(delete_import_job(&db, job_id)?)
    }

    /// 保存发票识别结果，并自动检测重复和生成 Embedding。
    pub fn save_invoice_extraction(
        &self,
        request: SaveInvoiceExtractionRequest,
    ) -> Result<InvoiceSummary, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let invoice = save_invoice_extraction(&mut db, request)?;
        if let Ok(dedupe_result) = run_dedupe_check(&db, invoice.id) {
            if let Some(best) = dedupe_result.candidates.iter().max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                if let Err(e) = event::record_duplicate_event(
                    &db,
                    invoice.id,
                    invoice.seller_name.as_deref(),
                    invoice.invoice_number.as_deref(),
                    best.candidate_invoice_id,
                    best.score,
                ) {
                    warn!(
                        "Failed to record duplicate event for invoice {}: {e}",
                        invoice.id
                    );
                }
            }
        }

        // Best-effort embedding generation (local ONNX inference)
        if self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).enabled
            && *self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner())
        {
            let mut engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref mut engine) = *engine_guard {
                let thumb_dir = self.paths.thumbnails_dir.clone();
                let invoice_id = invoice.id;
                let detail = get_invoice_detail(&db, &thumb_dir, invoice_id).ok();
                if let Some(detail) = detail {
                    spawn_embedding_for_invoice(Arc::clone(&self.db), engine, invoice_id, &detail);
                }
            }
        }

        self.invalidate_dashboard_cache();
        Ok(invoice)
    }

    /// 查询全部发票摘要列表。
    pub fn list_invoices(&self) -> Result<Vec<InvoiceSummary>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(list_invoices(&db)?)
    }

    /// 按条件搜索发票，支持关键词、日期、金额等多维度过滤。
    pub fn search_invoices(
        &self,
        params: InvoiceSearchParams,
    ) -> Result<InvoiceSearchResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(search_invoices(&db, params)?)
    }

    /// 获取所有可选的标签选项。
    pub fn get_tag_options(&self) -> Result<Vec<crate::extractor::TagOption>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(crate::extractor::get_tag_options(&db)?)
    }

    /// 获取单张发票的完整详情。
    pub fn get_invoice_detail(&self, invoice_id: i64) -> Result<InvoiceDetail, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(get_invoice_detail(
            &db,
            &self.paths.thumbnails_dir,
            invoice_id,
        )?)
    }

    /// 标记发票为已查看。
    pub fn mark_invoice_viewed(&self, invoice_id: i64) -> Result<bool, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(mark_invoice_viewed(&db, invoice_id)?)
    }

    /// 统计未查看的发票数量。
    pub fn count_unviewed_invoices(&self) -> Result<i64, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(count_unviewed_invoices(&db)?)
    }

    /// 获取发票关联的原始文件存储路径。
    pub fn raw_file_path_for_invoice(&self, invoice_id: i64) -> Result<PathBuf, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let path = db.query_row(
            "SELECT rf.storage_path
             FROM invoices inv
             JOIN raw_files rf ON rf.id = inv.raw_file_id
             WHERE inv.id = ?1",
            [invoice_id],
            |row| row.get::<_, String>(0),
        )?;
        Ok(PathBuf::from(path))
    }

    /// 更新发票字段，并自动重新检测重复和更新 Embedding。
    pub fn update_invoice(
        &self,
        request: UpdateInvoiceRequest,
    ) -> Result<UpdateInvoiceResult, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let result = update_invoice(&mut db, request)?;
        if let Ok(dedupe_result) = run_dedupe_check(&db, result.invoice.id) {
            if let Some(best) = dedupe_result.candidates.iter().max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                if let Err(e) = event::record_duplicate_event(
                    &db,
                    result.invoice.id,
                    result.invoice.seller_name.as_deref(),
                    result.invoice.invoice_number.as_deref(),
                    best.candidate_invoice_id,
                    best.score,
                ) {
                    warn!(
                        "Failed to record duplicate event for invoice {}: {e}",
                        result.invoice.id
                    );
                }
            }
        }

        // Best-effort embedding regeneration (local ONNX inference)
        if self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).enabled
            && *self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner())
        {
            let mut engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref mut engine) = *engine_guard {
                let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
                let thumb_dir = self.paths.thumbnails_dir.clone();
                let invoice_id = result.invoice.id;
                let detail = get_invoice_detail(&db, &thumb_dir, invoice_id).ok();
                if let Some(detail) = detail {
                    spawn_embedding_for_invoice(Arc::clone(&self.db), engine, invoice_id, &detail);
                }
            }
        }

        self.invalidate_dashboard_cache();
        Ok(result)
    }

    /// 更新发票明细行项目。
    pub fn update_invoice_items(
        &self,
        request: UpdateInvoiceItemsRequest,
    ) -> Result<Vec<InvoiceItemRow>, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(update_invoice_items(&mut db, request)?)
    }

    /// 批量更新多张发票的字段。
    pub fn batch_update_invoices(
        &self,
        request: BatchUpdateRequest,
    ) -> Result<Vec<InvoiceSummary>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let result = batch_update_invoices(&db, &request)?;
        self.invalidate_dashboard_cache();
        Ok(result)
    }

    /// 批量删除多张发票。
    pub fn batch_delete_invoices(&self, ids: Vec<i64>) -> Result<usize, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let result = batch_delete_invoices(&db, &ids)?;
        self.invalidate_dashboard_cache();
        Ok(result)
    }

    /// 检查指定发票的重复候选列表。
    pub fn check_invoice_duplicates(&self, invoice_id: i64) -> Result<DedupeCheckResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(run_dedupe_check(&db, invoice_id)?)
    }

    /// 解决重复发票冲突（保留或合并）。
    pub fn resolve_duplicate(
        &self,
        request: ResolveDuplicateRequest,
    ) -> Result<ResolveDuplicateResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(run_dedupe_resolve(&db, request)?)
    }

    /// 清除所有重复检测结果并重新全量检测。
    pub fn regenerate_all_duplicates(&self) -> Result<usize, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let deleted = db.execute("DELETE FROM dedupe_candidates", [])?;
        let reset = db.execute(
            "UPDATE invoices SET duplicate_status = 'unique' WHERE duplicate_status != 'unique'",
            [],
        )?;

        let invoice_ids: Vec<i64> = db
            .prepare("SELECT id FROM invoices")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = invoice_ids.len();
        info!("regenerate_all_duplicates: deleted {deleted} candidates, reset {reset} invoices, processing {count} invoices: {invoice_ids:?}");
        for id in &invoice_ids {
            if let Err(e) = crate::dedupe::detect_field_duplicates(&db, *id) {
                warn!("Failed to detect duplicates for invoice {id}: {e}");
            }
        }

        info!("Regenerated duplicate detection for {count} invoices");
        Ok(count)
    }

    /// 导出发票数据为 CSV 或 Excel 文件。
    pub fn export_invoices(
        &self,
        request: ExportInvoicesRequest,
    ) -> Result<ExportResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(export_invoices(&db, request)?)
    }

    /// 将多张源发票合并到目标发票。
    pub fn merge_invoices(
        &self,
        target_invoice_id: i64,
        source_invoice_ids: Vec<i64>,
    ) -> Result<MergeInvoicesResult, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let result = merge_invoices(&mut db, target_invoice_id, source_invoice_ids)?;
        self.invalidate_dashboard_cache();
        Ok(result)
    }

    /// 导出发票 PDF 报表。
    pub fn export_pdf_report(
        &self,
        request: PdfReportRequest,
    ) -> Result<PdfReportResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(export_pdf_report(&db, request)?)
    }

    /// 添加目录监控任务。
    pub fn add_watch_dir(&self, request: AddWatchDirRequest) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.add_watch_dir(request)?)
    }

    /// 移除目录监控任务。
    pub fn remove_watch_dir(&self, id: i64) -> Result<(), AppError> {
        Ok(self.watcher_manager.remove_watch_dir(id)?)
    }

    /// 列出所有目录监控任务及其状态。
    pub fn list_watch_dirs(&self) -> Result<Vec<WatchDirStatus>, AppError> {
        Ok(self.watcher_manager.list_watch_dirs()?)
    }

    /// 更新目录监控任务的配置。
    pub fn update_watch_dir(
        &self,
        id: i64,
        request: UpdateWatchDirRequest,
    ) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.update_watch_dir(id, request)?)
    }

    /// 切换目录监控的启用/禁用状态。
    pub fn toggle_watch_dir(&self, id: i64, enabled: bool) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.toggle_watch_dir(id, enabled)?)
    }

    // --- Email Sources ---

    /// 添加邮件源配置。
    pub fn add_email_source(
        &self,
        request: AddEmailSourceRequest,
    ) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.add_email_source(request)?)
    }

    /// 更新邮件源配置。
    pub fn update_email_source(
        &self,
        id: i64,
        request: UpdateEmailSourceRequest,
    ) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.update_email_source(id, request)?)
    }

    /// 删除邮件源配置。
    pub fn remove_email_source(&self, id: i64) -> Result<(), AppError> {
        Ok(self.email_manager.remove_email_source(id)?)
    }

    /// 列出所有邮件源配置。
    pub fn list_email_sources(&self) -> Result<Vec<EmailSource>, AppError> {
        Ok(self.email_manager.list_email_sources()?)
    }

    /// 切换邮件源的启用/禁用状态。
    pub fn toggle_email_source(&self, id: i64, enabled: bool) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.toggle_email_source(id, enabled)?)
    }

    /// 同步指定邮件源的新邮件。
    pub fn sync_email_source(&self, id: i64) -> Result<EmailSyncResult, AppError> {
        Ok(self.email_manager.sync_email_source(id)?)
    }

    /// 同步所有已启用的邮件源。
    pub fn sync_all_email_sources(&self) -> Result<Vec<EmailSyncResult>, AppError> {
        Ok(self.email_manager.sync_all_enabled()?)
    }

    /// 测试邮件服务器连接。
    pub fn test_email_connection(
        &self,
        protocol: &str,
        host: &str,
        port: i64,
        username: &str,
        password: &str,
        auth_method: &str,
        use_ssl: bool,
        folder: &str,
    ) -> Result<EmailTestResult, AppError> {
        Ok(self.email_manager.test_connection(
            protocol,
            host,
            port,
            username,
            password,
            auth_method,
            use_ssl,
            folder,
        )?)
    }

    /// 获取仪表盘统计数据，结果带缓存。
    pub fn get_dashboard_stats(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<DashboardStats, AppError> {
        let key = cache_key(&date_from, &date_to);
        if let Some(stats) = self.dashboard_cache.get(&key) {
            return Ok(stats);
        }
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let stats = get_dashboard_stats(&db, date_from.as_deref(), date_to.as_deref())?;
        self.dashboard_cache.insert(key, stats.clone());
        Ok(stats)
    }

    fn invalidate_dashboard_cache(&self) {
        self.dashboard_cache.invalidate_all();
    }

    /// 更新 ChromaDB 向量搜索配置。
    pub fn set_chroma_config(&self, config: ChromaConfig) -> Result<(), AppError> {
        let mut cfg = self.chroma_config.lock().unwrap_or_else(|e| e.into_inner());
        *cfg = config;
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = event::create_event(
            &db,
            "config_change",
            "更新向量搜索配置",
            "",
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record config change event: {e}");
        }
        Ok(())
    }

    /// 获取当前 ChromaDB 配置。
    pub fn get_chroma_config(&self) -> ChromaConfig {
        self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 启用或禁用本地 Embedding 功能。
    pub fn set_embedding_enabled(&self, enabled: bool) -> Result<(), AppError> {
        {
            let mut flag = self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner());
            *flag = enabled;
        }
        let json = serde_json::json!({ "enabled": enabled });
        if let Err(e) = write_config(&self.paths.app_data_dir, "embedding_enabled.json", &json) {
            warn!("Failed to persist embedding enabled config: {e}");
        }

        // Keep this setter cheap for the settings UI. The ONNX engine can take
        // several seconds to initialize on Windows and is loaded on first use.
        if !enabled {
            let mut engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
            *engine_guard = None;
        }

        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let label = if enabled { "启用" } else { "禁用" };
        if let Err(e) = event::create_event(
            &db,
            "config_change",
            &format!("{label} 本地 Embedding"),
            "",
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record embedding toggle event: {e}");
        }
        Ok(())
    }

    /// 查询 Embedding 状态：是否启用、模型是否加载、模型目录和向量维度。
    pub fn embedding_status(&self) -> (bool, bool, Option<String>, Option<usize>) {
        let enabled = *self.embedding_enabled.lock().unwrap_or_else(|e| e.into_inner());
        match self.local_embedding.try_lock() {
            Ok(guard) => {
                let (model_loaded, model_dir, dimensions) = match *guard {
                    Some(ref engine) => (
                        true,
                        Some(engine.model_dir().to_string_lossy().into_owned()),
                        Some(engine.dimensions()),
                    ),
                    None => (false, None, None),
                };
                (enabled, model_loaded, model_dir, dimensions)
            }
            Err(_) => (enabled, false, None, None),
        }
    }

    /// Scan invoices and regenerate any missing preview thumbnails from normalized images.
    pub fn regenerate_missing_previews(&self) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(_) => return,
        };
        let preview_root = self.paths.thumbnails_dir.join("previews");
        let normalized_root = self.paths.thumbnails_dir.join("normalized");

        let rows: Vec<(i64, i64, Option<String>)> =
            match db.prepare("SELECT id, raw_file_id, source_page_range FROM invoices") {
                Ok(mut stmt) => match stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                }) {
                    Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                    Err(_) => return,
                },
                Err(_) => return,
            };

        let mut fixed = 0usize;
        for (invoice_id, raw_file_id, source_page_range) in rows {
            let preview_dir = preview_root.join(raw_file_id.to_string());
            let normalized_dir = normalized_root.join(raw_file_id.to_string());

            // Determine the expected preview filename
            let label = source_page_range
                .as_ref()
                .and_then(|r| r.split('-').next().and_then(|s| s.parse::<usize>().ok()))
                .map(|p| format!("page-{p}.jpg"))
                .unwrap_or_else(|| "image.jpg".to_string());

            let preview_path = preview_dir.join(&label);
            if preview_path.exists() {
                continue;
            }

            let normalized_path = normalized_dir.join(&label);
            if !normalized_path.exists() {
                continue;
            }

            // Regenerate preview from normalized image
            if let Err(e) = fs::create_dir_all(&preview_dir) {
                warn!("Invoice {invoice_id}: failed to create preview dir: {e}");
                continue;
            }
            match crate::document::resize_and_save(&normalized_path, &preview_path, 800, 85) {
                Ok(()) => {
                    fixed += 1;
                    info!("Regenerated missing preview for invoice {invoice_id}");
                }
                Err(e) => {
                    warn!("Invoice {invoice_id}: preview resize failed: {e}");
                }
            }
        }
        if fixed > 0 {
            info!("Regenerated {fixed} missing preview(s)");
        }
    }

    /// 更新自定义标签配置。
    pub fn set_badge_config(&self, config: BadgeConfig) -> Result<(), AppError> {
        let sanitized = sanitize_badge_config(config);
        {
            let mut cfg = self.badge_config.lock().unwrap_or_else(|e| e.into_inner());
            *cfg = sanitized.clone();
        }
        if let Err(e) = write_config(&self.paths.app_data_dir, "badge_config.json", &sanitized) {
            error!("Failed to persist badge config: {e}");
        }
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = event::create_event(
            &db,
            "config_change",
            "更新 Badge 配置",
            "",
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record badge config change event: {e}");
        }
        Ok(())
    }

    /// 获取当前标签配置。
    pub fn get_badge_config(&self) -> BadgeConfig {
        self.badge_config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 更新 LLM 调用价格配置。
    pub fn set_price_config(&self, config: PriceConfig) -> Result<(), AppError> {
        {
            let mut cfg = self.price_config.lock().unwrap_or_else(|e| e.into_inner());
            *cfg = config.clone();
        }
        if let Err(e) = write_config(&self.paths.app_data_dir, "price_config.json", &config) {
            error!("Failed to persist price config: {e}");
        }
        Ok(())
    }

    /// 获取当前价格配置。
    pub fn get_price_config(&self) -> PriceConfig {
        self.price_config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 获取 LLM 用量统计数据，结果带缓存。
    pub fn get_llm_usage(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<crate::extractor::LlmUsageStats, AppError> {
        let key = cache_key(&date_from, &date_to);
        if let Some(stats) = self.llm_usage_cache.get(&key) {
            return Ok(stats);
        }
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let stats = crate::extractor::get_llm_usage(&db, date_from.as_deref(), date_to.as_deref())?;
        self.llm_usage_cache.insert(key, stats.clone());
        Ok(stats)
    }

    fn invalidate_llm_usage_cache(&self) {
        self.llm_usage_cache.invalidate_all();
    }

    /// 设置单张发票的标签值。
    pub fn set_invoice_badge(
        &self,
        invoice_id: i64,
        group_name: String,
        value: Option<String>,
    ) -> Result<Vec<InvoiceBadgeSelection>, AppError> {
        let mut db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(set_invoice_badge(&mut db, invoice_id, group_name, value)?)
    }

    /// 更新 LLM 服务配置（API 地址、密钥等）。
    pub fn set_llm_config(&self, config: LlmProviderConfig) -> Result<(), AppError> {
        let mut cfg = self.llm_config.lock().unwrap_or_else(|e| e.into_inner());
        *cfg = Some(config.clone());
        if let Err(e) = write_config(&self.paths.app_data_dir, "llm_config.json", &config) {
            error!("Failed to persist LLM config: {e}");
        }
        info!("LLM config updated");
        Ok(())
    }

    /// 获取当前 LLM 配置。
    pub fn get_llm_config(&self) -> Option<LlmProviderConfig> {
        self.llm_config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 获取 LLM 审计配置（启用时返回审计目录）。
    pub fn llm_audit_config(&self) -> Option<LlmAuditConfig> {
        (*self.llm_audit_enabled.lock().unwrap_or_else(|e| e.into_inner())).then(|| LlmAuditConfig {
            dir: self.paths.llm_audit_dir.clone(),
        })
    }

    /// 启用或禁用 LLM 请求审计记录。
    pub fn set_llm_audit_enabled(&self, enabled: bool) {
        *self.llm_audit_enabled.lock().unwrap_or_else(|e| e.into_inner()) = enabled;
        let json = serde_json::json!({"enabled": enabled});
        if let Err(e) = write_config(&self.paths.app_data_dir, "audit_config.json", &json) {
            error!("Failed to persist audit config: {e}");
        }
        info!("LLM audit enabled: {enabled}");
    }

    /// 查询 LLM 审计是否启用。
    pub fn get_llm_audit_enabled(&self) -> bool {
        *self.llm_audit_enabled.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 测试 ChromaDB 连接状态。
    pub fn test_chroma_connection(&self) -> Result<bool, AppError> {
        Ok(self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).enabled)
    }

    /// 测试本地 Embedding 引擎连通性。
    pub fn test_embedding_connection(&self) -> Result<EmbeddingTestResult, AppError> {
        self.load_embedding_engine_if_available()?;
        let mut guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
        let engine = guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;
        Ok(run_embedding_test(engine)?)
    }

    /// 全量重新生成所有发票的 Embedding 向量。
    pub fn regenerate_all_embeddings(&self) -> Result<RegenerateEmbeddingsResult, AppError> {
        self.load_embedding_engine_if_available()?;
        let mut engine_guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
        let engine = engine_guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;

        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let thumb_dir = self.paths.thumbnails_dir.clone();

        let invoice_ids: Vec<i64> = {
            let mut stmt = db.prepare("SELECT id FROM invoices")?;
            let ids: Vec<i64> = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        let total = invoice_ids.len();

        // Reset all embedding flags
        db.execute("UPDATE invoices SET has_embedding = 0", [])?;

        let mut success_count = 0usize;
        let mut failure_count = 0usize;

        for invoice_id in invoice_ids {
            let detail = match get_invoice_detail(&db, &thumb_dir, invoice_id) {
                Ok(d) => d,
                Err(e) => {
                    warn!("Failed to get invoice detail for {invoice_id}: {e}");
                    failure_count += 1;
                    continue;
                }
            };
            let text = invoice_to_embedding_text(&detail);
            match generate_embedding(engine, &text) {
                Ok(result) => {
                    if let Err(e) =
                        chroma::upsert_embedding(&db, invoice_id, &result.embedding, &text)
                    {
                        warn!("Failed to upsert embedding for invoice {invoice_id}: {e}");
                        failure_count += 1;
                        continue;
                    }
                    let _ = db.execute(
                        "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
                        [invoice_id],
                    );
                    let _ = crate::extractor::insert_usage_log(
                        &db,
                        "embedding",
                        EMBEDDING_MODEL_NAME,
                        result.prompt_tokens,
                        result.total_tokens.saturating_sub(result.prompt_tokens),
                        result.total_tokens,
                    );
                    success_count += 1;
                }
                Err(e) => {
                    warn!("Failed to generate embedding for invoice {invoice_id}: {e}");
                    failure_count += 1;
                }
            }
        }

        Ok(RegenerateEmbeddingsResult {
            total_invoices: total,
            success_count,
            failure_count,
        })
    }

    /// 基于语义向量搜索相似发票。
    pub fn search_invoices_semantic(
        &self,
        query: String,
        limit: usize,
    ) -> Result<Vec<crate::chroma::SimilarResult>, AppError> {
        if !self.chroma_config.lock().unwrap_or_else(|e| e.into_inner()).enabled {
            return Err(AppError::Chroma(chroma::ChromaError::NotConfigured));
        }
        let mut guard = self.local_embedding.lock().unwrap_or_else(|e| e.into_inner());
        let engine = guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;
        let result = generate_embedding(engine, &query)?;
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(chroma::query_similar(&db, &result.embedding, limit)?)
    }

    /// 获取待识别的原始文件元数据。
    pub fn raw_file_for_recognition(
        &self,
        raw_file_id: i64,
    ) -> Result<RawFileForRecognition, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let raw_file = db.query_row(
            "SELECT id, original_name, COALESCE(mime_type, ''), storage_path
            FROM raw_files
            WHERE id = ?1",
            [raw_file_id],
            |row| {
                Ok(RawFileForRecognition {
                    id: row.get(0)?,
                    original_name: row.get(1)?,
                    mime_type: row.get(2)?,
                    storage_path: PathBuf::from(row.get::<_, String>(3)?),
                })
            },
        )?;

        Ok(raw_file)
    }

    /// 渲染 PDF 文件为图片页面用于 OCR 识别。
    pub fn render_pdf_pages_for_recognition(
        &self,
        raw_file_id: i64,
        pdf_path: &Path,
    ) -> Result<Vec<RenderedPdfPage>, AppError> {
        Ok(render_pdf_pages(
            pdf_path,
            &self.paths.thumbnails_dir,
            raw_file_id,
        )?)
    }

    /// 预处理图片用于识别（缩放、标准化）。
    pub fn prepare_image_for_recognition(
        &self,
        raw_file_id: i64,
        image_path: &Path,
        page_number: Option<usize>,
    ) -> Result<PreparedImage, AppError> {
        Ok(prepare_image_for_recognition(
            image_path,
            &self.paths.thumbnails_dir,
            raw_file_id,
            page_number,
        )?)
    }

    // --- Event methods ---

    /// 分页查询事件列表。
    pub fn list_events(
        &self,
        page: i64,
        page_size: i64,
        event_type: Option<&str>,
    ) -> Result<EventListResult, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::list_events(&db, page, page_size, event_type)?)
    }

    /// 获取未读事件数量。
    pub fn get_unread_event_count(&self) -> Result<i64, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::get_unread_event_count(&db)?)
    }

    /// 获取未读的导入失败事件数量。
    pub fn get_unread_failed_import_event_count(&self) -> Result<i64, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::get_unread_failed_import_event_count(&db)?)
    }

    /// 标记单个事件为已读。
    pub fn mark_event_read(&self, id: i64) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::mark_event_read(&db, id)?)
    }

    /// 标记所有事件为已读。
    pub fn mark_all_events_read(&self) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::mark_all_events_read(&db)?)
    }

    /// 删除所有事件记录。
    pub fn delete_all_events(&self) -> Result<usize, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::delete_all_events(&db)?)
    }

    /// 记录 LLM 调用的 Token 用量。
    pub fn record_usage_log(
        &self,
        operation: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
    ) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        crate::extractor::insert_usage_log(
            &db,
            operation,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
        )?;
        self.invalidate_llm_usage_cache();
        Ok(())
    }

    /// 记录识别完成事件。
    pub fn record_recognition_event(
        &self,
        invoice_id: i64,
        invoice_title: &str,
        success: bool,
        duration_ms: u128,
        model: &str,
        page_count: usize,
    ) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(event::record_recognition_event(
            &db,
            invoice_id,
            invoice_title,
            success,
            duration_ms,
            model,
            page_count,
        )?)
    }

    // --- Agent methods ---

    /// 创建新的 Agent 会话。
    pub fn create_agent_session(&self) -> Result<AgentSession, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let session = agent::create_session(&db, None)?;
        // Create temp directory for this session
        let _ = paths::session_temp_dir(&self.paths.sessions_dir, &session.uuid);
        Ok(session)
    }

    /// 列出所有 Agent 会话。
    pub fn list_agent_sessions(&self) -> Result<Vec<AgentSession>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::list_sessions(&db)?)
    }

    /// 获取指定 Agent 会话的消息历史。
    pub fn get_agent_session(&self, session_id: i64) -> Result<Vec<AgentMessageRow>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::get_session_messages(&db, session_id)?)
    }

    /// 更新 Agent 会话标题。
    pub fn update_agent_session_title(
        &self,
        session_id: i64,
        title: &str,
    ) -> Result<AgentSession, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::set_session_title(&db, session_id, title)?)
    }

    /// 导出应用日志、数据库和配置文件为 ZIP 包。
    pub fn export_logs(&self, output_path: &str) -> Result<ExportLogsResult, AppError> {
        info!("Exporting logs to {}", output_path);
        let output_path = Path::new(output_path);
        let file = std::fs::File::create(output_path)?;
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Add database file
        if self.paths.database_path.exists() {
            stream_file_to_zip(
                &mut zip_writer,
                "invoicevault.sqlite3",
                &self.paths.database_path,
                options,
            )?;
        }

        // Add config files
        for config_name in &[
            "llm_config.json",
            "embedding_enabled.json",
            "recognition_config.json",
            "badge_config.json",
        ] {
            let config_path = self.paths.app_data_dir.join(config_name);
            if config_path.exists() {
                stream_file_to_zip(&mut zip_writer, config_name, &config_path, options)?;
            }
        }

        // Add log files
        if self.paths.logs_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.paths.logs_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            stream_file_to_zip(
                                &mut zip_writer,
                                &format!("logs/{}", name),
                                &path,
                                options,
                            )?;
                        }
                    }
                }
            }
        }

        if self.paths.llm_audit_dir.exists() {
            add_dir_to_zip(
                &mut zip_writer,
                &self.paths.app_data_dir,
                &self.paths.llm_audit_dir,
                options,
            )?;
        }

        // Add system info
        let health = self.health()?;
        let sys_info = format!(
            "InvoiceVault System Info\n\
             ======================\n\
             App Data Dir: {}\n\
             Database Path: {}\n\
             Migration Version: {}\n",
            health.app_data_dir, health.database_path, health.migration_version,
        );
        zip_writer
            .start_file("system_info.txt", options)
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        zip_writer
            .write_all(sys_info.as_bytes())
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let finished = zip_writer
            .finish()
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let metadata = finished.metadata()?;
        let file_size = metadata.len();

        info!("Log export complete: {} bytes", file_size);

        // Record event
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = event::create_event(
            &db,
            "export",
            "导出日志",
            &format!(
                "日志已导出至 {}，大小 {} 字节",
                output_path.display(),
                file_size
            ),
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record log export event: {e}");
        }

        Ok(ExportLogsResult {
            file_path: output_path.to_string_lossy().into_owned(),
            byte_size: file_size,
        })
    }

    /// 导出完整数据备份（排除大型可再生文件）。
    pub fn export_backup(&self, output_path: &str) -> Result<ExportLogsResult, AppError> {
        info!("Creating full backup at {}", output_path);
        let output_path = Path::new(output_path);
        let file = std::fs::File::create(output_path)?;
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let base = &self.paths.app_data_dir;
        // Skip large/regenerable directories to keep backup small
        let skip_dirs = [
            "storage",      // raw file archives (large binary files)
            "WebKitCache",  // webview cache (regenerable)
            DIR_MODELS,     // ONNX embedding models (downloadable)
            "localStorage", // webview local storage
            "CacheStorage", // webview cache storage
            "sample",       // sample/test files
        ];
        add_dir_to_zip_with_skip(&mut zip_writer, base, base, options, &skip_dirs)?;

        let finished = zip_writer
            .finish()
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let metadata = finished.metadata()?;
        let file_size = metadata.len();

        info!("Backup complete: {} bytes", file_size);

        // Record event
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = event::create_event(
            &db,
            "export",
            "基础备份",
            &format!(
                "基础备份已导出至 {}，大小 {} 字节",
                output_path.display(),
                file_size
            ),
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record backup export event: {e}");
        }

        Ok(ExportLogsResult {
            file_path: output_path.to_string_lossy().into_owned(),
            byte_size: file_size,
        })
    }

    /// 清理孤立文件和失效数据库记录以释放存储空间。
    pub fn cleanup_storage(&self) -> Result<CleanupStorageResult, AppError> {
        info!("Starting storage cleanup");
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());

        // Collect all known storage paths from raw_files table
        let mut stmt = db.prepare("SELECT id, storage_path FROM raw_files")?;
        let db_records: Vec<(i64, PathBuf)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    PathBuf::from(row.get::<_, String>(1)?),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let db_paths: std::collections::HashSet<PathBuf> =
            db_records.iter().map(|(_, p)| p.clone()).collect();

        // Scan raw_dir for all files
        let mut disk_files: Vec<PathBuf> = Vec::new();
        walk_dir(&self.paths.raw_dir, &mut disk_files)?;

        // 1. Remove files on disk not in DB (orphan files)
        let mut files_removed = 0_usize;
        let mut bytes_freed = 0_u64;
        for disk_file in &disk_files {
            if !db_paths.contains(disk_file) {
                if let Ok(meta) = std::fs::metadata(disk_file) {
                    bytes_freed += meta.len();
                }
                if std::fs::remove_file(disk_file).is_ok() {
                    files_removed += 1;
                }
            }
        }

        // 2. Remove DB records where file doesn't exist (orphan records)
        let mut db_records_removed = 0_usize;
        let mut orphan_ids: Vec<i64> = Vec::new();
        for (id, storage_path) in &db_records {
            if !storage_path.exists() {
                orphan_ids.push(*id);
            }
        }

        for raw_id in &orphan_ids {
            // Delete dependent records and the raw_file itself in order
            if let Err(e) = db.execute("DELETE FROM import_jobs WHERE raw_file_id = ?1", [*raw_id])
            {
                warn!("Failed to delete import_jobs for orphan raw_file {raw_id}: {e}");
            }
            if let Err(e) = db.execute(
                "DELETE FROM extraction_runs WHERE raw_file_id = ?1",
                [*raw_id],
            ) {
                warn!("Failed to delete extraction_runs for orphan raw_file {raw_id}: {e}");
            }
            if let Err(e) = db.execute("DELETE FROM invoices WHERE raw_file_id = ?1", [*raw_id]) {
                warn!("Failed to delete invoices for orphan raw_file {raw_id}: {e}");
            }
            if let Err(e) = db.execute("DELETE FROM raw_files WHERE id = ?1", [*raw_id]) {
                warn!("Failed to delete orphan raw_file {raw_id}: {e}");
            }
            db_records_removed += 1;
        }

        // Clean up empty year/month directories under raw_dir
        remove_empty_dirs(&self.paths.raw_dir);

        info!(
            "Storage cleanup done: {} files removed, {} db records removed, {} bytes freed",
            files_removed, db_records_removed, bytes_freed,
        );

        let result = CleanupStorageResult {
            files_removed,
            db_records_removed,
            bytes_freed,
        };

        // Record event
        if let Err(e) = event::create_event(
            &db,
            "agent",
            "存储清理",
            &format!(
                "清理了 {} 个失效文件（{} 字节），{} 条失效数据库记录",
                files_removed, bytes_freed, db_records_removed,
            ),
            "completed",
            None,
            None,
            None,
        ) {
            warn!("Failed to record storage cleanup event: {e}");
        }

        Ok(result)
    }

    /// 检查原始文件是否已关联发票。
    pub fn raw_file_has_invoices(&self, raw_file_id: i64) -> Result<bool, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM invoices WHERE raw_file_id = ?1",
            [raw_file_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// 根据原始文件 ID 更新关联导入任务的状态。
    pub fn set_import_job_status_for_raw_file(
        &self,
        raw_file_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(update_import_job_status_by_raw_file(
            &db,
            raw_file_id,
            status,
            error_message,
        )?)
    }

    /// 根据原始文件 ID 查找关联的发票 ID。
    pub fn invoice_id_for_raw_file(&self, raw_file_id: i64) -> Result<Option<i64>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let invoice_id = db
            .query_row(
                "SELECT id FROM invoices WHERE raw_file_id = ?1 ORDER BY id DESC LIMIT 1",
                [raw_file_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(invoice_id)
    }

    /// 为 Agent 会话附加文件（仅支持 xlsx/csv）。
    pub fn attach_agent_file(
        &self,
        session_id: i64,
        source_path: &str,
    ) -> Result<AgentAttachment, AppError> {
        let source = Path::new(source_path);
        let original_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("attachment")
            .to_owned();
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "xlsx" | "csv") {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "目前 Agent 仅支持上传 xlsx/csv 表格",
            )));
        }

        let session_dir = self.paths.agent_uploads_dir.join(session_id.to_string());
        fs::create_dir_all(&session_dir)?;
        let stored_name = format!(
            "{}-{}",
            chrono::Utc::now().timestamp_millis(),
            sanitize_filename(&original_name)
        );
        let dest = session_dir.join(stored_name);
        fs::copy(source, &dest)?;
        let metadata = fs::metadata(&dest)?;
        let mime = mime_guess::from_path(&dest)
            .first_raw()
            .map(|value| value.to_owned());

        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::insert_attachment(
            &db,
            session_id,
            &original_name,
            mime.as_deref(),
            metadata.len() as i64,
            &display_path(&dest),
        )?)
    }

    /// 列出 Agent 会话的所有附件。
    pub fn list_agent_attachments(
        &self,
        session_id: i64,
    ) -> Result<Vec<AgentAttachment>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::list_session_attachments(&db, session_id)?)
    }

    /// 删除 Agent 会话附件。
    pub fn remove_agent_attachment(&self, attachment_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::delete_attachment(&db, attachment_id)?)
    }

    /// 列出 Agent 会话的任务记录。
    pub fn list_agent_tasks(&self, session_id: i64) -> Result<Vec<AgentTask>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::list_session_tasks(&db, session_id)?)
    }

    /// 列出 Agent 会话的产物（导出文件等）。
    pub fn list_agent_artifacts(&self, session_id: i64) -> Result<Vec<AgentArtifact>, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::list_session_artifacts(&db, session_id)?)
    }

    /// 用系统默认程序打开 Agent 产物文件。
    pub fn open_agent_artifact_file(
        &self,
        session_id: i64,
        artifact_id: i64,
    ) -> Result<(), AppError> {
        let path = self.agent_artifact_path(session_id, artifact_id)?;
        if !path.exists() {
            return Err(AppError::InvalidOperation(format!(
                "产物文件不存在：{}",
                path.display()
            )));
        }
        open_path_with_system(&path)
    }

    /// 用系统文件管理器打开 Agent 产物所在目录。
    pub fn open_agent_artifact_folder(
        &self,
        session_id: i64,
        artifact_id: i64,
    ) -> Result<(), AppError> {
        let path = self.agent_artifact_path(session_id, artifact_id)?;
        let folder = if path.is_dir() {
            path
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| AppError::InvalidOperation("产物路径没有父目录".to_owned()))?
        };
        if !folder.exists() {
            return Err(AppError::InvalidOperation(format!(
                "产物目录不存在：{}",
                folder.display()
            )));
        }
        open_path_with_system(&folder)
    }

    /// 删除 Agent 产物记录。
    pub fn delete_agent_artifact(&self, session_id: i64, artifact_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        agent::delete_artifact(&db, session_id, artifact_id)?;
        Ok(())
    }

    fn agent_artifact_path(&self, session_id: i64, artifact_id: i64) -> Result<PathBuf, AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let artifact = agent::get_artifact(&db, artifact_id)?;
        if artifact.session_id != session_id {
            return Err(AppError::InvalidOperation("产物不属于当前会话".to_owned()));
        }
        let path = artifact
            .file_path
            .ok_or_else(|| AppError::InvalidOperation("产物没有可打开的文件路径".to_owned()))?;
        Ok(PathBuf::from(path))
    }

    /// 删除 Agent 会话及其所有关联数据。
    pub fn delete_agent_session(&self, session_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        Ok(agent::delete_session(&db, session_id)?)
    }

    /// 向 Agent 会话发送消息并获取回复。
    pub async fn send_agent_message(
        &self,
        session_id: i64,
        content: &str,
        attachment_ids: Vec<i64>,
        config: &crate::llm::LlmProviderConfig,
    ) -> Result<AgentResponse, AppError> {
        let attachment_context = self.agent_attachment_context(session_id, &attachment_ids)?;
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
            self.paths.sessions_dir.clone(),
            Arc::clone(&self.db),
            Arc::clone(&self.badge_config),
            Arc::clone(&self.price_config),
            self.app_handle.clone(),
            session_id,
        ));
        Ok(agent::run_agent_turn(
            &self.db,
            session_id,
            content,
            attachment_ids,
            attachment_context,
            config,
            self.llm_audit_config(),
            executor,
        )
        .await?)
    }

    /// 向 Agent 会话发送消息并以流式方式获取回复。
    pub async fn send_agent_message_stream(
        &self,
        session_id: i64,
        content: &str,
        attachment_ids: Vec<i64>,
        config: &crate::llm::LlmProviderConfig,
        stream_sink: agent::AgentStreamSink,
    ) -> Result<AgentResponse, AppError> {
        let attachment_context = self.agent_attachment_context(session_id, &attachment_ids)?;
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
            self.paths.sessions_dir.clone(),
            Arc::clone(&self.db),
            Arc::clone(&self.badge_config),
            Arc::clone(&self.price_config),
            self.app_handle.clone(),
            session_id,
        ));
        Ok(agent::run_agent_turn_stream(
            &self.db,
            session_id,
            content,
            attachment_ids,
            attachment_context,
            config,
            self.llm_audit_config(),
            executor,
            stream_sink,
        )
        .await?)
    }

    fn agent_attachment_context(
        &self,
        session_id: i64,
        attachment_ids: &[i64],
    ) -> Result<Option<String>, AppError> {
        if attachment_ids.is_empty() {
            return Ok(None);
        }
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let mut lines = vec!["用户本次消息附带了以下文件，可使用 list_message_attachments 和 inspect_spreadsheet 工具读取：".to_owned()];
        for id in attachment_ids {
            let attachment = agent::get_attachment(&db, *id)?;
            if attachment.session_id != session_id {
                continue;
            }
            lines.push(format!(
                "- attachment_id={} name={} mime={} size={} bytes",
                attachment.id,
                attachment.original_name,
                attachment.mime_type.as_deref().unwrap_or("unknown"),
                attachment.byte_size
            ));
        }
        Ok(Some(lines.join("\n")))
    }

    /// 确认或拒绝 Agent 待执行的操作。
    pub async fn confirm_agent_action(
        &self,
        request: ConfirmRequest,
        config: &crate::llm::LlmProviderConfig,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
            self.paths.sessions_dir.clone(),
            Arc::clone(&self.db),
            Arc::clone(&self.badge_config),
            Arc::clone(&self.price_config),
            self.app_handle.clone(),
            request.session_id,
        ));
        Ok(agent::continue_agent_turn(
            &self.db,
            request.session_id,
            request.confirmed,
            request.extra_params,
            config,
            self.llm_audit_config(),
            executor,
        )
        .await?)
    }

    /// 确认或拒绝 Agent 待执行的操作（流式版本）。
    pub async fn confirm_agent_action_stream(
        &self,
        request: ConfirmRequest,
        config: &crate::llm::LlmProviderConfig,
        stream_sink: agent::AgentStreamSink,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
            self.paths.sessions_dir.clone(),
            Arc::clone(&self.db),
            Arc::clone(&self.badge_config),
            Arc::clone(&self.price_config),
            self.app_handle.clone(),
            request.session_id,
        ));
        Ok(agent::continue_agent_turn_stream(
            &self.db,
            request.session_id,
            request.confirmed,
            request.extra_params,
            config,
            self.llm_audit_config(),
            executor,
            stream_sink,
        )
        .await?)
    }
}

/// Generate embedding for an invoice and persist the result.
///
/// This is the shared logic used by recognition, save, update, and regeneration paths.
/// It handles: embedding generation → ChromaDB upsert → mark invoice → semantic dedupe → usage log.
/// Runs asynchronously via `tauri::async_runtime::spawn`.
fn spawn_embedding_for_invoice(
    db: Arc<Mutex<Connection>>,
    engine: &mut crate::embedding::LocalEmbeddingEngine,
    invoice_id: i64,
    detail: &crate::extractor::InvoiceDetail,
) {
    let text = crate::extractor::invoice_to_embedding_text(detail);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate_embedding(engine, &text)
    }));
    let Ok(Ok(result)) = result else { return };
    let embedding = result.embedding;
    let prompt_tokens = result.prompt_tokens;
    let total_tokens = result.total_tokens;
    tauri::async_runtime::spawn(async move {
        let Ok(conn) = db.lock() else { return };
        if let Err(e) = chroma::upsert_embedding(&conn, invoice_id, &embedding, &text) {
            warn!("Failed to upsert embedding for invoice {invoice_id}: {e}");
        }
        if let Err(e) = conn.execute(
            "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
            [invoice_id],
        ) {
            warn!("Failed to mark invoice {invoice_id} as having embedding: {e}");
        }
        if let Ok(similar) = chroma::query_similar(&conn, &embedding, 5) {
            if let Err(e) = crate::dedupe::detect_semantic_duplicates(&conn, invoice_id, &similar) {
                warn!("Failed to detect semantic duplicates for invoice {invoice_id}: {e}");
            }
        }
        if let Err(e) = crate::extractor::insert_usage_log(
            &conn,
            "embedding",
            EMBEDDING_MODEL_NAME,
            prompt_tokens,
            total_tokens.saturating_sub(prompt_tokens),
            total_tokens,
        ) {
            warn!("Failed to insert embedding usage log: {e}");
        }
    });
}

/// Validate that a file path is safe for agent tool access.
/// Returns Ok(()) if the path is within the app data directory or a temp directory.
/// Rejects paths with ".." components to prevent traversal.
fn validate_agent_path(path: &str, app_data_dir: &std::path::Path) -> Result<(), String> {
    let p = std::path::Path::new(path);
    // Reject obvious traversal attempts
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("路径不允许包含 ..".to_owned());
    }
    // Allow paths within app data directory
    if p.starts_with(app_data_dir) {
        return Ok(());
    }
    // Allow temp directory
    if p.starts_with(std::env::temp_dir()) {
        return Ok(());
    }
    // Allow desktop (common export destination)
    if let Some(desktop) = dirs::desktop_dir() {
        if p.starts_with(&desktop) {
            return Ok(());
        }
    }
    Err(format!("路径不在允许的目录中: {}", path))
}

fn make_tool_executor(
    thumbnails_dir: std::path::PathBuf,
    app_data_dir: std::path::PathBuf,
    sessions_dir: std::path::PathBuf,
    db: Arc<Mutex<Connection>>,
    badge_config: Arc<Mutex<BadgeConfig>>,
    price_config: Arc<Mutex<PriceConfig>>,
    app_handle: AppHandle,
    session_id: i64,
) -> impl Fn(&str, &serde_json::Value) -> ToolExecResult {
    // Look up session UUID once for temp dir resolution
    let session_uuid = {
        let conn = db.lock().unwrap_or_else(|e| e.into_inner());
        agent::get_session(&conn, session_id)
            .ok()
            .map(|s| s.uuid)
            .unwrap_or_default()
    };
    move |tool_name: &str, args: &serde_json::Value| match tool_name {
        "search_invoices" => {
            let params = InvoiceSearchParams {
                query: args.get("query").and_then(|v| v.as_str()).map(String::from),
                date_from: args
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                date_to: args
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                seller_name: args
                    .get("seller_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                invoice_type: args
                    .get("invoice_type")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                category: args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                tag: args.get("tag").and_then(|v| v.as_str()).map(String::from),
                status: args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                page: args.get("page").and_then(|v| v.as_i64()),
                page_size: args.get("page_size").and_then(|v| v.as_i64()),
                buyer_name: args
                    .get("buyer_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                invoice_number: args
                    .get("invoice_number")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                amount_min: args
                    .get("amount_min")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                amount_max: args
                    .get("amount_max")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                duplicate_status: args
                    .get("duplicate_status")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                sort_by: args
                    .get("sort_by")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                sort_order: args
                    .get("sort_order")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match search_invoices(&conn, params) {
                Ok(result) => {
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    let truncated = crate::llm::truncate(&content, 4000);
                    ToolExecResult::Success { content: truncated }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_invoice_detail" => {
            let Some(invoice_id) = args.get("invoice_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 invoice_id 参数".to_owned(),
                };
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match get_invoice_detail(&conn, &thumbnails_dir, invoice_id) {
                Ok(detail) => {
                    let content = serde_json::to_string(&detail).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_dashboard_stats" => {
            let date_from = args
                .get("date_from")
                .and_then(|v| v.as_str())
                .map(String::from);
            let date_to = args
                .get("date_to")
                .and_then(|v| v.as_str())
                .map(String::from);
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match get_dashboard_stats(&conn, date_from.as_deref(), date_to.as_deref()) {
                Ok(stats) => {
                    let content = serde_json::to_string(&stats).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_current_date_context" => {
            let now = Local::now().date_naive();
            let month_start = now.with_day(1).unwrap_or(now);
            let next_month = if now.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap_or(month_start)
            } else {
                chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                    .unwrap_or(month_start)
            };
            let month_end = next_month.pred_opt().unwrap_or(now);
            let content = serde_json::json!({
                "today": now.to_string(),
                "current_month": {
                    "date_from": month_start.to_string(),
                    "date_to": month_end.to_string()
                },
                "year": now.year(),
                "month": now.month()
            })
            .to_string();
            ToolExecResult::Success { content }
        }
        "get_invoice_field_catalog" => {
            let content = serde_json::to_string(&export_column_catalog()).unwrap_or_default();
            ToolExecResult::Success { content }
        }
        "list_message_attachments" => {
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match agent::list_session_attachments(&conn, session_id) {
                Ok(attachments) => {
                    let content = serde_json::to_string(&attachments).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "inspect_spreadsheet" => {
            let Some(attachment_id) = args.get("attachment_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 attachment_id 参数".to_owned(),
                };
            };
            let max_rows = args
                .get("max_rows")
                .and_then(|v| v.as_u64())
                .unwrap_or(5)
                .clamp(1, 20) as usize;
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let attachment = match agent::get_attachment(&conn, attachment_id) {
                Ok(attachment) if attachment.session_id == session_id => attachment,
                Ok(_) => {
                    return ToolExecResult::Error {
                        message: "附件不属于当前会话".to_owned(),
                    }
                }
                Err(e) => {
                    return ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            };
            drop(conn);
            match inspect_spreadsheet_attachment(&attachment, max_rows) {
                Ok(result) => {
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error { message: e },
            }
        }
        "create_export_preview" => {
            let request = export_preview_request_from_args(args);
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match preview_export(&conn, request) {
                Ok(preview) => {
                    let content = serde_json::to_string(&preview).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "validate_xlsx" => {
            let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 file_path 参数".to_owned(),
                };
            };
            if let Err(e) = validate_agent_path(file_path, &app_data_dir) {
                return ToolExecResult::Error { message: e };
            }
            match crate::template_engine::validate_xlsx(file_path) {
                Ok(report) if report.valid => ToolExecResult::Success {
                    content: "XLSX XML 验证通过，所有 XML 文件结构合法".to_owned(),
                },
                Ok(report) => {
                    let error_lines: Vec<String> = report
                        .errors
                        .iter()
                        .map(|e| format!("[{}] {}: {}", e.file, e.line, e.message))
                        .collect();
                    ToolExecResult::Error {
                        message: format!(
                            "XLSX XML 验证失败，发现 {} 个错误：\n{}\n\
                             请分析错误原因，修复模板引擎的 XML 生成逻辑后重新导出并验证。",
                            report.errors.len(),
                            error_lines.join("\n"),
                        ),
                    }
                }
                Err(e) => ToolExecResult::Error {
                    message: format!("验证过程出错: {}", e),
                },
            }
        }
        "generate_template_plan" => {
            let Some(attachment_id) = args.get("attachment_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 attachment_id 参数".to_owned(),
                };
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let attachment = match agent::get_attachment(&conn, attachment_id) {
                Ok(attachment) if attachment.session_id == session_id => attachment,
                Ok(_) => {
                    return ToolExecResult::Error {
                        message: "附件不属于当前会话".to_owned(),
                    }
                }
                Err(e) => {
                    return ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            };
            drop(conn);

            match template_adapter::generate_plan_from_attachment(&attachment) {
                Ok(Some(plan)) => {
                    let content = serde_json::to_string_pretty(&plan)
                        .unwrap_or_else(|_| "序列化失败".to_owned());
                    ToolExecResult::Success { content }
                }
                Ok(None) => ToolExecResult::Error {
                    message: "模板表头未匹配到可导出的发票字段".to_owned(),
                },
                Err(e) => ToolExecResult::Error { message: e },
            }
        }
        "export_invoices_with_template" => {
            let Some(attachment_id) = args.get("attachment_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 attachment_id 参数".to_owned(),
                };
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let attachment = match agent::get_attachment(&conn, attachment_id) {
                Ok(attachment) if attachment.session_id == session_id => attachment,
                Ok(_) => {
                    return ToolExecResult::Error {
                        message: "附件不属于当前会话".to_owned(),
                    }
                }
                Err(e) => {
                    return ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            };
            drop(conn);

            // Use provided plan or generate one via heuristic
            let plan = if let Some(plan_json) = args.get("plan") {
                match serde_json::from_value::<crate::template_engine::plan::TemplatePlan>(
                    plan_json.clone(),
                ) {
                    Ok(plan) => plan,
                    Err(e) => {
                        return ToolExecResult::Error {
                            message: format!("plan 参数解析失败: {e}"),
                        }
                    }
                }
            } else {
                match template_adapter::generate_plan_from_attachment(&attachment) {
                    Ok(Some(plan)) => plan,
                    Ok(None) => {
                        return ToolExecResult::Error {
                            message: "模板表头未匹配到可导出的发票字段".to_owned(),
                        }
                    }
                    Err(e) => return ToolExecResult::Error { message: e },
                }
            };

            // Build column map from plan for the DataSource
            let matched_keys: Vec<(usize, String)> = plan
                .columns
                .iter()
                .map(|c| (c.col, c.field_key.clone()))
                .collect();
            let column_map = template_adapter::resolve_column_defs(&matched_keys);

            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let conn = db.lock().unwrap_or_else(|e| e.into_inner());
                let rows = template_adapter::load_invoices(
                    &conn,
                    json_i64_vec(args, "invoice_ids").as_deref(),
                    json_string(args, "date_from").as_deref(),
                    json_string(args, "date_to").as_deref(),
                )
                .unwrap_or_default();
                let row_count = rows.len();
                let mapped_count = plan.columns.len();
                let mut warnings = plan.warnings.clone();
                if plan.confidence < 0.8 {
                    warnings.push(format!("置信度: {:.0}%", plan.confidence * 100.0));
                }
                let warning_text = if warnings.is_empty() {
                    String::new()
                } else {
                    format!("\n注意: {}", warnings.join("; "))
                };

                // Check if user already specified a path
                let has_user_path = args
                    .get("output_path")
                    .and_then(|v| v.as_str())
                    .is_some();

                // Store the plan in arguments so it's available after confirmation
                let mut confirm_args = args.clone();
                confirm_args["plan"] =
                    serde_json::to_value(&plan).unwrap_or(serde_json::Value::Null);

                let options = if has_user_path {
                    None // User already specified path, no options needed
                } else {
                    Some(vec![
                        crate::agent::ConfirmOption {
                            label: "保存到桌面".into(),
                            value: "desktop".into(),
                            style: Some("primary".into()),
                        },
                        crate::agent::ConfirmOption {
                            label: "选择位置...".into(),
                            value: "pick_path".into(),
                            style: Some("secondary".into()),
                        },
                        crate::agent::ConfirmOption {
                            label: "取消".into(),
                            value: "cancel".into(),
                            style: Some("danger".into()),
                        },
                    ])
                };

                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_invoices_with_template".to_owned(),
                    arguments: confirm_args,
                    message: format!(
                        "将按模板「{}」导出 {row_count} 张发票（已映射 {mapped_count} 列）{warning_text}",
                        attachment.original_name,
                    ),
                    options,
                };
            }

            // Handle choice from options
            let choice = args
                .get("choice")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if choice == "cancel" {
                return ToolExecResult::Success {
                    content: "用户取消了导出".to_owned(),
                };
            }

            // Determine output path
            let output_path = if choice == "desktop" {
                let desktop = dirs::desktop_dir().unwrap_or_else(|| sessions_dir.clone());
                let filename = format!(
                    "发票导出_{}.xlsx",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                );
                desktop.join(filename).to_string_lossy().into_owned()
            } else if let Some(p) = args.get("output_path").and_then(|v| v.as_str()) {
                if let Err(e) = validate_agent_path(p, &app_data_dir) {
                    return ToolExecResult::Error { message: e };
                }
                p.to_owned()
            } else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };

            // Write to temp directory first
            let temp_dir = match paths::session_temp_dir(&sessions_dir, &session_uuid) {
                Ok(d) => d,
                Err(e) => return ToolExecResult::Error {
                    message: format!("创建临时目录失败: {e}"),
                },
            };
            let temp_filename = std::path::Path::new(&output_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let temp_path = temp_dir.join(&temp_filename).to_string_lossy().into_owned();
            let template_path = &attachment.storage_path;
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let rows = template_adapter::load_invoices(
                &conn,
                json_i64_vec(args, "invoice_ids").as_deref(),
                json_string(args, "date_from").as_deref(),
                json_string(args, "date_to").as_deref(),
            );
            let rows = match rows {
                Ok(rows) => rows,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            };
            let input_json = serde_json::to_string(args).unwrap_or_default();
            let task = match agent::create_task(
                &conn,
                session_id,
                "export_invoices_with_template",
                Some(&input_json),
            ) {
                Ok(task) => Some(task),
                Err(e) => {
                    warn!("Failed to create agent export task: {e}");
                    None
                }
            };
            let source = template_adapter::InvoiceDataSource {
                rows: &rows,
                column_map: &column_map,
            };
            // Export to temp path first
            match crate::template_engine::TemplateEngine::export_with_plan(
                template_path,
                &temp_path,
                &source,
                &plan,
            ) {
                Ok(te_result) => {
                    // Move temp file to final destination
                    let final_path = if temp_path != output_path {
                        match std::fs::rename(&temp_path, &output_path) {
                            Ok(_) => output_path.clone(),
                            Err(_) => {
                                // Fallback: copy + delete
                                match std::fs::copy(&temp_path, &output_path)
                                    .and_then(|_| std::fs::remove_file(&temp_path))
                                {
                                    Ok(_) => output_path.clone(),
                                    Err(_) => temp_path.clone(), // Keep in temp if move fails
                                }
                            }
                        }
                    } else {
                        output_path.clone()
                    };
                    let result = ExportResult {
                        file_path: final_path,
                        row_count: te_result.row_count,
                        format: "xlsx".to_owned(),
                        byte_size: te_result.byte_size,
                        columns: plan.columns.iter().map(|c| c.label.clone()).collect(),
                    };
                    let title = std::path::Path::new(&result.file_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "导出文件".to_owned());
                    let artifact = record_export_artifact(
                        &conn,
                        session_id,
                        task.as_ref().map(|task| task.id),
                        &title,
                        &result,
                    )
                    .ok();
                    let completed_task = task.as_ref().and_then(|task| {
                        let result_json = serde_json::to_string(&result).ok();
                        agent::complete_task(
                            &conn,
                            task.id,
                            "completed",
                            result_json.as_deref(),
                            None,
                        )
                        .ok()
                    });
                    let content = serde_json::json!({
                        "export": result,
                        "artifact": artifact,
                        "task": completed_task
                    })
                    .to_string();
                    ToolExecResult::Success { content }
                }
                Err(e) => {
                    if let Some(task) = task {
                        if let Err(e) = agent::complete_task(
                            &conn,
                            task.id,
                            "failed",
                            None,
                            Some(&e.to_string()),
                        ) {
                            warn!("Failed to mark agent task as failed: {e}");
                        }
                    }
                    ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            }
        }
        "export_invoices" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("csv");
                let count_hint = args
                    .get("invoice_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let scope = if count_hint > 0 {
                    format!("{count_hint} 张发票")
                } else {
                    "匹配条件的发票".to_owned()
                };
                let column_desc = args
                    .get("columns")
                    .and_then(|v| v.as_array())
                    .map(|columns| {
                        let names = columns
                            .iter()
                            .filter_map(|value| value.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        if names.is_empty() {
                            "全部默认列".to_owned()
                        } else {
                            format!("列: {names}")
                        }
                    })
                    .unwrap_or_else(|| "全部默认列".to_owned());
                let date_desc = match (
                    args.get("date_from").and_then(|v| v.as_str()),
                    args.get("date_to").and_then(|v| v.as_str()),
                ) {
                    (Some(from), Some(to)) => format!("，日期 {from} 至 {to}"),
                    (Some(from), None) => format!("，日期从 {from} 起"),
                    (None, Some(to)) => format!("，日期截至 {to}"),
                    _ => String::new(),
                };
                let desc = format!(
                    "将导出{scope}为 {format} 格式，{column_desc}{date_desc}。请选择保存位置。"
                );

                let has_user_path = args
                    .get("output_path")
                    .and_then(|v| v.as_str())
                    .is_some();
                let options = if has_user_path {
                    None
                } else {
                    Some(vec![
                        crate::agent::ConfirmOption {
                            label: "保存到桌面".into(),
                            value: "desktop".into(),
                            style: Some("primary".into()),
                        },
                        crate::agent::ConfirmOption {
                            label: "选择位置...".into(),
                            value: "pick_path".into(),
                            style: Some("secondary".into()),
                        },
                        crate::agent::ConfirmOption {
                            label: "取消".into(),
                            value: "cancel".into(),
                            style: Some("danger".into()),
                        },
                    ])
                };

                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_invoices".to_owned(),
                    arguments: args.clone(),
                    message: desc,
                    options,
                };
            }

            // Handle choice from options
            let choice = args
                .get("choice")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if choice == "cancel" {
                return ToolExecResult::Success {
                    content: "用户取消了导出".to_owned(),
                };
            }

            // Determine output path
            let output_path = if choice == "desktop" {
                let desktop = dirs::desktop_dir().unwrap_or_else(|| sessions_dir.clone());
                let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("csv");
                let ext = if format == "xlsx" { "xlsx" } else { "csv" };
                let filename = format!(
                    "发票导出_{}.{}",
                    chrono::Local::now().format("%Y%m%d_%H%M%S"),
                    ext
                );
                desktop.join(filename).to_string_lossy().into_owned()
            } else if let Some(p) = args.get("output_path").and_then(|v| v.as_str()) {
                if let Err(e) = validate_agent_path(p, &app_data_dir) {
                    return ToolExecResult::Error { message: e };
                }
                p.to_owned()
            } else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("csv")
                .to_owned();

            // Write to temp directory first
            let temp_dir = match paths::session_temp_dir(&sessions_dir, &session_uuid) {
                Ok(d) => d,
                Err(e) => return ToolExecResult::Error {
                    message: format!("创建临时目录失败: {e}"),
                },
            };
            let ext = if format == "xlsx" { "xlsx" } else { "csv" };
            let temp_filename = format!(
                "发票导出_{}.{}",
                chrono::Local::now().format("%Y%m%d_%H%M%S"),
                ext
            );
            let temp_path = temp_dir.join(&temp_filename).to_string_lossy().into_owned();

            let invoice_ids: Option<Vec<i64>> = args
                .get("invoice_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect());
            let columns: Option<Vec<String>> = json_string_vec(args, "columns");
            let request = ExportInvoicesRequest {
                format,
                output_path: temp_path.clone(),
                invoice_ids,
                columns,
                date_from: args
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                date_to: args
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let input_json = serde_json::to_string(args).unwrap_or_default();
            let task =
                match agent::create_task(&conn, session_id, "export_invoices", Some(&input_json)) {
                    Ok(task) => Some(task),
                    Err(e) => {
                        warn!("Failed to create agent export task: {e}");
                        None
                    }
                };
            match export_invoices(&conn, request) {
                Ok(result) => {
                    // Move temp file to final destination
                    let final_path = if temp_path != output_path {
                        match std::fs::rename(&temp_path, &output_path) {
                            Ok(_) => output_path.clone(),
                            Err(_) => {
                                match std::fs::copy(&temp_path, &output_path)
                                    .and_then(|_| std::fs::remove_file(&temp_path))
                                {
                                    Ok(_) => output_path.clone(),
                                    Err(_) => temp_path.clone(),
                                }
                            }
                        }
                    } else {
                        output_path.clone()
                    };
                    let mut result = result;
                    result.file_path = final_path;
                    let count = result.row_count;
                    let format = &result.format;
                    if let Err(e) = event::record_agent_event(
                        &conn,
                        "export",
                        &format!("Agent 导出 {count} 张发票为 {format}"),
                        "",
                        None,
                        None,
                    ) {
                        warn!("Failed to record agent export event: {e}");
                    }
                    let title = std::path::Path::new(&result.file_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "导出文件".to_owned());
                    let artifact = record_export_artifact(
                        &conn,
                        session_id,
                        task.as_ref().map(|task| task.id),
                        &title,
                        &result,
                    )
                    .ok();
                    let completed_task = task.as_ref().and_then(|task| {
                        let result_json = serde_json::to_string(&result).ok();
                        agent::complete_task(
                            &conn,
                            task.id,
                            "completed",
                            result_json.as_deref(),
                            None,
                        )
                        .ok()
                    });
                    let content = serde_json::json!({
                        "export": result,
                        "artifact": artifact,
                        "task": completed_task
                    })
                    .to_string();
                    ToolExecResult::Success { content }
                }
                Err(e) => {
                    if let Some(task) = task {
                        if let Err(e) = agent::complete_task(
                            &conn,
                            task.id,
                            "failed",
                            None,
                            Some(&e.to_string()),
                        ) {
                            warn!("Failed to mark agent task as failed: {e}");
                        }
                    }
                    ToolExecResult::Error {
                        message: e.to_string(),
                    }
                }
            }
        }
        "merge_invoices" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let target_id = args
                    .get("target_invoice_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let source_ids: Vec<i64> = args
                    .get("source_invoice_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
                    .unwrap_or_default();
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "merge_invoices".to_owned(),
                    arguments: args.clone(),
                    message: format!(
                        "将发票 {:?} 合并到发票 #{}，是否确认？",
                        source_ids, target_id
                    ),
                    options: None,
                };
            }
            let target_id = args
                .get("target_invoice_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let source_ids: Vec<i64> = args
                .get("source_invoice_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
                .unwrap_or_default();
            let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match merge_invoices(&mut conn, target_id, source_ids) {
                Ok(result) => {
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "export_pdf_report" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let count = args
                    .get("invoice_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_pdf_report".to_owned(),
                    arguments: args.clone(),
                    message: format!("将导出 {count} 张发票的 PDF 报表，是否确认？"),
                                    options: None,
                };
            }
            let invoice_ids: Option<Vec<i64>> = args
                .get("invoice_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect());
            let request = PdfReportRequest {
                output_path: args
                    .get("output_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/tmp/invoicevault_report.pdf")
                    .to_string(),
                invoice_ids,
                date_from: args
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                date_to: args
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                thumbnails_dir: Some(thumbnails_dir.to_string_lossy().to_string()),
            };
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match export_pdf_report(&conn, request) {
                Ok(result) => {
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "update_invoice" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let invoice_id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let seller = args
                    .get("seller_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "update_invoice".to_owned(),
                    arguments: args.clone(),
                    message: format!("将更新发票 #{invoice_id} ({seller}) 的字段信息，是否确认？"),
                                    options: None,
                };
            }
            let request: UpdateInvoiceRequest = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("参数解析失败: {e}"),
                    }
                }
            };
            let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let invoice_id = request.id;
            match update_invoice(&mut conn, request) {
                Ok(result) => {
                    if let Err(e) = event::record_agent_event(
                        &conn,
                        "update",
                        &format!("Agent 更新发票 #{invoice_id}"),
                        "",
                        Some("invoice"),
                        Some(invoice_id),
                    ) {
                        warn!("Failed to record agent update event: {e}");
                    }
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_badge_config" => {
            let badge_config: BadgeConfig =
                load_config_raw(&app_data_dir, "badge_config.json").unwrap_or_default();
            match serde_json::to_string(&badge_config) {
                Ok(content) => ToolExecResult::Success { content },
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "set_badge_config" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "set_badge_config".to_owned(),
                    arguments: args.clone(),
                    message: "将更新自定义标签配置（替换所有分组和选项），是否确认？".to_owned(),
                                    options: None,
                };
            }
            let config: BadgeConfig = match serde_json::from_value(
                args.get("config").cloned().unwrap_or_else(|| args.clone()),
            ) {
                Ok(c) => c,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("参数解析失败: {e}"),
                    }
                }
            };
            let sanitized = sanitize_badge_config(config);
            if let Err(e) = write_config(&app_data_dir, "badge_config.json", &sanitized) {
                return ToolExecResult::Error {
                    message: format!("写入配置失败: {e}"),
                };
            }
            {
                let mut cfg = badge_config.lock().unwrap_or_else(|e| e.into_inner());
                *cfg = sanitized.clone();
            }
            match serde_json::to_string(&sanitized) {
                Ok(content) => {
                    let conn = db.lock().unwrap_or_else(|e| e.into_inner());
                    if let Err(e) = event::create_event(
                        &conn,
                        "config_change",
                        "Agent 更新 Badge 配置",
                        "",
                        "completed",
                        None,
                        None,
                        None,
                    ) {
                        warn!("Failed to record badge config change event: {e}");
                    }
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "set_invoice_badge" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let group = args
                    .get("group_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(取消)");
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "set_invoice_badge".to_owned(),
                    arguments: args.clone(),
                    message: format!(
                        "将设置发票 #{invoice_id} 的标签 {group} = {value}，是否确认？"
                    ),
                                    options: None,
                };
            }
            let Some(invoice_id) = args.get("invoice_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 invoice_id 参数".to_owned(),
                };
            };
            let Some(group_name) = args.get("group_name").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 group_name 参数".to_owned(),
                };
            };
            let value = args.get("value").and_then(|v| v.as_str()).map(String::from);
            let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
            match set_invoice_badge(&mut conn, invoice_id, group_name.to_owned(), value) {
                Ok(badges) => {
                    let content = serde_json::to_string(&badges).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_price_config" => {
            let price_config: PriceConfig =
                load_config_raw(&app_data_dir, "price_config.json").unwrap_or_default();
            match serde_json::to_string(&price_config) {
                Ok(content) => ToolExecResult::Success { content },
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "set_price_config" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "set_price_config".to_owned(),
                    arguments: args.clone(),
                    message: "将更新 LLM 价格配置，是否确认？".to_owned(),
                                    options: None,
                };
            }
            let config: PriceConfig = match serde_json::from_value(
                args.get("config").cloned().unwrap_or_else(|| args.clone()),
            ) {
                Ok(c) => c,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("参数解析失败: {e}"),
                    }
                }
            };
            if let Err(e) = write_config(&app_data_dir, "price_config.json", &config) {
                return ToolExecResult::Error {
                    message: format!("写入配置失败: {e}"),
                };
            }
            {
                let mut cfg = price_config.lock().unwrap_or_else(|e| e.into_inner());
                *cfg = config.clone();
            }
            match serde_json::to_string(&config) {
                Ok(content) => ToolExecResult::Success { content },
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "get_theme" => {
            let theme = load_config_raw::<serde_json::Value>(&app_data_dir, "theme.json")
                .and_then(|v| v.get("theme").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_else(|| "light".to_owned());
            let content = serde_json::json!({ "theme": theme }).to_string();
            ToolExecResult::Success { content }
        }
        "set_theme" => {
            let Some(theme) = args.get("theme").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 theme 参数".to_owned(),
                };
            };
            if theme != "light" && theme != "dark" {
                return ToolExecResult::Error {
                    message: "theme 必须是 'light' 或 'dark'".to_owned(),
                };
            }
            let theme_config = serde_json::json!({ "theme": theme });
            if let Err(e) = write_config(&app_data_dir, "theme.json", &theme_config) {
                return ToolExecResult::Error {
                    message: format!("写入主题配置失败: {e}"),
                };
            }
            if let Err(e) = app_handle.emit("theme-change", serde_json::json!({ "theme": theme })) {
                warn!("Failed to emit theme-change event: {e}");
            }
            let content = serde_json::json!({ "theme": theme }).to_string();
            ToolExecResult::Success { content }
        }
        "export_logs" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_logs".to_owned(),
                    arguments: args.clone(),
                    message: "将导出应用日志文件，请选择保存位置。".to_owned(),
                                    options: None,
                };
            }
            let Some(output_path) = args.get("output_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let logs_dir = app_data_dir.join(DIR_LOGS);
            let file = match std::fs::File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("创建文件失败: {e}"),
                    }
                }
            };
            let mut zip_writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let mut ok = true;
            let mut err_msg = String::new();
            let db_path = app_data_dir.join("invoicevault.sqlite3");
            if db_path.exists() {
                if let Err(e) =
                    stream_file_to_zip(&mut zip_writer, "invoicevault.sqlite3", &db_path, options)
                {
                    ok = false;
                    err_msg = e.to_string();
                }
            }
            for config_name in &[
                "llm_config.json",
                "embedding_enabled.json",
                "recognition_config.json",
                "badge_config.json",
            ] {
                let config_path = app_data_dir.join(config_name);
                if config_path.exists() {
                    let _ = stream_file_to_zip(&mut zip_writer, config_name, &config_path, options);
                }
            }
            if logs_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&logs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                let _ = stream_file_to_zip(
                                    &mut zip_writer,
                                    &format!("logs/{name}"),
                                    &path,
                                    options,
                                );
                            }
                        }
                    }
                }
            }
            let llm_audit_dir = app_data_dir.join("llm_audit");
            if llm_audit_dir.exists() {
                let _ = add_dir_to_zip(&mut zip_writer, &app_data_dir, &llm_audit_dir, options);
            }
            if !ok {
                return ToolExecResult::Error { message: err_msg };
            }
            match zip_writer.finish() {
                Ok(finished) => match finished.metadata() {
                    Ok(metadata) => {
                        let content = serde_json::json!({
                            "file_path": output_path,
                            "byte_size": metadata.len()
                        })
                        .to_string();
                        ToolExecResult::Success { content }
                    }
                    Err(e) => ToolExecResult::Error {
                        message: e.to_string(),
                    },
                },
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "export_backup" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_backup".to_owned(),
                    arguments: args.clone(),
                    message: "将导出数据库和配置文件的备份包，请选择保存位置。".to_owned(),
                                    options: None,
                };
            }
            let Some(output_path) = args.get("output_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let file = match std::fs::File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("创建文件失败: {e}"),
                    }
                }
            };
            let mut zip_writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            if let Err(e) = add_dir_to_zip(&mut zip_writer, &app_data_dir, &app_data_dir, options) {
                return ToolExecResult::Error {
                    message: format!("打包备份失败: {e}"),
                };
            }
            match zip_writer.finish() {
                Ok(finished) => match finished.metadata() {
                    Ok(metadata) => {
                        let content = serde_json::json!({
                            "file_path": output_path,
                            "byte_size": metadata.len()
                        })
                        .to_string();
                        ToolExecResult::Success { content }
                    }
                    Err(e) => ToolExecResult::Error {
                        message: e.to_string(),
                    },
                },
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        "cleanup_storage" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "cleanup_storage".to_owned(),
                    arguments: args.clone(),
                    message: "将清理孤立文件和过期数据以释放存储空间，是否确认？".to_owned(),
                                    options: None,
                };
            }
            let conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let mut files_removed = 0usize;
            let mut bytes_freed: u64 = 0;
            let raw_dir = app_data_dir.join("raw");
            if raw_dir.exists() {
                let mut referenced_paths: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                if let Ok(mut stmt) = conn.prepare("SELECT storage_path FROM raw_files") {
                    if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                        for row in rows.flatten() {
                            referenced_paths.insert(row);
                        }
                    }
                }
                if let Ok(entries) = std::fs::read_dir(&raw_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let path_str = path.to_string_lossy().to_string();
                        if !referenced_paths.contains(&path_str) {
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                bytes_freed += metadata.len();
                            }
                            if std::fs::remove_file(&path).is_ok() {
                                files_removed += 1;
                            }
                        }
                    }
                }
            }
            let content = serde_json::json!({
                "files_removed": files_removed,
                "db_records_removed": 0,
                "bytes_freed": bytes_freed
            })
            .to_string();
            ToolExecResult::Success { content }
        }
        "get_app_info" => {
            let db_path = app_data_dir.join("invoicevault.sqlite3");
            let migration_version: i64 = {
                let conn = db.lock().unwrap_or_else(|e| e.into_inner());
                conn.query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0)
            };
            let content = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "app_data_dir": app_data_dir.to_string_lossy(),
                "database_path": db_path.to_string_lossy(),
                "migration_version": migration_version,
            })
            .to_string();
            ToolExecResult::Success { content }
        }
        "get_sysinfo" => {
            let os_name = detect_os_name();
            let shell = detect_shell();
            let desktop = dirs::desktop_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let home = dirs::home_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let content = serde_json::json!({
                "os": os_name,
                "shell": shell,
                "desktop_path": desktop,
                "home_path": home,
            })
            .to_string();
            ToolExecResult::Success { content }
        }
        "ask_user" => {
            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let message = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("请确认")
                    .to_owned();
                let options: Option<Vec<crate::agent::ConfirmOption>> = args
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
                ToolExecResult::ConfirmationRequired {
                    tool_name: "ask_user".to_owned(),
                    arguments: args.clone(),
                    message,
                    options,
                }
            } else {
                let choice = args
                    .get("choice")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                // Resolve "desktop" choice to actual path
                let result = if choice == "desktop" {
                    let desktop = dirs::desktop_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    serde_json::json!({
                        "selected": "desktop",
                        "resolved_path": desktop.to_string_lossy(),
                    })
                } else if choice == "__other__" && !text.is_empty() {
                    serde_json::json!({ "selected": "__other__", "text": text })
                } else if !choice.is_empty() {
                    serde_json::json!({ "selected": choice })
                } else {
                    serde_json::json!({ "selected": serde_json::Value::Null })
                };
                ToolExecResult::Success {
                    content: result.to_string(),
                }
            }
        }
        _ => ToolExecResult::Error {
            message: format!("未知工具: {tool_name}"),
        },
    }
}

fn export_preview_request_from_args(args: &serde_json::Value) -> ExportPreviewRequest {
    ExportPreviewRequest {
        invoice_ids: json_i64_vec(args, "invoice_ids"),
        columns: json_string_vec(args, "columns"),
        date_from: json_string(args, "date_from"),
        date_to: json_string(args, "date_to"),
        limit: args
            .get("limit")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize),
    }
}

fn record_export_artifact(
    conn: &Connection,
    session_id: i64,
    task_id: Option<i64>,
    title: &str,
    result: &ExportResult,
) -> Result<AgentArtifact, AgentError> {
    let mime_type = match result.format.as_str() {
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "csv" => Some("text/csv"),
        _ => None,
    };
    let metadata = serde_json::to_string(&serde_json::json!({
        "row_count": result.row_count,
        "format": result.format,
        "columns": result.columns,
    }))
    .ok();
    agent::insert_artifact(
        conn,
        session_id,
        task_id,
        "export",
        title,
        Some(&result.file_path),
        mime_type,
        Some(result.byte_size as i64),
        metadata.as_deref(),
    )
}

fn json_i64_vec(args: &serde_json::Value, key: &str) -> Option<Vec<i64>> {
    args.get(key)
        .and_then(|value| value.as_array())
        .map(|items| items.iter().filter_map(|value| value.as_i64()).collect())
        .filter(|v: &Vec<i64>| !v.is_empty())
}

fn json_string_vec(args: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    args.get(key)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|item| item.to_owned()))
                .collect()
        })
}

fn json_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|value| value.as_str())
        .map(String::from)
}

fn inspect_spreadsheet_attachment(
    attachment: &AgentAttachment,
    max_rows: usize,
) -> Result<SpreadsheetInspection, String> {
    let path = Path::new(&attachment.storage_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let sheet = match extension.as_str() {
        "csv" => inspect_csv(path, max_rows)?,
        "xlsx" => inspect_xlsx(path, max_rows)?,
        _ => return Err("目前仅支持检查 csv/xlsx 表格".to_owned()),
    };

    Ok(SpreadsheetInspection {
        attachment_id: attachment.id,
        file_name: attachment.original_name.clone(),
        file_type: extension,
        sheets: vec![sheet],
    })
}

fn inspect_csv(path: &Path, max_rows: usize) -> Result<SpreadsheetSheet, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for record in reader.records().take(max_rows + 1) {
        let record = record.map_err(|e| e.to_string())?;
        rows.push(
            record
                .iter()
                .map(|cell| cell.trim().to_owned())
                .collect::<Vec<_>>(),
        );
    }
    Ok(sheet_from_rows("CSV", rows, max_rows))
}

fn inspect_xlsx(path: &Path, max_rows: usize) -> Result<SpreadsheetSheet, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let shared_strings = read_xlsx_shared_strings(&mut archive).unwrap_or_default();
    let mut sheet_xml = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .map_err(|_| "未找到第一个工作表".to_owned())?
        .read_to_string(&mut sheet_xml)
        .map_err(|e| e.to_string())?;
    let rows = parse_xlsx_sheet_rows(&sheet_xml, &shared_strings, max_rows + 1);
    Ok(sheet_from_rows("Sheet1", rows, max_rows))
}

fn read_xlsx_shared_strings<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<String>, String> {
    let mut xml = String::new();
    archive
        .by_name("xl/sharedStrings.xml")
        .map_err(|e| e.to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    let mut values = Vec::new();
    for item in xml.split("<si").skip(1) {
        let segment = item.split("</si>").next().unwrap_or("");
        let mut text = String::new();
        if segment.contains("<r>") || segment.contains("<r ") {
            // Rich text: extract <t> only from within each <r>…</r> block
            for r_part in segment.split("<r>").skip(1) {
                let r_block = r_part.split("</r>").next().unwrap_or("");
                if let Some(after) = r_block.split("<t").nth(1) {
                    if let Some(value) =
                        after.split('>').nth(1).and_then(|v| v.split("</t>").next())
                    {
                        text.push_str(&xml_unescape(value));
                    }
                }
            }
        } else {
            // Simple text: extract from the single <t> element
            if let Some(after) = segment.split("<t").nth(1) {
                if let Some(value) = after.split('>').nth(1).and_then(|v| v.split("</t>").next()) {
                    text.push_str(&xml_unescape(value));
                }
            }
        }
        values.push(text);
    }
    Ok(values)
}

fn parse_xlsx_sheet_rows(
    xml: &str,
    shared_strings: &[String],
    max_rows: usize,
) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for row_xml in xml.split("<row").skip(1).take(max_rows) {
        let segment = row_xml.split("</row>").next().unwrap_or("");
        let mut cells: Vec<(usize, String)> = Vec::new();
        for cell_xml in segment.split("<c").skip(1) {
            let cell_segment = cell_xml.split("</c>").next().unwrap_or("");
            let col = cell_reference_to_index(cell_segment).unwrap_or(cells.len());
            let is_shared = cell_segment.contains(" t=\"s\"") || cell_segment.contains(" t='s'");
            let is_inline = cell_segment.contains(" t=\"inlineStr\"")
                || cell_segment.contains(" t='inlineStr'");
            let value = if is_inline {
                extract_between(cell_segment, "<t", "</t>")
                    .and_then(|raw| raw.split('>').nth(1).map(xml_unescape))
                    .unwrap_or_default()
            } else {
                let raw = extract_between(cell_segment, "<v>", "</v>").unwrap_or_default();
                if is_shared {
                    raw.parse::<usize>()
                        .ok()
                        .and_then(|idx| shared_strings.get(idx).cloned())
                        .unwrap_or_default()
                } else {
                    xml_unescape(&raw)
                }
            };
            cells.push((col, value));
        }
        let width = cells
            .iter()
            .map(|(idx, _)| *idx)
            .max()
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let mut row = vec![String::new(); width];
        for (idx, value) in cells {
            if idx < row.len() {
                row[idx] = value;
            }
        }
        if row.iter().any(|cell| !cell.trim().is_empty()) {
            rows.push(row);
        }
    }
    rows
}

fn sheet_from_rows(name: &str, rows: Vec<Vec<String>>, max_rows: usize) -> SpreadsheetSheet {
    // Find the first row with 3+ non-empty cells (skip title/subtitle rows)
    let header_index = rows
        .iter()
        .position(|row| row.iter().filter(|cell| !cell.trim().is_empty()).count() >= 3)
        .unwrap_or(0);
    let header = rows.get(header_index).cloned().unwrap_or_default();
    let columns = header
        .iter()
        .enumerate()
        .filter_map(|(idx, label)| {
            let label = label.trim();
            (!label.is_empty()).then(|| SpreadsheetColumn {
                index: idx + 1,
                label: label.to_owned(),
            })
        })
        .collect();
    let sample_rows = rows
        .into_iter()
        .skip(header_index + 1)
        .take(max_rows)
        .collect();
    SpreadsheetSheet {
        name: name.to_owned(),
        header_row: header_index + 1,
        columns,
        sample_rows,
    }
}

fn cell_reference_to_index(cell_segment: &str) -> Option<usize> {
    let reference = cell_segment
        .split(" r=\"")
        .nth(1)
        .and_then(|part| part.split('"').next())
        .or_else(|| {
            cell_segment
                .split(" r='")
                .nth(1)
                .and_then(|part| part.split('\'').next())
        })?;
    let mut value = 0usize;
    let mut found = false;
    for ch in reference.chars().take_while(|ch| ch.is_ascii_alphabetic()) {
        found = true;
        value = value * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    found.then(|| value.saturating_sub(1))
}

fn extract_between(value: &str, start: &str, end: &str) -> Option<String> {
    let after = value.split(start).nth(1)?;
    Some(after.split(end).next()?.to_owned())
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn detect_os_name() -> String {
    match std::env::consts::OS {
        "macos" => "macOS".to_owned(),
        "windows" => "Windows".to_owned(),
        "linux" => {
            // Try to read /etc/os-release for distro name
            if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
                let pretty = content.lines().find_map(|line| {
                    let line = line.trim();
                    if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                        // Remove surrounding quotes
                        let val = rest.trim_matches('"');
                        if !val.is_empty() {
                            return Some(val.to_owned());
                        }
                    }
                    None
                });
                if let Some(name) = pretty {
                    return name;
                }
            }
            "Linux".to_owned()
        }
        other => other.to_owned(),
    }
}

fn detect_shell() -> String {
    if cfg!(target_os = "windows") {
        // On Windows: check SHELL env (Git Bash, MSYS2) then ComSpec
        if let Ok(shell) = std::env::var("SHELL") {
            return std::path::Path::new(&shell)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(shell);
        }
        if let Ok(comspec) = std::env::var("ComSpec") {
            return std::path::Path::new(&comspec)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(comspec);
        }
        "cmd".to_owned()
    } else {
        // Unix: $SHELL
        std::env::var("SHELL")
            .map(|s| {
                std::path::Path::new(&s)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or(s)
            })
            .unwrap_or_else(|_| "sh".to_owned())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Storage(StorageError::Database(value))
    }
}
