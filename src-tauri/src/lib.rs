mod app_core;
mod storage;

use app_core::{AppHealth, AppState};
use tauri::{Manager, State};

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> Result<AppHealth, String> {
    state.health().map_err(|err| err.to_string())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "receiptier=info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_health])
        .run(tauri::generate_context!())
        .expect("failed to run Receiptier");
}
