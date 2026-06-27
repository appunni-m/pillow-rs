//! Font loading and text rendering.
//!
//! Supports two font backends:
//! - **TrueTypeFont** — uses pillow-rs-font (pure-Rust FreeType compatible) for font rendering
//! - **BitmapFont** — uses pre-rendered glyphs from PIL's default font for exact parity
//!
//! Both implement the same text rendering interface.

use std::sync::Arc;

use crate::bitmap_font::BitmapFont;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;

/// A loaded font that can render text to bitmaps.
pub enum Font {
    /// TrueType/OpenType font rendered via pillow-rs-font (pure-Rust FreeType-compatible).
    TrueType(TrueTypeFont),
    /// Pre-rendered bitmap font matching PIL's default font exactly.
    Bitmap(BitmapFont),
}

/// A TrueType font loaded via pillow-rs-font (pure-Rust FreeType equivalent).
pub struct TrueTypeFont {
    inner: Arc<pillow_rs_font::Font>,
    size: f32,
}

impl Font {
    /// Load a TrueType font from raw bytes at a given point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let inner = pillow_rs_font::Font::truetype(&data, size, pillow_rs_font::BitmapBackend::PIL)
            .map_err(|e| PilError::ValueError(format!("Failed to load font: {}", e)))?;
        Ok(Font::TrueType(TrueTypeFont {
            inner: Arc::new(inner),
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
        match self {
            Font::TrueType(ttf) => {
                let bbox = ttf.inner.getbbox(text);
                let w = (bbox.2 - bbox.0).max(0) as u32;
                let h = (bbox.3 - bbox.1).max(0) as u32;
                (w, h)
            }
            Font::Bitmap(bf) => bf.text_bbox(text),
        }
    }

    /// Render text as an alpha mask (L-mode). PIL: getmask().
    pub fn getmask(&self, text: &str) -> (u32, u32, Vec<u8>) {
        match self {
            Font::TrueType(ttf) => {
                if text.is_empty() {
                    return (0, 0, vec![]);
                }

                let mut total_w = 0f32;
                let mut total_h = 0u32;
                let mut glyphs: Vec<pillow_rs_font::GlyphMask> = Vec::new();

                for ch in text.chars() {
                    // Render each character individually to build the composite canvas
                    match ttf.inner.getmask(&ch.to_string()) {
                        Ok(mask) => {
                            total_w += mask.advance_width as f32;
                            total_h = total_h.max(mask.height);
                            glyphs.push(mask);
                        }
                        Err(_) => continue,
                    }
                }

                let w = total_w.round() as u32;
                let h = total_h;
                if w == 0 || h == 0 {
                    return (w, h, vec![]);
                }

                let mut canvas = match CheckedDims::new(w, h, 1) {
                    Ok(dims) => dims.alloc_buffer(),
                    Err(_) => return (0, 0, vec![]),
                };

                let mut xo = 0i32;
                for mask in &glyphs {
                    let dx = (xo as i64 + mask.xmin as i64).max(0).min(u32::MAX as i64) as u32;
                    for gy in 0..mask.height {
                        for gx in 0..mask.width {
                            let a = mask.pixels[(gy * mask.width + gx) as usize];
                            if a > 0 {
                                let cx = dx + gx;
                                let cy = gy;
                                if cx < w && cy < h {
                                    let d = (cy * w + cx) as usize;
                                    canvas[d] = canvas[d].max(a);
                                }
                            }
                        }
                    }
                    xo = xo.saturating_add(mask.advance_width as i32);
                }
                (w, h, canvas)
            }
            Font::Bitmap(bf) => bf.getmask(text),
        }
    }

    /// Render text to an RGBA image. Returns (width, height, pixel_data).
    pub fn render_text(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        spacing: f32,
    ) -> (u32, u32, Vec<u8>) {
        self.render_text_impl(text, fill, spacing, false)
    }

    /// Render text in binary mode (fontmode="1").
    /// Coverage values are thresholded at 128: >= 128 → 255, < 128 → 0.
    /// This matches PIL's FT_LOAD_TARGET_MONO behavior for modes "1", "P", "I", "F".
    pub fn render_text_binary(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        spacing: f32,
    ) -> (u32, u32, Vec<u8>) {
        self.render_text_impl(text, fill, spacing, true)
    }

    fn render_text_impl(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        _spacing: f32,
        binary: bool,
    ) -> (u32, u32, Vec<u8>) {
        match self {
            Font::TrueType(ttf) => {
                if text.is_empty() {
                    return (0, 0, vec![]);
                }

                // Layout: gather all glyphs with positions
                let mut glyphs: Vec<pillow_rs_font::GlyphMask> = Vec::new();
                let mut total_w = 0f32;
                let mut total_h = 0u32;

                for ch in text.chars() {
                    match ttf.inner.getmask(&ch.to_string()) {
                        Ok(mask) => {
                            total_w += mask.advance_width as f32;
                            total_h = total_h.max(mask.height);
                            glyphs.push(mask);
                        }
                        Err(_) => continue,
                    }
                }

                let w = total_w.round() as u32;
                let h = total_h;
                if w == 0 || h == 0 {
                    return (w, h, vec![]);
                }

                // Compose glyphs onto a single RGBA canvas
                let mut canvas = match CheckedDims::new(w, h, 4) {
                    Ok(dims) => dims.alloc_buffer(),
                    Err(_) => return (0, 0, vec![]),
                };
                let mut x_offset = 0i32;

                for mask in &glyphs {
                    let gw = mask.width;
                    let gh = mask.height;
                    let dx = (x_offset as i64 + mask.xmin as i64)
                        .max(0)
                        .min(u32::MAX as i64) as u32;
                    let dy = (-(mask.ymin as i64)).max(0).min(u32::MAX as i64) as u32;

                    for gy in 0..gh {
                        for gx in 0..gw {
                            let cov = mask.pixels[(gy * gw + gx) as usize];
                            let effective_cov = if binary {
                                if cov >= 128 {
                                    255u8
                                } else {
                                    0u8
                                }
                            } else {
                                cov
                            };
                            if effective_cov > 0 {
                                let cx = dx + gx;
                                let cy = dy + gy;
                                if cx < w && cy < h {
                                    let dst_off = ((cy * w + cx) * 4) as usize;
                                    if dst_off + 3 < canvas.len() {
                                        let sa = effective_cov as u16;
                                        let da = canvas[dst_off + 3] as u16;

                                        if binary {
                                            // Binary mode: max alpha
                                            let out_a = sa.max(da);
                                            if out_a > 0 {
                                                canvas[dst_off] = fill.0;
                                                canvas[dst_off + 1] = fill.1;
                                                canvas[dst_off + 2] = fill.2;
                                                canvas[dst_off + 3] = out_a as u8;
                                            }
                                        } else {
                                            // Normal blend: over compositing
                                            let out_a = sa + da * (255 - sa) / 255;
                                            if out_a > 0 {
                                                canvas[dst_off] = ((fill.0 as u16 * sa
                                                    + canvas[dst_off] as u16 * da * (255 - sa)
                                                        / 255)
                                                    / out_a)
                                                    as u8;
                                                canvas[dst_off + 1] = ((fill.1 as u16 * sa
                                                    + canvas[dst_off + 1] as u16 * da * (255 - sa)
                                                        / 255)
                                                    / out_a)
                                                    as u8;
                                                canvas[dst_off + 2] = ((fill.2 as u16 * sa
                                                    + canvas[dst_off + 2] as u16 * da * (255 - sa)
                                                        / 255)
                                                    / out_a)
                                                    as u8;
                                                canvas[dst_off + 3] = out_a as u8;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    x_offset = x_offset.saturating_add(mask.advance_width as i32);
                }

                (w, h, canvas)
            }
            Font::Bitmap(bf) => {
                if binary {
                    bf.render_text_binary(text, fill, _spacing)
                } else {
                    bf.render_text(text, fill, _spacing)
                }
            }
        }
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
