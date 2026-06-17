//! pillow-rs-decode — zero-dependency pixel-perfect image decoders.
//!
//! Goal: produce pixel-identical output to libjpeg/libpng so pillow-rs
//! parity tests pass. No external crates. Works on WASM.
//!
//! Architecture:
//!   &[u8] → Decoder::decode() → DecodedImage { width, height, pixels, color }
//!   pillow-rs-core wraps DecodedImage into DynamicImage/Image::Loaded.

/// Raw decoded pixel buffer — no dependencies on any image crate.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Flat pixel data. Layout depends on `color`:
    ///   Luma8:    1 byte/pixel
    ///   LumaA8:   2 bytes/pixel (L, A)
    ///   Rgb8:     3 bytes/pixel (R, G, B)
    ///   Rgba8:    4 bytes/pixel (R, G, B, A)
    pub pixels: Vec<u8>,
    /// Number of color channels
    pub color: ColorType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorType {
    Luma8,   // 1 channel — grayscale
    LumaA8,  // 2 channels — grayscale + alpha
    Rgb8,    // 3 channels — RGB
    Rgba8,   // 4 channels — RGBA
}

impl DecodedImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, color: ColorType) -> Self {
        Self { width, height, pixels, color }
    }
}

/// Detect image format from magic bytes.
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() < 8 { return None; }
    if data[0] == 0xFF && data[1] == 0xD8 { return Some(ImageFormat::Jpeg); }
    if &data[0..8] == b"\x89PNG\r\n\x1a\n" { return Some(ImageFormat::Png); }
    if &data[0..4] == b"GIF8" { return Some(ImageFormat::Gif); }
    if &data[0..2] == b"BM" { return Some(ImageFormat::Bmp); }
    if data.len() >= 12 && &data[8..12] == b"WEBP" { return Some(ImageFormat::WebP); }
    if &data[0..4] == b"II\x2a\x00" || &data[0..4] == b"MM\x00\x2a" { return Some(ImageFormat::Tiff); }
    if &data[0..4] == b"\x00\x00\x01\x00" { return Some(ImageFormat::Ico); }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Bmp,
    WebP,
    Tiff,
    Ico,
}

/// Decode any supported format. Returns None if format is unrecognized.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let format = detect_format(data)?;
    match format {
        ImageFormat::Jpeg => jpeg::decode(data),
        ImageFormat::Png  => png::decode(data),
        ImageFormat::Gif  => gif::decode(data),
        ImageFormat::Bmp  => bmp::decode(data),
        ImageFormat::WebP => webp::decode(data),
        ImageFormat::Tiff => tiff::decode(data),
        ImageFormat::Ico  => ico::decode(data),
    }
}

// Sub-modules — each implements decode(&[u8]) -> Option<DecodedImage>
pub mod jpeg;
pub mod png;
pub mod gif;
pub mod bmp;
pub mod webp;
pub mod tiff;
pub mod ico;
