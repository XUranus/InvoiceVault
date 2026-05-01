use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::raw_store::{ingest_file, RawFileInput, RawStoreError};

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJobSummary {
    pub id: i64,
    pub raw_file_id: Option<i64>,
    pub source_path: String,
    pub original_name: Option<String>,
    pub status: String,
    pub sha256: Option<String>,
    pub storage_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("no import paths provided")]
    EmptyRequest,
    #[error("raw store error: {0}")]
    RawStore(#[from] RawStoreError),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn import_files(
    conn: &mut Connection,
    raw_dir: &Path,
    paths: Vec<String>,
) -> Result<Vec<ImportJobSummary>, ImportError> {
    if paths.is_empty() {
        return Err(ImportError::EmptyRequest);
    }

    let mut summaries = Vec::with_capacity(paths.len());
    for source_path in paths {
        summaries.push(import_one(conn, raw_dir, PathBuf::from(source_path))?);
    }
    Ok(summaries)
}

pub fn list_import_jobs(conn: &Connection) -> Result<Vec<ImportJobSummary>, ImportError> {
    let mut stmt = conn.prepare(
        "SELECT
            ij.id,
            ij.raw_file_id,
            ij.source_path,
            rf.original_name,
            ij.status,
            rf.sha256,
            rf.storage_path,
            ij.error_message,
            ij.created_at,
            ij.updated_at
        FROM import_jobs ij
        LEFT JOIN raw_files rf ON rf.id = ij.raw_file_id
        ORDER BY ij.id DESC
        LIMIT 100",
    )?;

    let jobs = stmt
        .query_map([], row_to_import_job)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(jobs)
}

fn import_one(
    conn: &mut Connection,
    raw_dir: &Path,
    source_path: PathBuf,
) -> Result<ImportJobSummary, ImportError> {
    let source_path_text = source_path.to_string_lossy().into_owned();

    match ingest_file(raw_dir, &source_path) {
        Ok(raw_file) => {
            let tx = conn.transaction()?;
            let (raw_file_id, status) = upsert_raw_file(&tx, &raw_file)?;
            let job_id =
                insert_import_job(&tx, Some(raw_file_id), &source_path_text, status, None)?;
            tx.commit()?;
            load_import_job(conn, job_id)
        }
        Err(err) => {
            let status = "failed";
            let error_message = err.to_string();
            let job_id =
                insert_import_job(conn, None, &source_path_text, status, Some(&error_message))?;
            load_import_job(conn, job_id)
        }
    }
}

fn upsert_raw_file(
    conn: &Connection,
    raw_file: &RawFileInput,
) -> Result<(i64, &'static str), ImportError> {
    let existing_id = conn
        .query_row(
            "SELECT id FROM raw_files WHERE sha256 = ?1",
            [&raw_file.sha256],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;

    if let Some(id) = existing_id {
        return Ok((id, "duplicate"));
    }

    conn.execute(
        "INSERT INTO raw_files (
            sha256,
            md5,
            original_name,
            extension,
            mime_type,
            byte_size,
            storage_path
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            raw_file.sha256,
            raw_file.md5,
            raw_file.original_name,
            raw_file.extension,
            raw_file.mime_type,
            raw_file.byte_size as i64,
            raw_file.storage_path.to_string_lossy().as_ref()
        ],
    )?;

    Ok((conn.last_insert_rowid(), "completed"))
}

fn insert_import_job(
    conn: &Connection,
    raw_file_id: Option<i64>,
    source_path: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<i64, ImportError> {
    let now = current_timestamp();
    conn.execute(
        "INSERT INTO import_jobs (
            raw_file_id,
            source_path,
            status,
            error_message,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![raw_file_id, source_path, status, error_message, now],
    )?;

    Ok(conn.last_insert_rowid())
}

fn load_import_job(conn: &Connection, job_id: i64) -> Result<ImportJobSummary, ImportError> {
    conn.query_row(
        "SELECT
            ij.id,
            ij.raw_file_id,
            ij.source_path,
            rf.original_name,
            ij.status,
            rf.sha256,
            rf.storage_path,
            ij.error_message,
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
        source_path: row.get(2)?,
        original_name: row.get(3)?,
        status: row.get(4)?,
        sha256: row.get(5)?,
        storage_path: row.get(6)?,
        error_message: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
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
        )
        .expect("first import");
        let second = import_files(
            &mut conn,
            &temp_dir.path().join("raw"),
            vec![source_path.to_string_lossy().into_owned()],
        )
        .expect("second import");

        assert_eq!(first[0].status, "completed");
        assert_eq!(second[0].status, "duplicate");
        assert_eq!(first[0].raw_file_id, second[0].raw_file_id);
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

        let jobs = import_files(&mut conn, &temp_dir.path().join("raw"), sample_paths)
            .expect("import sample receipts");

        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| job.status == "completed"));
        assert!(jobs.iter().all(|job| job.storage_path.is_some()));
    }
}
