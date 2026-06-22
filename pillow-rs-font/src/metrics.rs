//! Glyph metrics computation -- getbbox, getmetrics, getlength, getname.
//!
//! Matches PIL's ImageFont metrics exactly.

use crate::scaler::{mul_fix, pixel_ceil, pixel_round, ScaleMetrics};
use crate::tables::Font;

/// Rendered glyph mask with metrics.
#[derive(Debug, Clone)]
pub struct GlyphMask {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major alpha pixels (0-255).
    pub pixels: Vec<u8>,
    /// Horizontal offset for compositing.
    pub xmin: i32,
    /// Vertical offset for compositing.
    pub ymin: i32,
    /// Advance width in pixels.
    pub advance_width: f32,
}

impl Font {
    /// Get font metrics: (ascent, descent) in pixels.
    ///
    /// Matches PIL's FreeType: uses hhea ascender/descender with ceil rounding,
    /// which matches FreeType's default face-level metrics computation.
    pub fn getmetrics(&self) -> (u32, u32) {
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);

        // Always use hhea values (matches FreeType's default face-level metrics)
        let asc_26 = mul_fix(data.hhea.ascent as i32, scale.y_scale);
        let desc_26 = mul_fix(data.hhea.descent as i32, scale.y_scale);

        // FreeType/FreeType: round UP for metrics (ceiling)
        let asc = pixel_ceil(asc_26);
        let desc = pixel_ceil(-desc_26); // desc is negative, negate for positive value

