//! Font loading and text rendering.
//!
//! Supports two font backends:
//! - **TrueTypeFont** — uses pillow-rs-freetype (pure-Rust FreeType compatible) for font rendering
//! - **BitmapFont** — uses pre-rendered glyphs from PIL's default font for exact parity
//!
//! Both implement the same text rendering interface.

use std::rc::Rc;

use crate::bitmap_font::BitmapFont;
use crate::error::PilError;

/// PIL `_imagingft.c` integration adapter.
pub mod imagingft;

/// A loaded font that can render text to bitmaps.
pub enum Font {
    /// TrueType/OpenType font rendered via pillow-rs-freetype (pure-Rust FreeType-compatible).
    TrueType(TrueTypeFont),
    /// Pre-rendered bitmap font matching PIL's default font exactly.
    Bitmap(BitmapFont),
}

/// A TrueType font loaded via pillow-rs-freetype (pure-Rust FreeType equivalent).
pub struct TrueTypeFont {
    inner: Rc<pillow_rs_freetype::Font>,
    size: f32,
}

impl Font {
    /// Load a TrueType font from raw bytes at a given point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let inner =
            pillow_rs_freetype::Font::truetype(&data, size, pillow_rs_freetype::BitmapBackend::PIL)
                .map_err(|e| PilError::ValueError(format!("Failed to load font: {}", e)))?;
        Ok(Font::TrueType(TrueTypeFont {
            inner: Rc::new(inner),
            size,
        }))
    }

    /// Create a default bitmap font matching PIL's `load_default()`.
    pub fn load_default(size: f32) -> Self {
        Font::Bitmap(BitmapFont::new(size))
    }

    /// Get font size in pixels.
    pub fn font_size(&self) -> f32 {
        match self {
            Font::TrueType(ttf) => ttf.size,
            Font::Bitmap(bf) => bf.font_size(),
        }
    }

    /// Compute the bounding box of a text string.
    /// Returns (width, height).
    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let bbox = imagingft::getbbox(self, text);
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        (w, h)
    }

    /// Render text as an L-mode alpha mask.
    ///
    /// This is the public font-object surface, matching Pillow's
    /// `ImageFont.getmask`/`FreeTypeFont.getmask`. The `_imagingft`-style
    /// adapter remains an implementation detail behind this method.
    pub fn getmask(&self, text: &str) -> (u32, u32, Vec<u8>) {
        imagingft::getmask(self, text)
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Font::TrueType(_) => write!(f, "Font::TrueType({}px)", self.font_size()),
            Font::Bitmap(_) => write!(f, "Font::Bitmap({}px)", self.font_size()),
        }
    }
}
