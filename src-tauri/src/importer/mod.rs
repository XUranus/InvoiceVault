use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::raw_store::{inspect_file, store_original_file, RawFileInput, RawStoreError};

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJobSummary {
    pub id: i64,
    pub raw_file_id: Option<i64>,
    pub invoice_id: Option<i64>,
    pub source_path: String,
    pub original_name: Option<String>,
    pub current_name: Option<String>,
    pub status: String,
    pub sha256: Option<String>,
    pub storage_path: Option<String>,
    pub mime_type: Option<String>,
    pub error_message: Option<String>,
    pub source_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJobListResult {
    pub jobs: Vec<ImportJobSummary>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("no import paths provided")]
    EmptyRequest,
    #[error("raw store error: {0}")]
    RawStore(#[from] RawStoreError),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("stored raw file metadata is incomplete")]
    MissingStoredMetadata,
}

pub fn import_files(
    conn: &mut Connection,
    raw_dir: &Path,
    paths: Vec<String>,
    source_type: &str,
) -> Result<Vec<ImportJobSummary>, ImportError> {
    if paths.is_empty() {
        return Err(ImportError::EmptyRequest);
    }

    let mut summaries = Vec::with_capacity(paths.len());
    for source_path in paths {
        summaries.push(import_one(conn, raw_dir, PathBuf::from(source_path), source_type)?);
    }
    Ok(summaries)
}

pub fn list_import_jobs(
    conn: &Connection,
    page: i64,
    page_size: i64,
) -> Result<ImportJobListResult, ImportError> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM import_jobs", [], |row| row.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT
            ij.id,
            ij.raw_file_id,
            (
                SELECT inv.id
                FROM invoices inv
                WHERE inv.raw_file_id = ij.raw_file_id
                ORDER BY inv.id DESC
                LIMIT 1
            ) AS invoice_id,
            ij.source_path,
            rf.original_name,
            rf.current_name,
            ij.status,
            rf.sha256,
            rf.storage_path,
            rf.mime_type,
            ij.error_message,
            ij.source_type,
            ij.created_at,
            ij.updated_at
        FROM import_jobs ij
        LEFT JOIN raw_files rf ON rf.id = ij.raw_file_id
        ORDER BY ij.id DESC
        LIMIT ?1 OFFSET ?2",
    )?;

    let jobs = stmt
        .query_map(params![page_size, offset], row_to_import_job)?
        .collect::<Result<Vec<_>, _>>()?;

    let total_pages = (total_count + page_size - 1) / page_size;

    Ok(ImportJobListResult {
        jobs,
        total_count,
        page,
        page_size,
        total_pages,
    })
}

pub fn delete_import_job(conn: &Connection, job_id: i64) -> Result<(), ImportError> {
    conn.execute("DELETE FROM import_jobs WHERE id = ?1", [job_id])?;
    Ok(())
}

fn import_one(
    conn: &mut Connection,
    raw_dir: &Path,
    source_path: PathBuf,
    source_type: &str,
) -> Result<ImportJobSummary, ImportError> {
    let source_path_text = source_path.to_string_lossy().into_owned();
    let job_id = insert_import_job(conn, None, &source_path_text, "importing", None, source_type)?;

    match inspect_file(&source_path) {
        Ok(raw_file) => {
            if let Some(raw_file_id) = find_raw_file_by_hash(conn, &raw_file.sha256)? {
                update_import_job_status(conn, job_id, Some(raw_file_id), "duplicate", None)?;
                return load_import_job(conn, job_id);
            }

            let stored_raw_file = match store_original_file(raw_dir, raw_file) {
                Ok(stored_raw_file) => stored_raw_file,
                Err(err) => {
                    error!("Import failed for {source_path_text}: {err}");
                    let error_message = err.to_string();
                    update_import_job_status(conn, job_id, None, "failed", Some(&error_message))?;
                    return load_import_job(conn, job_id);
                }
            };
            let tx = conn.transaction()?;
            let raw_file_id = insert_raw_file(&tx, &stored_raw_file)?;
            update_import_job_status(&tx, job_id, Some(raw_file_id), "imported", None)?;
            tx.commit()?;
            load_import_job(conn, job_id)
        }
        Err(err) => {
            error!("Import failed for {source_path_text}: {err}");
            let status = "failed";
            let error_message = err.to_string();
            update_import_job_status(conn, job_id, None, status, Some(&error_message))?;
            load_import_job(conn, job_id)
        }
    }
}

