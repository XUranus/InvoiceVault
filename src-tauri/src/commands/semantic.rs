use tauri::State;

use crate::app_core::AppState;
use crate::chroma::SimilarResult;

#[tauri::command]
pub fn search_invoices_semantic(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<SimilarResult>, String> {
    state
        .search_invoices_semantic(query, limit)
        .map_err(|err| err.to_string())
}
