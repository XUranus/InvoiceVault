use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use super::AppError;
use crate::process_utils::command_no_window;

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub raw_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub llm_audit_dir: PathBuf,
    pub agent_uploads_dir: PathBuf,
    pub sessions_dir: PathBuf,
}

pub fn create_app_paths(app_data_dir: &Path) -> Result<AppPaths, AppError> {
    let raw_dir = app_data_dir.join("raw");
    let thumbnails_dir = app_data_dir.join("thumbnails");
    let logs_dir = app_data_dir.join("logs");
    let llm_audit_dir = app_data_dir.join("llm_audit");
    let agent_uploads_dir = app_data_dir.join("agent_uploads");
    let sessions_dir = app_data_dir.join("sessions");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&thumbnails_dir)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&llm_audit_dir)?;
    fs::create_dir_all(&agent_uploads_dir)?;
    fs::create_dir_all(&sessions_dir)?;

    Ok(AppPaths {
        app_data_dir: app_data_dir.to_path_buf(),
        database_path: app_data_dir.join("invoicevault.sqlite3"),
        raw_dir,
        thumbnails_dir,
        logs_dir,
        llm_audit_dir,
        agent_uploads_dir,
        sessions_dir,
    })
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Get the temp directory for a session, creating it if needed.
pub fn session_temp_dir(sessions_dir: &Path, session_uuid: &str) -> Result<PathBuf, AppError> {
    let temp_dir = sessions_dir.join(session_uuid).join("temp");
    fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

pub fn open_path_with_system(path: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = command_no_window("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = command_no_window("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = command_no_window("xdg-open");
        command.arg(path);
        command
    };

    command.spawn()?;
    Ok(())
}
