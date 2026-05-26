use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracing::{debug, info};

use crate::app_core::{AppHealth, AppState};
use crate::process_utils::command_no_window;

#[derive(Debug, Serialize)]
pub struct ExternalDependencyStatus {
    name: String,
    command: String,
    available: bool,
    version: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub async fn check_external_dependencies() -> Vec<ExternalDependencyStatus> {
    info!("[dep] check_external_dependencies: start");
    let result = tauri::async_runtime::spawn_blocking(|| {
        vec![check_external_dependency(
            "Poppler PDF renderer",
            "pdftoppm",
            &["-h"],
        )]
    })
    .await
    .unwrap_or_default();
    info!("[dep] check_external_dependencies: done");
    result
}

fn check_external_dependency(name: &str, command: &str, args: &[&str]) -> ExternalDependencyStatus {
    let mut child = match command_no_window(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return ExternalDependencyStatus {
                name: name.to_owned(),
                command: command.to_owned(),
                available: false,
                version: None,
                error: Some(err.to_string()),
            };
        }
    };

    let timeout = Duration::from_secs(10);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ExternalDependencyStatus {
                        name: name.to_owned(),
                        command: command.to_owned(),
                        available: false,
                        version: None,
                        error: Some("检测超时（10s）".to_owned()),
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => {
                return ExternalDependencyStatus {
                    name: name.to_owned(),
                    command: command.to_owned(),
                    available: false,
                    version: None,
                    error: Some(err.to_string()),
                };
            }
        }
    }

    match child.wait_with_output() {
        Ok(output) => {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let version = combined
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(160).collect::<String>());
            ExternalDependencyStatus {
                name: name.to_owned(),
                command: command.to_owned(),
                available: output.status.success() || version.is_some(),
                version,
                error: (!output.status.success()).then(|| output.status.to_string()),
            }
        }
        Err(err) => ExternalDependencyStatus {
            name: name.to_owned(),
            command: command.to_owned(),
            available: false,
            version: None,
            error: Some(err.to_string()),
        },
    }
}

#[tauri::command]
pub fn frontend_heartbeat(seq: u64) {
    debug!("[hb] frontend heartbeat #{}", seq);
}

#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("GIT_VERSION")
}

#[tauri::command]
pub async fn app_health(app: AppHandle) -> Result<AppHealth, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.health().map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}
