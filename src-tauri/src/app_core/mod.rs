use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    importer::{import_files, list_import_jobs, ImportError, ImportJobSummary},
    storage::{run_migrations, StorageError},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("failed to resolve application data directory")]
    MissingAppDataDir,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("import error: {0}")]
    Import(#[from] ImportError),
}

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub raw_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppHealth {
    pub app_data_dir: String,
    pub database_path: String,
    pub migration_version: i64,
}

pub struct AppState {
    paths: AppPaths,
    db: Mutex<Connection>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, AppError> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::MissingAppDataDir)?;
        let paths = create_app_paths(&app_data_dir)?;
        let mut db = Connection::open(&paths.database_path)?;
        run_migrations(&mut db)?;

        Ok(Self {
            paths,
            db: Mutex::new(db),
        })
    }

    pub fn health(&self) -> Result<AppHealth, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        let migration_version = db.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?;

        Ok(AppHealth {
            app_data_dir: display_path(&self.paths.app_data_dir),
            database_path: display_path(&self.paths.database_path),
            migration_version,
        })
    }

    pub fn import_files(&self, paths: Vec<String>) -> Result<Vec<ImportJobSummary>, AppError> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        Ok(import_files(&mut db, &self.paths.raw_dir, paths)?)
    }

    pub fn list_import_jobs(&self) -> Result<Vec<ImportJobSummary>, AppError> {
        let db = self.db.lock().expect("database mutex poisoned");
        Ok(list_import_jobs(&db)?)
    }
}

fn create_app_paths(app_data_dir: &Path) -> Result<AppPaths, AppError> {
    let raw_dir = app_data_dir.join("raw");
    let thumbnails_dir = app_data_dir.join("thumbnails");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&thumbnails_dir)?;

    Ok(AppPaths {
        app_data_dir: app_data_dir.to_path_buf(),
        database_path: app_data_dir.join("receiptier.sqlite3"),
        raw_dir,
        thumbnails_dir,
    })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Storage(StorageError::Database(value))
    }
}
