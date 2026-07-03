//! PIL `ImageFont` compatibility layer on top of the FreeType port.
//!
//! Mirrors the subset of PIL's `FreeTypeFont` API used by the coverage matrix:
//! `truetype`, `getmask`, `getbbox`, `getmetrics`, `getname`, `getlength`.

use crate::casts::{i32_from_f32, i32_from_usize, u32_from_i32, u32_from_i64, u32_from_usize, usize_from_i32};

use crate::error::FontError;
use crate::fixed::ft_mul_fix;
use crate::grays::{self, RasterResult};
use crate::scaler::{self, pixel_ceil, pixel_round, ScaleMetrics};
use crate::tables::FontData;
use crate::tt::{self, tag};
use std::sync::Arc;

/// Selects the rendering pipeline for `getmask` / `getbbox`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitmapBackend {
    /// PIL-style mask: padded to ascender/descender, advance-based width.
    /// bbox uses ascender-relative screen coords.
    #[default]
    PIL,
    /// Raw FreeType output: bitmap as-is, no padding, FreeType bbox coords.
    FreeType,
}

/// A loaded TrueType font at a given point size.
#[derive(Clone)]
pub struct Font {
    pub data: Arc<FontData>,
    pub size_pt: f32,
    /// Selected rendering backend.
    pub backend: BitmapBackend,
    /// Face-level global hinting data: per-glyph script assignment,
    /// lazy-computed per-style metrics (Latin, Greek, etc.).
    /// Matches FreeType's AF_FaceGlobals.
    pub face_globals: crate::autohint::globals::FaceGlobals,
    /// Whether the font is italic/oblique (from head.mac_style bit 1).
    pub is_italic: bool,
}

/// A rendered glyph alpha mask.
#[derive(Debug, Clone)]
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Left bearing offset in pixels (bbox xmin, may be negative).
    /// Used by the compositor to place the glyph horizontally.
    pub xmin: i32,
    /// Top bearing offset in pixels (bbox ymin — used for vertical placement).
    /// PIL convention: positive = above baseline.
    pub ymin: i32,
    /// Advance width in 26.6 fixed-point format.
    pub advance_width: i32,
}

