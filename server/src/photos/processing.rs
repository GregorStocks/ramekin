use std::io::Cursor;

use image::{ImageFormat, ImageReader};

// Re-export shared constants from ramekin-core for use by other server modules
pub use ramekin_core::image::{ALLOWED_FORMATS, MAX_FILE_SIZE};

pub const THUMBNAIL_SIZE: u32 = 200;
pub const MAX_THUMBNAIL_SIZE: u32 = 800;
pub const EXPORT_PHOTO_DATA_SIZE: u32 = 280;

/// Max dimension (longest side) for photos embedded in paprikarecipes exports.
/// Originals can be up to MAX_FILE_SIZE (10MB) each; for exports we trade full
/// resolution for bounded memory/archive size.
pub const EXPORT_PHOTO_MAX_DIMENSION: u32 = 1600;

pub struct ProcessedImage {
    pub content_type: String,
    pub thumbnail: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Process an image: detect format from magic bytes, validate it's allowed, and generate thumbnail.
pub fn process_image(data: &[u8]) -> Result<ProcessedImage, String> {
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to read image: {}", e))?;

    let format = reader
        .format()
        .ok_or_else(|| "Could not detect image format".to_string())?;

    if !ALLOWED_FORMATS.contains(&format) {
        return Err(format!(
            "Unsupported image format: {:?}. Allowed: JPEG, PNG, GIF, WebP",
            format
        ));
    }

    let content_type = format.to_mime_type().to_string();

    let img = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let width = img.width();
    let height = img.height();

    // thumbnail() preserves aspect ratio, fitting within the given dimensions
    let thumbnail_img = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);

    let mut thumbnail_buf = Cursor::new(Vec::new());
    thumbnail_img
        .write_to(&mut thumbnail_buf, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode thumbnail: {}", e))?;

    Ok(ProcessedImage {
        content_type,
        thumbnail: thumbnail_buf.into_inner(),
        width,
        height,
    })
}

/// Generate a thumbnail at a specific size from raw image data.
/// Returns JPEG bytes.
pub fn generate_thumbnail(data: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let size = size.clamp(1, MAX_THUMBNAIL_SIZE);

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to read image: {}", e))?;

    let img = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let thumbnail_img = img.thumbnail(size, size);

    let mut buf = Cursor::new(Vec::new());
    thumbnail_img
        .write_to(&mut buf, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode thumbnail: {}", e))?;

    Ok(buf.into_inner())
}

/// Resize an image for inclusion in a paprikarecipes export.
///
/// Fits the image within `EXPORT_PHOTO_MAX_DIMENSION` on the longest side
/// (preserving aspect ratio) and re-encodes as JPEG. If the original is
/// already smaller, `thumbnail` is a no-op on dimensions but still re-encodes
/// as JPEG — this is fine and keeps the Paprika consumer side simple.
pub fn resize_for_export(data: &[u8]) -> Result<Vec<u8>, String> {
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to read image: {}", e))?;

    let img = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let resized = img.thumbnail(EXPORT_PHOTO_MAX_DIMENSION, EXPORT_PHOTO_MAX_DIMENSION);

    let mut buf = Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode export image: {}", e))?;

    Ok(buf.into_inner())
}
