use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use md5::Md5;
use serde::Serialize;
use sha2::{Digest, Sha256};

const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "jpeg"];

#[derive(Debug, Clone, Serialize)]
pub struct RawFileInput {
    pub sha256: String,
    pub md5: String,
    pub original_name: String,
    pub extension: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub storage_path: PathBuf,
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

pub fn ingest_file(raw_dir: &Path, source_path: &Path) -> Result<RawFileInput, RawStoreError> {
    validate_source(source_path)?;

    let extension = normalized_extension(source_path)?;
    let original_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| RawStoreError::MissingFileName(source_path.to_path_buf()))?
        .to_owned();

    let (sha256, md5, byte_size) = hash_file(source_path)?;
    let storage_path = content_addressed_path(raw_dir, &sha256, &extension);

    if !storage_path.exists() {
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_path, &storage_path)?;
    }

    let mime_type = mime_guess::from_ext(&extension)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();

    Ok(RawFileInput {
        sha256,
        md5,
        original_name,
        extension,
        mime_type,
        byte_size,
        storage_path,
    })
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

fn content_addressed_path(raw_dir: &Path, sha256: &str, extension: &str) -> PathBuf {
    raw_dir
        .join(&sha256[..2])
        .join(format!("{sha256}.{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_file_uses_content_addressed_storage() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("invoice.PDF");
        fs::write(&source_path, b"invoice-bytes").expect("write source");

        let raw_dir = temp_dir.path().join("raw");
        let first = ingest_file(&raw_dir, &source_path).expect("first ingest");
        let second = ingest_file(&raw_dir, &source_path).expect("second ingest");

        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.storage_path, second.storage_path);
        assert!(first.storage_path.exists());
        assert_eq!(first.extension, "pdf");
    }

    #[test]
    fn rejects_unsupported_extensions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let source_path = temp_dir.path().join("notes.txt");
        fs::write(&source_path, b"not an invoice").expect("write source");

        let err = ingest_file(&temp_dir.path().join("raw"), &source_path).expect_err("reject txt");

        assert!(matches!(err, RawStoreError::UnsupportedExtension(_)));
    }
}
