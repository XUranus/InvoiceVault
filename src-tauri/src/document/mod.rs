use std::{
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{codecs::jpeg::JpegEncoder, DynamicImage, GenericImageView, ImageReader};
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
    #[error("image decode error: {0}")]
    ImageDecode(#[from] image::ImageError),
    #[error("PDF renderer `pdftoppm` was not found in PATH")]
    MissingPdfRenderer,
    #[error("PDF renderer failed with status {status}: {stderr}")]
    PdfRendererFailed { status: String, stderr: String },
    #[error("PDF renderer did not produce any page images")]
    NoRenderedPages,
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
    // Use plain PPM output for compatibility with both Poppler and xpdf pdftoppm.
    // Poppler supports -jpeg/-jpegopt but xpdf does not; PPM is the common denominator.
    let output = crate::process_utils::command_no_window("pdftoppm")
        .arg("-r")
        .arg("180")
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
        let stderr = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(800)
            .collect::<String>();
        error!("PDF renderer failed: {stderr}");
        return Err(DocumentError::PdfRendererFailed {
            status: output.status.to_string(),
            stderr,
        });
    }

    // Convert rendered PPM pages to JPEG and collect paths
    let mut pages = convert_ppm_pages_to_jpeg(&render_dir)?;
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

    resize_and_save(image_path, &normalized_path, 1800, 85)?;
    resize_and_save(image_path, &thumbnail_path, 800, 85)?;

    Ok(PreparedImage {
        image_path: normalized_path,
        thumbnail_path,
        mime_type: "image/jpeg".to_owned(),
    })
}

/// Read EXIF orientation from a JPEG file. Returns 1 (normal) if not found or on error.
fn read_exif_orientation(path: &Path) -> u32 {
    let Ok(file) = fs::File::open(path) else {
        return 1;
    };
    let mut bufreader = BufReader::new(&file);
    let Ok(exifreader) = exif::Reader::new().read_from_container(&mut bufreader) else {
        return 1;
    };
    if let Some(field) = exifreader.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        match field.value.get_uint(0) {
            Some(v @ 1..=8) => v,
            _ => 1,
        }
    } else {
        1
    }
}

/// Apply EXIF orientation to an image by rotating/flipping it so it displays correctly.
fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        1 => img,
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.fliph().rotate90(),
        6 => img.rotate90(),
        7 => img.fliph().rotate270(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Composite alpha onto a white background, returning an RGB image.
fn flatten_alpha(img: DynamicImage) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(w, h);
    for (src, dst) in rgba.pixels().zip(rgb.pixels_mut()) {
        let [r, g, b, a] = src.0;
        let alpha = a as f32 / 255.0;
        let inv = 1.0 - alpha;
        dst.0[0] = (r as f32 * alpha + 255.0 * inv) as u8;
        dst.0[1] = (g as f32 * alpha + 255.0 * inv) as u8;
        dst.0[2] = (b as f32 * alpha + 255.0 * inv) as u8;
    }
    DynamicImage::ImageRgb8(rgb)
}

/// Resize, auto-orient, flatten alpha, and save as JPEG with the given quality.
pub fn resize_and_save(
    input_path: &Path,
    output_path: &Path,
    max_dimension: u32,
    quality: u8,
) -> Result<(), DocumentError> {
    let reader = ImageReader::open(input_path)?.with_guessed_format()?;
    let img = reader.decode()?;

    // Apply EXIF orientation (only meaningful for JPEG inputs)
    let orientation = read_exif_orientation(input_path);
    let img = apply_orientation(img, orientation);

    // Flatten alpha onto white background
    let img = flatten_alpha(img);

    // Resize: only downscale, never upscale
    let (w, h) = img.dimensions();
    let needs_resize = w > max_dimension || h > max_dimension;
    let img = if needs_resize {
        img.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    // Write JPEG — no EXIF/metadata is written, effectively stripping it
    let output_file = fs::File::create(output_path)?;
    let mut encoder = JpegEncoder::new_with_quality(output_file, quality);
    encoder.encode_image(&img)?;

    Ok(())
}

/// Scan for PPM files produced by pdftoppm and convert each to JPEG.
/// Compatible with both Poppler (zero-padded names) and xpdf (no padding).
fn convert_ppm_pages_to_jpeg(render_dir: &Path) -> Result<Vec<(usize, PathBuf)>, DocumentError> {
    let mut ppm_files: Vec<(usize, PathBuf)> = Vec::new();

    for entry in fs::read_dir(render_dir)? {
        let entry = entry?;
        let path = entry.path();
        let ext = path.extension().and_then(|v| v.to_str());
        if ext != Some("ppm") {
            continue;
        }
        let Some(page_number) = path
            .file_stem()
            .and_then(|v| v.to_str())
            .and_then(page_number_from_stem)
        else {
            continue;
        };
        ppm_files.push((page_number, path));
    }

    ppm_files.sort_by_key(|(n, _)| *n);

    let mut pages = Vec::with_capacity(ppm_files.len());
    for (page_number, ppm_path) in ppm_files {
        let jpg_path = ppm_path.with_extension("jpg");
        resize_and_save(&ppm_path, &jpg_path, 1800, 85)?;
        // Remove the large PPM source to save disk space
        let _ = fs::remove_file(&ppm_path);
        pages.push((page_number, jpg_path));
    }

    Ok(pages)
}

fn page_number_from_stem(stem: &str) -> Option<usize> {
    let (_, suffix) = stem.rsplit_once('-')?;
    suffix.parse().ok()
}

fn unique_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
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
    fn prepares_sample_image() {
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
            Err(DocumentError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("prepare image failed: {err}"),
        }
    }
}
