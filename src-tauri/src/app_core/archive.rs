use std::{
    io::{Read, Write},
    path::Path,
};

use super::AppError;

fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
}

fn io_err(e: std::io::Error) -> AppError {
    AppError::Io(e)
}

pub fn stream_file_to_zip(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    zip_path: &str,
    file_path: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), AppError> {
    zip_writer.start_file(zip_path, options).map_err(zip_err)?;
    let mut reader = std::io::BufReader::new(std::fs::File::open(file_path)?);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        zip_writer.write_all(&buf[..n]).map_err(io_err)?;
    }
    Ok(())
}

pub fn add_dir_to_zip(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), AppError> {
    add_dir_to_zip_inner(zip_writer, base, dir, options, &[])
}

pub fn add_dir_to_zip_with_skip(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: zip::write::SimpleFileOptions,
    skip_dirs: &[&str],
) -> Result<(), AppError> {
    add_dir_to_zip_inner(zip_writer, base, dir, options, skip_dirs)
}

fn add_dir_to_zip_inner(
    zip_writer: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: zip::write::SimpleFileOptions,
    skip_dirs: &[&str],
) -> Result<(), AppError> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let relative = path.strip_prefix(base).unwrap_or(&path);
            let name = relative.to_string_lossy().replace('\\', "/");
            stream_file_to_zip(zip_writer, &name, &path, options)?;
        } else if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if skip_dirs.contains(&dir_name) {
                continue;
            }
            add_dir_to_zip_inner(zip_writer, base, &path, options, skip_dirs)?;
        }
    }
    Ok(())
}
