//! Pillow-compatible font loading and text rendering.
//!
//! TrueType rendering uses `fontdone::ffi` (FreeType FFI facade —
//! proven pixel-identical parity with C FreeType 2.14.3: 4,097/4,097).
//! The compact `fontdone::Font` API is used for getname() and getmetrics()
//! metadata; all glyph loading, rendering, advance, and kerning go through
//! the FFI facade.

use crate::bitmap_font::BitmapFont;
use crate::error::PilError;

pub mod imagingft;

pub enum Font {
    TrueType(TrueTypeFont),
    Bitmap(BitmapFont),
}

pub struct TrueTypeFont {
    /// Compact API handle — used for getname().
    pub inner: fontdone::Font,
    /// FFI facade face — used for pixel-identical rendering.
    pub face: fontdone::ffi::FT_Face,
    /// Ascender in 26.6 fixed point from FT_Size_Metrics.
    pub ascender_26dot6: i64,
    pub library: fontdone::ffi::FT_Library,
}

impl Font {
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let compact = fontdone::Font::truetype(&data, size)
            .map_err(|e| PilError::ValueError(format!("Failed to load font: {}", e)))?;

        let library = fontdone::ffi::FT_Init_FreeType();
        let mut face = fontdone::ffi::FT_New_Memory_Face(&library, &data, 0, size)
            .map_err(|e| PilError::ValueError(format!("FT_New_Memory_Face: error {e}")))?;

        let pp = size as u32;
        if fontdone::ffi::FT_Set_Pixel_Sizes(&mut face, pp, pp) != 0 {
            return Err(PilError::ValueError("FT_Set_Pixel_Sizes failed".into()));
        }

        let m = fontdone::ffi::FT_Size_Metrics(&face);
        Ok(Font::TrueType(TrueTypeFont {
            inner: compact,
            face,
            ascender_26dot6: m.ascender,
            library,
        }))
    }

    pub fn load_default(size: f32) -> Self {
        Font::Bitmap(BitmapFont::new(size))
    }

    pub fn font_size(&self) -> f32 {
        match self {
            Font::TrueType(ttf) => ttf.inner.size_pt,
            Font::Bitmap(bf) => bf.font_size(),
        }
    }

    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let bbox = imagingft::getbbox(self, text);
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        (w, h)
    }

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
