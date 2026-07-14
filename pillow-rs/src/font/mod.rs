//! Pillow-compatible font loading and text rendering.
//!
//! TrueType loading, metadata, glyph rendering, advance, and kerning go
//! through the `_imagingft` adapter so this public module does not expose
//! FreeType-core helper APIs.

use crate::bitmap_font::BitmapFont;
use crate::error::PilError;

pub mod imagingft;

pub enum Font {
    TrueType(TrueTypeFont),
    Bitmap(BitmapFont),
}

pub struct TrueTypeFont {
    engine: imagingft::TrueTypeEngine,
}

impl Font {
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        imagingft::load_truetype(data, size).map(Font::TrueType)
    }

    pub fn load_default(size: f32) -> Self {
        Font::Bitmap(BitmapFont::new(size))
    }

    pub fn font_size(&self) -> f32 {
        match self {
            Font::TrueType(ttf) => ttf.engine.size_pt,
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
