use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use chrono::{Datelike, Local};
use tracing::{error, info, warn};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

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
        export_column_catalog, export_invoices, export_pdf_report, preview_export,
        resolve_export_column_keys_from_labels, ExportError, ExportInvoicesRequest,
        ExportPreviewRequest, ExportResult, PdfReportRequest, PdfReportResult,
    },
    extractor::invoice_to_embedding_text,
    extractor::{
        batch_delete_invoices, batch_update_invoices, count_unviewed_invoices, get_dashboard_stats,
        get_invoice_detail, list_invoices, mark_invoice_viewed, merge_invoices,
        save_invoice_extraction, search_invoices, set_invoice_badge, update_invoice,
        update_invoice_items, BadgeConfig, BadgeGroupConfig, BatchUpdateRequest, DashboardStats,
        ExtractorError, InvoiceBadgeSelection, InvoiceDetail, InvoiceItemRow, InvoiceSearchParams,
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

const EMBEDDING_MODEL_NAME: &str = "bge-small-zh-v1.5";

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("failed to resolve application data directory")]
    MissingAppDataDir,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("import error: {0}")]
    Import(#[from] ImportError),
    #[error("extractor error: {0}")]
    Extractor(#[from] ExtractorError),
    #[error("dedupe error: {0}")]
    Dedupe(#[from] DedupeError),
    #[error("export error: {0}")]
    Export(#[from] ExportError),
    #[error("document error: {0}")]
    Document(#[from] DocumentError),
    #[error("watcher error: {0}")]
    Watcher(#[from] WatcherError),
    #[error("email error: {0}")]
    Email(#[from] EmailError),
    #[error("chromadb error: {0}")]
    Chroma(#[from] ChromaError),
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
    #[error("event error: {0}")]
    Event(#[from] EventError),
    #[error("{0}")]
    InvalidOperation(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub raw_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub llm_audit_dir: PathBuf,
    pub agent_uploads_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppHealth {
    pub app_data_dir: String,
    pub database_path: String,
    pub migration_version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawFileForRecognition {
    pub id: i64,
    pub original_name: String,
    pub mime_type: String,
    pub storage_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecognitionQueueStatus {
    pub pending: i64,
    pub running: i64,
    pub max_concurrent: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportLogsResult {
    pub file_path: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupStorageResult {
    pub files_removed: usize,
    pub db_records_removed: usize,
    pub bytes_freed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegenerateEmbeddingsResult {
    pub total_invoices: usize,
    pub success_count: usize,
    pub failure_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetInspection {
    pub attachment_id: i64,
    pub file_name: String,
    pub file_type: String,
    pub sheets: Vec<SpreadsheetSheet>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetSheet {
    pub name: String,
    pub header_row: usize,
    pub columns: Vec<SpreadsheetColumn>,
    pub sample_rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetColumn {
    pub index: usize,
    pub label: String,
}

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

pub fn sanitize_badge_config(config: BadgeConfig) -> BadgeConfig {
    let mut groups = Vec::new();
    for group in config.groups {
        let name = group.name.trim();
        if name.is_empty() {
            continue;
        }

        let mut options = Vec::new();
        for option in group.options {
            let value = option.trim();
            if value.is_empty() || options.iter().any(|existing| existing == value) {
                continue;
            }
            options.push(value.to_owned());
        }

        if groups
            .iter()
            .any(|existing: &BadgeGroupConfig| existing.name == name)
        {
            continue;
        }

        groups.push(BadgeGroupConfig {
            name: name.to_owned(),
            options,
        });
    }

    BadgeConfig { groups }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PriceConfig {
    pub llm_input_price_per_1k: f64,
    pub llm_output_price_per_1k: f64,
    pub embedding_input_price_per_1k: f64,
    pub embedding_output_price_per_1k: f64,
}

impl Default for PriceConfig {
    fn default() -> Self {
        Self {
            llm_input_price_per_1k: 0.0008,
            llm_output_price_per_1k: 0.002,
            embedding_input_price_per_1k: 0.0007,
            embedding_output_price_per_1k: 0.0007,
        }
    }
}

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
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, AppError> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::MissingAppDataDir)?;
        let paths = create_app_paths(&app_data_dir)?;
        let mut db = Connection::open(&paths.database_path)?;
        run_migrations(&mut db)?;
        let recovered_jobs = recover_interrupted_import_jobs(&db)?;
        if recovered_jobs > 0 {
            warn!("Recovered {recovered_jobs} interrupted import jobs");
        }
        let db = Arc::new(Mutex::new(db));

        let chroma_config = ChromaConfig::default();

        // Load persisted configs
        let embedding_enabled: bool =
            Self::load_config_raw::<serde_json::Value>(&app_data_dir, "embedding_enabled.json")
                .and_then(|v| v.get("enabled").and_then(|v| v.as_bool()))
                .unwrap_or(true);

        let llm_config: Arc<Mutex<Option<LlmProviderConfig>>> = {
            let saved =
                Self::load_config_raw::<LlmProviderConfig>(&app_data_dir, "llm_config.json");
            Arc::new(Mutex::new(saved))
        };

        let llm_audit_enabled: Arc<Mutex<bool>> = {
            let saved =
                Self::load_config_raw::<serde_json::Value>(&app_data_dir, "audit_config.json")
                    .and_then(|v| v.get("enabled").and_then(|v| v.as_bool()))
                    .unwrap_or(true);
            Arc::new(Mutex::new(saved))
        };

        let badge_config = Self::load_config_raw::<BadgeConfig>(&app_data_dir, "badge_config.json")
            .unwrap_or_default();

        let price_config = Self::load_config_raw::<PriceConfig>(&app_data_dir, "price_config.json")
            .unwrap_or_default();

        let watcher_manager = WatcherManager::new(
            Arc::clone(&db),
            paths.raw_dir.clone(),
            paths.thumbnails_dir.clone(),
            paths.llm_audit_dir.clone(),
            Arc::clone(&llm_config),
            Arc::clone(&llm_audit_enabled),
            app.clone(),
        )?;

        let email_manager = EmailManager::new(
            Arc::clone(&db),
            paths.raw_dir.clone(),
            paths.thumbnails_dir.clone(),
            Arc::clone(&llm_config),
            Arc::clone(&llm_audit_enabled),
        );

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
        };

        // Load local embedding engine if enabled and model exists
        if embedding_enabled {
            let model_dir = state
                .paths
                .app_data_dir
                .join("models")
                .join(EMBEDDING_MODEL_NAME);
            let onnx_path = model_dir.join("onnx").join("model_q4.onnx");
            let tok_path = model_dir.join("tokenizer.json");
            if onnx_path.exists() && tok_path.exists() {
                match LocalEmbeddingEngine::load(&model_dir) {
                    Ok(engine) => {
                        *state.local_embedding.lock().expect("lock") = Some(engine);
                        info!("Local embedding engine loaded from {}", model_dir.display());
                    }
                    Err(e) => {
                        error!("Failed to load local embedding engine: {e}");
                    }
                }
            } else {
                info!(
                    "Embedding model not found at {}, will download on first use",
                    model_dir.display()
                );
            }
        }

        // Regenerate any missing preview thumbnails from normalized images
        state.regenerate_missing_previews();

        info!("AppState initialized");
        Ok(state)
    }

    pub fn health(&self) -> Result<AppHealth, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
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

    pub fn app_data_dir(&self) -> &Path {
        &self.paths.app_data_dir
    }

    pub fn db(&self) -> &Arc<Mutex<Connection>> {
        &self.db
    }

    pub fn import_files(
        &self,
        paths: Vec<String>,
        app: &AppHandle,
    ) -> Result<Vec<ImportJobSummary>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let source_paths: Vec<String> = paths.iter().map(|p| p.clone()).collect();
        info!("Importing {} files", paths.len());
        let jobs = import_files(&mut db, &self.paths.raw_dir, paths, "manual")?;
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
        let config = self.llm_config.lock().expect("lock").clone();
        if let Some(cfg) = config {
            if !cfg.api_key.is_empty() {
                for job in &jobs {
                    if job.status == "imported" && job.raw_file_id.is_some() {
                        self.spawn_recognition_task(
                            job.id,
                            job.raw_file_id.unwrap(),
                            cfg.clone(),
                            self.llm_audit_config(),
                            app.clone(),
                        );
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
        let chroma_enabled = self.chroma_config.lock().expect("lock").enabled;
        let embedding_on = *self.embedding_enabled.lock().expect("lock");
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

                    // Best-effort embedding generation
                    if chroma_enabled && embedding_on {
                        if let Some(ref mut engine) = *embedding_engine.lock().expect("lock") {
                            let invoice_id = invoice.id;
                            let thumb_dir = thumbnails_dir.clone();
                            let detail = get_invoice_detail(&conn, &thumb_dir, invoice_id).ok();
                            if let Some(detail) = detail {
                                let text = invoice_to_embedding_text(&detail);
                                if let Ok(result) = generate_embedding(engine, &text) {
                                    let embedding = result.embedding;
                                    let prompt_tokens = result.prompt_tokens;
                                    let total_tokens = result.total_tokens;
                                    let db_for_emb = Arc::clone(&db);
                                    tauri::async_runtime::spawn(async move {
                                        if let Ok(conn) = db_for_emb.lock() {
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
                                                if let Err(e) = crate::dedupe::detect_semantic_duplicates(
                                                    &conn, invoice_id, &similar,
                                                ) {
                                                    warn!("Failed to detect semantic duplicates for invoice {invoice_id}: {e}");
                                                }
                                            }
                                            if let Err(e) = crate::extractor::insert_usage_log(
                                                &conn, "embedding", EMBEDDING_MODEL_NAME,
                                                prompt_tokens, total_tokens.saturating_sub(prompt_tokens), total_tokens,
                                            ) {
                                                warn!("Failed to insert embedding usage log: {e}");
                                            }
                                        }
                                    });
                                }
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
                    // Mark as failed, detach from raw_file
                    if let Err(e) = db.execute(
                        "UPDATE import_jobs SET status = 'failed', error_message = ?1, raw_file_id = NULL WHERE id = ?2",
                        rusqlite::params![message, job_id],
                    ) {
                        error!("Failed to mark import job {job_id} as failed: {e}");
                    }

                    // Record failure event
                    if let Err(e) = event::create_event(
                        &db,
                        "recognition",
                        "自动识别失败",
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
                        let dir_path = thumbnails_dir
                            .join(subdir)
                            .join(raw_file_id.to_string());
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
                    if let Err(e) = db.execute("DELETE FROM raw_files WHERE id = ?1", [raw_file_id]) {
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

    pub fn list_import_jobs(
        &self,
        page: i64,
        page_size: i64,
    ) -> Result<ImportJobListResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(list_import_jobs(&db, page, page_size)?)
    }

    pub fn delete_import_job(&self, job_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(delete_import_job(&db, job_id)?)
    }

    pub fn save_invoice_extraction(
        &self,
        request: SaveInvoiceExtractionRequest,
    ) -> Result<InvoiceSummary, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
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
                    warn!("Failed to record duplicate event for invoice {}: {e}", invoice.id);
                }
            }
        }

        // Best-effort embedding generation (local ONNX inference)
        if self.chroma_config.lock().expect("lock").enabled
            && *self.embedding_enabled.lock().expect("lock")
        {
            let mut engine_guard = self.local_embedding.lock().expect("lock");
            if let Some(ref mut engine) = *engine_guard {
                let thumb_dir = self.paths.thumbnails_dir.clone();
                let invoice_id = invoice.id;
                let detail = get_invoice_detail(&db, &thumb_dir, invoice_id).ok();
                if let Some(detail) = detail {
                    let text = invoice_to_embedding_text(&detail);
                    if let Ok(result) = generate_embedding(engine, &text) {
                        let embedding = result.embedding;
                        let prompt_tokens = result.prompt_tokens;
                        let total_tokens = result.total_tokens;
                        let db_arc = Arc::clone(&self.db);
                        tauri::async_runtime::spawn(async move {
                            if let Ok(db) = db_arc.lock() {
                                if let Err(e) =
                                    chroma::upsert_embedding(&db, invoice_id, &embedding, &text)
                                {
                                    warn!("Failed to upsert embedding for invoice {invoice_id}: {e}");
                                }
                                if let Err(e) = db.execute(
                                    "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
                                    [invoice_id],
                                ) {
                                    warn!("Failed to mark invoice {invoice_id} as having embedding: {e}");
                                }
                                if let Ok(similar) = chroma::query_similar(&db, &embedding, 5) {
                                    if let Err(e) = crate::dedupe::detect_semantic_duplicates(
                                        &db, invoice_id, &similar,
                                    ) {
                                        warn!("Failed to detect semantic duplicates for invoice {invoice_id}: {e}");
                                    }
                                }
                                if let Err(e) = crate::extractor::insert_usage_log(
                                    &db,
                                    "embedding",
                                    EMBEDDING_MODEL_NAME,
                                    prompt_tokens,
                                    total_tokens.saturating_sub(prompt_tokens),
                                    total_tokens,
                                ) {
                                    warn!("Failed to insert embedding usage log: {e}");
                                }
                            }
                        });
                    }
                }
            }
        }

        Ok(invoice)
    }

    pub fn list_invoices(&self) -> Result<Vec<InvoiceSummary>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(list_invoices(&db)?)
    }

    pub fn search_invoices(
        &self,
        params: InvoiceSearchParams,
    ) -> Result<InvoiceSearchResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(search_invoices(&db, params)?)
    }

    pub fn get_invoice_detail(&self, invoice_id: i64) -> Result<InvoiceDetail, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(get_invoice_detail(
            &db,
            &self.paths.thumbnails_dir,
            invoice_id,
        )?)
    }

    pub fn mark_invoice_viewed(&self, invoice_id: i64) -> Result<bool, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(mark_invoice_viewed(&db, invoice_id)?)
    }

    pub fn count_unviewed_invoices(&self) -> Result<i64, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(count_unviewed_invoices(&db)?)
    }

    pub fn raw_file_path_for_invoice(&self, invoice_id: i64) -> Result<PathBuf, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
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

    pub fn update_invoice(
        &self,
        request: UpdateInvoiceRequest,
    ) -> Result<UpdateInvoiceResult, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
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
                    warn!("Failed to record duplicate event for invoice {}: {e}", result.invoice.id);
                }
            }
        }

        // Best-effort embedding regeneration (local ONNX inference)
        if self.chroma_config.lock().expect("lock").enabled
            && *self.embedding_enabled.lock().expect("lock")
        {
            let mut engine_guard = self.local_embedding.lock().expect("lock");
            if let Some(ref mut engine) = *engine_guard {
                let db = self.db.lock().expect("db lock");
                let thumb_dir = self.paths.thumbnails_dir.clone();
                let invoice_id = result.invoice.id;
                let detail = get_invoice_detail(&db, &thumb_dir, invoice_id).ok();
                if let Some(detail) = detail {
                    let text = invoice_to_embedding_text(&detail);
                    if let Ok(emb_result) = generate_embedding(engine, &text) {
                        let embedding = emb_result.embedding;
                        let prompt_tokens = emb_result.prompt_tokens;
                        let total_tokens = emb_result.total_tokens;
                        let db_arc = Arc::clone(&self.db);
                        tauri::async_runtime::spawn(async move {
                            if let Ok(db) = db_arc.lock() {
                                if let Err(e) =
                                    chroma::upsert_embedding(&db, invoice_id, &embedding, &text)
                                {
                                    warn!("Failed to upsert embedding for invoice {invoice_id}: {e}");
                                }
                                if let Err(e) = db.execute(
                                    "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
                                    [invoice_id],
                                ) {
                                    warn!("Failed to mark invoice {invoice_id} as having embedding: {e}");
                                }
                                if let Ok(similar) = chroma::query_similar(&db, &embedding, 5) {
                                    if let Err(e) = crate::dedupe::detect_semantic_duplicates(
                                        &db, invoice_id, &similar,
                                    ) {
                                        warn!("Failed to detect semantic duplicates for invoice {invoice_id}: {e}");
                                    }
                                }
                                if let Err(e) = crate::extractor::insert_usage_log(
                                    &db,
                                    "embedding",
                                    EMBEDDING_MODEL_NAME,
                                    prompt_tokens,
                                    total_tokens.saturating_sub(prompt_tokens),
                                    total_tokens,
                                ) {
                                    warn!("Failed to insert embedding usage log: {e}");
                                }
                            }
                        });
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn update_invoice_items(
        &self,
        request: UpdateInvoiceItemsRequest,
    ) -> Result<Vec<InvoiceItemRow>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(update_invoice_items(&mut db, request)?)
    }

    pub fn batch_update_invoices(
        &self,
        request: BatchUpdateRequest,
    ) -> Result<Vec<InvoiceSummary>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(batch_update_invoices(&db, &request)?)
    }

    pub fn batch_delete_invoices(&self, ids: Vec<i64>) -> Result<usize, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(batch_delete_invoices(&db, &ids)?)
    }

    pub fn check_invoice_duplicates(&self, invoice_id: i64) -> Result<DedupeCheckResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(run_dedupe_check(&db, invoice_id)?)
    }

    pub fn resolve_duplicate(
        &self,
        request: ResolveDuplicateRequest,
    ) -> Result<ResolveDuplicateResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(run_dedupe_resolve(&db, request)?)
    }

    pub fn export_invoices(
        &self,
        request: ExportInvoicesRequest,
    ) -> Result<ExportResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(export_invoices(&db, request)?)
    }

    pub fn merge_invoices(
        &self,
        target_invoice_id: i64,
        source_invoice_ids: Vec<i64>,
    ) -> Result<MergeInvoicesResult, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(merge_invoices(
            &mut db,
            target_invoice_id,
            source_invoice_ids,
        )?)
    }

    pub fn export_pdf_report(
        &self,
        request: PdfReportRequest,
    ) -> Result<PdfReportResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(export_pdf_report(&db, request)?)
    }

    pub fn add_watch_dir(&self, request: AddWatchDirRequest) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.add_watch_dir(request)?)
    }

    pub fn remove_watch_dir(&self, id: i64) -> Result<(), AppError> {
        Ok(self.watcher_manager.remove_watch_dir(id)?)
    }

    pub fn list_watch_dirs(&self) -> Result<Vec<WatchDirStatus>, AppError> {
        Ok(self.watcher_manager.list_watch_dirs()?)
    }

    pub fn update_watch_dir(
        &self,
        id: i64,
        request: UpdateWatchDirRequest,
    ) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.update_watch_dir(id, request)?)
    }

    pub fn toggle_watch_dir(&self, id: i64, enabled: bool) -> Result<WatchDirStatus, AppError> {
        Ok(self.watcher_manager.toggle_watch_dir(id, enabled)?)
    }

    // --- Email Sources ---

    pub fn add_email_source(
        &self,
        request: AddEmailSourceRequest,
    ) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.add_email_source(request)?)
    }

    pub fn update_email_source(
        &self,
        id: i64,
        request: UpdateEmailSourceRequest,
    ) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.update_email_source(id, request)?)
    }

    pub fn remove_email_source(&self, id: i64) -> Result<(), AppError> {
        Ok(self.email_manager.remove_email_source(id)?)
    }

    pub fn list_email_sources(&self) -> Result<Vec<EmailSource>, AppError> {
        Ok(self.email_manager.list_email_sources()?)
    }

    pub fn toggle_email_source(&self, id: i64, enabled: bool) -> Result<EmailSource, AppError> {
        Ok(self.email_manager.toggle_email_source(id, enabled)?)
    }

    pub fn sync_email_source(&self, id: i64) -> Result<EmailSyncResult, AppError> {
        Ok(self.email_manager.sync_email_source(id)?)
    }

    pub fn sync_all_email_sources(&self) -> Result<Vec<EmailSyncResult>, AppError> {
        Ok(self.email_manager.sync_all_enabled()?)
    }

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
        Ok(self
            .email_manager
            .test_connection(protocol, host, port, username, password, auth_method, use_ssl, folder)?)
    }

    pub fn get_dashboard_stats(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<DashboardStats, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(get_dashboard_stats(
            &db,
            date_from.as_deref(),
            date_to.as_deref(),
        )?)
    }

    pub fn set_chroma_config(&self, config: ChromaConfig) -> Result<(), AppError> {
        let mut cfg = self.chroma_config.lock().expect("lock");
        *cfg = config;
        let db = self.db.lock().expect("db lock");
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

    pub fn get_chroma_config(&self) -> ChromaConfig {
        self.chroma_config.lock().expect("lock").clone()
    }

    pub fn set_embedding_enabled(&self, enabled: bool) -> Result<(), AppError> {
        {
            let mut flag = self.embedding_enabled.lock().expect("lock");
            *flag = enabled;
        }
        // Persist to file
        let path = self.paths.app_data_dir.join("embedding_enabled.json");
        let json = serde_json::json!({ "enabled": enabled });
        if let Ok(s) = serde_json::to_string_pretty(&json) {
            if let Err(e) = std::fs::write(&path, s) {
                warn!("Failed to persist embedding enabled config: {e}");
            }
        }

        // If enabling and engine not loaded, try to load
        if enabled {
            let mut engine_guard = self.local_embedding.lock().expect("lock");
            if engine_guard.is_none() {
                let model_dir = self
                    .paths
                    .app_data_dir
                    .join("models")
                    .join(EMBEDDING_MODEL_NAME);
                if model_dir.exists() {
                    match LocalEmbeddingEngine::load(&model_dir) {
                        Ok(engine) => {
                            *engine_guard = Some(engine);
                            info!("Local embedding engine loaded");
                        }
                        Err(e) => {
                            error!("Failed to load embedding engine: {e}");
                        }
                    }
                }
            }
        }

        let db = self.db.lock().expect("db lock");
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

    pub fn embedding_status(&self) -> (bool, bool, Option<String>, Option<usize>) {
        let enabled = *self.embedding_enabled.lock().expect("lock");
        let guard = self.local_embedding.lock().expect("lock");
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

    pub fn set_embedding_engine(&self, engine: LocalEmbeddingEngine) {
        *self.local_embedding.lock().expect("lock") = Some(engine);
    }

    /// Scan invoices and regenerate any missing preview thumbnails from normalized images.
    fn regenerate_missing_previews(&self) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(_) => return,
        };
        let preview_root = self.paths.thumbnails_dir.join("previews");
        let normalized_root = self.paths.thumbnails_dir.join("normalized");

        let rows: Vec<(i64, i64, Option<String>)> = match db.prepare(
            "SELECT id, raw_file_id, source_page_range FROM invoices",
        ) {
            Ok(mut stmt) => match stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?))
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
            let output = Command::new("magick")
                .arg(&normalized_path)
                .arg("-auto-orient")
                .arg("-resize")
                .arg("800x800>")
                .arg("-background")
                .arg("white")
                .arg("-alpha")
                .arg("remove")
                .arg("-alpha")
                .arg("off")
                .arg("-strip")
                .arg("-quality")
                .arg("85")
                .arg(&preview_path)
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    fixed += 1;
                    info!("Regenerated missing preview for invoice {invoice_id}");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!("Invoice {invoice_id}: magick failed: {stderr}");
                }
                Err(e) => {
                    warn!("Invoice {invoice_id}: magick not found or failed to run: {e}");
                }
            }
        }
        if fixed > 0 {
            info!("Regenerated {fixed} missing preview(s)");
        }
    }

    pub fn set_badge_config(&self, config: BadgeConfig) -> Result<(), AppError> {
        let sanitized = sanitize_badge_config(config);
        {
            let mut cfg = self.badge_config.lock().expect("lock");
            *cfg = sanitized.clone();
        }
        let path = self.paths.app_data_dir.join("badge_config.json");
        if let Ok(json) = serde_json::to_string_pretty(&sanitized) {
            if let Err(e) = std::fs::write(&path, json) {
                error!("Failed to persist badge config: {e}");
            }
        }
        let db = self.db.lock().expect("db lock");
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

    pub fn get_badge_config(&self) -> BadgeConfig {
        self.badge_config.lock().expect("lock").clone()
    }

    pub fn set_price_config(&self, config: PriceConfig) -> Result<(), AppError> {
        {
            let mut cfg = self.price_config.lock().expect("lock");
            *cfg = config.clone();
        }
        let path = self.paths.app_data_dir.join("price_config.json");
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            if let Err(e) = std::fs::write(&path, json) {
                error!("Failed to persist price config: {e}");
            }
        }
        Ok(())
    }

    pub fn get_price_config(&self) -> PriceConfig {
        self.price_config.lock().expect("lock").clone()
    }

    pub fn get_llm_usage(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<crate::extractor::LlmUsageStats, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(crate::extractor::get_llm_usage(
            &db,
            date_from.as_deref(),
            date_to.as_deref(),
        )?)
    }

    pub fn set_invoice_badge(
        &self,
        invoice_id: i64,
        group_name: String,
        value: Option<String>,
    ) -> Result<Vec<InvoiceBadgeSelection>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(set_invoice_badge(&mut db, invoice_id, group_name, value)?)
    }

    pub fn set_llm_config(&self, config: LlmProviderConfig) -> Result<(), AppError> {
        let mut cfg = self.llm_config.lock().expect("lock");
        *cfg = Some(config.clone());
        let path = self.paths.app_data_dir.join("llm_config.json");
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            if let Err(e) = std::fs::write(&path, json) {
                error!("Failed to persist LLM config: {e}");
            }
        }
        info!("LLM config updated");
        Ok(())
    }

    pub fn get_llm_config(&self) -> Option<LlmProviderConfig> {
        self.llm_config.lock().expect("lock").clone()
    }

    pub fn llm_audit_config(&self) -> Option<LlmAuditConfig> {
        (*self.llm_audit_enabled.lock().expect("lock")).then(|| LlmAuditConfig {
            dir: self.paths.llm_audit_dir.clone(),
        })
    }

    pub fn set_llm_audit_enabled(&self, enabled: bool) {
        *self.llm_audit_enabled.lock().expect("lock") = enabled;
        let path = self.paths.app_data_dir.join("audit_config.json");
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({"enabled": enabled})) {
            if let Err(e) = std::fs::write(&path, json) {
                error!("Failed to persist audit config: {e}");
            }
        }
        info!("LLM audit enabled: {enabled}");
    }

    pub fn get_llm_audit_enabled(&self) -> bool {
        *self.llm_audit_enabled.lock().expect("lock")
    }

    pub fn load_config_raw<T: serde::de::DeserializeOwned>(
        app_data_dir: &Path,
        filename: &str,
    ) -> Option<T> {
        let path = app_data_dir.join(filename);
        let json = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&json).ok()
    }

    pub fn test_chroma_connection(&self) -> Result<bool, AppError> {
        Ok(self.chroma_config.lock().expect("lock").enabled)
    }

    pub fn test_embedding_connection(&self) -> Result<EmbeddingTestResult, AppError> {
        let mut guard = self.local_embedding.lock().expect("lock");
        let engine = guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;
        Ok(run_embedding_test(engine)?)
    }

    pub fn regenerate_all_embeddings(&self) -> Result<RegenerateEmbeddingsResult, AppError> {
        let mut engine_guard = self.local_embedding.lock().expect("lock");
        let engine = engine_guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;

        let db = self.db.lock().expect("db lock");
        let thumb_dir = self.paths.thumbnails_dir.clone();

        let invoice_ids: Vec<i64> = {
            let mut stmt = db.prepare("SELECT id FROM invoices")?;
            let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
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
                    if let Err(e) = chroma::upsert_embedding(
                        &db,
                        invoice_id,
                        &result.embedding,
                        &text,
                    ) {
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

    pub fn search_invoices_semantic(
        &self,
        query: String,
        limit: usize,
    ) -> Result<Vec<crate::chroma::SimilarResult>, AppError> {
        if !self.chroma_config.lock().expect("lock").enabled {
            return Err(AppError::Chroma(chroma::ChromaError::NotConfigured));
        }
        let mut guard = self.local_embedding.lock().expect("lock");
        let engine = guard
            .as_mut()
            .ok_or(AppError::Embedding(EmbeddingError::NotLoaded))?;
        let result = generate_embedding(engine, &query)?;
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(chroma::query_similar(&db, &result.embedding, limit)?)
    }

    pub fn raw_file_for_recognition(
        &self,
        raw_file_id: i64,
    ) -> Result<RawFileForRecognition, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
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

    pub fn list_events(
        &self,
        page: i64,
        page_size: i64,
        event_type: Option<&str>,
    ) -> Result<EventListResult, AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::list_events(&db, page, page_size, event_type)?)
    }

    pub fn get_unread_event_count(&self) -> Result<i64, AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::get_unread_event_count(&db)?)
    }

    pub fn get_unread_failed_import_event_count(&self) -> Result<i64, AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::get_unread_failed_import_event_count(&db)?)
    }

    pub fn mark_event_read(&self, id: i64) -> Result<(), AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::mark_event_read(&db, id)?)
    }

    pub fn mark_all_events_read(&self) -> Result<(), AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::mark_all_events_read(&db)?)
    }

    pub fn delete_all_events(&self) -> Result<usize, AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(event::delete_all_events(&db)?)
    }

    pub fn record_usage_log(
        &self,
        operation: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
    ) -> Result<(), AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(crate::extractor::insert_usage_log(
            &db,
            operation,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
        )?)
    }

    pub fn record_recognition_event(
        &self,
        invoice_id: i64,
        invoice_title: &str,
        success: bool,
        duration_ms: u128,
        model: &str,
        page_count: usize,
    ) -> Result<(), AppError> {
        let db = self.db.lock().expect("db lock");
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

    pub fn create_agent_session(&self) -> Result<AgentSession, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::create_session(&db, None)?)
    }

    pub fn list_agent_sessions(&self) -> Result<Vec<AgentSession>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::list_sessions(&db)?)
    }

    pub fn get_agent_session(&self, session_id: i64) -> Result<Vec<AgentMessageRow>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::get_session_messages(&db, session_id)?)
    }

    pub fn export_logs(&self, output_path: &str) -> Result<ExportLogsResult, AppError> {
        info!("Exporting logs to {}", output_path);
        let output_path = Path::new(output_path);
        let file = std::fs::File::create(output_path)?;
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Add database file
        if self.paths.database_path.exists() {
            stream_file_to_zip(&mut zip_writer, "invoicevault.sqlite3", &self.paths.database_path, options)?;
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
                            stream_file_to_zip(&mut zip_writer, &format!("logs/{}", name), &path, options)?;
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
        let db = self.db.lock().expect("db lock");
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

    pub fn export_backup(&self, output_path: &str) -> Result<ExportLogsResult, AppError> {
        info!("Creating full backup at {}", output_path);
        let output_path = Path::new(output_path);
        let file = std::fs::File::create(output_path)?;
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let base = &self.paths.app_data_dir;
        add_dir_to_zip(&mut zip_writer, base, base, options)?;

        let finished = zip_writer
            .finish()
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let metadata = finished.metadata()?;
        let file_size = metadata.len();

        info!("Backup complete: {} bytes", file_size);

        // Record event
        let db = self.db.lock().expect("db lock");
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

    pub fn cleanup_storage(&self) -> Result<CleanupStorageResult, AppError> {
        info!("Starting storage cleanup");
        let db = self.db.lock().expect("db lock");

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
            if let Err(e) = db.execute("DELETE FROM import_jobs WHERE raw_file_id = ?1", [*raw_id]) {
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

    pub fn raw_file_has_invoices(&self, raw_file_id: i64) -> Result<bool, AppError> {
        let db = self.db.lock().expect("db lock");
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM invoices WHERE raw_file_id = ?1",
            [raw_file_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn set_import_job_status_for_raw_file(
        &self,
        raw_file_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let db = self.db.lock().expect("db lock");
        Ok(update_import_job_status_by_raw_file(
            &db,
            raw_file_id,
            status,
            error_message,
        )?)
    }

    pub fn invoice_id_for_raw_file(&self, raw_file_id: i64) -> Result<Option<i64>, AppError> {
        let db = self.db.lock().expect("db lock");
        let invoice_id = db
            .query_row(
                "SELECT id FROM invoices WHERE raw_file_id = ?1 ORDER BY id DESC LIMIT 1",
                [raw_file_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(invoice_id)
    }

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

        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::insert_attachment(
            &db,
            session_id,
            &original_name,
            mime.as_deref(),
            metadata.len() as i64,
            &display_path(&dest),
        )?)
    }

    pub fn list_agent_attachments(
        &self,
        session_id: i64,
    ) -> Result<Vec<AgentAttachment>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::list_session_attachments(&db, session_id)?)
    }

    pub fn list_agent_tasks(&self, session_id: i64) -> Result<Vec<AgentTask>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::list_session_tasks(&db, session_id)?)
    }

    pub fn list_agent_artifacts(&self, session_id: i64) -> Result<Vec<AgentArtifact>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::list_session_artifacts(&db, session_id)?)
    }

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

    pub fn delete_agent_artifact(&self, session_id: i64, artifact_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        agent::delete_artifact(&db, session_id, artifact_id)?;
        Ok(())
    }

    fn agent_artifact_path(&self, session_id: i64, artifact_id: i64) -> Result<PathBuf, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        let artifact = agent::get_artifact(&db, artifact_id)?;
        if artifact.session_id != session_id {
            return Err(AppError::InvalidOperation("产物不属于当前会话".to_owned()));
        }
        let path = artifact
            .file_path
            .ok_or_else(|| AppError::InvalidOperation("产物没有可打开的文件路径".to_owned()))?;
        Ok(PathBuf::from(path))
    }

    pub fn delete_agent_session(&self, session_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::delete_session(&db, session_id)?)
    }

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
        let db = self.db.lock().expect("database mutex poisoned");
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

    pub async fn confirm_agent_action(
        &self,
        request: ConfirmRequest,
        config: &crate::llm::LlmProviderConfig,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
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

    pub async fn confirm_agent_action_stream(
        &self,
        request: ConfirmRequest,
        config: &crate::llm::LlmProviderConfig,
        stream_sink: agent::AgentStreamSink,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            self.paths.app_data_dir.clone(),
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

fn make_tool_executor(
    thumbnails_dir: std::path::PathBuf,
    app_data_dir: std::path::PathBuf,
    db: Arc<Mutex<Connection>>,
    badge_config: Arc<Mutex<BadgeConfig>>,
    price_config: Arc<Mutex<PriceConfig>>,
    app_handle: AppHandle,
    session_id: i64,
) -> impl Fn(&str, &serde_json::Value) -> ToolExecResult {
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
            let conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
        "export_invoices_with_template" => {
            let Some(attachment_id) = args.get("attachment_id").and_then(|v| v.as_i64()) else {
                return ToolExecResult::Error {
                    message: "缺少 attachment_id 参数".to_owned(),
                };
            };
            let conn = db.lock().expect("db lock");
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

            let columns = match export_columns_from_template(&attachment) {
                Ok(columns) if !columns.is_empty() => columns,
                Ok(_) => {
                    return ToolExecResult::Error {
                        message: "模板表头未匹配到可导出的发票字段".to_owned(),
                    }
                }
                Err(e) => return ToolExecResult::Error { message: e },
            };

            let is_confirmed = args
                .get("_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_confirmed {
                let format = args
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("xlsx");
                let preview_request = ExportPreviewRequest {
                    columns: Some(columns.clone()),
                    ..export_preview_request_from_args(args)
                };
                let conn = db.lock().expect("db lock");
                let preview = preview_export(&conn, preview_request).ok();
                let row_count = preview.as_ref().map(|p| p.row_count).unwrap_or(0);
                let column_desc = columns.join(", ");
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_invoices_with_template".to_owned(),
                    arguments: args.clone(),
                    message: format!(
                        "将按模板 {} 的表头导出 {row_count} 张发票为 {format} 格式，字段: {column_desc}。请选择保存位置。",
                        attachment.original_name
                    ),
                };
            }

            let Some(output_path) = args.get("output_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("xlsx")
                .to_owned();
            let request = ExportInvoicesRequest {
                format,
                output_path: output_path.to_owned(),
                invoice_ids: json_i64_vec(args, "invoice_ids"),
                columns: Some(columns),
                date_from: json_string(args, "date_from"),
                date_to: json_string(args, "date_to"),
            };
            let conn = db.lock().expect("db lock");
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
            match export_invoices(&conn, request) {
                Ok(result) => {
                    let artifact = record_export_artifact(
                        &conn,
                        session_id,
                        task.as_ref().map(|task| task.id),
                        "模板导出结果",
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
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "export_invoices".to_owned(),
                    arguments: args.clone(),
                    message: desc,
                };
            }
            let Some(output_path) = args.get("output_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("csv")
                .to_owned();
            let invoice_ids: Option<Vec<i64>> = args
                .get("invoice_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect());
            let columns: Option<Vec<String>> = json_string_vec(args, "columns");
            let request = ExportInvoicesRequest {
                format,
                output_path: output_path.to_owned(),
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
            let conn = db.lock().expect("db lock");
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
                    let artifact = record_export_artifact(
                        &conn,
                        session_id,
                        task.as_ref().map(|task| task.id),
                        "发票导出结果",
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
            let mut conn = db.lock().expect("db lock");
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
            let conn = db.lock().expect("db lock");
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
            let mut conn = db.lock().expect("db lock");
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
                crate::AppState::load_config_raw(&app_data_dir, "badge_config.json")
                    .unwrap_or_default();
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
                };
            }
            let config: BadgeConfig = match serde_json::from_value(
                args.get("config")
                    .cloned()
                    .unwrap_or_else(|| args.clone()),
            ) {
                Ok(c) => c,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("参数解析失败: {e}"),
                    }
                }
            };
            let sanitized = sanitize_badge_config(config);
            if let Ok(json) = serde_json::to_string_pretty(&sanitized) {
                if let Err(e) = std::fs::write(app_data_dir.join("badge_config.json"), json) {
                    return ToolExecResult::Error {
                        message: format!("写入配置失败: {e}"),
                    };
                }
            }
            {
                let mut cfg = badge_config.lock().expect("badge_config lock");
                *cfg = sanitized.clone();
            }
            match serde_json::to_string(&sanitized) {
                Ok(content) => {
                    let conn = db.lock().expect("db lock");
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
                let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("(取消)");
                return ToolExecResult::ConfirmationRequired {
                    tool_name: "set_invoice_badge".to_owned(),
                    arguments: args.clone(),
                    message: format!("将设置发票 #{invoice_id} 的标签 {group} = {value}，是否确认？"),
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
            let mut conn = db.lock().expect("db lock");
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
                crate::AppState::load_config_raw(&app_data_dir, "price_config.json")
                    .unwrap_or_default();
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
                };
            }
            let config: PriceConfig = match serde_json::from_value(
                args.get("config")
                    .cloned()
                    .unwrap_or_else(|| args.clone()),
            ) {
                Ok(c) => c,
                Err(e) => {
                    return ToolExecResult::Error {
                        message: format!("参数解析失败: {e}"),
                    }
                }
            };
            if let Ok(json) = serde_json::to_string_pretty(&config) {
                if let Err(e) = std::fs::write(app_data_dir.join("price_config.json"), json) {
                    return ToolExecResult::Error {
                        message: format!("写入配置失败: {e}"),
                    };
                }
            }
            {
                let mut cfg = price_config.lock().expect("price_config lock");
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
            let theme_path = app_data_dir.join("theme.json");
            let theme = std::fs::read_to_string(&theme_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
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
            if let Ok(json) =
                serde_json::to_string_pretty(&serde_json::json!({ "theme": theme }))
            {
                if let Err(e) = std::fs::write(app_data_dir.join("theme.json"), json) {
                    return ToolExecResult::Error {
                        message: format!("写入主题配置失败: {e}"),
                    };
                }
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
                };
            }
            let Some(output_path) = args.get("output_path").and_then(|v| v.as_str()) else {
                return ToolExecResult::Error {
                    message: "缺少 output_path 参数".to_owned(),
                };
            };
            let logs_dir = app_data_dir.join("logs");
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
                    let _ = stream_file_to_zip(
                        &mut zip_writer,
                        config_name,
                        &config_path,
                        options,
                    );
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
                let _ = add_dir_to_zip(
                    &mut zip_writer,
                    &app_data_dir,
                    &llm_audit_dir,
                    options,
                );
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
            if let Err(e) =
                add_dir_to_zip(&mut zip_writer, &app_data_dir, &app_data_dir, options)
            {
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
                };
            }
            let conn = db.lock().expect("db lock");
            let mut files_removed = 0usize;
            let mut bytes_freed: u64 = 0;
            let raw_dir = app_data_dir.join("raw");
            if raw_dir.exists() {
                let mut referenced_paths: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                if let Ok(mut stmt) = conn.prepare("SELECT storage_path FROM raw_files") {
                    if let Ok(rows) =
                        stmt.query_map([], |row| row.get::<_, String>(0))
                    {
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
                let conn = db.lock().expect("db lock");
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

fn export_columns_from_template(attachment: &AgentAttachment) -> Result<Vec<String>, String> {
    let inspection = inspect_spreadsheet_attachment(attachment, 3)?;
    let labels = inspection
        .sheets
        .first()
        .map(|sheet| {
            sheet
                .columns
                .iter()
                .map(|column| column.label.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(resolve_export_column_keys_from_labels(&labels))
}

fn json_i64_vec(args: &serde_json::Value, key: &str) -> Option<Vec<i64>> {
    args.get(key)
        .and_then(|value| value.as_array())
        .map(|items| items.iter().filter_map(|value| value.as_i64()).collect())
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
        for part in segment.split("<t").skip(1) {
            if let Some(after) = part.split('>').nth(1) {
                if let Some(value) = after.split("</t>").next() {
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
    let header_index = rows
        .iter()
        .position(|row| row.iter().any(|cell| !cell.trim().is_empty()))
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

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect()
}

fn create_app_paths(app_data_dir: &Path) -> Result<AppPaths, AppError> {
    let raw_dir = app_data_dir.join("raw");
    let thumbnails_dir = app_data_dir.join("thumbnails");
    let logs_dir = app_data_dir.join("logs");
    let llm_audit_dir = app_data_dir.join("llm_audit");
    let agent_uploads_dir = app_data_dir.join("agent_uploads");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&thumbnails_dir)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&llm_audit_dir)?;
    fs::create_dir_all(&agent_uploads_dir)?;

    Ok(AppPaths {
        app_data_dir: app_data_dir.to_path_buf(),
        database_path: app_data_dir.join("invoicevault.sqlite3"),
        raw_dir,
        thumbnails_dir,
        logs_dir,
        llm_audit_dir,
        agent_uploads_dir,
    })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn open_path_with_system(path: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn()?;
    Ok(())
}

fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
}

fn io_err(e: std::io::Error) -> AppError {
    AppError::Io(e)
}

pub fn stream_file_to_zip(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    zip_path: &str,
    file_path: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), AppError> {
    zip_writer.start_file(zip_path, options).map_err(zip_err)?;
    let mut reader = std::io::BufReader::new(std::fs::File::open(file_path)?);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        zip_writer.write_all(&buf[..n]).map_err(io_err)?;
    }
    Ok(())
}

pub fn add_dir_to_zip(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), AppError> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let relative = path.strip_prefix(base).unwrap_or(&path);
            let name = relative.to_string_lossy().replace('\\', "/");
            stream_file_to_zip(zip_writer, &name, &path, options)?;
        } else if path.is_dir() {
            add_dir_to_zip(zip_writer, base, &path, options)?;
        }
    }
    Ok(())
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            walk_dir(&path, files)?;
        }
    }
    Ok(())
}

fn remove_empty_dirs(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_empty_dirs(&path);
                // Best-effort: only succeeds if dir is truly empty
                let _ = std::fs::remove_dir(&path);
            }
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Storage(StorageError::Database(value))
    }
}
