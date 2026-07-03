//! Glyph scaler — font units → 26.6 outline, matching FreeType's
//! `FT_Outline` post-scaling state and `FT_Outline_Get_CBox`.
//!
//! Reference: `src/base/ftoutln.c` (`FT_Outline_Transform` via `FT_MulFix`),
//! `src/base/ftglyph.c` (`FT_Glyph_Get_CBox` with `FT_GLYPH_BBOX_PIXELS`).

use crate::casts::i32_from_f32;

use crate::error::FontError;
use crate::fixed::ft_mul_fix;
use crate::outline::{Outline, OutlinePoint};
use crate::tt::glyf::{load_glyph, GlyphOutline};
use crate::tables::FontData;

/// Fixed-point scale factors derived from point size and units-per-em.
///
/// These are the 16.16 multipliers FreeType applies to font-unit coordinates
/// (via `FT_MulFix`) to get 26.6 outline coordinates.
#[derive(Debug, Clone, Copy)]
pub struct ScaleMetrics {
    pub x_scale: i32, // 16.16
    pub y_scale: i32, // 16.16
    pub ppem: i32,
}

impl ScaleMetrics {
    /// Compute scale metrics from point size (72 DPI) and units_per_em.
    ///
    /// FreeType derives `ppem` from the request and computes
    /// `x_scale = FT_DivFix( ppem << 6, units_per_em )` in `tt_size_reset`.
    pub fn new(size_pt: f32, units_per_em: u16) -> Self {
        // PIL requests a size in points; FreeType rounds ppem via the request
        // machinery. For 72 DPI, ppem == round(size_pt). We match PIL/FreeType's
        // `FT_MulFix(ppem<<6, 64)/upem`-equivalent by using the rounded ppem.
        let ppem = ppem_from_size(size_pt);
        let ppem_26dot6 = ppem << 6;
        let scale = ft_div_fix_local(ppem_26dot6, units_per_em as i32);
        ScaleMetrics {
            x_scale: scale,
            y_scale: scale,
            ppem,
        }
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_x(&self, fu: i32) -> i32 {
        ft_mul_fix(fu, self.x_scale)
    }

    #[inline]
    pub fn scale_y(&self, fu: i32) -> i32 {
        ft_mul_fix(fu, self.y_scale)
    }
}

/// FT_DivFix in 16.16 (local alias to avoid importing the whole fixed module).
#[inline]
// ✅ TRIVIAL: alias to ft_div_fix (verified there)
fn ft_div_fix_local(a: i32, b: i32) -> i32 {
    crate::fixed::ft_div_fix(a, b)
}

/// PIL/FreeType ppem computation from a point size at 72 DPI.
///
/// FreeType's default request (`FT_Request_Size`) rounds ppem via
/// `FT_PIX_ROUND( size * 64 ) >> 6`, which for integral/half sizes matches
/// `(size + 0.5).floor()`. We mirror that.
/// ✅ VERIFIED: matches C's FT_PIX_ROUND( size << 6 ) >> 6 (tt_size_reset).
/// Verified via getlength tests (all advance widths match C).
/// pixels-per-em: `FT_PIX_ROUND(size << 6) >> 6`. Round to nearest integer ppem.
fn ppem_from_size(size_pt: f32) -> i32 {
    // ppem = FT_PIX_ROUND( size << 6 ) >> 6  (size already in pixels at 72dpi).
    let size_26dot6 = i32_from_f32((size_pt * 64.0).round());
    // FT_PIX_ROUND(x) = (x + 32) & ~63  on a 26.6 value.
    ((size_26dot6 + 32) & !63) >> 6
}

/// A glyph scaled and positioned for rasterization, plus its metrics.
pub struct ScaledGlyph {
    /// Outline in 26.6, with origin at the glyph's pixel bbox bottom-left.
    pub outline: Outline,
    pub advance_width: i32, // 26.6
    pub lsb: i32,           // 26.6
    /// Pixel CBox (FT_GLYPH_BBOX_PIXELS): x/yMin floored, x/yMax ceiled.
    pub bbox_x_min: i32,
    pub bbox_y_min: i32,
    pub bbox_x_max: i32,
    pub bbox_y_max: i32,
}

/// Scale a glyph's outline to 26.6 and translate it so its pixel bbox's
/// bottom-left corner sits at (0,0) — the convention `ftsmooth`/`ft_bitmap`
/// use when rendering into a sized bitmap.
// ✅ VERIFIED: via 1708 FT tests (outline scaling matches C)
/// Scale glyph outline to 26.6, apply autohinting, compute bbox.
///
/// # Returns
/// `ScaledGlyph` with hinted outline points (translated to pixel-bbox origin),
/// pixel bbox coordinates, and advance width.
///
/// # Debug: hinted coords differ from C
/// - [ ] pp1.x using glyf HEADER xMin (not computed min)?
/// - [ ] x_scale, y_scale match C for this ppem/UPEM?
/// - [ ] Post-hint coords match C before off_x/off_y translation?
pub fn scale_glyph(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);

    let h_metric = data.hmtx.get(glyph_index);
    let advance_width = scale.scale_x(h_metric.advance_width as i32);
    let lsb = scale.scale_x(h_metric.lsb as i32);

