//! Font loading and text rendering using fontdue (pure-Rust FreeType equivalent).
//! Supports TrueType/OpenType fonts, works on WASM with zero C dependencies.

use std::sync::Arc;

/// A loaded font that can render text to bitmaps.
pub struct Font {
    inner: Arc<fontdue::Font>,
    size: f32,
}

impl Font {
    /// Load a font from raw bytes at a given point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, crate::error::PilError> {
        let settings = fontdue::FontSettings {
            collection_index: 0,
            scale: size,
            load_substitutions: true,
        };
        let inner = fontdue::Font::from_bytes(data, settings)
            .map_err(|e| crate::error::PilError::ValueError(format!("Failed to load font: {}", e)))?;
        Ok(Font { inner: Arc::new(inner), size })
    }

    /// Get font size in pixels.
    pub fn font_size(&self) -> f32 { self.size }

    /// Compute the bounding box of a text string.
    /// Returns (width, height).
    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let mut w = 0f32;
        for ch in text.chars() {
            let (metrics, _bitmap) = self.inner.rasterize(ch, self.size);
            w += metrics.advance_width;
        }
        (w.round() as u32, self.size.round() as u32)
    }

    /// Render text as an alpha mask (L-mode). PIL: getmask().
    pub fn getmask(&self, text: &str) -> (u32, u32, Vec<u8>) {
        if text.is_empty() { return (0, 0, vec![]); }
        let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
        let mut total_w = 0f32; let mut max_h = 0u32;
        for ch in text.chars() {
            let (metrics, bitmap) = self.inner.rasterize(ch, self.size);
            total_w += metrics.advance_width;
            max_h = max_h.max(metrics.height as u32);
            glyphs.push((metrics, bitmap));
        }
        let w = total_w.round() as u32; let h = max_h;
        if w == 0 || h == 0 { return (w, h, vec![]); }
        let mut canvas = vec![0u8; (w * h) as usize];
        let mut xo = 0i32;
        for (metrics, data) in &glyphs {
            let dx = (xo as i64 + metrics.xmin as i64).max(0).min(u32::MAX as i64) as u32;
            for gy in 0..metrics.height { for gx in 0..metrics.width {
                let a = data[gy * metrics.width + gx];
                if a > 0 { let cx = dx + gx as u32; let cy = gy as u32;
                    if cx < w && cy < h { let d = (cy * w + cx) as usize; canvas[d] = canvas[d].max(a); } }
            }}
            xo = xo.saturating_add(metrics.advance_width as i32);
        }
        (w, h, canvas)
    }

    /// Render text to an RGBA image. Returns (width, height, pixel_data).
    pub fn render_text(&self, text: &str, fill: (u8, u8, u8, u8), _spacing: f32) -> (u32, u32, Vec<u8>) {
        if text.is_empty() {
            return (0, 0, vec![]);
        }

        // Layout: gather all glyphs with positions
        let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
        let mut total_w = 0f32;
        let mut max_h = 0u32;

        for ch in text.chars() {
            let (metrics, bitmap) = self.inner.rasterize(ch, self.size);
            total_w += metrics.advance_width;
            max_h = max_h.max(metrics.height as u32);
            // Convert coverage bitmap to RGBA
            let mut rgba = vec![0u8; metrics.width * metrics.height * 4];
            for y in 0..metrics.height {
                for x in 0..metrics.width {
                    let cov = bitmap[y * metrics.width + x];
                    if cov > 0 {
                        let off = (y * metrics.width + x) * 4;
                        let alpha = (cov as u16 * fill.3 as u16 / 255) as u8;
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
            let dx = (x_offset as i64 + metrics.xmin as i64).max(0).min(u32::MAX as i64) as u32;
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
                                let out_a = sa + da * (255 - sa) / 255;
                                if out_a > 0 {
                                    canvas[dst_off] = ((rgba[idx] as u16 * sa + canvas[dst_off] as u16 * da * (255 - sa) / 255) / out_a) as u8;
                                    canvas[dst_off + 1] = ((rgba[idx+1] as u16 * sa + canvas[dst_off+1] as u16 * da * (255 - sa) / 255) / out_a) as u8;
                                    canvas[dst_off + 2] = ((rgba[idx+2] as u16 * sa + canvas[dst_off+2] as u16 * da * (255 - sa) / 255) / out_a) as u8;
                                    canvas[dst_off + 3] = out_a as u8;
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
}
