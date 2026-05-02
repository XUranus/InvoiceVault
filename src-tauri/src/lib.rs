mod app_core;
mod dedupe;
mod document;
mod exporter;
mod extractor;
mod importer;
mod llm;
mod raw_store;
mod storage;
mod watcher;

use app_core::{AppHealth, AppState};
use dedupe::{DedupeCheckResult, ResolveDuplicateRequest};
use exporter::{ExportInvoicesRequest, ExportResult};
use extractor::{
    InvoiceDetail, InvoiceItemRow, InvoiceSearchParams, InvoiceSearchResult,
    InvoiceSummary, SaveInvoiceExtractionRequest, UpdateInvoiceItemsRequest,
    UpdateInvoiceRequest, UpdateInvoiceResult,
};
use importer::{ImportJobSummary, ImportRequest};
use llm::{
    recognize_invoice_image, test_llm_connection as run_llm_connection_test,
    LlmConnectionTestResult, LlmProviderConfig,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use watcher::{
    AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus,
};

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> Result<AppHealth, String> {
    state.health().map_err(|err| err.to_string())
}

#[tauri::command]
fn import_files(
    state: State<'_, AppState>,
    request: ImportRequest,
) -> Result<Vec<ImportJobSummary>, String> {
    state
        .import_files(request.paths)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_import_jobs(state: State<'_, AppState>) -> Result<Vec<ImportJobSummary>, String> {
    state.list_import_jobs().map_err(|err| err.to_string())
}

#[tauri::command]
fn save_invoice_extraction(
    state: State<'_, AppState>,
    request: SaveInvoiceExtractionRequest,
) -> Result<InvoiceSummary, String> {
    state
        .save_invoice_extraction(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_invoices(state: State<'_, AppState>) -> Result<Vec<InvoiceSummary>, String> {
    state.list_invoices().map_err(|err| err.to_string())
}

#[tauri::command]
fn search_invoices(
    state: State<'_, AppState>,
    params: InvoiceSearchParams,
) -> Result<InvoiceSearchResult, String> {
    state
        .search_invoices(params)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_invoice_detail(
    state: State<'_, AppState>,
    invoice_id: i64,
) -> Result<InvoiceDetail, String> {
    state
        .get_invoice_detail(invoice_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_invoice(
    state: State<'_, AppState>,
    request: UpdateInvoiceRequest,
) -> Result<UpdateInvoiceResult, String> {
    state
        .update_invoice(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_invoice_items(
    state: State<'_, AppState>,
    request: UpdateInvoiceItemsRequest,
) -> Result<Vec<InvoiceItemRow>, String> {
    state
        .update_invoice_items(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn check_invoice_duplicates(
    state: State<'_, AppState>,
    invoice_id: i64,
) -> Result<DedupeCheckResult, String> {
    state
        .check_invoice_duplicates(invoice_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn resolve_duplicate(
    state: State<'_, AppState>,
    request: ResolveDuplicateRequest,
) -> Result<(), String> {
    state
        .resolve_duplicate(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn export_invoices(
    state: State<'_, AppState>,
    request: ExportInvoicesRequest,
) -> Result<ExportResult, String> {
    state
        .export_invoices(request)
        .map_err(|err| err.to_string())
}

#[derive(Debug, Deserialize)]
struct RecognizeRawFileRequest {
    raw_file_id: i64,
    config: LlmProviderConfig,
}

#[derive(Debug, Serialize)]
struct RecognizeRawFileResult {
    invoices: Vec<InvoiceSummary>,
    model: String,
    duration_ms: u128,
    response_preview: String,
    page_count: usize,
    thumbnail_paths: Vec<String>,
}

#[tauri::command]
async fn recognize_raw_file(
    state: State<'_, AppState>,
    request: RecognizeRawFileRequest,
) -> Result<RecognizeRawFileResult, String> {
    let raw_file = state
        .raw_file_for_recognition(request.raw_file_id)
        .map_err(|err| err.to_string())?;
    let recognition_inputs = if raw_file.mime_type == "application/pdf" {
        state
            .render_pdf_pages_for_recognition(raw_file.id, &raw_file.storage_path)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|page| {
                let prepared = state
                    .prepare_image_for_recognition(
                        raw_file.id,
                        &page.image_path,
                        Some(page.page_number),
                    )
                    .map_err(|err| err.to_string())?;
                Ok(RecognitionInput {
                    source_page_range: Some(page.page_number.to_string()),
                    image_path: prepared.image_path,
                    thumbnail_path: prepared.thumbnail_path,
                    mime_type: prepared.mime_type,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        let prepared = state
            .prepare_image_for_recognition(raw_file.id, &raw_file.storage_path, None)
            .map_err(|err| err.to_string())?;
        vec![RecognitionInput {
            source_page_range: None,
            image_path: prepared.image_path,
            thumbnail_path: prepared.thumbnail_path,
            mime_type: prepared.mime_type,
        }]
    };

    let page_count = recognition_inputs.len();
    let mut invoices = Vec::new();
    let mut total_duration_ms = 0_u128;
    let mut response_previews = Vec::new();
    let mut thumbnail_paths = Vec::new();
    let mut model = request.config.model.clone();

    for input in recognition_inputs {
        thumbnail_paths.push(input.thumbnail_path.to_string_lossy().into_owned());
        let recognition =
            recognize_invoice_image(request.config.clone(), &input.image_path, &input.mime_type)
                .await
                .map_err(|err| err.to_string())?;

        model = recognition.model.clone();
        total_duration_ms += recognition.duration_ms;
        response_previews.push(format!(
            "{}: {}",
            input
                .source_page_range
                .as_deref()
                .map(|page| format!("page {page}"))
                .unwrap_or_else(|| "image".to_owned()),
            recognition.response_preview
        ));

        let invoice = state
            .save_invoice_extraction(SaveInvoiceExtractionRequest {
                raw_file_id: raw_file.id,
                source_page_range: input.source_page_range,
                provider_name: Some(request.config.base_url.clone()),
                model: Some(recognition.model),
                response_json: recognition.response_json,
            })
            .map_err(|err| err.to_string())?;
        invoices.push(invoice);
    }

    Ok(RecognizeRawFileResult {
        invoices,
        model,
        duration_ms: total_duration_ms,
        response_preview: response_previews.join("\n"),
        page_count,
        thumbnail_paths,
    })
}

#[derive(Debug)]
struct RecognitionInput {
    source_page_range: Option<String>,
    image_path: std::path::PathBuf,
    thumbnail_path: std::path::PathBuf,
    mime_type: String,
}

#[tauri::command]
async fn test_llm_connection(config: LlmProviderConfig) -> Result<LlmConnectionTestResult, String> {
    run_llm_connection_test(config)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn add_watch_dir(
    state: State<'_, AppState>,
    request: AddWatchDirRequest,
) -> Result<WatchDirStatus, String> {
    state.add_watch_dir(request).map_err(|err| err.to_string())
}

#[tauri::command]
fn remove_watch_dir(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.remove_watch_dir(id).map_err(|err| err.to_string())
}

#[tauri::command]
fn list_watch_dirs(state: State<'_, AppState>) -> Result<Vec<WatchDirStatus>, String> {
    state.list_watch_dirs().map_err(|err| err.to_string())
}

#[tauri::command]
fn update_watch_dir(
    state: State<'_, AppState>,
    id: i64,
    request: UpdateWatchDirRequest,
) -> Result<WatchDirStatus, String> {
    state
        .update_watch_dir(id, request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn toggle_watch_dir(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<WatchDirStatus, String> {
    state
        .toggle_watch_dir(id, enabled)
        .map_err(|err| err.to_string())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "receiptier=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            import_files,
            list_import_jobs,
            save_invoice_extraction,
            list_invoices,
            search_invoices,
            get_invoice_detail,
            update_invoice,
            update_invoice_items,
            check_invoice_duplicates,
            resolve_duplicate,
            export_invoices,
            recognize_raw_file,
            test_llm_connection,
            add_watch_dir,
            remove_watch_dir,
            list_watch_dirs,
            update_watch_dir,
            toggle_watch_dir
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Receiptier");
}
