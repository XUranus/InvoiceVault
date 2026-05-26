use tauri::State;

use crate::app_core::AppState;
use crate::watcher::{AddWatchDirRequest, UpdateWatchDirRequest, WatchDirStatus};

#[tauri::command]
pub fn add_watch_dir(
    state: State<'_, AppState>,
    request: AddWatchDirRequest,
) -> Result<WatchDirStatus, String> {
    state.add_watch_dir(request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_watch_dir(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.remove_watch_dir(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_watch_dirs(state: State<'_, AppState>) -> Result<Vec<WatchDirStatus>, String> {
    state.list_watch_dirs().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_watch_dir(
    state: State<'_, AppState>,
    id: i64,
    request: UpdateWatchDirRequest,
) -> Result<WatchDirStatus, String> {
    state
        .update_watch_dir(id, request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn toggle_watch_dir(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<WatchDirStatus, String> {
    state
        .toggle_watch_dir(id, enabled)
        .map_err(|err| err.to_string())
}
