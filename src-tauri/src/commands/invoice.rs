use std::path::Path;

use tauri::{AppHandle, Manager, State};

use crate::app_core::AppState;
use crate::dedupe::{DedupeCheckResult, ResolveDuplicateRequest, ResolveDuplicateResult};
use crate::extractor::{
    BatchUpdateRequest, InvoiceDetail, InvoiceItemRow, InvoiceSearchParams,
    InvoiceSearchResult, InvoiceSummary, MergeInvoicesResult, SaveInvoiceExtractionRequest,
    TagOption, UpdateInvoiceItemsRequest, UpdateInvoiceRequest, UpdateInvoiceResult,
};
use crate::process_utils::command_no_window;

#[tauri::command]
pub fn save_invoice_extraction(
    state: State<'_, AppState>,
    request: SaveInvoiceExtractionRequest,
) -> Result<InvoiceSummary, String> {
    state
        .save_invoice_extraction(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_invoices(app: AppHandle) -> Result<Vec<InvoiceSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.list_invoices().map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn search_invoices(
    app: AppHandle,
    params: InvoiceSearchParams,
) -> Result<InvoiceSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.search_invoices(params).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn get_tag_options(app: AppHandle) -> Result<Vec<TagOption>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.get_tag_options().map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn get_invoice_detail(app: AppHandle, invoice_id: i64) -> Result<InvoiceDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .get_invoice_detail(invoice_id)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn mark_invoice_viewed(state: State<'_, AppState>, invoice_id: i64) -> Result<bool, String> {
    state
        .mark_invoice_viewed(invoice_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn count_unviewed_invoices(app: AppHandle) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .count_unviewed_invoices()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn open_invoice_raw_file_in_browser(
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

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = command_no_window("explorer");
        command.arg(&canonical);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = command_no_window("open");
        command.arg(&canonical);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = command_no_window("xdg-open");
        command.arg(&canonical);
        command
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn update_invoice(
    app: AppHandle,
    request: UpdateInvoiceRequest,
) -> Result<UpdateInvoiceResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.update_invoice(request).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn update_invoice_items(
    app: AppHandle,
    request: UpdateInvoiceItemsRequest,
) -> Result<Vec<InvoiceItemRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .update_invoice_items(request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn batch_update_invoices(
    app: AppHandle,
    request: BatchUpdateRequest,
) -> Result<Vec<InvoiceSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .batch_update_invoices(request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn batch_delete_invoices(app: AppHandle, ids: Vec<i64>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .batch_delete_invoices(ids)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn check_invoice_duplicates(
    state: State<'_, AppState>,
    invoice_id: i64,
) -> Result<DedupeCheckResult, String> {
    state
        .check_invoice_duplicates(invoice_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn resolve_duplicate(
    state: State<'_, AppState>,
    request: ResolveDuplicateRequest,
) -> Result<ResolveDuplicateResult, String> {
    state
        .resolve_duplicate(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn regenerate_all_duplicates(app: AppHandle) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .regenerate_all_duplicates()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn merge_invoices(
    app: AppHandle,
    target_invoice_id: i64,
    source_invoice_ids: Vec<i64>,
) -> Result<MergeInvoicesResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .merge_invoices(target_invoice_id, source_invoice_ids)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}