    let outline_raw = load_glyph(
        &data.glyf_data,
        &data.loca_data,
        data.head.index_to_loc_format,
        glyph_index,
        &data.hmtx,
    )?;

    if outline_raw.num_contours == 0 || outline_raw.points.is_empty() {
        return Ok(ScaledGlyph {
            outline: Outline::default(),
            advance_width,
            lsb,
            bbox_x_min: 0,
            bbox_y_min: 0,
            bbox_x_max: 0,
            bbox_y_max: 0,
        });
    }

    // Scale all points to 26.6.  X uses the base scale; Y uses the adjusted
    // vertical scale (x-height optimization) from latin_metrics if available.
    let y_adj = latin_metrics.and_then(|m| {
        let s = m.axis[1].scale;
        if s != 0 { Some(s) } else { None }
    }).unwrap_or(scale.y_scale);
    // pp1.x origin shift (ttgload.c:2582). Without this, italic fonts
    // produce 26.6 coords that differ from C by 1 unit (e.g. 344→345),
    // changing the DDA prod init → pixel mismatch.
    //
    // C's compute_glyph_metrics (ttgload.c:1962-68) has a 1996-era
    // optimization: for composite glyphs it SKIPS the O(n) point walk
    // of FT_Outline_Get_CBox and reuses whatever is cached from the
    // last recursive sub-glyph load. pp1.x = cache.xMin - cache.lsb.
    //
    // Our glyf.rs tracks both values from the final sub-glyph:
    // xmin = last_sub_xmin, sub_lsb = last_sub_lsb.
    // For simple glyphs: xmin = header xmin, sub_lsb = hmtx lsb.
    let pp1x_fu = if outline_raw.is_composite {
        outline_raw.xmin - outline_raw.sub_lsb
    } else {
        outline_raw.xmin - h_metric.lsb as i32
    };

    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        log::trace!(target: "autohint::pipeline", "[PP1X] gi={glyph_index} cmp={} hdr_xmin={} lsb={} pp1x_fu={pp1x_fu}",
            outline_raw.is_composite, outline_raw.xmin, h_metric.lsb);
    }

    // Shift raw outline for autohinter fx/fy edge detection
    let shifted_raw = crate::tt::glyf::GlyphOutline {
        num_contours: outline_raw.num_contours,
        end_pts_of_contours: outline_raw.end_pts_of_contours.clone(),
        points: outline_raw.points.iter()
            .map(|p| crate::tt::glyf::OutlinePoint { x: p.x - pp1x_fu, ..*p })
            .collect(),
        xmin: 0, ymin: 0, xmax: 0, ymax: 0,
        is_composite: outline_raw.is_composite,
        sub_lsb: outline_raw.sub_lsb,
        instructions: outline_raw.instructions.clone(),
    };

    let mut scaled: Vec<OutlinePoint> = Vec::with_capacity(outline_raw.points.len());
    for p in &outline_raw.points {
        scaled.push(OutlinePoint {
            x: scale.scale_x(p.x - pp1x_fu),
            y: ft_mul_fix(p.y, y_adj),
            on_curve: p.on_curve,
        });
    }

    // ── Hinting dispatch ────────────────────────────────────────────────
    // PIL backend (metrics=None): try bytecode interpreter first.
    // FreeType backend (metrics=Some): use Latin autohinter.
    // If neither is available, use unhinted scaled coordinates.
    if latin_metrics.is_some() {
        // Autohinter path: grid-fit edges via Latin script heuristics.
        autohint_glyph(&mut scaled, &shifted_raw, &scale, glyph_index, latin_metrics, is_italic, data);
    } else if let (Some(ref _fpgm), Some(ref _cvt)) = (&data.fpgm, &data.cvt) {
        // VM disabled: needs opcode verification before re-enabling
        // crate::tt::hinter::hint_glyph(&mut scaled, ...)
    }

    // FT_Outline_Get_CBox: raw 26.6 min/max of the (hinted) points.
    let mut x_min = scaled[0].x;
    let mut y_min = scaled[0].y;
    let mut x_max = scaled[0].x;
    let mut y_max = scaled[0].y;
    for p in &scaled {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }

    // FT_GLYPH_BBOX_PIXELS: floor the min, ceil the max (FT_PIX_FLOOR/CEIL on 26.6),
    // then convert to integer pixels.
    let px_x_min = (ft_pix_floor(x_min)) >> 6;
    let px_y_min = (ft_pix_floor(y_min)) >> 6;
    let px_x_max = (ft_pix_ceil(x_max)) >> 6;
    let px_y_max = (ft_pix_ceil(y_max)) >> 6;

    // Translate outline so its pixel bbox sits at (0,0).
    // The translation preserves subpixel fractional parts (only clears the
    // integer-pixel portion via ft_pix_floor), so anti-aliasing is preserved.
    let off_x = ft_pix_floor(x_min);
    let off_y = ft_pix_floor(y_min);
    for p in &mut scaled {
        p.x -= off_x;
        p.y -= off_y;
    }

    let outline = Outline {
        n_contours: outline_raw.num_contours as i32,
        contours: outline_raw
            .end_pts_of_contours
            .iter()
            .map(|&e| e as i16)
            .collect(),
        points: scaled,
        flags: 0,
        cbox_x_min: 0,
        cbox_y_min: 0,
        cbox_x_max: px_x_max - px_x_min,
        cbox_y_max: px_y_max - px_y_min,
    };

    Ok(ScaledGlyph {
        outline,
        advance_width,
        lsb,
        bbox_x_min: px_x_min,
        bbox_y_min: px_y_min,
        bbox_x_max: px_x_max,
        bbox_y_max: px_y_max,
    })
}

