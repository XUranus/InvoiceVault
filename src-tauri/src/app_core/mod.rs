use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    agent::{
        self, AgentError, AgentMessageRow, AgentResponse, AgentSession, ConfirmRequest,
        ToolExecResult,
    },
    chroma::{self, ChromaConfig, ChromaError},
    dedupe::{
        check_invoice_duplicates as run_dedupe_check, resolve_duplicate as run_dedupe_resolve,
        DedupeCheckResult, DedupeError, ResolveDuplicateRequest,
    },
    document::{
        prepare_image_for_recognition, render_pdf_pages, DocumentError, PreparedImage,
        RenderedPdfPage,
    },
    embedding::{
        generate_embedding, test_embedding_connection as run_embedding_test, EmbeddingConfig,
        EmbeddingError, EmbeddingTestResult,
    },
    exporter::{export_invoices, ExportError, ExportInvoicesRequest, ExportResult},
    extractor::invoice_to_embedding_text,
    extractor::{
        get_dashboard_stats, get_invoice_detail, list_invoices, save_invoice_extraction,
        search_invoices, update_invoice, update_invoice_items, DashboardStats, ExtractorError,
        InvoiceDetail, InvoiceItemRow, InvoiceSearchParams, InvoiceSearchResult, InvoiceSummary,
        SaveInvoiceExtractionRequest, UpdateInvoiceItemsRequest, UpdateInvoiceRequest,
        UpdateInvoiceResult,
    },
    importer::{import_files, list_import_jobs, ImportError, ImportJobListResult, ImportJobSummary},
    storage::{run_migrations, StorageError},
    watcher::{
        AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus, WatcherError, WatcherManager,
    },
};

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
    #[error("chromadb error: {0}")]
    Chroma(#[from] ChromaError),
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
}

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub raw_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
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

pub struct AppState {
    paths: AppPaths,
    db: Arc<Mutex<Connection>>,
    watcher_manager: WatcherManager,
    chroma_config: Mutex<ChromaConfig>,
    embedding_config: Mutex<EmbeddingConfig>,
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
        let db = Arc::new(Mutex::new(db));

        let watcher_manager =
            WatcherManager::new(Arc::clone(&db), paths.raw_dir.clone(), app.clone())?;

        let chroma_config = ChromaConfig::default();
        let embedding_config = EmbeddingConfig::default();

        Ok(Self {
            paths,
            db,
            watcher_manager,
            chroma_config: Mutex::new(chroma_config),
            embedding_config: Mutex::new(embedding_config),
        })
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

