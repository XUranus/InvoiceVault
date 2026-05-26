use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::app_core::AppState;
use crate::importer::{ImportJobListResult, ImportJobSummary, ImportRequest};

#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    request: ImportRequest,
) -> Result<Vec<ImportJobSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .import_files(request.paths, &app)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn poll_dropped_files(app: AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    state.take_dropped_files()
}

#[tauri::command]
pub async fn import_dropped_file(
    app: AppHandle,
    name: String,
    data: Vec<u8>,
) -> Result<Vec<ImportJobSummary>, String> {
    let tmp = std::env::temp_dir().join(format!("iv_drop_{}", name));
    std::fs::write(&tmp, &data).map_err(|e| format!("write temp: {e}"))?;
    let path = tmp.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .import_files(vec![path.clone()], &app)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn pick_invoice_files(app: AppHandle) -> Result<Vec<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("发票文件", &["pdf", "png", "jpg", "jpeg"])
        .pick_files(move |paths| {
            let selected = paths
                .unwrap_or_default()
                .into_iter()
                .map(|path| path.to_string())
                .collect::<Vec<_>>();
            let _ = tx.send(selected);
        });
    rx.await.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn pick_any_files(app: AppHandle) -> Result<Vec<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_files(move |paths| {
        let selected = paths
            .unwrap_or_default()
            .into_iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>();
        let _ = tx.send(selected);
    });
    rx.await.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn pick_save_file(
    app: AppHandle,
    default_path: String,
    filters: Vec<(String, Vec<String>)>,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file();
    dialog = dialog.set_file_name(&default_path);
    for (name, exts) in &filters {
        let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(name, &ext_refs);
    }
    dialog.save_file(move |path| {
        let result = path.map(|p| p.to_string());
        let _ = tx.send(result);
    });
    rx.await.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_import_jobs(
    app: AppHandle,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<ImportJobListResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .list_import_jobs(page.unwrap_or(1), page_size.unwrap_or(50))
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}
