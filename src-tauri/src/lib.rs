mod agent;
mod app_core;
mod chroma;
mod dedupe;
mod document;
mod embedding;
mod event;
mod exporter;
mod extractor;
mod importer;
mod llm;
mod raw_store;
mod storage;
mod watcher;

use agent::{AgentMessageRow, AgentResponse, AgentSession, ConfirmRequest};
use app_core::{AppHealth, AppState};
use chroma::{ChromaConfig, SimilarResult};
use dedupe::{DedupeCheckResult, ResolveDuplicateRequest};
use embedding::{EmbeddingConfig, EmbeddingTestResult};
use event::{EventListResult, NotificationRow};
use exporter::{ExportInvoicesRequest, ExportResult};
use llm::LlmProviderConfig;
use extractor::{
    DashboardStats, InvoiceDetail, InvoiceItemRow, InvoiceSearchParams, InvoiceSearchResult,
    InvoiceSummary, SaveInvoiceExtractionRequest, UpdateInvoiceItemsRequest, UpdateInvoiceRequest,
    UpdateInvoiceResult,
};
use importer::{ImportJobListResult, ImportJobSummary, ImportRequest};
use llm::{
    recognize_invoice_image, test_llm_connection as run_llm_connection_test,
    LlmConnectionTestResult,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct WindowSizeState {
    width: f64,
    height: f64,
}
use tauri::{Manager, State};
use watcher::{AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus};

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
fn list_import_jobs(
    state: State<'_, AppState>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<ImportJobListResult, String> {
    state
        .list_import_jobs(page.unwrap_or(1), page_size.unwrap_or(50))
        .map_err(|err| err.to_string())
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
    state.search_invoices(params).map_err(|err| err.to_string())
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
    state.update_invoice(request).map_err(|err| err.to_string())
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

        let rec_model = recognition.model.clone();
        let invoice = state
            .save_invoice_extraction(SaveInvoiceExtractionRequest {
                raw_file_id: raw_file.id,
                source_page_range: input.source_page_range,
                provider_name: Some(request.config.base_url.clone()),
                model: Some(recognition.model),
                response_json: recognition.response_json,
            })
            .map_err(|err| err.to_string())?;

        let title = invoice.seller_name.clone().unwrap_or_else(|| "未知".into());
        let _ = state.record_recognition_event(
            invoice.id,
            &title,
            true,
            recognition.duration_ms,
            &rec_model,
            1,
        );
        invoices.push(invoice);
    }

    // Create notification
    let count = invoices.len();
    let _ = state.create_notification(
        "info",
        &format!("识别完成: {count} 张发票"),
        &format!("共识别 {count} 张发票，模型 {model}，耗时 {total_duration_ms}ms"),
        None,
        None,
    );

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
fn get_dashboard_stats(state: State<'_, AppState>) -> Result<DashboardStats, String> {
    state.get_dashboard_stats().map_err(|err| err.to_string())
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

#[tauri::command]
fn set_chroma_config(state: State<'_, AppState>, config: ChromaConfig) -> Result<(), String> {
    state
        .set_chroma_config(config)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_chroma_config(state: State<'_, AppState>) -> Result<ChromaConfig, String> {
    Ok(state.get_chroma_config())
}

#[tauri::command]
fn set_embedding_config(state: State<'_, AppState>, config: EmbeddingConfig) -> Result<(), String> {
    state
        .set_embedding_config(config)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_embedding_config(state: State<'_, AppState>) -> Result<EmbeddingConfig, String> {
    Ok(state.get_embedding_config())
}

#[tauri::command]
fn test_chroma_connection(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .test_chroma_connection()
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn test_embedding_connection(
    state: State<'_, AppState>,
) -> Result<EmbeddingTestResult, String> {
    state
        .test_embedding_connection()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn search_invoices_semantic(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<SimilarResult>, String> {
    state
        .search_invoices_semantic(query, limit)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_agent_session(state: State<'_, AppState>) -> Result<AgentSession, String> {
    state.create_agent_session().map_err(|err| err.to_string())
}

#[tauri::command]
fn list_agent_sessions(state: State<'_, AppState>) -> Result<Vec<AgentSession>, String> {
    state.list_agent_sessions().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_agent_session(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentMessageRow>, String> {
    state
        .get_agent_session(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_agent_session(state: State<'_, AppState>, session_id: i64) -> Result<(), String> {
    state
        .delete_agent_session(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn send_agent_message(
    state: State<'_, AppState>,
    session_id: i64,
    content: String,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    state
        .send_agent_message(session_id, &content, &config)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn confirm_agent_action(
    state: State<'_, AppState>,
    request: ConfirmRequest,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    state
        .confirm_agent_action(request, &config)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_events(
    state: State<'_, AppState>,
    page: Option<i64>,
    page_size: Option<i64>,
    event_type: Option<String>,
) -> Result<EventListResult, String> {
    state
        .list_events(
            page.unwrap_or(1),
            page_size.unwrap_or(20),
            event_type.as_deref(),
        )
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_notifications(state: State<'_, AppState>) -> Result<Vec<NotificationRow>, String> {
    state.list_notifications().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_unread_notification_count(state: State<'_, AppState>) -> Result<i64, String> {
    state
        .get_unread_notification_count()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn mark_notification_read(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state
        .mark_notification_read(id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn mark_all_notifications_read(state: State<'_, AppState>) -> Result<(), String> {
    state
        .mark_all_notifications_read()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn dismiss_notification(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state
        .dismiss_notification(id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_llm_config(
    state: State<'_, AppState>,
    config: LlmProviderConfig,
) -> Result<(), String> {
    state.set_llm_config(config).map_err(|err| err.to_string())
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
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);

            // Restore and persist window size
            let window = app.get_webview_window("main").expect("main window");
            let state_path = app
                .path()
                .app_data_dir()
                .expect("app data dir")
                .join("window_state.json");

            if let Ok(json) = std::fs::read_to_string(&state_path) {
                if let Ok(saved) = serde_json::from_str::<WindowSizeState>(&json) {
                    use tauri::LogicalSize;
                    let _ = window.set_size(LogicalSize {
                        width: saved.width,
                        height: saved.height,
                    });
                }
            }

            let save_path = state_path.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(size) = event {
                    if let Ok(json) = serde_json::to_string(&WindowSizeState {
                        width: size.width as f64,
                        height: size.height as f64,
                    }) {
                        let _ = std::fs::write(&save_path, json);
                    }
                }
            });

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
            get_dashboard_stats,
            add_watch_dir,
            remove_watch_dir,
            list_watch_dirs,
            update_watch_dir,
            toggle_watch_dir,
            set_chroma_config,
            get_chroma_config,
            set_embedding_config,
            get_embedding_config,
            test_chroma_connection,
            test_embedding_connection,
            search_invoices_semantic,
            create_agent_session,
            list_agent_sessions,
            get_agent_session,
            delete_agent_session,
            send_agent_message,
            confirm_agent_action,
            list_events,
            list_notifications,
            get_unread_notification_count,
            mark_notification_read,
            mark_all_notifications_read,
            dismiss_notification,
            set_llm_config
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Receiptier");
}
