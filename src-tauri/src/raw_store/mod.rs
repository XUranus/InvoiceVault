use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use chrono::{Datelike, Local};
use tracing::error;
use md5::Md5;
use serde::Serialize;
use sha2::{Digest, Sha256};

const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "jpeg"];

#[derive(Debug, Clone, Serialize)]
pub struct RawFileInput {
    pub source_path: PathBuf,
    pub sha256: String,
    pub md5: String,
    pub original_name: String,
    pub current_name: Option<String>,
    pub extension: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub storage_path: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum RawStoreError {
    #[error("source path does not exist: {0}")]
    MissingSource(PathBuf),
    #[error("source path is not a file: {0}")]
    NotAFile(PathBuf),
    #[error("unsupported file extension: {0}")]
    UnsupportedExtension(String),
    #[error("file name is missing: {0}")]
    MissingFileName(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub fn inspect_file(source_path: &Path) -> Result<RawFileInput, RawStoreError> {
    validate_source(source_path)?;

    let extension = normalized_extension(source_path)?;
    let original_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| RawStoreError::MissingFileName(source_path.to_path_buf()))?
        .to_owned();

    let (sha256, md5, byte_size) = hash_file(source_path)
        .inspect_err(|e| error!("File hash failed for {}: {e}", source_path.display()))?;
    let mime_type = mime_guess::from_ext(&extension)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();

    Ok(RawFileInput {
        source_path: source_path.to_path_buf(),
        sha256,
        md5,
        original_name,
        current_name: None,
        extension,
        mime_type,
        byte_size,
        storage_path: None,
    })
}

pub fn store_original_file(
    raw_dir: &Path,
    mut raw_file: RawFileInput,
) -> Result<RawFileInput, RawStoreError> {
    let now = Local::now();
    let month_dir = raw_dir
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()));
    fs::create_dir_all(&month_dir)?;

    let current_name = available_file_name(&month_dir, &raw_file.original_name);
    let storage_path = month_dir.join(&current_name);
    fs::copy(&raw_file.source_path, &storage_path)?;

    raw_file.current_name = Some(current_name);
    raw_file.storage_path = Some(storage_path);
    Ok(raw_file)
}

fn validate_source(source_path: &Path) -> Result<(), RawStoreError> {
    if !source_path.exists() {
        return Err(RawStoreError::MissingSource(source_path.to_path_buf()));
    }
    if !source_path.is_file() {
        return Err(RawStoreError::NotAFile(source_path.to_path_buf()));
    }
    normalized_extension(source_path)?;
    Ok(())
}

fn normalized_extension(source_path: &Path) -> Result<String, RawStoreError> {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        Ok(extension)
    } else {
        Err(RawStoreError::UnsupportedExtension(extension))
    }
}

fn hash_file(source_path: &Path) -> Result<(String, String, u64), RawStoreError> {
    let file = File::open(source_path)?;
    let mut reader = BufReader::new(file);
    let mut sha256 = Sha256::new();
    let mut md5 = Md5::new();
    let mut byte_size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
        md5.update(&buffer[..read]);
        byte_size += read as u64;
    }

    Ok((
        hex::encode(sha256.finalize()),
        hex::encode(md5.finalize()),
        byte_size,
    ))
}

fn available_file_name(target_dir: &Path, original_name: &str) -> String {
    let original_path = Path::new(original_name);
    let stem = original_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("invoice");
    let extension = original_path.extension().and_then(|value| value.to_str());

    for index in 0.. {
        let candidate = if index == 0 {
            original_name.to_owned()
        } else if let Some(extension) = extension {
            format!("{stem}-{index}.{extension}")
        } else {
            format!("{stem}-{index}")
        };

        if !target_dir.join(&candidate).exists() {
            return candidate;
        }
    }

    unreachable!("unbounded filename search should always return");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_file_under_year_month_with_original_format() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("invoice.PDF");
        fs::write(&source_path, b"invoice-bytes").expect("write source");

        let raw_dir = temp_dir.path().join("raw");
        let inspected = inspect_file(&source_path).expect("inspect");
        let stored = store_original_file(&raw_dir, inspected).expect("store");
        let storage_path = stored.storage_path.expect("storage path");

        assert!(storage_path.exists());
        assert_eq!(stored.original_name, "invoice.PDF");
        assert_eq!(stored.current_name.as_deref(), Some("invoice.PDF"));
        assert_eq!(stored.extension, "pdf");
        assert_eq!(
            storage_path.extension().and_then(|value| value.to_str()),
            Some("PDF")
        );
        assert_eq!(
            storage_path
                .parent()
                .and_then(|value| value.parent())
                .and_then(|value| value.parent()),
            Some(raw_dir.as_path())
        );
    }

    #[test]
    fn resolves_filename_collisions_without_changing_format() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("invoice.jpg");
        fs::write(&source_path, b"image-bytes").expect("write source");

        let raw_dir = temp_dir.path().join("raw");
        let first =
            store_original_file(&raw_dir, inspect_file(&source_path).expect("inspect first"))
                .expect("store first");
        let second = store_original_file(
            &raw_dir,
            inspect_file(&source_path).expect("inspect second"),
        )
        .expect("store second");

        assert_eq!(first.current_name.as_deref(), Some("invoice.jpg"));
        assert_eq!(second.current_name.as_deref(), Some("invoice-1.jpg"));
    }

    #[test]
    fn rejects_unsupported_extensions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("notes.txt");
        fs::write(&source_path, b"not an invoice").expect("write source");

        let err = inspect_file(&source_path).expect_err("reject txt");

        assert!(matches!(err, RawStoreError::UnsupportedExtension(_)));
    }
}
