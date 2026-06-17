//! Font loading and text rendering.
//!
//! Supports two font backends:
//! - **TrueTypeFont** — uses fontdue (pure-Rust) for TrueType/OpenType font rendering
//! - **BitmapFont** — uses pre-rendered glyphs from PIL's default font for exact parity
//!
//! Both implement the same text rendering interface.

use std::sync::Arc;

use crate::bitmap_font::BitmapFont;
use crate::error::PilError;

/// A loaded font that can render text to bitmaps.
pub enum Font {
    /// TrueType/OpenType font rendered via fontdue.
    TrueType(TrueTypeFont),
    /// Pre-rendered bitmap font matching PIL's default font exactly.
    Bitmap(BitmapFont),
}

/// A TrueType font loaded via fontdue (pure-Rust FreeType equivalent).
pub struct TrueTypeFont {
    inner: Arc<fontdue::Font>,
    size: f32,
}

impl Font {
    /// Load a TrueType font from raw bytes at a given point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let settings = fontdue::FontSettings {
            collection_index: 0,
            scale: size,
            load_substitutions: true,
        };
        let inner = fontdue::Font::from_bytes(data, settings)
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
                let mut w = 0f32;
                for ch in text.chars() {
                    let (metrics, _bitmap) = ttf.inner.rasterize(ch, ttf.size);
                    w += metrics.advance_width;
                }
                (w.round() as u32, ttf.size.round() as u32)
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
                let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
                let mut total_w = 0f32;
                let mut max_h = 0u32;
                for ch in text.chars() {
                    let (metrics, bitmap) = ttf.inner.rasterize(ch, ttf.size);
                    total_w += metrics.advance_width;
                    max_h = max_h.max(metrics.height as u32);
                    glyphs.push((metrics, bitmap));
                }
                let w = total_w.round() as u32;
                let h = max_h;
                if w == 0 || h == 0 {
                    return (w, h, vec![]);
                }
                let mut canvas = vec![0u8; (w * h) as usize];
                let mut xo = 0i32;
                for (metrics, data) in &glyphs {
                    let dx = (xo as i64 + metrics.xmin as i64)
                        .max(0)
                        .min(u32::MAX as i64) as u32;
                    for gy in 0..metrics.height {
                        for gx in 0..metrics.width {
                            let a = data[gy * metrics.width + gx];
                            if a > 0 {
                                let cx = dx + gx as u32;
                                let cy = gy as u32;
                                if cx < w && cy < h {
                                    let d = (cy * w + cx) as usize;
                                    canvas[d] = canvas[d].max(a);
                                }
                            }
                        }
                    }
                    xo = xo.saturating_add(metrics.advance_width as i32);
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
        spacing: f32,
        binary: bool,
    ) -> (u32, u32, Vec<u8>) {
        match self {
            Font::TrueType(ttf) => {
                if text.is_empty() {
                    return (0, 0, vec![]);
                }

                // Layout: gather all glyphs with positions
                let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
                let mut total_w = 0f32;
                let mut max_h = 0u32;

                for ch in text.chars() {
                    let (metrics, bitmap) = ttf.inner.rasterize(ch, ttf.size);
                    total_w += metrics.advance_width;
                    max_h = max_h.max(metrics.height as u32);
                    // Convert coverage bitmap to RGBA
                    let mut rgba = vec![0u8; metrics.width * metrics.height * 4];
                    for y in 0..metrics.height {
                        for x in 0..metrics.width {
                            let cov = bitmap[y * metrics.width + x];
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
                                let off = (y * metrics.width + x) * 4;
                                let alpha = (effective_cov as u16 * fill.3 as u16 / 255) as u8;
                                if alpha > 0 {
                                    rgba[off] = fill.0;
                                    rgba[off + 1] = fill.1;
                                    rgba[off + 2] = fill.2;
                                    rgba[off + 3] = alpha;
                                }
                            }
                        }
                    }
                    glyphs.push((metrics, rgba));
                }

                let w = total_w.round() as u32;
                let h = max_h;
                if w == 0 || h == 0 {
                    return (w, h, vec![]);
                }

                // Compose glyphs onto a single RGBA canvas
                let mut canvas = vec![0u8; (w * h * 4) as usize];
                let mut x_offset = 0i32;

                for (metrics, rgba) in &glyphs {
                    let gw = metrics.width as u32;
                    let gh = metrics.height as u32;
                    let dx = (x_offset as i64 + metrics.xmin as i64)
                        .max(0)
                        .min(u32::MAX as i64) as u32;
                    let dy = (-(metrics.ymin as i64)).max(0).min(u32::MAX as i64) as u32;

                    for gy in 0..gh {
                        for gx in 0..gw {
                            let idx = (gy * gw + gx) as usize * 4;
                            if idx + 3 < rgba.len() && rgba[idx + 3] > 0 {
                                let cx = dx + gx;
                                let cy = dy + gy;
                                if cx < w && cy < h {
                                    let dst_off = ((cy * w + cx) * 4) as usize;
                                    if dst_off + 3 < canvas.len() {
                                        let sa = rgba[idx + 3] as u16;
                                        let da = canvas[dst_off + 3] as u16;
                                        let out_a = if binary {
                                            sa.max(da as u16)
                                        } else {
                                            sa + da * (255 - sa) / 255
                                        };
                                        if out_a > 0 {
                                            if binary {
                                                canvas[dst_off] = rgba[idx];
                                                canvas[dst_off + 1] = rgba[idx + 1];
                                                canvas[dst_off + 2] = rgba[idx + 2];
                                                canvas[dst_off + 3] = out_a as u8;
                                            } else {
                                                canvas[dst_off] = ((rgba[idx] as u16 * sa
                                                    + canvas[dst_off] as u16 * da * (255 - sa)
                                                        / 255)
                                                    / out_a)
                                                    as u8;
                                                canvas[dst_off + 1] = ((rgba[idx + 1] as u16 * sa
                                                    + canvas[dst_off + 1] as u16 * da * (255 - sa)
                                                        / 255)
                                                    / out_a)
                                                    as u8;
                                                canvas[dst_off + 2] = ((rgba[idx + 2] as u16 * sa
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
                    x_offset = x_offset.saturating_add(metrics.advance_width as i32);
                }

                (w, h, canvas)
            }
            Font::Bitmap(bf) => {
                if binary {
                    bf.render_text_binary(text, fill, spacing)
                } else {
                    bf.render_text(text, fill, spacing)
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
