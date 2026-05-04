use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tracing::{error, info};

use notify::{Event, EventKind, RecursiveMode, Watcher};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{
    document::render_pdf_pages,
    event,
    extractor::{save_invoice_extraction, SaveInvoiceExtractionRequest},
    importer::{import_files, ImportJobSummary},
    llm::{recognize_invoice_image, LlmAuditConfig, LlmProviderConfig},
    storage::StorageError,
};

#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("watch path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("filesystem watcher error: {0}")]
    Notify(#[from] notify::Error),
    #[error("watch dir not found: {0}")]
    NotFound(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchDir {
    pub id: i64,
    pub path: String,
    pub extensions: String,
    pub recursive: bool,
    pub enabled: bool,
    pub stable_wait_ms: i64,
    pub archive_after_import: bool,
    pub archive_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatchDirStatus {
    #[serde(flatten)]
    pub config: WatchDir,
    pub running: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddWatchDirRequest {
    pub path: String,
    pub extensions: Option<String>,
    pub recursive: Option<bool>,
    pub stable_wait_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWatchDirRequest {
    pub path: Option<String>,
    pub extensions: Option<String>,
    pub recursive: Option<bool>,
    pub stable_wait_ms: Option<i64>,
    pub archive_after_import: Option<bool>,
    pub archive_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WatcherImportEvent {
    watch_dir_id: i64,
    watch_dir_path: String,
    imported_count: usize,
    jobs: Vec<ImportJobSummary>,
}

struct WatchHandle {
    stop_flag: Arc<AtomicBool>,
}

pub struct WatcherManager {
    db: Arc<Mutex<Connection>>,
    raw_dir: PathBuf,
    thumbnails_dir: PathBuf,
    llm_audit_dir: PathBuf,
    llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
    handles: Mutex<HashMap<i64, WatchHandle>>,
    app_handle: AppHandle,
}

impl WatcherManager {
    pub fn new(
        db: Arc<Mutex<Connection>>,
        raw_dir: PathBuf,
        thumbnails_dir: PathBuf,
        llm_audit_dir: PathBuf,
        llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
        app_handle: AppHandle,
    ) -> Result<Self, WatcherError> {
        let manager = Self {
            db,
            raw_dir,
            thumbnails_dir,
            llm_audit_dir,
            llm_config,
            handles: Mutex::new(HashMap::new()),
            app_handle,
        };
        manager.resume_enabled()?;
        Ok(manager)
    }

    fn resume_enabled(&self) -> Result<(), WatcherError> {
        let db = self.db.lock().expect("db lock");
        let mut stmt = db.prepare(
            "SELECT id, path, extensions, recursive, stable_wait_ms FROM watch_dirs WHERE enabled = 1",
        )?;
        let dirs: Vec<(i64, String, String, bool, i64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(db);

        for (id, path, extensions, recursive, stable_wait_ms) in dirs {
            let _ = self.start_watching(
                id,
                PathBuf::from(&path),
                recursive,
                extensions,
                stable_wait_ms as u64,
            );
        }

        Ok(())
    }

    pub fn add_watch_dir(
        &self,
        request: AddWatchDirRequest,
    ) -> Result<WatchDirStatus, WatcherError> {
        let path = PathBuf::from(&request.path);
        if !path.exists() {
            return Err(WatcherError::PathNotFound(path));
        }

        let extensions = request.extensions.unwrap_or_default();
        let recursive = request.recursive.unwrap_or(true);
        let stable_wait_ms = request.stable_wait_ms.unwrap_or(2000);

        let db = self.db.lock().expect("db lock");
        db.execute(
            "INSERT INTO watch_dirs (path, extensions, recursive, stable_wait_ms) VALUES (?1, ?2, ?3, ?4)",
            params![request.path, extensions, recursive as i32, stable_wait_ms],
        )?;
        let id = db.last_insert_rowid();
        drop(db);

        let _ = self.start_watching(id, path, recursive, extensions, stable_wait_ms as u64);

        self.get_status(id)
    }

    pub fn remove_watch_dir(&self, id: i64) -> Result<(), WatcherError> {
        self.stop_watching(id);

        let db = self.db.lock().expect("db lock");
        let affected = db.execute("DELETE FROM watch_dirs WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(WatcherError::NotFound(id));
        }
        Ok(())
    }

    pub fn list_watch_dirs(&self) -> Result<Vec<WatchDirStatus>, WatcherError> {
        let db = self.db.lock().expect("db lock");
        let mut stmt = db.prepare(
            "SELECT id, path, extensions, recursive, enabled, stable_wait_ms, archive_after_import, archive_path, created_at, updated_at
            FROM watch_dirs ORDER BY id",
        )?;
        let dirs: Vec<WatchDir> = stmt
            .query_map([], |row| {
                Ok(WatchDir {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    extensions: row.get(2)?,
                    recursive: row.get::<_, i32>(3)? != 0,
                    enabled: row.get::<_, i32>(4)? != 0,
                    stable_wait_ms: row.get(5)?,
                    archive_after_import: row.get::<_, i32>(6)? != 0,
                    archive_path: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(db);

        let handles = self.handles.lock().expect("handles lock");
        let result: Vec<WatchDirStatus> = dirs
            .into_iter()
            .map(|d| {
                let running = handles.contains_key(&d.id) && d.enabled;
                WatchDirStatus {
                    config: d,
                    running,
                    error: None,
                }
            })
            .collect();

        Ok(result)
    }

    pub fn update_watch_dir(
        &self,
        id: i64,
        request: UpdateWatchDirRequest,
    ) -> Result<WatchDirStatus, WatcherError> {
        // Stop existing watcher first
        self.stop_watching(id);

        let db = self.db.lock().expect("db lock");
        let mut sets = Vec::new();
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref path) = request.path {
            vals.push(Box::new(path.clone()));
            sets.push(format!("path = ?{}", vals.len()));
        }
        if let Some(ref extensions) = request.extensions {
            vals.push(Box::new(extensions.clone()));
            sets.push(format!("extensions = ?{}", vals.len()));
        }
        if let Some(recursive) = request.recursive {
            vals.push(Box::new(recursive as i32));
            sets.push(format!("recursive = ?{}", vals.len()));
        }
        if let Some(stable_wait_ms) = request.stable_wait_ms {
            vals.push(Box::new(stable_wait_ms));
            sets.push(format!("stable_wait_ms = ?{}", vals.len()));
        }
        if let Some(archive_after_import) = request.archive_after_import {
            vals.push(Box::new(archive_after_import as i32));
            sets.push(format!("archive_after_import = ?{}", vals.len()));
        }
        if let Some(ref archive_path) = request.archive_path {
            vals.push(Box::new(archive_path.clone()));
            sets.push(format!("archive_path = ?{}", vals.len()));
        }

        if sets.is_empty() {
            return self.get_status(id);
        }

        let sql = format!(
            "UPDATE watch_dirs SET {}, updated_at = CURRENT_TIMESTAMP WHERE id = ?{}",
            sets.join(", "),
            vals.len() + 1,
        );
        vals.push(Box::new(id));
        let refs: Vec<&dyn rusqlite::types::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
        db.execute(&sql, refs.as_slice())?;
        drop(db);

        // Re-read config and restart if enabled
        let config = self.get_watch_dir(id)?;
        if config.enabled && PathBuf::from(&config.path).exists() {
            let _ = self.start_watching(
                id,
                PathBuf::from(&config.path),
                config.recursive,
                config.extensions,
                config.stable_wait_ms as u64,
            );
        }

        self.get_status(id)
    }

    pub fn toggle_watch_dir(&self, id: i64, enabled: bool) -> Result<WatchDirStatus, WatcherError> {
        let db = self.db.lock().expect("db lock");
        db.execute(
            "UPDATE watch_dirs SET enabled = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, enabled as i32],
        )?;
        drop(db);

        if enabled {
            let config = self.get_watch_dir(id)?;
            let path = PathBuf::from(&config.path);
            if !path.exists() {
                return Ok(WatchDirStatus {
                    running: false,
                    error: Some("path does not exist".into()),
                    config,
                });
            }
            let _ = self.start_watching(
                id,
                path,
                config.recursive,
                config.extensions,
                config.stable_wait_ms as u64,
            );
        } else {
            self.stop_watching(id);
        }

        self.get_status(id)
    }

    fn get_watch_dir(&self, id: i64) -> Result<WatchDir, WatcherError> {
        let db = self.db.lock().expect("db lock");
        let result = db.query_row(
            "SELECT id, path, extensions, recursive, enabled, stable_wait_ms, archive_after_import, archive_path, created_at, updated_at
            FROM watch_dirs WHERE id = ?1",
            [id],
            |row| {
                Ok(WatchDir {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    extensions: row.get(2)?,
                    recursive: row.get::<_, i32>(3)? != 0,
                    enabled: row.get::<_, i32>(4)? != 0,
                    stable_wait_ms: row.get(5)?,
                    archive_after_import: row.get::<_, i32>(6)? != 0,
                    archive_path: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?;

        Ok(result)
    }

    fn get_status(&self, id: i64) -> Result<WatchDirStatus, WatcherError> {
        let config = self.get_watch_dir(id)?;
        let handles = self.handles.lock().expect("handles lock");
        let running = handles.contains_key(&id) && config.enabled;
        Ok(WatchDirStatus {
            config,
            running,
            error: None,
        })
    }

    fn start_watching(
        &self,
        id: i64,
        path: PathBuf,
        recursive: bool,
        extensions: String,
        stable_wait_ms: u64,
    ) -> Result<(), WatcherError> {
        // Stop existing watcher for this ID first
        self.stop_watching(id);

        let stop_flag = Arc::new(AtomicBool::new(false));
        let handle = WatchHandle {
            stop_flag: stop_flag.clone(),
        };
        self.handles
            .lock()
            .expect("handles lock")
            .insert(id, handle);

        let db = Arc::clone(&self.db);
        let raw_dir = self.raw_dir.clone();
        let thumbnails_dir = self.thumbnails_dir.clone();
        let llm_audit_dir = self.llm_audit_dir.clone();
        let llm_config = Arc::clone(&self.llm_config);
        let app_handle = self.app_handle.clone();

        std::thread::Builder::new()
            .name(format!("watch-dir-{id}"))
            .spawn(move || {
                watch_loop(
                    db,
                    raw_dir,
                    thumbnails_dir,
                    llm_audit_dir,
                    llm_config,
                    app_handle,
                    id,
                    path,
                    recursive,
                    extensions,
                    stable_wait_ms,
                    stop_flag,
                );
            })
            .expect("spawn watch thread");

        Ok(())
    }

    fn stop_watching(&self, id: i64) {
        if let Some(handle) = self.handles.lock().expect("handles lock").remove(&id) {
            handle.stop_flag.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for WatcherManager {
    fn drop(&mut self) {
        let ids: Vec<i64> = self
            .handles
            .lock()
            .expect("handles lock")
            .keys()
            .copied()
            .collect();
        for id in ids {
            self.stop_watching(id);
        }
    }
}

fn watch_loop(
    db: Arc<Mutex<Connection>>,
    raw_dir: PathBuf,
    thumbnails_dir: PathBuf,
    llm_audit_dir: PathBuf,
    llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
    app_handle: AppHandle,
    watch_id: i64,
    path: PathBuf,
    recursive: bool,
    extensions: String,
    stable_wait_ms: u64,
    stop_flag: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel();
    let path_for_watcher = path.clone();

    let mut watcher = match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    }) {
        Ok(w) => w,
        Err(_e) => return,
    };

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    if watcher.watch(&path_for_watcher, mode).is_err() {
        return;
    }

    let ext_filter: Vec<String> = extensions
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let check_interval = Duration::from_millis(100);
    let stable_duration = Duration::from_millis(stable_wait_ms);
    let mut pending_paths: HashSet<PathBuf> = HashSet::new();
    let mut last_event = Instant::now();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            if !pending_paths.is_empty() {
                process_pending(
                    &db,
                    &raw_dir,
                    &thumbnails_dir,
                    &llm_audit_dir,
                    &llm_config,
                    &app_handle,
                    watch_id,
                    &path,
                    &mut pending_paths,
                );
            }
            break;
        }

        match rx.recv_timeout(check_interval) {
            Ok(event) => {
                if !is_relevant_event(&event) {
                    continue;
                }
                for p in event.paths {
                    if p.is_file() && matches_filter(&p, &ext_filter) {
                        pending_paths.insert(p);
                    }
                }
                last_event = Instant::now();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !pending_paths.is_empty() && last_event.elapsed() >= stable_duration {
                    process_pending(
                        &db,
                        &raw_dir,
                        &thumbnails_dir,
                        &llm_audit_dir,
                        &llm_config,
                        &app_handle,
                        watch_id,
                        &path,
                        &mut pending_paths,
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn is_relevant_event(event: &Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
}

fn matches_filter(path: &Path, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| extensions.iter().any(|ext| ext.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn process_pending(
    db: &Arc<Mutex<Connection>>,
    raw_dir: &Path,
    thumbnails_dir: &Path,
    llm_audit_dir: &Path,
    llm_config: &Arc<Mutex<Option<LlmProviderConfig>>>,
    app_handle: &AppHandle,
    watch_id: i64,
    watch_path: &Path,
    pending: &mut HashSet<PathBuf>,
) {
    let path_strs: Vec<String> = pending
        .drain()
        .filter_map(|p| p.to_str().map(String::from))
        .collect();

    if path_strs.is_empty() {
        return;
    }

    let count = path_strs.len();
    info!("Watcher import: {count} files from watch_dir {watch_id}");
    let (imported, raw_file_ids) = match db.lock() {
        Ok(mut conn) => {
            let jobs = import_files(&mut conn, raw_dir, path_strs).unwrap_or_default();
            let ids: Vec<i64> = jobs
                .iter()
                .filter(|j| j.status == "imported" && j.raw_file_id.is_some())
                .filter_map(|j| j.raw_file_id)
                .collect();

            // Record event and notification
            let total = jobs.len();
            let success = jobs.iter().filter(|j| j.status == "imported").count();
            let dups = jobs.iter().filter(|j| j.status == "duplicate").count();
            let failed = jobs.iter().filter(|j| j.status == "failed").count();
            let _ = event::record_import_event(&conn, total, success, dups, failed, &[], &ids);
            let _ = event::create_notification(
                &conn,
                "info",
                &format!("监控导入: {total} 个文件"),
                &format!("成功 {success}，重复 {dups}，失败 {failed}"),
                None,
                None,
            );

            (jobs, ids)
        }
        Err(_) => return,
    };

    let _ = app_handle.emit(
        "watcher-import",
        WatcherImportEvent {
            watch_dir_id: watch_id,
            watch_dir_path: watch_path.to_string_lossy().into_owned(),
            imported_count: count,
            jobs: imported,
        },
    );

    // Auto-recognize each imported file if LLM is configured
    if !raw_file_ids.is_empty() {
        if let Some(config) = llm_config.lock().ok().and_then(|c| c.clone()) {
            let db = Arc::clone(db);
            let thumbnails_dir = thumbnails_dir.to_path_buf();
            let audit = config.audit_enabled.then(|| LlmAuditConfig {
                dir: llm_audit_dir.to_path_buf(),
            });

            tauri::async_runtime::spawn(async move {
                for raw_file_id in raw_file_ids {
                    recognize_raw_file_async(
                        &db,
                        &thumbnails_dir,
                        raw_file_id,
                        &config,
                        audit.as_ref(),
                    )
                    .await;
                }
            });
        }
    }
}

async fn recognize_raw_file_async(
    db: &Arc<Mutex<Connection>>,
    thumbnails_dir: &Path,
    raw_file_id: i64,
    config: &LlmProviderConfig,
    audit: Option<&LlmAuditConfig>,
) {
    let (storage_path, mime_type) = match db.lock() {
        Ok(conn) => {
            match conn.query_row(
                "SELECT storage_path, COALESCE(mime_type, '') FROM raw_files WHERE id = ?1",
                [raw_file_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ) {
                Ok(r) => r,
                Err(e) => {
                    error!("Watcher: raw_file {raw_file_id} not found: {e}");
                    return;
                }
            }
        }
        Err(e) => {
            error!("Watcher: failed to lock db for recognition: {e}");
            return;
        }
    };

    let storage_path = std::path::PathBuf::from(&storage_path);
    let thumb_dir = thumbnails_dir.to_path_buf();

    let recognition_inputs: Vec<(Option<String>, std::path::PathBuf, String)> =
        if mime_type == "application/pdf" {
            match render_pdf_pages(&storage_path, &thumb_dir, raw_file_id) {
                Ok(pages) => pages
                    .into_iter()
                    .map(|page| {
                        (
                            Some(page.page_number.to_string()),
                            page.image_path,
                            "image/png".to_owned(),
                        )
                    })
                    .collect(),
                Err(e) => {
                    error!("Watcher: PDF render failed for raw_file {raw_file_id}: {e}");
                    return;
                }
            }
        } else {
            vec![(None, storage_path, mime_type)]
        };

    for (page_range, image_path, mime) in recognition_inputs {
        let recognition =
            match recognize_invoice_image(config.clone(), &image_path, &mime, audit).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Watcher: recognition failed for raw_file {raw_file_id}: {e}");
                    continue;
                }
            };

        let invoice = match db.lock() {
            Ok(mut conn) => save_invoice_extraction(
                &mut conn,
                SaveInvoiceExtractionRequest {
                    raw_file_id,
                    source_page_range: page_range,
                    provider_name: Some(config.base_url.clone()),
                    model: Some(recognition.model.clone()),
                    response_json: recognition.response_json,
                },
            )
            .ok(),
            Err(_) => None,
        };

        if let Some(invoice) = invoice {
            let title = invoice.seller_name.clone().unwrap_or_else(|| "未知".into());
            if let Ok(db) = db.lock() {
                let _ = event::record_recognition_event(
                    &db,
                    invoice.id,
                    &title,
                    true,
                    recognition.duration_ms,
                    &recognition.model,
                    1,
                );
                let _ = event::create_notification(
                    &db,
                    "info",
                    &format!("自动识别完成: {title}"),
                    &format!(
                        "监控目录文件已自动识别为发票，模型 {}，耗时 {}ms",
                        recognition.model, recognition.duration_ms
                    ),
                    Some("invoice"),
                    Some(invoice.id),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;
    use tempfile::TempDir;

    fn setup_db() -> (Connection, TempDir) {
        let tmp = TempDir::new().expect("tmp");
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");
        (conn, tmp)
    }

    fn insert_watch_dir(conn: &Connection, path: &str, enabled: bool) -> i64 {
        conn.execute(
            "INSERT INTO watch_dirs (path, enabled) VALUES (?1, ?2)",
            params![path, enabled as i32],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_add_and_remove_watch_dir_db() {
        let (conn, _tmp) = setup_db();
        let id = insert_watch_dir(&conn, "/tmp/test-watch", true);
        assert!(id > 0);

        let path: String = conn
            .query_row("SELECT path FROM watch_dirs WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .expect("read");
        assert_eq!(path, "/tmp/test-watch");

        conn.execute("DELETE FROM watch_dirs WHERE id = ?1", [id])
            .expect("delete");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM watch_dirs WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_toggle_watch_dir_db() {
        let (conn, _tmp) = setup_db();
        let id = insert_watch_dir(&conn, "/tmp/test-watch", true);

        conn.execute(
            "UPDATE watch_dirs SET enabled = ?2 WHERE id = ?1",
            params![id, false as i32],
        )
        .expect("update");

        let enabled: bool = conn
            .query_row(
                "SELECT enabled FROM watch_dirs WHERE id = ?1",
                [id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .expect("read");
        assert!(!enabled);
    }

    #[test]
    fn test_unique_path_constraint() {
        let (conn, _tmp) = setup_db();
        insert_watch_dir(&conn, "/tmp/test-watch", true);
        let result = conn.execute(
            "INSERT INTO watch_dirs (path) VALUES (?1)",
            params!["/tmp/test-watch"],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_default_values() {
        let (conn, _tmp) = setup_db();
        conn.execute(
            "INSERT INTO watch_dirs (path) VALUES (?1)",
            params!["/tmp/defaults"],
        )
        .expect("insert");

        let (recursive, enabled, stable_wait_ms, extensions): (bool, bool, i64, String) = conn
            .query_row(
                "SELECT recursive, enabled, stable_wait_ms, extensions FROM watch_dirs WHERE path = '/tmp/defaults'",
                [],
                |row| Ok((row.get::<_, i32>(0)? != 0, row.get::<_, i32>(1)? != 0, row.get(2)?, row.get(3)?)),
            )
            .expect("read");

        assert!(recursive);
        assert!(enabled);
        assert_eq!(stable_wait_ms, 2000);
        assert_eq!(extensions, "");
    }
}
