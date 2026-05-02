use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use tracing::error;

#[derive(Debug, Clone)]
pub struct RenderedPdfPage {
    pub page_number: usize,
    pub image_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PreparedImage {
    pub image_path: PathBuf,
    pub thumbnail_path: PathBuf,
    pub mime_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF renderer `pdftoppm` was not found in PATH")]
    MissingPdfRenderer,
    #[error("PDF renderer failed with status {status}: {stderr}")]
    PdfRendererFailed { status: String, stderr: String },
    #[error("PDF renderer did not produce any page images")]
    NoRenderedPages,
    #[error("image processor `magick` was not found in PATH")]
    MissingImageProcessor,
    #[error("image processor failed with status {status}: {stderr}")]
    ImageProcessorFailed { status: String, stderr: String },
}

pub fn render_pdf_pages(
    pdf_path: &Path,
    cache_root: &Path,
    raw_file_id: i64,
) -> Result<Vec<RenderedPdfPage>, DocumentError> {
    let render_dir = cache_root
        .join("pdf-pages")
        .join(raw_file_id.to_string())
        .join(unique_run_id());
    fs::create_dir_all(&render_dir)?;

    let output_prefix = render_dir.join("page");
    let output = Command::new("pdftoppm")
        .arg("-jpeg")
        .arg("-r")
        .arg("180")
        .arg("-jpegopt")
        .arg("quality=85")
        .arg(pdf_path)
        .arg(&output_prefix)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                DocumentError::MissingPdfRenderer
            } else {
                DocumentError::Io(err)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).chars().take(800).collect::<String>();
        error!("PDF renderer failed: {stderr}");
        return Err(DocumentError::PdfRendererFailed {
            status: output.status.to_string(),
            stderr,
        });
    }

    let mut pages = rendered_page_paths(&render_dir)?;
    if pages.is_empty() {
        error!("PDF renderer produced no pages");
        return Err(DocumentError::NoRenderedPages);
    }

    pages.sort_by_key(|(page_number, _)| *page_number);
    Ok(pages
        .into_iter()
        .map(|(page_number, image_path)| RenderedPdfPage {
            page_number,
            image_path,
        })
        .collect())
}

pub fn prepare_image_for_recognition(
    image_path: &Path,
    cache_root: &Path,
    raw_file_id: i64,
    page_number: Option<usize>,
) -> Result<PreparedImage, DocumentError> {
    let label = page_number
        .map(|page_number| format!("page-{page_number}"))
        .unwrap_or_else(|| "image".to_owned());
    let normalized_dir = cache_root.join("normalized").join(raw_file_id.to_string());
    let thumbnail_dir = cache_root.join("previews").join(raw_file_id.to_string());
    fs::create_dir_all(&normalized_dir)?;
    fs::create_dir_all(&thumbnail_dir)?;

    let normalized_path = normalized_dir.join(format!("{label}.jpg"));
    let thumbnail_path = thumbnail_dir.join(format!("{label}.jpg"));

    run_magick_resize(image_path, &normalized_path, 1800, 85)?;
    run_magick_resize(image_path, &thumbnail_path, 420, 78)?;

    Ok(PreparedImage {
        image_path: normalized_path,
        thumbnail_path,
        mime_type: "image/jpeg".to_owned(),
    })
}

fn rendered_page_paths(render_dir: &Path) -> Result<Vec<(usize, PathBuf)>, DocumentError> {
    let mut pages = Vec::new();

    for entry in fs::read_dir(render_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jpg") {
            continue;
        }
        let Some(page_number) = path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(page_number_from_stem)
        else {
            continue;
        };
        pages.push((page_number, path));
    }

    Ok(pages)
}

fn run_magick_resize(
    input_path: &Path,
    output_path: &Path,
    max_dimension: u16,
    quality: u8,
) -> Result<(), DocumentError> {
    let output = Command::new("magick")
        .arg(input_path)
        .arg("-auto-orient")
        .arg("-resize")
        .arg(format!("{max_dimension}x{max_dimension}>"))
        .arg("-background")
        .arg("white")
        .arg("-alpha")
        .arg("remove")
        .arg("-alpha")
        .arg("off")
        .arg("-strip")
        .arg("-quality")
        .arg(quality.to_string())
        .arg(output_path)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                DocumentError::MissingImageProcessor
            } else {
                DocumentError::Io(err)
            }
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).chars().take(800).collect::<String>();
        error!("Image processor (magick) failed: {stderr}");
        Err(DocumentError::ImageProcessorFailed {
            status: output.status.to_string(),
            stderr,
        })
    }
}

fn page_number_from_stem(stem: &str) -> Option<usize> {
    let (_, suffix) = stem.rsplit_once('-')?;
    suffix.parse().ok()
}

fn unique_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis();
    format!("{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pdftoppm_page_number_from_stem() {
        assert_eq!(page_number_from_stem("page-1"), Some(1));
        assert_eq!(page_number_from_stem("page-000012"), Some(12));
        assert_eq!(page_number_from_stem("page"), None);
    }

    #[test]
    fn prepares_sample_image_when_magick_is_available() {
        let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sample_path = repo_dir
            .join("receipts")
            .join("微信图片_20260430161538.jpg");
        let temp_dir = tempfile::tempdir().expect("tempdir");

        match prepare_image_for_recognition(&sample_path, temp_dir.path(), 1, None) {
            Ok(prepared) => {
                assert!(prepared.image_path.exists());
                assert!(prepared.thumbnail_path.exists());
                assert_eq!(prepared.mime_type, "image/jpeg");
            }
            Err(DocumentError::MissingImageProcessor) => {}
            Err(err) => panic!("prepare image failed: {err}"),
        }
    }
}