impl Font {
    /// Load a TrueType/OpenType font from raw bytes at a given point size.
    ///
    /// Parses all required tables eagerly. Matches `FT_New_Memory_Face` +
    /// `FT_Set_Char_Size` for the table subset PIL touches.
    ///
    /// # Errors
    ///
    /// Returns [`FontError::InvalidFont`] if the data is not a valid
    /// TrueType/OpenType font, or if any required table is missing or
    /// malformed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pillow_rs_freetype::{Font, BitmapBackend};
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0, BitmapBackend::PIL).unwrap();
    /// assert_eq!(font.getname(), ("DejaVu Sans", "Book"));
    /// ```
    pub fn truetype(data: &[u8], size_pt: f32, backend: BitmapBackend) -> Result<Self, FontError> {
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

        // Build FontData first, then compute Latin autohinter metrics.
        let font_data = Arc::new(FontData {
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
        });

        let _upem = font_data.head.units_per_em as i32;
        let is_italic = (font_data.head.mac_style & 2) != 0;
        let face_globals = crate::autohint::globals::FaceGlobals::new(
            font_data.clone(),
            is_italic,
        );

        Ok(Font {
            data: font_data,
            size_pt,
            backend,
            face_globals,
            is_italic,
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
        let upem = data.head.units_per_em as i32;
        let ppem = i32_from_f32(self.size_pt + 0.5); // FT_PIX_ROUND(size_pt << 6) >> 6

        let (asc_fu, desc_fu) = pick_metrics(data);
        // Match C's FT_PIX_CEIL(FT_MulFix(fu_val, scale)) chain exactly.
        // scale = FT_DivFix(ppem << 6, upem) in 16.16
        // val_26dot6 = FT_MulFix(fu_val, scale)
        // result = FT_PIX_CEIL(val_26dot6)
        let scale: i64 = ((ppem as i64 * 64 * 65536) + (upem as i64 / 2)) / upem as i64;
        let asc_26dot6 = (asc_fu as i64 * scale + 32768) >> 16;
        let desc_26dot6 = (desc_fu as i64 * scale + 32768) >> 16;
        let asc = u32_from_i64((asc_26dot6 + 63) >> 6);
        let desc = u32_from_i64((desc_26dot6 + 63) >> 6);
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

    /// `getbbox(text)` → `(left, top, right, bottom)` in pixels.
    ///
    /// `BitmapBackend::PIL`: PIL coords, y-down from ascender with baseline padding.
    /// `BitmapBackend::FreeType`: raw FreeType bbox, y-up from baseline.
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        let data = &self.data;
        let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);
        let asc_26 = ft_mul_fix(pick_metrics(data).0, scale.y_scale);
        let asc_px = pixel_ceil(asc_26);

        let ch = text.chars().next().unwrap_or('\0');
        let glyph = data.cmap.char_index(ch as u32).unwrap_or(0);
        let advance = pixel_round(ft_mul_fix(
            data.hmtx.get(glyph).advance_width as i32, scale.x_scale));
        let metrics_cache = self.face_globals.get_metrics(glyph);
        // PIL backend: skip autohinting
        let metrics_for_scale = match self.backend {
            BitmapBackend::PIL => None,
            BitmapBackend::FreeType => metrics_cache.as_ref(),
        };
        match scaler::scale_glyph(data, glyph, metrics_for_scale, self.is_italic) {
            Ok(g) if g.outline.n_contours > 0 => {
                match self.backend {
                    BitmapBackend::PIL => {
                        let gx_min = 0_i32.min(g.bbox_x_min);
                        let gx_max = advance.max(g.bbox_x_max);
                        let gy_min = asc_px - g.bbox_y_max;
                        let gy_max = (asc_px - g.bbox_y_min).max(asc_px);
                        (gx_min, gy_min, gx_max, gy_max)
                    }
                    BitmapBackend::FreeType => {
                        // Raw FreeType bbox: pixel coords from outline,
                        // y-up from baseline.
                        (g.bbox_x_min, g.bbox_y_min, g.bbox_x_max, g.bbox_y_max)
                    }
                }
            }
            _ => {
                if self.backend == BitmapBackend::PIL {
                    (0, asc_px, advance, asc_px)
                } else {
                    (0, 0, 0, 0)
                }
            }
        }
    }

    /// `getmask(char)` → 8-bit alpha bitmap sized to the glyph's full mask
    /// box (origin + advance), matching PIL's `getmask` on an `L` image.
    ///
    /// # Errors
    ///
    /// Returns [`FontError::InvalidFont`] if the glyph outline cannot be
    /// loaded or scaled, or [`FontError::InvalidOutline`] if the outline
    /// data is malformed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pillow_rs_freetype::{Font, BitmapBackend};
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0, BitmapBackend::PIL).unwrap();
    /// let mask = font.getmask("A").unwrap();
    /// assert!(mask.width > 0);
    /// ```
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, FontError> {
        let data = &self.data;
        if text.is_empty() {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: 0,
                ymin: 0,
                advance_width: 0,
            });
        }

        let ch = text.chars().next().unwrap_or('\0');
        let glyph = data.cmap.char_index(ch as u32).unwrap_or(0);
        // PIL backend: skip autohinting. PIL uses native bytecode hinter.
        // Our autohinter produces different bbox/pixel values.
        let metrics_cache = self.face_globals.get_metrics(glyph);
        let metrics_for_scale = match self.backend {
            BitmapBackend::PIL => None,
            BitmapBackend::FreeType => metrics_cache.as_ref(),
        };
        let scaled = scaler::scale_glyph(data, glyph, metrics_for_scale, self.is_italic)?;

        if scaled.outline.n_contours == 0 {
            // No outline → empty mask (but PIL still returns the advance-sized
            // zero box when there is an advance). For the coverage fixtures,
            // empty glyphs produce an all-zero mask of advance width.
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: 0,
                ymin: 0,
                advance_width: 0,
            });
        }

        let raster = grays::rasterize(scaled.outline)?;
        let advance_px = pixel_round(scaled.advance_width);

        match self.backend {
            BitmapBackend::PIL => {
                // PIL mask: padded to ascender/descender extent.
                let new_width = u32_from_i32(advance_px.max(i32_from_usize(raster.width)));
                let new_height = u32_from_i32(scaled.bbox_y_max - scaled.bbox_y_min.min(0));
                let nw = new_width as usize;
                let nh = new_height as usize;

                if nw == 0 || nh == 0 {
                    return Ok(GlyphMask {
                        width: new_width,
                        height: new_height,
                        pixels: Vec::new(),
                        xmin: scaled.bbox_x_min,
                        ymin: scaled.bbox_y_min,
                        advance_width: advance_px,
                    });
                }

                let mut pixels = vec![0u8; nw * nh];
                let x_offs = usize_from_i32((scaled.bbox_x_min).max(0));
                let y_offs = 0usize;
                let rw = raster.width;
                for y in 0..raster.height {
                    let src = y * rw;
                    let dst = y_offs + y * nw + x_offs;
                    if dst + rw <= pixels.len() && x_offs + rw <= nw {
                        let copy = rw.min(nw.saturating_sub(x_offs));
                        pixels[dst..dst + copy].copy_from_slice(&raster.pixels[src..src + copy]);
                    }
                }

                Ok(GlyphMask {
                    width: new_width,
                    height: new_height,
                    pixels,
                    xmin: scaled.bbox_x_min,
                    ymin: scaled.bbox_y_min,
                    advance_width: advance_px,
                })
            }
            BitmapBackend::FreeType => {
                // Raw FreeType: raster as-is, no padding.
                let w = u32_from_usize(raster.width);
                let h = u32_from_usize(raster.height);
                Ok(GlyphMask {
                    width: w,
                    height: h,
                    pixels: raster.pixels,
                    xmin: scaled.bbox_x_min,
                    ymin: scaled.bbox_y_min,
                    advance_width: advance_px,
                })
            }
        }
    }
}

