//! Bitmap font renderer for Pillow's default font.
//!
//! This module embeds all 95 ASCII printable characters pre-rendered using
//! PIL's default font (Aileron, size 10) via FreeType. By using the exact
//! same pixel data as PIL, we achieve pixel-for-pixel parity in text rendering
//! without needing to link FreeType or deal with font rasterization differences.

mod data;
use data::BITMAP_GLYPH_DATA;
use data::BITMAP_GLYPH_DATA_BINARY;

use crate::checked_dims::CheckedDims;

/// Pre-rendered bitmap font matching Pillow `ImageFont.load_default`.
#[derive(Debug, Clone)]
pub struct BitmapFont {
    size: f32,
}

impl BitmapFont {
    /// Creates a bitmap font handle with the requested display size.
    ///
    /// The bundled glyph atlas is fixed; `size` is carried for API compatibility
    /// and caller-visible font metadata.
    pub fn new(size: f32) -> Self {
        BitmapFont { size }
    }

    /// Returns the configured font size.
    pub fn font_size(&self) -> f32 {
        self.size
    }

    /// Returns the `(width, height)` of `text` in pixels.
    ///
    /// Height includes the glyph y-offset used by Pillow's default font.
    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let mut w = 0u32;
        let mut max_ymax = 0i32;
        let mut min_ymin = i32::MAX;
        for ch in text.chars() {
            let idx = (ch as u8).wrapping_sub(32) as usize;
            if idx < 95 {
                let (gw, gh, ymin, _) = BITMAP_GLYPH_DATA[idx];
                w += gw;
                if ymin < min_ymin {
                    min_ymin = ymin;
                }
                let ymax = ymin + gh as i32;
                if ymax > max_ymax {
                    max_ymax = ymax;
                }
            } else {
                w += 2;
            }
        }
        if max_ymax <= 0 || min_ymin == i32::MAX {
            return (w, 0);
        }
        (w, (max_ymax - min_ymin) as u32)
    }

    /// Renders text as an `L`-mode alpha mask.
    ///
    /// Returns `(width, height, mask_data)`. The mask is pre-shifted so that
    /// placing it at (text_x, text_y) yields the same positioning as PIL.
    /// Row 0 of the mask corresponds to font coordinate y = min_ymin,
    /// which means the tallest glyph's top is at text_y + min_ymin on the image.
    pub fn getmask(&self, text: &str) -> (u32, u32, Vec<u8>) {
        if text.is_empty() {
            return (0, 0, vec![]);
        }

        // Compute layout metrics
        let mut glyphs: Vec<(u32, u32, i32, &[u8])> = Vec::new();
        let mut total_w = 0u32;
        let mut max_ymax = 0i32;

        for ch in text.chars() {
            let idx = (ch as u8).wrapping_sub(32) as usize;
            if idx < 95 {
                let (gw, gh, ymin, data) = BITMAP_GLYPH_DATA[idx];
                total_w += gw;
                let ymax = ymin + gh as i32;
                if ymax > max_ymax {
                    max_ymax = ymax;
                }
                glyphs.push((gw, gh, ymin, data));
            } else {
                total_w += 2;
            }
        }

        if total_w == 0 || max_ymax <= 0 {
            return (total_w, 0, vec![]);
        }

        // The mask spans from font y=0 to y=max_ymax.
        // This includes blank rows at the top (the font's y-offset).
        let line_h = max_ymax as u32;

        // Allocate canvas
        let mut canvas = match CheckedDims::new(total_w, line_h, 1) {
            Ok(dims) => dims.alloc_buffer(),
            Err(_) => return (total_w, 0, vec![]),
        };
        let mut cx = 0u32;

        for &(gw, gh, ymin, data) in &glyphs {
            // Each glyph starts at row = ymin in the canvas
            // (ymin blank rows at the top, then the glyph data)
            let gy_offset = ymin as u32;
            for dy in 0..gh {
                for dx in 0..gw {
                    let src_idx = (dy * gw + dx) as usize;
                    if src_idx < data.len() {
                        let alpha = data[src_idx];
                        if alpha > 0 {
                            let canvas_x = cx + dx;
                            let canvas_y = gy_offset + dy;
                            if canvas_x < total_w && canvas_y < line_h {
                                let dst_idx = (canvas_y * total_w + canvas_x) as usize;
                                canvas[dst_idx] = canvas[dst_idx].max(alpha);
                            }
                        }
                    }
                }
            }
            cx += gw;
        }

        (total_w, line_h, canvas)
    }

    /// Render text to an RGBA image. Returns (width, height, pixel_data).
    pub fn render_text(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        _spacing: f32,
    ) -> (u32, u32, Vec<u8>) {
        self.render_text_impl(text, fill, false)
    }

    /// Render text in binary mode (fontmode="1").
    /// Coverage values are thresholded at 128: >= 128 → 255, < 128 → 0.
    pub fn render_text_binary(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        _spacing: f32,
    ) -> (u32, u32, Vec<u8>) {
        self.render_text_impl(text, fill, true)
    }

    fn render_text_impl(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        binary: bool,
    ) -> (u32, u32, Vec<u8>) {
        if text.is_empty() {
            return (0, 0, vec![]);
        }

        let (w, h, mask) = if binary {
            self.getmask_binary(text)
        } else {
            self.getmask(text)
        };
        if w == 0 || h == 0 {
            return (w, h, vec![]);
        }

        // Convert mask (alpha values) to RGBA using fill color
        let mut pixels = match CheckedDims::new(w, h, 4) {
            Ok(dims) => dims.alloc_buffer(),
            Err(_) => return (w, h, vec![]),
        };
        for y in 0..h {
            for x in 0..w {
                let alpha = mask[(y * w + x) as usize];
                if alpha > 0 {
                    let off = ((y * w + x) * 4) as usize;
                    let a = (alpha as u16 * fill.3 as u16 / 255) as u8;
                    if a > 0 {
                        pixels[off] = fill.0;
                        pixels[off + 1] = fill.1;
                        pixels[off + 2] = fill.2;
                        pixels[off + 3] = a;
                    }
                }
            }
        }

        (w, h, pixels)
    }

    /// Render text mask in binary mode using PIL's exact monochrome glyph data.
    /// Returns (width, height, mask_data) with values 0 or 255.
    fn getmask_binary(&self, text: &str) -> (u32, u32, Vec<u8>) {
        if text.is_empty() {
            return (0, 0, vec![]);
        }

        // Compute layout using binary glyph metrics
        let mut glyphs: Vec<(u32, u32, i32, &[u8])> = Vec::new();
        let mut total_w = 0u32;
        let mut max_ymax = 0i32;

        for ch in text.chars() {
            let idx = (ch as u8).wrapping_sub(32) as usize;
            if idx < 95 {
                let (gw, gh, ymin, data) = BITMAP_GLYPH_DATA_BINARY[idx];
                total_w += gw;
                let ymax = ymin + gh as i32;
                if ymax > max_ymax {
                    max_ymax = ymax;
                }
                glyphs.push((gw, gh, ymin, data));
            } else {
                total_w += 2;
            }
        }

        if total_w == 0 || max_ymax <= 0 {
            return (total_w, 0, vec![]);
        }

        let line_h = max_ymax as u32;
        let mut canvas = match CheckedDims::new(total_w, line_h, 1) {
            Ok(dims) => dims.alloc_buffer(),
            Err(_) => return (total_w, 0, vec![]),
        };
        let mut cx = 0u32;

        for &(gw, gh, ymin, data) in &glyphs {
            let gy_offset = ymin as u32;
            for dy in 0..gh {
                for dx in 0..gw {
                    let src_idx = (dy * gw + dx) as usize;
                    if src_idx < data.len() {
                        let alpha = data[src_idx];
                        if alpha > 0 {
                            let canvas_x = cx + dx;
                            let canvas_y = gy_offset + dy;
                            if canvas_x < total_w && canvas_y < line_h {
                                let dst_idx = (canvas_y * total_w + canvas_x) as usize;
                                canvas[dst_idx] = canvas[dst_idx].max(alpha);
                            }
                        }
                    }
                }
            }
            cx += gw;
        }

        (total_w, line_h, canvas)
    }
}

/// Compute default bitmap font bounding box.
/// Uses fixed 6 px width and 11 px height per character (PIL default font).
pub fn font_default_bbox(text: &str) -> (i32, i32, i32, i32) {
    let lines: Vec<&str> = text.split('\n').collect();
    let max_width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i32 * 6;
    let total_height = lines.len() as i32 * 11;
    (0, 0, max_width, total_height)
}

/// Compute default bitmap font text length (width in pixels).
pub fn font_default_length(text: &str) -> u32 {
    let lines: Vec<&str> = text.split('\n').collect();
    let max_line = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    max_line as u32 * 6
}

/// Compute default bitmap font mask size for fallback blank mask.
/// Returns (width, height) with minimum of 1 for each dimension.
pub fn font_default_mask_size(text: &str) -> (u32, u32) {
    let w = font_default_length(text);
    let h = if text.contains('\n') {
        let line_count = text.chars().filter(|&c| c == '\n').count() + 1;
        (line_count * 11) as u32
    } else {
        11
    };
    (w.max(1), h.max(1))
}