    pub fn import_files(&self, paths: Vec<String>) -> Result<Vec<ImportJobSummary>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(import_files(&mut db, &self.paths.raw_dir, paths)?)
    }

    pub fn list_import_jobs(
        &self,
        page: i64,
        page_size: i64,
    ) -> Result<ImportJobListResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(list_import_jobs(&db, page, page_size)?)
    }

    pub fn save_invoice_extraction(
        &self,
        request: SaveInvoiceExtractionRequest,
    ) -> Result<InvoiceSummary, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let invoice = save_invoice_extraction(&mut db, request)?;
        let _ = run_dedupe_check(&db, invoice.id);

        // Best-effort embedding generation
        if self.chroma_config.lock().expect("lock").enabled {
            let emb_config = self.embedding_config.lock().expect("lock").clone();
            let db_arc = Arc::clone(&self.db);
            let thumb_dir = self.paths.thumbnails_dir.clone();
            let invoice_id = invoice.id;

            tauri::async_runtime::spawn(async move {
                let detail = match db_arc.lock() {
                    Ok(db) => get_invoice_detail(&db, &thumb_dir, invoice_id).ok(),
                    Err(_) => None,
                };
                if let Some(detail) = detail {
                    let text = invoice_to_embedding_text(&detail);
                    if let Ok(embedding) = generate_embedding(&emb_config, &text).await {
                        if let Ok(db) = db_arc.lock() {
                            let _ = chroma::upsert_embedding(
                                &db, invoice_id, &embedding, &text,
                            );
                            let _ = db.execute(
                                "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
                                [invoice_id],
                            );
                            if let Ok(similar) =
                                chroma::query_similar(&db, &embedding, 5)
                            {
                                let _ = crate::dedupe::detect_semantic_duplicates(
                                    &db, invoice_id, &similar,
                                );
                            }
                        }
                    }
                }
            });
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

    pub fn update_invoice(
        &self,
        request: UpdateInvoiceRequest,
    ) -> Result<UpdateInvoiceResult, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let result = update_invoice(&mut db, request)?;
        let _ = run_dedupe_check(&db, result.invoice.id);

        // Best-effort embedding regeneration
        if self.chroma_config.lock().expect("lock").enabled {
            let emb_config = self.embedding_config.lock().expect("lock").clone();
            let db_arc = Arc::clone(&self.db);
            let thumb_dir = self.paths.thumbnails_dir.clone();
            let invoice_id = result.invoice.id;

            tauri::async_runtime::spawn(async move {
                let detail = match db_arc.lock() {
                    Ok(db) => get_invoice_detail(&db, &thumb_dir, invoice_id).ok(),
                    Err(_) => None,
                };
                if let Some(detail) = detail {
                    let text = invoice_to_embedding_text(&detail);
                    if let Ok(embedding) = generate_embedding(&emb_config, &text).await {
                        if let Ok(db) = db_arc.lock() {
                            let _ = chroma::upsert_embedding(
                                &db, invoice_id, &embedding, &text,
                            );
                            let _ = db.execute(
                                "UPDATE invoices SET has_embedding = 1 WHERE id = ?1",
                                [invoice_id],
                            );
                            if let Ok(similar) =
                                chroma::query_similar(&db, &embedding, 5)
                            {
                                let _ = crate::dedupe::detect_semantic_duplicates(
                                    &db, invoice_id, &similar,
                                );
                            }
                        }
                    }
                }
            });
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

    pub fn check_invoice_duplicates(&self, invoice_id: i64) -> Result<DedupeCheckResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(run_dedupe_check(&db, invoice_id)?)
    }

    pub fn resolve_duplicate(&self, request: ResolveDuplicateRequest) -> Result<(), AppError> {
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

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(get_dashboard_stats(&db)?)
    }

    pub fn set_chroma_config(&self, config: ChromaConfig) -> Result<(), AppError> {
        let mut cfg = self.chroma_config.lock().expect("lock");
        *cfg = config;
        Ok(())
    }

    pub fn get_chroma_config(&self) -> ChromaConfig {
        self.chroma_config.lock().expect("lock").clone()
    }

    pub fn set_embedding_config(&self, config: EmbeddingConfig) -> Result<(), AppError> {
        let mut cfg = self.embedding_config.lock().expect("lock");
        *cfg = config;
        Ok(())
    }

    pub fn get_embedding_config(&self) -> EmbeddingConfig {
        self.embedding_config.lock().expect("lock").clone()
    }

    pub fn test_chroma_connection(&self) -> Result<bool, AppError> {
        Ok(self.chroma_config.lock().expect("lock").enabled)
    }

    pub async fn test_embedding_connection(&self) -> Result<EmbeddingTestResult, AppError> {
        let config = self.embedding_config.lock().expect("lock").clone();
        Ok(run_embedding_test(&config).await?)
    }

    pub async fn search_invoices_semantic(
        &self,
        query: String,
        limit: usize,
    ) -> Result<Vec<crate::chroma::SimilarResult>, AppError> {
        if !self.chroma_config.lock().expect("lock").enabled {
            return Err(AppError::Chroma(chroma::ChromaError::NotConfigured));
        }
        let emb_config = self.embedding_config.lock().expect("lock").clone();
        let embedding = generate_embedding(&emb_config, &query).await?;
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(chroma::query_similar(&db, &embedding, limit)?)
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

    pub fn delete_agent_session(&self, session_id: i64) -> Result<(), AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(agent::delete_session(&db, session_id)?)
    }

    pub async fn send_agent_message(
        &self,
        session_id: i64,
        content: &str,
        config: &crate::llm::LlmProviderConfig,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            Arc::clone(&self.db),
        ));
        Ok(agent::run_agent_turn(&self.db, session_id, content, config, executor).await?)
    }

    pub async fn confirm_agent_action(
        &self,
        request: ConfirmRequest,
        config: &crate::llm::LlmProviderConfig,
    ) -> Result<AgentResponse, AppError> {
        let executor = Arc::new(make_tool_executor(
            self.paths.thumbnails_dir.clone(),
            Arc::clone(&self.db),
        ));
        Ok(agent::continue_agent_turn(
            &self.db,
            request.session_id,
            request.confirmed,
            request.extra_params,
            config,
            executor,
        )
        .await?)
    }
}

fn make_tool_executor(
    thumbnails_dir: std::path::PathBuf,
    db: Arc<Mutex<Connection>>,
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
                status: args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                page: args.get("page").and_then(|v| v.as_i64()),
                page_size: args.get("page_size").and_then(|v| v.as_i64()),
                buyer_name: None,
                invoice_number: None,
                amount_min: None,
                amount_max: None,
                duplicate_status: None,
                sort_by: None,
                sort_order: None,
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
            let conn = db.lock().expect("db lock");
            match get_dashboard_stats(&conn) {
                Ok(stats) => {
                    let content = serde_json::to_string(&stats).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
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
                let desc = if count_hint > 0 {
                    format!("将导出 {count_hint} 张发票为 {format} 格式，请选择保存位置。")
                } else {
                    format!("将导出所有发票为 {format} 格式，请选择保存位置。")
                };
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
            let request = ExportInvoicesRequest {
                format,
                output_path: output_path.to_owned(),
                invoice_ids,
            };
            let conn = db.lock().expect("db lock");
            match export_invoices(&conn, request) {
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
            match update_invoice(&mut conn, request) {
                Ok(result) => {
                    let content = serde_json::to_string(&result).unwrap_or_default();
                    ToolExecResult::Success { content }
                }
                Err(e) => ToolExecResult::Error {
                    message: e.to_string(),
                },
            }
        }
        _ => ToolExecResult::Error {
            message: format!("未知工具: {tool_name}"),
        },
    }
}

fn create_app_paths(app_data_dir: &Path) -> Result<AppPaths, AppError> {
    let raw_dir = app_data_dir.join("raw");
    let thumbnails_dir = app_data_dir.join("thumbnails");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&thumbnails_dir)?;

    Ok(AppPaths {
        app_data_dir: app_data_dir.to_path_buf(),
        database_path: app_data_dir.join("receiptier.sqlite3"),
        raw_dir,
        thumbnails_dir,
    })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Storage(StorageError::Database(value))
    }
}
