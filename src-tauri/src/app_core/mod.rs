use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    dedupe::{
        check_invoice_duplicates as run_dedupe_check, resolve_duplicate as run_dedupe_resolve,
        DedupeCheckResult, DedupeError, ResolveDuplicateRequest,
    },
    document::{
        prepare_image_for_recognition, render_pdf_pages, DocumentError, PreparedImage,
        RenderedPdfPage,
    },
    extractor::{
        get_invoice_detail, list_invoices, save_invoice_extraction, search_invoices,
        update_invoice, update_invoice_items, ExtractorError, InvoiceDetail,
        InvoiceItemRow, InvoiceSearchParams, InvoiceSearchResult, InvoiceSummary,
        SaveInvoiceExtractionRequest, UpdateInvoiceItemsRequest, UpdateInvoiceRequest,
        UpdateInvoiceResult,
    },
    exporter::{export_invoices, ExportError, ExportInvoicesRequest, ExportResult},
    importer::{import_files, list_import_jobs, ImportError, ImportJobSummary},
    storage::{run_migrations, StorageError},
    watcher::{
        AddWatchDirRequest, UpdateWatchDirRequest, WatcherError, WatcherManager, WatchDirStatus,
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

        let watcher_manager = WatcherManager::new(
            Arc::clone(&db),
            paths.raw_dir.clone(),
            app.clone(),
        )?;

        Ok(Self {
            paths,
            db,
            watcher_manager,
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

    pub fn list_import_jobs(&self) -> Result<Vec<ImportJobSummary>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(list_import_jobs(&db)?)
    }

    pub fn save_invoice_extraction(
        &self,
        request: SaveInvoiceExtractionRequest,
    ) -> Result<InvoiceSummary, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let invoice = save_invoice_extraction(&mut db, request)?;
        // Trigger dedupe check (best-effort, don't fail on error)
        let _ = run_dedupe_check(&db, invoice.id);
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
        Ok(get_invoice_detail(&db, &self.paths.thumbnails_dir, invoice_id)?)
    }

    pub fn update_invoice(
        &self,
        request: UpdateInvoiceRequest,
    ) -> Result<UpdateInvoiceResult, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let result = update_invoice(&mut db, request)?;
        // Trigger dedupe check (best-effort)
        let _ = run_dedupe_check(&db, result.invoice.id);
        Ok(result)
    }

    pub fn update_invoice_items(
        &self,
        request: UpdateInvoiceItemsRequest,
    ) -> Result<Vec<InvoiceItemRow>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(update_invoice_items(&mut db, request)?)
    }

    pub fn check_invoice_duplicates(
        &self,
        invoice_id: i64,
    ) -> Result<DedupeCheckResult, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(run_dedupe_check(&db, invoice_id)?)
    }

    pub fn resolve_duplicate(
        &self,
        request: ResolveDuplicateRequest,
    ) -> Result<(), AppError> {
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

    pub fn add_watch_dir(
        &self,
        request: AddWatchDirRequest,
    ) -> Result<WatchDirStatus, AppError> {
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