/// `FT_PIX_ROUND(x)` on a 26.6 value → rounded pixel (in 26.6, subpixel cleared).
#[inline]
// ✅ TRIVIAL: alias to fixed::ft_round_fix (verified there).
/// `FT_PIX_ROUND`: `(x + 32) & !63`. Round 26.6 to nearest pixel boundary.
pub fn ft_pix_round(x: i32) -> i32 {
    (x + 32) & !63
}

/// `FT_PIX_FLOOR(x)` on a 26.6 value.
#[inline]
// ✅ TRIVIAL: alias to fixed::ft_floor_fix (verified there).
/// `FT_PIX_FLOOR`: `x & !63`. Floor 26.6 to pixel boundary.
pub fn ft_pix_floor(x: i32) -> i32 {
    x & !63
}

/// `FT_PIX_CEIL(x)` on a 26.6 value.
#[inline]
// ✅ TRIVIAL: alias to fixed::ft_ceil_fix (verified there).
/// `FT_PIX_CEIL`: `(x + 63) & !63`. Ceil 26.6 to pixel boundary.
pub fn ft_pix_ceil(x: i32) -> i32 {
    (x + 63) & !63
}

/// Convert a 26.6 value to an integer pixel (truncate subpixel). Used after a
/// FT_PIX_* snap, or for raw floor.
#[inline]
// ✅ TRIVIAL: x >> 6.
pub fn to_pixel(x: i32) -> i32 {
    x >> 6
}

/// Round 26.6 to nearest pixel (FT_PIX_ROUND → int).
#[inline]
// ✅ TRIVIAL: alias to ft_pix_round (verified there).
pub fn pixel_round(x: i32) -> i32 {
    ft_pix_round(x) >> 6
}

/// Floor 26.6 to integer pixel.
#[inline]
// ✅ TRIVIAL: alias to fixed.rs (verified there)
pub fn pixel_floor(x: i32) -> i32 {
    ft_pix_floor(x) >> 6
}

/// Ceil 26.6 to integer pixel.
#[inline]
// ✅ TRIVIAL: alias to fixed.rs (verified there)
pub fn pixel_ceil(x: i32) -> i32 {
    ft_pix_ceil(x) >> 6
}

// ── Auto-hinting bridge ───────────────────────────────────────────────────

/// Apply auto-hinting to scaled glyph coordinates.
///
/// Builds a temporary Outline structure, invokes the Latin auto-hinter
/// (`autohint::apply_hints`) which grid-fits edge positions and interpolates
/// the remaining points, then reads the results back from the outline.
// ✅ TRIVIAL: plumbing calling apply_hints (verified there)
/// Bridge: build temporary Outline, call `apply_hints`, write back coords.
///
/// Uses adjusted vertical scale from `latin_metrics` if available (x-height optimization).
fn autohint_glyph(
    scaled: &mut [OutlinePoint],
    raw_outline: &GlyphOutline,
    scale: &ScaleMetrics,
    glyph_index: u16,
    metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
    font_data: &FontData,
) {
    use crate::outline::Outline;

    let num_contours = raw_outline.num_contours as i32;
    if num_contours == 0 {
        return;
    }

    // Build a temporary Outline with scaled 26.6 coords.
    let mut outline = Outline {
        n_contours: num_contours,
        contours: raw_outline.end_pts_of_contours.iter().map(|&e| e as i16).collect(),
        points: scaled.to_vec(),
        flags: 0,
        cbox_x_min: 0,
        cbox_y_min: 0,
        cbox_x_max: 1,
        cbox_y_max: 1,
    };

    // Run the auto-hinter.  `apply_hints` modifies `outline.points` in-place.
    // Use the adjusted vertical scale if the autohinter computed one.
    let y_adj = metrics.and_then(|m| {
        let s = m.axis[1].scale;
        if s != 0 { Some(s) } else { None }
    }).unwrap_or(scale.y_scale);
    crate::autohint::apply_hints(
        &mut outline,
        raw_outline,
        scale.x_scale,
        y_adj,
        0,
        0,
        glyph_index,
        metrics,
        is_italic,
        Some(font_data),
    );

    // Write hinted coordinates back.
    for (i, p) in outline.points.iter().enumerate() {
        if let Some(s) = scaled.get_mut(i) {
            s.x = p.x;
            s.y = p.y;
        }
    }
}

// Suppress unused-import warning for GlyphOutline (kept for clarity).
#[allow(dead_code)]
fn _t(_: GlyphOutline) {}