fn find_raw_file_by_hash(conn: &Connection, sha256: &str) -> Result<Option<i64>, ImportError> {
    Ok(conn
        .query_row(
            "SELECT id FROM raw_files WHERE sha256 = ?1",
            [sha256],
            |row| row.get::<_, i64>(0),
        )
        .optional()?)
}

fn insert_raw_file(conn: &Connection, raw_file: &RawFileInput) -> Result<i64, ImportError> {
    let current_name = raw_file
        .current_name
        .as_deref()
        .ok_or(ImportError::MissingStoredMetadata)?;
    let storage_path = raw_file
        .storage_path
        .as_ref()
        .ok_or(ImportError::MissingStoredMetadata)?;
    conn.execute(
        "INSERT INTO raw_files (
            sha256,
            md5,
            original_name,
            current_name,
            extension,
            mime_type,
            byte_size,
            storage_path
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            raw_file.sha256,
            raw_file.md5,
            raw_file.original_name,
            current_name,
            raw_file.extension,
            raw_file.mime_type,
            raw_file.byte_size as i64,
            storage_path.to_string_lossy().as_ref()
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

fn insert_import_job(
    conn: &Connection,
    raw_file_id: Option<i64>,
    source_path: &str,
    status: &str,
    error_message: Option<&str>,
    source_type: &str,
) -> Result<i64, ImportError> {
    let now = current_timestamp();
    conn.execute(
        "INSERT INTO import_jobs (
            raw_file_id,
            source_path,
            status,
            error_message,
            source_type,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![raw_file_id, source_path, status, error_message, source_type, now],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn update_import_job_status(
    conn: &Connection,
    job_id: i64,
    raw_file_id: Option<i64>,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), ImportError> {
    let now = current_timestamp();
    conn.execute(
        "UPDATE import_jobs
         SET raw_file_id = COALESCE(?1, raw_file_id),
             status = ?2,
             error_message = ?3,
             updated_at = ?4
         WHERE id = ?5",
        params![raw_file_id, status, error_message, now, job_id],
    )?;
    Ok(())
}

pub fn update_import_job_status_by_raw_file(
    conn: &Connection,
    raw_file_id: i64,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), ImportError> {
    let now = current_timestamp();
    conn.execute(
        "UPDATE import_jobs
         SET status = ?1,
             error_message = ?2,
             updated_at = ?3
         WHERE raw_file_id = ?4",
        params![status, error_message, now, raw_file_id],
    )?;
    Ok(())
}

pub fn recover_interrupted_import_jobs(conn: &Connection) -> Result<usize, ImportError> {
    let now = current_timestamp();
    let message = "上次运行中断，任务已停止。请重新导入或重新识别。";
    let changed = conn.execute(
        "UPDATE import_jobs
         SET status = CASE
                 WHEN raw_file_id IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM invoices inv WHERE inv.raw_file_id = import_jobs.raw_file_id
                   )
                 THEN 'imported'
                 ELSE 'failed'
             END,
             error_message = CASE
                 WHEN raw_file_id IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM invoices inv WHERE inv.raw_file_id = import_jobs.raw_file_id
                   )
                 THEN NULL
                 ELSE ?1
             END,
             updated_at = ?2
         WHERE status IN ('importing', 'pending', 'processing', 'recognizing')",
        params![message, now],
    )?;
    Ok(changed)
}

fn load_import_job(conn: &Connection, job_id: i64) -> Result<ImportJobSummary, ImportError> {
    conn.query_row(
        "SELECT
            ij.id,
            ij.raw_file_id,
            (
                SELECT inv.id
                FROM invoices inv
                WHERE inv.raw_file_id = ij.raw_file_id
                ORDER BY inv.id DESC
                LIMIT 1
            ) AS invoice_id,
            ij.source_path,
            rf.original_name,
            rf.current_name,
            ij.status,
            rf.sha256,
            rf.storage_path,
            rf.mime_type,
            ij.error_message,
            ij.source_type,
            ij.created_at,
            ij.updated_at
        FROM import_jobs ij
        LEFT JOIN raw_files rf ON rf.id = ij.raw_file_id
        WHERE ij.id = ?1",
        [job_id],
        row_to_import_job,
    )
    .map_err(ImportError::from)
}

fn row_to_import_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportJobSummary> {
    Ok(ImportJobSummary {
        id: row.get(0)?,
        raw_file_id: row.get(1)?,
        invoice_id: row.get(2)?,
        source_path: row.get(3)?,
        original_name: row.get(4)?,
        current_name: row.get(5)?,
        status: row.get(6)?,
        sha256: row.get(7)?,
        storage_path: row.get(8)?,
        mime_type: row.get(9)?,
        error_message: row.get(10)?,
        source_type: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn current_timestamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis();
    millis.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;

    #[test]
    fn import_files_records_duplicates() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("receipt.jpg");
        std::fs::write(&source_path, b"image-bytes").expect("write source");

        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");

        let first = import_files(
            &mut conn,
            &temp_dir.path().join("raw"),
            vec![source_path.to_string_lossy().into_owned()],
            "manual",
        )
        .expect("first import");
        let second = import_files(
            &mut conn,
            &temp_dir.path().join("raw"),
            vec![source_path.to_string_lossy().into_owned()],
            "manual",
        )
        .expect("second import");

        assert_eq!(first[0].status, "imported");
        assert_eq!(second[0].status, "duplicate");
        assert_eq!(first[0].raw_file_id, second[0].raw_file_id);
        assert_eq!(first[0].original_name.as_deref(), Some("receipt.jpg"));
        assert_eq!(first[0].current_name.as_deref(), Some("receipt.jpg"));
    }

    #[test]
    fn sample_receipts_can_be_imported() {
        let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sample_paths = vec![
            repo_dir
                .join("receipts")
                .join("微信图片_20260430161538.jpg")
                .to_string_lossy()
                .into_owned(),
            repo_dir
                .join("receipts")
                .join("拼多多商家电子发票.pdf")
                .to_string_lossy()
                .into_owned(),
        ];

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");

        let jobs = import_files(&mut conn, &temp_dir.path().join("raw"), sample_paths, "manual")
            .expect("import sample receipts");

        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| job.status == "imported"));
        assert!(jobs.iter().all(|job| job.storage_path.is_some()));
    }

    #[test]
    fn recovers_interrupted_import_jobs() {
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");

        let importing_id = insert_import_job(&conn, None, "/tmp/a.pdf", "importing", None, "manual")
            .expect("insert importing job");

        let raw_file_id = {
            conn.execute(
                "INSERT INTO raw_files (
                    sha256, md5, original_name, current_name, extension, mime_type, byte_size, storage_path
                 ) VALUES ('hash', NULL, 'b.pdf', 'b.pdf', 'pdf', 'application/pdf', 10, '/tmp/b.pdf')",
                [],
            )
            .expect("insert raw file");
            conn.last_insert_rowid()
        };
        let recognizing_id =
            insert_import_job(&conn, Some(raw_file_id), "/tmp/b.pdf", "recognizing", None, "manual")
                .expect("insert recognizing job");
        conn.execute(
            "INSERT INTO invoices (raw_file_id, currency, status, duplicate_status)
             VALUES (?1, 'CNY', 'recognized', 'unknown')",
            [raw_file_id],
        )
        .expect("insert invoice");

        let changed = recover_interrupted_import_jobs(&conn).expect("recover jobs");
        assert_eq!(changed, 2);

        let importing = load_import_job(&conn, importing_id).expect("load importing");
        let recognizing = load_import_job(&conn, recognizing_id).expect("load recognizing");

        assert_eq!(importing.status, "failed");
        assert!(importing.error_message.is_some());
        assert_eq!(recognizing.status, "imported");
        assert!(recognizing.error_message.is_none());
    }
}
