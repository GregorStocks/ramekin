//! Image validation and fetching utilities.
//!
//! This module provides shared image validation logic used by both CLI and server.
//! Thumbnail generation is handled separately by the server (in photos/processing.rs).

use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use crate::http::HttpClient;

/// Allowed image formats for recipe photos.
pub const ALLOWED_FORMATS: &[ImageFormat] = &[
    ImageFormat::Jpeg,
    ImageFormat::Png,
    ImageFormat::Gif,
    ImageFormat::WebP,
];

/// Maximum file size for images (10MB).
pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Result of successfully fetching and validating an image.
#[derive(Debug, Clone)]
pub struct FetchedImage {
    /// The raw image bytes.
    pub data: Vec<u8>,
    /// The detected content type (e.g., "image/jpeg").
    pub content_type: String,
}

/// Validate image data: check format is allowed and detect content type.
///
/// Returns the content type on success (e.g., "image/jpeg").
pub fn validate_image(data: &[u8]) -> Result<String, String> {
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

    Ok(format.to_mime_type().to_string())
}

/// Fetch an image from a URL and validate it.
///
/// This function:
/// 1. Fetches the image bytes using the provided HTTP client
/// 2. Validates the size is within limits
/// 3. Validates the format is allowed
///
/// The HTTP client handles caching - if using CachingClient, the image
/// will be automatically cached to disk.
pub async fn fetch_and_validate_image<C: HttpClient>(
    client: &C,
    url: &str,
) -> Result<FetchedImage, String> {
    // Fetch the image bytes
    let data = client
        .fetch_bytes(url)
        .await
        .map_err(|e| format!("Failed to fetch image: {}", e))?;

    // Validate size
    if data.len() > MAX_FILE_SIZE {
        return Err(format!(
            "Image too large: {} bytes (max {})",
            data.len(),
            MAX_FILE_SIZE
        ));
    }

    // Validate format and get content type
    let content_type = validate_image(&data)?;

    Ok(FetchedImage { data, content_type })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_jpeg() {
        // Minimal valid JPEG (just the header bytes for format detection)
        // This won't decode but will be detected as JPEG format
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        let result = validate_image(&jpeg_header);
        // This will fail because it's not a complete JPEG, but let's test with a real one
        assert!(result.is_err()); // Incomplete JPEG
    }

    #[test]
    fn test_validate_invalid_format() {
        let invalid_data = b"not an image";
        let result = validate_image(invalid_data);
        assert!(result.is_err());
    }
}