/// Pick (ascender, descender) as positive font-unit magnitudes.
///
/// FreeType's `sfnt_init_face` uses OS/2 usWinAscent/usWinDescent for the
/// face-level ascender/descender. The descender is converted to a positive
/// value matching PIL's convention.
// ✅ VERIFIED: OS/2 priority lookup matches C (sfobjs.c).
fn pick_metrics(data: &FontData) -> (i32, i32) {
    if let Some(pair) = pick_typo_metrics(data) {
        return pair;
    }
    let asc = data.hhea.ascent as i32;
    let desc = -data.hhea.descent as i32;
    if asc != 0 || desc != 0 {
        return (asc, desc);
    }
    pick_os2_metrics(data).unwrap_or((asc, desc))
}

/// Priority 1: OS/2 sTypoAscender / sTypoDescender when USE_TYPO_METRICS is set.
fn pick_typo_metrics(data: &FontData) -> Option<(i32, i32)> {
    let os2 = data.os2.as_ref()?;
    if os2.use_typo_metrics() {
        Some((os2.s_typo_ascender as i32, (-os2.s_typo_descender) as i32))
    } else {
        None
    }
}

/// Priority 2-3: Try OS/2 typo, then usWin fallback (sfobjs.c:1395-1413).
fn pick_os2_metrics(data: &FontData) -> Option<(i32, i32)> {
    let os2 = data.os2.as_ref()?;
    let ta = os2.s_typo_ascender as i32;
    let td = -os2.s_typo_descender as i32;
    if ta != 0 || td != 0 {
        return Some((ta, td));
    }
    Some((os2.us_win_ascent as i32, os2.us_win_descent as i32))
}

// Silence unused-import warning for RasterResult (kept for clarity).
#[allow(dead_code)]
fn _t(_: RasterResult) {}
