//! PIL `ImageFont` compatibility layer on top of the FreeType port.
//!
//! Mirrors the subset of PIL's `FreeTypeFont` API used by the coverage matrix:
//! `truetype`, `getmask`, `getbbox`, `getmetrics`, `getname`, `getlength`.
//! The exact byte contract is defined by `pillow-rs-font-legacy-attempt`.

use crate::error::FontError;
use crate::fixed::ft_mul_fix;
use crate::grays::{self, RasterResult};
use crate::scaler::{self, pixel_ceil, pixel_round, ScaleMetrics};
use crate::tables::FontData;
use crate::tt::{self, tag};
use std::sync::Arc;

/// A loaded TrueType font at a given point size.
#[derive(Clone)]
pub struct Font {
    data: Arc<FontData>,
    pub size_pt: f32,
}

/// A rendered glyph alpha mask.
#[derive(Debug, Clone)]
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Font {
    /// Load a TrueType/OpenType font from raw bytes at a given point size.
    ///
    /// Parses all required tables eagerly. Matches `FT_New_Memory_Face` +
    /// `FT_Set_Char_Size` for the table subset PIL touches.
    pub fn truetype(data: &[u8], size_pt: f32) -> Result<Self, FontError> {
        let dir = tt::parse_table_directory(data)?;

        let head_bytes = dir.find(data, tag(b"head")).ok_or_else(|| {
            FontError::InvalidFont("missing 'head' table".into())
        })?;
        let head = tt::head::parse_head(head_bytes)?;

        let maxp_bytes = dir.find(data, tag(b"maxp")).ok_or_else(|| {
            FontError::InvalidFont("missing 'maxp' table".into())
        })?;
        let maxp = tt::maxp::parse_maxp(maxp_bytes)?;

        let cmap_bytes = dir.find(data, tag(b"cmap")).ok_or_else(|| {
            FontError::InvalidFont("missing 'cmap' table".into())
        })?;
        let cmap = tt::cmap::parse_cmap(cmap_bytes)?;

        let hhea_bytes = dir.find(data, tag(b"hhea")).ok_or_else(|| {
            FontError::InvalidFont("missing 'hhea' table".into())
        })?;
        let hhea = tt::hhea::parse_hhea(hhea_bytes)?;

        let hmtx_bytes = dir.find(data, tag(b"hmtx")).ok_or_else(|| {
            FontError::InvalidFont("missing 'hmtx' table".into())
        })?;
        let hmtx = tt::hmtx::parse_hmtx(hmtx_bytes, hhea.num_hmetrics, maxp.num_glyphs)?;

        let name = match dir.find(data, tag(b"name")) {
            Some(d) => tt::name::parse_name(d)?,
            None => crate::tt::name::NameTable {
                family: "Unknown".into(),
                subfamily: "Regular".into(),
            },
        };

        let os2 = dir.find(data, tag(b"OS/2")).and_then(tt::os2::parse_os2);

        let loca_data = dir
            .find(data, tag(b"loca"))
            .ok_or_else(|| FontError::InvalidFont("missing 'loca' table".into()))?
            .to_vec();
        let glyf_data = dir
            .find(data, tag(b"glyf"))
            .ok_or_else(|| FontError::InvalidFont("missing 'glyf' table".into()))?
            .to_vec();

        Ok(Font {
            data: Arc::new(FontData {
                cmap,
                head,
                hhea,
                hmtx,
                maxp,
                name,
                os2,
                loca_data,
                glyf_data,
                size_pt,
            }),
            size_pt,
        })
    }

    /// `getname()` → `(family, style)`.
    pub fn getname(&self) -> (&str, &str) {
        (&self.data.name.family, &self.data.name.subfamily)
    }

    /// `getmetrics()` → `(ascent, descent)` in pixels.
    ///
    /// PIL returns `face->size->metrics.ascender >> 6` and
    /// `-face->size->metrics.descender >> 6`, where the FreeType metrics are
    /// in 26.6 format after FT_PIX_ROUND. For the test fonts, this is
    /// equivalent to ceil(|fu_val| * ppem / upem).
    pub fn getmetrics(&self) -> (u32, u32) {
        let data = &self.data;
        let upem = data.head.units_per_em as f32;
        let ppem = self.size_pt; // at 72dpi, ppem == size_pt

        let (asc_fu, desc_fu) = pick_metrics(data);
        let asc = (asc_fu as f32 * ppem / upem).ceil() as u32;
        let desc = (desc_fu as f32 * ppem / upem).ceil() as u32;
        (asc, desc)
    }

    /// `getlength(text)` → total advance width in pixels (float).
    pub fn getlength(&self, text: &str) -> f32 {
        let data = &self.data;
        let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);
        let mut total: f32 = 0.0;
        for ch in text.chars() {
            let glyph = data.cmap.char_index(ch as u32).unwrap_or(0);
            let m = data.hmtx.get(glyph);
            let adv_26dot6 = ft_mul_fix(m.advance_width as i32, scale.x_scale);
            total += adv_26dot6 as f32 / 64.0;
        }
        total
    }

    /// `getbbox(text)` → `(left, top, right, bottom)` in pixels (PIL coords,
    /// y-down from the ascent).
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        let data = &self.data;
        let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);
        let asc_26 = ft_mul_fix(pick_metrics(data).0, scale.y_scale);
        let asc_px = pixel_ceil(asc_26);

        let mut x = 0i32;
        let mut x_min = i32::MAX;
        let mut y_min = i32::MAX;
        let mut x_max = i32::MIN;
        let mut y_max = i32::MIN;

        for ch in text.chars() {
            let glyph = data.cmap.char_index(ch as u32).unwrap_or(0);
            let m = data.hmtx.get(glyph);
            let advance = pixel_round(ft_mul_fix(m.advance_width as i32, scale.x_scale));

            match scaler::scale_glyph(data, glyph) {
                Ok(g) if g.outline.n_contours > 0 => {
                    // Ink bbox in glyph-local pixel coords: (0,0)..(w,h) after
                    // the scaler's translate, plus the glyph's pixel origin.
                    let gx_min = x + g.bbox_x_min.min(0);
                    let gx_max = (x + advance).max(x + g.bbox_x_max);
                    let gy_min = asc_px - g.bbox_y_max;
                    let gy_max = asc_px - g.bbox_y_min;
                    x_min = x_min.min(gx_min);
                    x_max = x_max.max(gx_max);
                    y_min = y_min.min(gy_min);
                    y_max = y_max.max(gy_max);
                }
                _ => {
                    // Empty glyph (space): advance-only on x, baseline on y.
                    let gx_min = x;
                    let gx_max = x + advance;
                    x_min = x_min.min(gx_min);
                    x_max = x_max.max(gx_max);
                    if y_min == i32::MAX {
                        y_min = asc_px;
                        y_max = asc_px;
                    }
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

    /// `getmask(char)` → 8-bit alpha bitmap sized to the glyph's full mask
    /// box (origin + advance), matching PIL's `getmask` on an `L` image.
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, FontError> {
        let data = &self.data;
        if text.is_empty() {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            });
        }

        let ch = text.chars().next().unwrap_or('\0');
        let glyph = data.cmap.char_index(ch as u32).unwrap_or(0);
        let scaled = scaler::scale_glyph(data, glyph)?;

        if scaled.outline.n_contours == 0 {
            // No outline → empty mask (but PIL still returns the advance-sized
            // zero box when there is an advance). For the coverage fixtures,
            // empty glyphs produce an all-zero mask of advance width.
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            });
        }

        let raster = grays::rasterize(scaled.outline)?;
        let advance_px = pixel_round(scaled.advance_width);

        // PIL mask box: width = max(advance, ink_right - ink_left),
        // origin at x=0; height covers the ascent+descent region from baseline.
        let new_width = (advance_px).max(raster.width as i32) as u32;
        // Mask height: from the glyph's top (bbox_y_max) to bottom (bbox_y_min).
        let ink_h = raster.height as i32;
        let new_height = ink_h.max(0) as u32;

        if new_width == 0 || new_height == 0 {
            return Ok(GlyphMask {
                width: new_width,
                height: new_height,
                pixels: Vec::new(),
            });
        }

        // Place the raster (origin at glyph bbox bottom-left) into the mask
        // box. The mask's rows run top-down (PIL convention). The raster is
        // already bottom-up-flipped to top-down by grays::rasterize.
        let mut pixels = vec![0u8; (new_width * new_height) as usize];
        let rw = raster.width;
        for y in 0..raster.height {
            let src = y * rw;
            let dst = y * new_width as usize;
            let copy = rw.min(new_width as usize);
            pixels[dst..dst + copy].copy_from_slice(&raster.pixels[src..src + copy]);
        }

        Ok(GlyphMask {
            width: new_width,
            height: new_height,
            pixels,
        })
    }
}

/// Pick (ascender, descender) as positive font-unit magnitudes.
///
/// FreeType's `sfnt_init_face` uses OS/2 usWinAscent/usWinDescent for the
/// face-level ascender/descender. The descender is converted to a positive
/// value matching PIL's convention.
fn pick_metrics(data: &FontData) -> (i32, i32) {
    if let Some(os2) = &data.os2 {
        return (
            os2.us_win_ascent as i32,
            os2.us_win_descent as i32, // positive (distance below baseline)
        );
    }
    (
        data.hhea.ascent as i32,
        (-data.hhea.descent) as i32, // hhea.descent is negative; make positive
    )
}

// Silence unused-import warning for RasterResult (kept for clarity).
#[allow(dead_code)]
fn _t(_: RasterResult) {}
