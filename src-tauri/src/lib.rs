mod app_core;
mod importer;
mod llm;
mod raw_store;
mod storage;

use app_core::{AppHealth, AppState};
use importer::{ImportJobSummary, ImportRequest};
use llm::{
    test_llm_connection as run_llm_connection_test, LlmConnectionTestResult, LlmProviderConfig,
};
use tauri::{Manager, State};

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
fn list_import_jobs(state: State<'_, AppState>) -> Result<Vec<ImportJobSummary>, String> {
    state.list_import_jobs().map_err(|err| err.to_string())
}

#[tauri::command]
async fn test_llm_connection(config: LlmProviderConfig) -> Result<LlmConnectionTestResult, String> {
    run_llm_connection_test(config)
        .await
        .map_err(|err| err.to_string())
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
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            import_files,
            list_import_jobs,
            test_llm_connection
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Receiptier");
}
