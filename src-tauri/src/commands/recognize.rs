use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, warn};

use crate::app_core::{import_failure_message, AppState};
use crate::extractor::{InvoiceSummary, SaveInvoiceExtractionRequest};
use crate::llm::{recognize_invoice_with_retries, LlmProviderConfig};

#[derive(Debug, Deserialize)]
pub struct RecognizeRawFileRequest {
    raw_file_id: i64,
    config: LlmProviderConfig,
}

#[derive(Debug, Serialize)]
pub struct RecognizeRawFileResult {
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

#[derive(Debug)]
struct RecognitionInput {
    source_page_range: Option<String>,
    image_path: std::path::PathBuf,
    thumbnail_path: std::path::PathBuf,
    mime_type: String,
}

#[tauri::command]
pub async fn recognize_raw_file(
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
    if let Err(e) = state.set_import_job_status_for_raw_file(raw_file.id, "recognizing", None) {
        warn!("Failed to set import job status to recognizing: {e}");
    }
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
        let recognition = match recognize_invoice_with_retries(
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
                if let Err(e) =
                    state.set_import_job_status_for_raw_file(raw_file.id, "failed", Some(&message))
                {
                    error!("Failed to mark import job as failed: {e}");
                }
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
                if let Err(e) =
                    state.set_import_job_status_for_raw_file(raw_file.id, "failed", Some(&message))
                {
                    error!("Failed to mark import job as failed: {e}");
                }
                return Err(message);
            }
        };

        let title = invoice.seller_name.clone().unwrap_or_else(|| "未知".into());
        if let Err(e) = state.record_recognition_event(
            invoice.id,
            &title,
            true,
            recognition.duration_ms,
            &rec_model,
            1,
        ) {
            warn!(
                "Failed to record recognition event for invoice {}: {e}",
                invoice.id
            );
        }
        total_prompt_tokens += recognition.prompt_tokens;
        total_completion_tokens += recognition.completion_tokens;
        total_total_tokens += recognition.total_tokens;
        if let Err(e) = state.record_usage_log(
            "llm_recognition",
            &rec_model,
            recognition.prompt_tokens,
            recognition.completion_tokens,
            recognition.total_tokens,
        ) {
            warn!("Failed to record LLM usage log: {e}");
        }
        invoices.push(invoice);
    }

    let count = invoices.len();
    info!("Recognition complete: {count} invoices, model {model}, {total_duration_ms}ms");
    if let Err(e) = state.set_import_job_status_for_raw_file(raw_file.id, "imported", None) {
        warn!("Failed to set import job status to imported: {e}");
    }

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