        (asc as u32, desc as u32)
    }

    /// Get font family and style name. Returns ("Family", "Style").
    pub fn getname(&self) -> (&str, &str) {
        (&self.data.name.family, &self.data.name.subfamily)
    }

    /// Get the advance-width sum for text in pixels.
    pub fn getlength(&self, text: &str) -> f32 {
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);
        let mut total: f32 = 0.0;
        for ch in text.chars() {
            let cp = ch as u32;
            let glyph_idx = data.cmap.map(cp).unwrap_or(0);
            let metric = data.hmtx.get(glyph_idx);
            let advance_26dot6 = mul_fix(metric.advance_width as i32, scale.x_scale);
            total += advance_26dot6 as f32 / 64.0;
        }
        total
    }

    /// Get the bounding box for text. Returns (left, top, right, bottom).
    ///
    /// Uses PIX_FLOOR/PIX_CEIL to match FreeType's FT_Outline_Get_CBox convention.
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);

        // Pre-compute ascender (FreeType face->ascender → ceil to pixels)
        let asc_26 = mul_fix(data.hhea.ascent as i32, scale.y_scale);
        let asc_px = pixel_ceil(asc_26);

        let mut x = 0i32;
        let mut x_min = i32::MAX;
        let mut y_min = i32::MAX;
        let mut x_max = i32::MIN;
        let mut y_max = i32::MIN;

        for ch in text.chars() {
            let cp = ch as u32;
            let glyph_idx = data.cmap.map(cp).unwrap_or(0);
            let metric = data.hmtx.get(glyph_idx);

            let lsb = pixel_round(mul_fix(metric.lsb as i32, scale.x_scale));
            let advance = pixel_round(mul_fix(metric.advance_width as i32, scale.x_scale));

            // Get scaled glyph (with hinting if available) for ink extent
            let scaled = match crate::scaler::scale_glyph(data, glyph_idx) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if scaled.num_contours > 0 {
                let floor_x = scaled.xmin; // PIX_FLOOR
                let ceil_x = scaled.xmax; // PIX_CEIL
                let floor_y = scaled.ymin;
                let ceil_y = scaled.ymax;

                // PIL coordinates: y increases downward, baseline at asc_px
                // PIL's bbox includes the origin (0) and the pen advance,
                // not just the ink extent (matching FT_GLYPH_BBOX_PIXELS + advance)
                let gx_min = x.min(x + floor_x);
                let gx_max = (x + advance).max(x + ceil_x);
                // FreeType y-up: ceil_y = topmost pixel above baseline
                // floor_y = bottommost pixel (negative = below baseline)
                let gy_min = asc_px - ceil_y;
                let gy_max = asc_px - floor_y;

                x_min = x_min.min(gx_min);
                x_max = x_max.max(gx_max);
                y_min = y_min.min(gy_min);
                y_max = y_max.max(gy_max);
            } else {
                // Empty glyph (space, etc.) — use advance width for x,
                // baseline position for y
                let gx_min = x + lsb;
                let gx_max = gx_min + advance;

                x_min = x_min.min(gx_min);
                x_max = x_max.max(gx_max);
                if y_min == i32::MAX {
                    y_min = asc_px;
                    y_max = asc_px;
                }
            }

            x += advance;
        }

        if x_min == i32::MAX {
            (0, 0, 0, 0)
        } else {
            (x_min, y_min, x_max, y_max)
        }
    }

    /// Render a glyph as alpha mask (PIL: getmask).
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, crate::error::FontError> {
        let data = &self.data;

        if text.is_empty() {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: vec![],
                xmin: 0,
                ymin: 0,
                advance_width: 0.0,
            });
        }

        // Render first character only (multi-char composition is handled at higher level)
        // SAFETY: empty text is checked above — the next() call always succeeds here
        let ch = text.chars().next().unwrap_or('\0');
        let cp = ch as u32;
        let glyph_idx = data.cmap.map(cp).unwrap_or(0);

        let scaled = crate::scaler::scale_glyph(data, glyph_idx)?;
        /* TODO: re-enable hinting when engine is fixed
        let scaled = if let Some(ref engine) = self.hint_engine {
            crate::scaler::scale_and_hint(data, glyph_idx, &mut engine.borrow_mut())?
        } else {
            crate::scaler::scale_glyph(data, glyph_idx)?
        };
        */
        let raster = crate::raster::rasterize(&scaled);

        let advance_26dot6 = scaled.advance_width;
        let advance_px = pixel_round(advance_26dot6) as i32;

        // PIL convention: mask covers the full bbox (origin + advance width),
        // not just the ink extent. Pad the rasterized bitmap to match.
        let x_min = 0i32.min(raster.xmin);
        let x_max = advance_px.max(raster.xmin + raster.width as i32);
        let y_min = 0i32.min(raster.ymin);
        let y_max = 0i32.max(raster.ymin + raster.height as i32);

        let new_width = (x_max - x_min) as u32;
        let new_height = (y_max - y_min) as u32;

        let pixels = if new_width == raster.width && new_height == raster.height {
            raster.pixels
        } else if new_width == 0 || new_height == 0 {
            vec![]
        } else {
            let mut padded = vec![0u8; (new_width * new_height) as usize];
            let x_off = (raster.xmin - x_min) as u32;
            let y_off = (raster.ymin - y_min) as u32;
            for y in 0..raster.height {
                let src_row = (y * raster.width) as usize;
                let dst_row = ((y + y_off) * new_width + x_off) as usize;
                padded[dst_row..dst_row + raster.width as usize]
                    .copy_from_slice(&raster.pixels[src_row..src_row + raster.width as usize]);
            }
            padded
        };

        Ok(GlyphMask {
            width: new_width,
            height: new_height,
            pixels,
            xmin: x_min,
            ymin: y_min,
            advance_width: advance_26dot6 as f32 / 64.0,
        })
    }

    /// Create a font variant with overridden size.
    pub fn font_variant(&self, size: Option<f32>) -> Font {
        Font {
            data: self.data.clone(),
            size_pt: size.unwrap_or(self.size_pt),
            hint_engine: self.hint_engine.clone(),
        }
    }

    /// Create a font variant without hinting (for comparison).
    pub fn font_variant_no_hint(&self) -> Font {
        Font {
            data: self.data.clone(),
            size_pt: self.size_pt,
            hint_engine: None,
        }
    }
}

#[cfg(test)]
mod tests {}
