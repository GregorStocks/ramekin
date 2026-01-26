use std::io::Cursor;

use image::{ImageFormat, ImageReader};

// Re-export shared constants from ramekin-core for use by other server modules
pub use ramekin_core::image::{ALLOWED_FORMATS, MAX_FILE_SIZE};

pub const THUMBNAIL_SIZE: u32 = 200;

/// Process an image: detect format from magic bytes, validate it's allowed, and generate thumbnail.
/// Returns (content_type, thumbnail_bytes) on success.
pub fn process_image(data: &[u8]) -> Result<(String, Vec<u8>), String> {
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

    // thumbnail() preserves aspect ratio, fitting within the given dimensions
    let thumbnail_img = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);

    let mut thumbnail_buf = Cursor::new(Vec::new());
    thumbnail_img
        .write_to(&mut thumbnail_buf, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode thumbnail: {}", e))?;

    Ok((content_type, thumbnail_buf.into_inner()))
}
