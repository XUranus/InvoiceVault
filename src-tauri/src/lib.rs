mod agent;
mod app_core;
mod chroma;
mod dedupe;
mod document;
mod email_manager;
mod embedding;
mod event;
mod exporter;
mod extractor;
mod importer;
mod llm;
mod raw_store;
mod storage;
mod watcher;

use agent::{
    AgentArtifact, AgentAttachment, AgentMessageRow, AgentResponse, AgentSession, AgentTask,
    ConfirmRequest,
};
use app_core::{
    import_failure_message, AppHealth, AppState, CleanupStorageResult, ExportLogsResult,
    RecognitionQueueStatus,
};
use chroma::{ChromaConfig, SimilarResult};
use dedupe::{DedupeCheckResult, ResolveDuplicateRequest, ResolveDuplicateResult};
use embedding::{EmbeddingConfig, EmbeddingTestResult};
use event::EventListResult;
use exporter::{ExportInvoicesRequest, ExportResult};
use extractor::{
    BadgeConfig, DashboardStats, InvoiceBadgeSelection, InvoiceDetail, InvoiceItemRow,
    InvoiceSearchParams, InvoiceSearchResult, InvoiceSummary, SaveInvoiceExtractionRequest,
    UpdateInvoiceItemsRequest, UpdateInvoiceRequest, UpdateInvoiceResult,
};
use importer::{ImportJobListResult, ImportJobSummary, ImportRequest};
use llm::LlmProviderConfig;
use llm::{
    recognize_invoice_image, test_llm_connection as run_llm_connection_test,
    LlmConnectionTestResult,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{path::Path, process::Command};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent,
};
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
struct WindowSizeState {
    width: f64,
    height: f64,
}
use watcher::{AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const TRAY_WORKBENCH_ID: &str = "tray-workbench";
const TRAY_VERSION_ID: &str = "tray-version";
const TRAY_QUIT_ID: &str = "tray-quit";

#[derive(Debug, Serialize)]
struct ExternalDependencyStatus {
    name: String,
    command: String,
    available: bool,
    version: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AgentStreamPayload {
    stream_id: String,
    session_id: i64,
    #[serde(flatten)]
    event: agent::AgentStreamEvent,
}

#[tauri::command]
fn check_external_dependencies() -> Vec<ExternalDependencyStatus> {
    vec![
        check_external_dependency("Poppler PDF renderer", "pdftoppm", &["-h"]),
        check_external_dependency("ImageMagick", "magick", &["-version"]),
    ]
}

fn check_external_dependency(name: &str, command: &str, args: &[&str]) -> ExternalDependencyStatus {
    match Command::new(command).args(args).output() {
        Ok(output) => {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let version = combined
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(160).collect::<String>());
            ExternalDependencyStatus {
                name: name.to_owned(),
                command: command.to_owned(),
                available: output.status.success() || version.is_some(),
                version,
                error: (!output.status.success()).then(|| output.status.to_string()),
            }
        }
        Err(err) => ExternalDependencyStatus {
            name: name.to_owned(),
            command: command.to_owned(),
            available: false,
            version: None,
            error: Some(err.to_string()),
        },
    }
}

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> Result<AppHealth, String> {
    state.health().map_err(|err| err.to_string())
}

#[tauri::command]
fn import_files(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportRequest,
) -> Result<Vec<ImportJobSummary>, String> {
    state
        .import_files(request.paths, &app)
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
fn mark_invoice_viewed(state: State<'_, AppState>, invoice_id: i64) -> Result<bool, String> {
    state
        .mark_invoice_viewed(invoice_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn count_unviewed_invoices(state: State<'_, AppState>) -> Result<i64, String> {
    state
        .count_unviewed_invoices()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn open_invoice_raw_file_in_browser(
    state: State<'_, AppState>,
    invoice_id: i64,
) -> Result<(), String> {
    let path = state
        .raw_file_path_for_invoice(invoice_id)
        .map_err(|err| err.to_string())?;
    if !path.exists() {
        return Err(format!("原文件不存在: {}", path.display()));
    }
    open_file_url_with_system_handler(&path)
}

fn open_file_url_with_system_handler(path: &Path) -> Result<(), String> {
    let canonical = path.canonicalize().map_err(|err| err.to_string())?;
    let url = file_url_from_path(&canonical);

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler").arg(&url);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&url);
        command
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

fn file_url_from_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let path_part = if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        format!("/{normalized}")
    } else {
        normalized
    };
    format!("file://{}", percent_encode_file_path(&path_part))
}

fn percent_encode_file_path(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':');
        if keep {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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
fn batch_update_invoices(
    state: State<'_, AppState>,
    request: extractor::BatchUpdateRequest,
) -> Result<Vec<InvoiceSummary>, String> {
    state
        .batch_update_invoices(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn batch_delete_invoices(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    state
        .batch_delete_invoices(ids)
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
) -> Result<ResolveDuplicateResult, String> {
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
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

#[tauri::command]
async fn recognize_raw_file(
    state: State<'_, AppState>,
    request: RecognizeRawFileRequest,
) -> Result<RecognizeRawFileResult, String> {
    let raw_file = state
        .raw_file_for_recognition(request.raw_file_id)
        .inspect_err(|e| error!("Failed to load raw file for recognition: {e}"))
        .map_err(|err| err.to_string())?;
    let recognition_inputs = if raw_file.mime_type == "application/pdf" {
        let pages = state
            .render_pdf_pages_for_recognition(raw_file.id, &raw_file.storage_path)
            .inspect_err(|e| error!("PDF render failed: {e}"))
            .map_err(|err| err.to_string())?;
        pages
            .into_iter()
            .map(|page| {
                let prepared = state
                    .prepare_image_for_recognition(
                        raw_file.id,
                        &page.image_path,
                        Some(page.page_number),
                    )
                    .inspect_err(|e| {
                        error!(
                            "Image preparation failed for page {}: {e}",
                            page.page_number
                        )
                    })
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
            .inspect_err(|e| error!("Image preparation failed: {e}"))
            .map_err(|err| err.to_string())?;
        vec![RecognitionInput {
            source_page_range: None,
            image_path: prepared.image_path,
            thumbnail_path: prepared.thumbnail_path,
            mime_type: prepared.mime_type,
        }]
    };

    info!(
        "Starting recognition for {} pages",
        recognition_inputs.len()
    );
    let _ = state.set_import_job_status_for_raw_file(raw_file.id, "recognizing", None);
    let page_count = recognition_inputs.len();
    let mut invoices = Vec::new();
    let mut total_duration_ms = 0_u128;
    let mut response_previews = Vec::new();
    let mut thumbnail_paths = Vec::new();
    let mut total_prompt_tokens: i64 = 0;
    let mut total_completion_tokens: i64 = 0;
    let mut total_total_tokens: i64 = 0;
    let mut model = request.config.model.clone();
    let audit_config = state.llm_audit_config();

    for input in recognition_inputs {
        thumbnail_paths.push(input.thumbnail_path.to_string_lossy().into_owned());
        let recognition = match recognize_invoice_image(
            request.config.clone(),
            &input.image_path,
            &input.mime_type,
            audit_config.as_ref(),
        )
        .await
        {
            Ok(recognition) => recognition,
            Err(err) => {
                error!("LLM recognition failed: {err}");
                let message = import_failure_message(&err.to_string());
                let _ =
                    state.set_import_job_status_for_raw_file(raw_file.id, "failed", Some(&message));
                return Err(message);
            }
        };

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
        let invoice = match state.save_invoice_extraction(SaveInvoiceExtractionRequest {
            raw_file_id: raw_file.id,
            source_page_range: input.source_page_range,
            provider_name: Some(request.config.base_url.clone()),
            model: Some(recognition.model),
            response_json: recognition.response_json,
        }) {
            Ok(invoice) => invoice,
            Err(err) => {
                error!("Failed to save invoice extraction: {err}");
                let message = import_failure_message(&err.to_string());
                let _ =
                    state.set_import_job_status_for_raw_file(raw_file.id, "failed", Some(&message));
                return Err(message);
            }
        };

        let title = invoice.seller_name.clone().unwrap_or_else(|| "未知".into());
        let _ = state.record_recognition_event(
            invoice.id,
            &title,
            true,
            recognition.duration_ms,
            &rec_model,
            1,
        );
        total_prompt_tokens += recognition.prompt_tokens;
        total_completion_tokens += recognition.completion_tokens;
        total_total_tokens += recognition.total_tokens;
        let _ = state.record_usage_log(
            "llm_recognition",
            &rec_model,
            recognition.prompt_tokens,
            recognition.completion_tokens,
            recognition.total_tokens,
        );
        invoices.push(invoice);
    }

    let count = invoices.len();
    info!("Recognition complete: {count} invoices, model {model}, {total_duration_ms}ms");
    let _ = state.set_import_job_status_for_raw_file(raw_file.id, "imported", None);

    Ok(RecognizeRawFileResult {
        invoices,
        model,
        duration_ms: total_duration_ms,
        response_preview: response_previews.join("\n"),
        page_count,
        thumbnail_paths,
        prompt_tokens: total_prompt_tokens,
        completion_tokens: total_completion_tokens,
        total_tokens: total_total_tokens,
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
fn get_dashboard_stats(
    state: State<'_, AppState>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<DashboardStats, String> {
    state
        .get_dashboard_stats(date_from, date_to)
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn test_llm_connection(
    state: State<'_, AppState>,
    config: LlmProviderConfig,
) -> Result<LlmConnectionTestResult, String> {
    let audit_config = state.llm_audit_config();
    run_llm_connection_test(config, audit_config.as_ref())
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

// --- Email Source Commands ---

#[tauri::command]
fn add_email_source(
    state: State<'_, AppState>,
    request: email_manager::AddEmailSourceRequest,
) -> Result<email_manager::EmailSource, String> {
    state
        .add_email_source(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_email_source(
    state: State<'_, AppState>,
    id: i64,
    request: email_manager::UpdateEmailSourceRequest,
) -> Result<email_manager::EmailSource, String> {
    state
        .update_email_source(id, request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn remove_email_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.remove_email_source(id).map_err(|err| err.to_string())
}

#[tauri::command]
fn list_email_sources(state: State<'_, AppState>) -> Result<Vec<email_manager::EmailSource>, String> {
    state.list_email_sources().map_err(|err| err.to_string())
}

#[tauri::command]
fn toggle_email_source(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<email_manager::EmailSource, String> {
    state
        .toggle_email_source(id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn sync_email_source(
    state: State<'_, AppState>,
    id: i64,
) -> Result<email_manager::EmailSyncResult, String> {
    state
        .sync_email_source(id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn sync_all_email_sources(
    state: State<'_, AppState>,
) -> Result<Vec<email_manager::EmailSyncResult>, String> {
    state
        .sync_all_email_sources()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn test_email_connection(
    state: State<'_, AppState>,
    protocol: String,
    host: String,
    port: i64,
    username: String,
    password: String,
    use_ssl: bool,
    folder: String,
) -> Result<email_manager::EmailTestResult, String> {
    state
        .test_email_connection(&protocol, &host, port, &username, &password, use_ssl, &folder)
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
fn set_badge_config(state: State<'_, AppState>, config: BadgeConfig) -> Result<(), String> {
    state
        .set_badge_config(config)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_badge_config(state: State<'_, AppState>) -> Result<BadgeConfig, String> {
    Ok(state.get_badge_config())
}

#[tauri::command]
fn set_invoice_badge(
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
    attachment_ids: Option<Vec<i64>>,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    state
        .send_agent_message(
            session_id,
            &content,
            attachment_ids.unwrap_or_default(),
            &config,
        )
        .await
        .map_err(|err| err.to_string())
}

fn make_agent_stream_sink(
    app: AppHandle,
    stream_id: String,
    session_id: i64,
) -> agent::AgentStreamSink {
    Arc::new(move |event| {
        let payload = AgentStreamPayload {
            stream_id: stream_id.clone(),
            session_id,
            event,
        };
        let _ = app.emit("agent://stream", payload);
    })
}

#[tauri::command]
async fn send_agent_message_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    stream_id: String,
    session_id: i64,
    content: String,
    attachment_ids: Option<Vec<i64>>,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    let sink = make_agent_stream_sink(app, stream_id, session_id);
    sink(agent::AgentStreamEvent::Started);
    let result = state
        .send_agent_message_stream(
            session_id,
            &content,
            attachment_ids.unwrap_or_default(),
            &config,
            Arc::clone(&sink),
        )
        .await;
    match result {
        Ok(response) => {
            sink(agent::AgentStreamEvent::Finished);
            Ok(response)
        }
        Err(err) => {
            let message = err.to_string();
            sink(agent::AgentStreamEvent::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
}

#[tauri::command]
fn attach_agent_file(
    state: State<'_, AppState>,
    session_id: i64,
    path: String,
) -> Result<AgentAttachment, String> {
    state
        .attach_agent_file(session_id, &path)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_agent_attachments(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentAttachment>, String> {
    state
        .list_agent_attachments(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_agent_tasks(state: State<'_, AppState>, session_id: i64) -> Result<Vec<AgentTask>, String> {
    state
        .list_agent_tasks(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_agent_artifacts(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentArtifact>, String> {
    state
        .list_agent_artifacts(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn open_agent_artifact_file(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .open_agent_artifact_file(session_id, artifact_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn open_agent_artifact_folder(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .open_agent_artifact_folder(session_id, artifact_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_agent_artifact(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .delete_agent_artifact(session_id, artifact_id)
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
async fn confirm_agent_action_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    stream_id: String,
    request: ConfirmRequest,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    let session_id = request.session_id;
    let sink = make_agent_stream_sink(app, stream_id, session_id);
    sink(agent::AgentStreamEvent::Started);
    let result = state
        .confirm_agent_action_stream(request, &config, Arc::clone(&sink))
        .await;
    match result {
        Ok(response) => {
            sink(agent::AgentStreamEvent::Finished);
            Ok(response)
        }
        Err(err) => {
            let message = err.to_string();
            sink(agent::AgentStreamEvent::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
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
fn get_unread_event_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.get_unread_event_count().map_err(|err| err.to_string())
}

#[tauri::command]
fn mark_event_read(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.mark_event_read(id).map_err(|err| err.to_string())
}

#[tauri::command]
fn mark_all_events_read(state: State<'_, AppState>) -> Result<(), String> {
    state
        .mark_all_events_read()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_llm_config(state: State<'_, AppState>, config: LlmProviderConfig) -> Result<(), String> {
    state.set_llm_config(config).map_err(|err| err.to_string())
}

#[tauri::command]
fn get_llm_config(state: State<'_, AppState>) -> Result<Option<LlmProviderConfig>, String> {
    Ok(state.get_llm_config())
}

#[tauri::command]
fn set_agent_llm_config(state: State<'_, AppState>, config: LlmProviderConfig) -> Result<(), String> {
    state.set_agent_llm_config(config);
    Ok(())
}

#[tauri::command]
fn get_agent_llm_config(state: State<'_, AppState>) -> Result<Option<LlmProviderConfig>, String> {
    Ok(state.get_agent_llm_config())
}

#[tauri::command]
fn set_llm_audit_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.set_llm_audit_enabled(enabled);
    Ok(())
}

#[tauri::command]
fn get_llm_audit_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.get_llm_audit_enabled())
}

#[tauri::command]
fn get_recognition_queue_status(
    state: State<'_, AppState>,
) -> Result<RecognitionQueueStatus, String> {
    Ok(state.get_recognition_queue_status())
}

#[tauri::command]
fn set_recognition_concurrency(
    state: State<'_, AppState>,
    max_concurrent: usize,
) -> Result<(), String> {
    state
        .set_recognition_concurrency(max_concurrent)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn raw_file_has_invoices(state: State<'_, AppState>, raw_file_id: i64) -> Result<bool, String> {
    state
        .raw_file_has_invoices(raw_file_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_invoice_id_by_raw_file(
    state: State<'_, AppState>,
    raw_file_id: i64,
) -> Result<Option<i64>, String> {
    state
        .invoice_id_for_raw_file(raw_file_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_logs(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<ExportLogsResult, String> {
    state.export_logs(&output_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_backup(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<ExportLogsResult, String> {
    state.export_backup(&output_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn cleanup_storage(state: State<'_, AppState>) -> Result<CleanupStorageResult, String> {
    state.cleanup_storage().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_all_events(state: State<'_, AppState>) -> Result<usize, String> {
    state.delete_all_events().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_import_job(state: State<'_, AppState>, job_id: i64) -> Result<(), String> {
    state.delete_import_job(job_id).map_err(|e| e.to_string())
}


#[tauri::command]
fn get_llm_usage(
    state: State<'_, AppState>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<extractor::LlmUsageStats, String> {
    state
        .get_llm_usage(date_from, date_to)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_price_config(state: State<'_, AppState>) -> Result<app_core::PriceConfig, String> {
    Ok(state.get_price_config())
}

#[tauri::command]
fn set_price_config(
    state: State<'_, AppState>,
    config: app_core::PriceConfig,
) -> Result<(), String> {
    state
        .set_price_config(config)
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Set up logging directory and file-based tracing subscriber
            let app_data_dir = app.path().app_data_dir().expect("app data dir");
            let log_dir = app_data_dir.join("logs");
            std::fs::create_dir_all(&log_dir).expect("create log dir");

            let file_appender = tracing_appender::rolling::daily(&log_dir, "invoicevault");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "invoicevault=info".into());

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(non_blocking)
                .with_ansi(false)
                .init();

            // Keep the guard alive — when dropped it flushes remaining logs
            // Leak it so it lives for the lifetime of the app
            std::mem::forget(_guard);

            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            setup_tray(app.handle())?;

            // Restore and persist window size
            let window = app
                .get_webview_window(MAIN_WINDOW_LABEL)
                .expect("main window");
            apply_window_material(&window);
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
            let window_app = window.app_handle().clone();
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if let Some(window) = window_app.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = window.hide();
                    }
                }
                WindowEvent::Resized(size) => {
                    if let Ok(json) = serde_json::to_string(&WindowSizeState {
                        width: size.width as f64,
                        height: size.height as f64,
                    }) {
                        let _ = std::fs::write(&save_path, json);
                    }
                }
                _ => {}
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
            mark_invoice_viewed,
            count_unviewed_invoices,
            open_invoice_raw_file_in_browser,
            update_invoice,
            update_invoice_items,
            batch_update_invoices,
            batch_delete_invoices,
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
            add_email_source,
            update_email_source,
            remove_email_source,
            list_email_sources,
            toggle_email_source,
            sync_email_source,
            sync_all_email_sources,
            test_email_connection,
            set_chroma_config,
            get_chroma_config,
            set_embedding_config,
            get_embedding_config,
            set_badge_config,
            get_badge_config,
            set_invoice_badge,
            test_chroma_connection,
            test_embedding_connection,
            search_invoices_semantic,
            create_agent_session,
            list_agent_sessions,
            get_agent_session,
            delete_agent_session,
            send_agent_message,
            send_agent_message_stream,
            attach_agent_file,
            list_agent_attachments,
            list_agent_tasks,
            list_agent_artifacts,
            open_agent_artifact_file,
            open_agent_artifact_folder,
            delete_agent_artifact,
            confirm_agent_action,
            confirm_agent_action_stream,
            list_events,
            get_unread_event_count,
            mark_event_read,
            mark_all_events_read,
            set_llm_config,
            get_llm_config,
            set_agent_llm_config,
            get_agent_llm_config,
            set_llm_audit_enabled,
            get_llm_audit_enabled,
            get_recognition_queue_status,
            set_recognition_concurrency,
            raw_file_has_invoices,
            get_invoice_id_by_raw_file,
            delete_all_events,
            delete_import_job,
            export_logs,
            export_backup,
            cleanup_storage,
            get_llm_usage,
            get_price_config,
            set_price_config,
            check_external_dependencies
        ])
        .run(tauri::generate_context!())
        .expect("failed to run InvoiceVault");
}

fn apply_window_material(_window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use tauri::window::{Effect, EffectState, EffectsBuilder};

        if let Err(err) = _window.set_effects(
            EffectsBuilder::new()
                .effects([Effect::Sidebar, Effect::WindowBackground])
                .state(EffectState::FollowsWindowActiveState)
                .radius(10.0)
                .build(),
        ) {
            tracing::warn!("failed to apply macOS window vibrancy: {err}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};

        if let Err(err) = _window.set_effects(
            EffectsBuilder::new()
                .effect(Effect::Mica)
                .color(Color(16, 20, 26, 170))
                .build(),
        ) {
            tracing::warn!("failed to apply Windows Mica effect: {err}");
        }
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let workbench = MenuItem::with_id(app, TRAY_WORKBENCH_ID, "工作台", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let version = MenuItem::with_id(
        app,
        TRAY_VERSION_ID,
        format!("版本 {}", app.package_info().version),
        false,
        None::<&str>,
    )?;
    let quit_separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&workbench, &separator, &version, &quit_separator, &quit],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("../icons/tray.png"))
        .tooltip(format!(
            "{} {}",
            app.package_info().name,
            app.package_info().version
        ))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == TRAY_WORKBENCH_ID {
                restore_main_window(app);
            } else if event.id() == TRAY_QUIT_ID {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                restore_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn restore_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
