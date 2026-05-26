use tauri::{AppHandle, Manager};

use crate::app_core::AppState;
use crate::exporter::{ExportInvoicesRequest, ExportResult, PdfReportRequest, PdfReportResult};

#[tauri::command]
pub async fn export_invoices(
    app: AppHandle,
    request: ExportInvoicesRequest,
) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .export_invoices(request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn export_pdf_report(
    app: AppHandle,
    request: PdfReportRequest,
) -> Result<PdfReportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .export_pdf_report(request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn move_export_file(source_path: String, dest_path: String) -> Result<(), String> {
    std::fs::rename(&source_path, &dest_path).map_err(|e| {
        // Fallback: copy + delete (works across filesystems)
        std::fs::copy(&source_path, &dest_path)
            .and_then(|_| std::fs::remove_file(&source_path))
            .map_err(|e2| format!("移动文件失败: {e2}"))
            .err()
            .unwrap_or_else(|| e.to_string())
    })
}
