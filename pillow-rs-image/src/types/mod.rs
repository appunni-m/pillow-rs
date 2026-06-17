//! The pillow-rs-image type system.
//!
//! This module provides the core image types matching the `image` crate's API,
//! allowing `pillow-rs` to swap `use image::*` for `use pillow_rs_image::*`.

pub mod buffer;
pub mod color;
pub mod dynamic;
pub mod error;
pub mod traits;

// Re-exports matching the `image` crate's top-level API.
pub use self::buffer::{
    ConvertBuffer,
    // Iterators
    EnumeratePixels,
    EnumeratePixelsMut,
    EnumerateRows,
    EnumerateRowsMut,
    GrayAlphaImage,
    GrayImage,
    ImageBuffer,
    Pixels,
    PixelsMut,
    Rgb32FImage,
    RgbImage,
    Rgba32FImage,
    RgbaImage,
    Rows,
    RowsMut,
};
pub use self::color::{
    ColorType, ExtendedColorType, FromColor, FromPrimitive, Luma, LumaA, Rgb, Rgba,
};
pub use self::dynamic::DynamicImage;
pub use self::error::{ImageError, ImageResult, Rect};
pub use self::traits::{
    EncodableLayout, Enlargeable, GenericImage, GenericImageView, Pixel, Primitive,
};

// ---------------------------------------------------------------------------
// ImageFormat — supported encoding/decoding formats
// ---------------------------------------------------------------------------

/// Supported image formats for encoding and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// JPEG
    Jpeg,
    /// PNG
    Png,
    /// GIF
    Gif,
    /// BMP
    Bmp,
    /// WebP
    WebP,
    /// TIFF
    Tiff,
    /// ICO
    Ico,
    /// AVIF
    Avif,
}

impl ImageFormat {
    /// Attempt to detect the image format from a file path extension.
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<ImageFormat, ImageError> {
        let ext = path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "png" => Ok(ImageFormat::Png),
            "gif" => Ok(ImageFormat::Gif),
            "bmp" => Ok(ImageFormat::Bmp),
            "webp" => Ok(ImageFormat::WebP),
            "tiff" | "tif" => Ok(ImageFormat::Tiff),
            "ico" => Ok(ImageFormat::Ico),
            "avif" => Ok(ImageFormat::Avif),
            _ => Err(ImageError::Unsupported(format!(
                "unknown extension: {}",
                ext
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// DecodedImage — raw decoded pixel buffer
// ---------------------------------------------------------------------------

/// Raw decoded pixel buffer produced by decoders and consumed by encoders.
///
/// This is a format-agnostic representation carrying the pixel data in a flat
/// `Vec<u8>`. The `color` field indicates the channel layout:
///   - `L8`:     1 byte/pixel (grayscale)
///   - `La8`:    2 bytes/pixel (L, A)
///   - `Rgb8`:   3 bytes/pixel (R, G, B)
///   - `Rgba8`:  4 bytes/pixel (R, G, B, A)
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Flat pixel data. Layout depends on `color`.
    pub pixels: Vec<u8>,
    /// Number of color channels.
    pub color: ColorType,
}

impl DecodedImage {
    /// Create a new decoded image.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, color: ColorType) -> Self {
        Self {
            width,
            height,
            pixels,
            color,
        }
    }

    /// Return raw pixel bytes for comparison against PIL reference.
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }
}
