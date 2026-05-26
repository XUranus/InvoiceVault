use tauri::{AppHandle, Manager};

use crate::app_core::AppState;
use crate::event::EventListResult;
use tauri::State;

#[tauri::command]
pub async fn list_events(
    app: AppHandle,
    page: Option<i64>,
    page_size: Option<i64>,
    event_type: Option<String>,
) -> Result<EventListResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .list_events(
                page.unwrap_or(1),
                page_size.unwrap_or(20),
                event_type.as_deref(),
            )
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn get_unread_event_count(app: AppHandle) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .get_unread_event_count()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn get_unread_failed_import_event_count(state: State<'_, AppState>) -> Result<i64, String> {
    state
        .get_unread_failed_import_event_count()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn mark_event_read(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.mark_event_read(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn mark_all_events_read(state: State<'_, AppState>) -> Result<(), String> {
    state.mark_all_events_read().map_err(|err| err.to_string())
}
