//! Pillow-compatible font loading and text rendering.
//!
//! This module exposes the font surface used by drawing and binding crates.
//! TrueType/OpenType rendering is delegated to `pillow-rs-freetype`, a pure Rust
//! FreeType-compatible implementation. The default bitmap font uses pre-rendered
//! Pillow glyph data for exact `ImageFont.load_default()` behavior.
//!
//! Font APIs return Rust primitives: dimensions, bounding boxes, and mask bytes.
//! Binding crates translate those into host-language objects.

use std::rc::Rc;

use crate::bitmap_font::BitmapFont;
use crate::error::PilError;

/// PIL `_imagingft.c` integration adapter.
pub mod imagingft;

/// Loaded font source for Pillow-style text measurement and masks.
pub enum Font {
    /// TrueType/OpenType font rendered via pillow-rs-freetype (pure-Rust FreeType-compatible).
    TrueType(TrueTypeFont),
    /// Pre-rendered bitmap font matching PIL's default font exactly.
    Bitmap(BitmapFont),
}

/// TrueType/OpenType font loaded through `pillow-rs-freetype`.
pub struct TrueTypeFont {
    inner: Rc<pillow_rs_freetype::Font>,
    size: f32,
}

impl Font {
    /// Loads a TrueType/OpenType font from raw bytes.
    ///
    /// `size` is the requested point size used by the FreeType-compatible
    /// backend.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the font bytes cannot be parsed.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let inner = pillow_rs_freetype::Font::truetype(&data, size)
            .map_err(|e| PilError::ValueError(format!("Failed to load font: {}", e)))?;
        Ok(Font::TrueType(TrueTypeFont {
            inner: Rc::new(inner),
            size,
        }))
    }

    /// Creates the default bitmap font matching Pillow `ImageFont.load_default`.
    pub fn load_default(size: f32) -> Self {
        Font::Bitmap(BitmapFont::new(size))
    }

    /// Returns the configured font size.
    pub fn font_size(&self) -> f32 {
        match self {
            Font::TrueType(ttf) => ttf.size,
            Font::Bitmap(bf) => bf.font_size(),
        }
    }

    /// Returns the width and height of `text` in pixels.
    ///
    /// This convenience method collapses the `_imagingft` bbox into dimensions.
    /// Use [`crate::font::imagingft::getbbox`] when left/top offsets matter.
    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let bbox = imagingft::getbbox(self, text);
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        (w, h)
    }

    /// Renders text as an `L`-mode alpha mask.
    ///
    /// This is the public font-object surface, matching Pillow's
    /// `ImageFont.getmask`/`FreeTypeFont.getmask`. The `_imagingft`-style
    /// adapter remains an implementation detail behind this method.
    ///
    /// # Returns
    ///
    /// `(width, height, mask_bytes)` where `mask_bytes` contains one coverage
    /// byte per pixel.
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
