use tauri::{AppHandle, Manager, State};
use tracing::debug;

use crate::app_core::AppState;
use crate::email_manager;

#[tauri::command]
pub fn add_email_source(
    state: State<'_, AppState>,
    request: email_manager::AddEmailSourceRequest,
) -> Result<email_manager::EmailSource, String> {
    state
        .add_email_source(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_email_source(
    state: State<'_, AppState>,
    id: i64,
    request: email_manager::UpdateEmailSourceRequest,
) -> Result<email_manager::EmailSource, String> {
    state
        .update_email_source(id, request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_email_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.remove_email_source(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_email_sources(app: AppHandle) -> Result<Vec<email_manager::EmailSource>, String> {
    debug!("[poll] list_email_sources: start");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.list_email_sources().map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?;
    debug!("[poll] list_email_sources: done");
    result
}

#[tauri::command]
pub fn toggle_email_source(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<email_manager::EmailSource, String> {
    state
        .toggle_email_source(id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn sync_email_source(
    state: State<'_, AppState>,
    id: i64,
) -> Result<email_manager::EmailSyncResult, String> {
    state.sync_email_source(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn sync_all_email_sources(
    app: AppHandle,
) -> Result<Vec<email_manager::EmailSyncResult>, String> {
    debug!("[poll] sync_all_email_sources: start");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .sync_all_email_sources()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?;
    debug!("[poll] sync_all_email_sources: done");
    result
}

#[tauri::command]
pub async fn test_email_connection(
    state: State<'_, AppState>,
    protocol: String,
    host: String,
    port: i64,
    username: String,
    password: String,
    auth_method: String,
    use_ssl: bool,
    folder: String,
) -> Result<email_manager::EmailTestResult, String> {
    state
        .test_email_connection(
            &protocol,
            &host,
            port,
            &username,
            &password,
            &auth_method,
            use_ssl,
            &folder,
        )
        .map_err(|err| err.to_string())
}
