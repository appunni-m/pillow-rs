//! Glyph scaler — 26.6 fixed-point scaling matching FreeType's tt_size_reset.
//!
//! Converts font-unit glyph outlines to 26.6 pixel coordinates.
//! 1 pixel = 64 sub-pixel units.

use crate::error::FontError;
use crate::hinting::HintingEngine;
use crate::parser::loca_glyf::parse_glyph;
use crate::tables::FontData;

/// A glyph scaled to 26.6 fixed-point coordinates, ready for rasterization.
#[derive(Debug, Clone)]
pub(crate) struct ScaledGlyph {
    /// Scaled outline points in 26.6 format (x and y multiplied by scale).
    pub points: Vec<(i32, i32)>,
    /// Point flags (on_curve or off_curve).
    pub on_curve: Vec<bool>,
    /// End point indices for each contour.
    pub end_pts: Vec<u16>,
    /// Number of contours.
    pub num_contours: u16,
    /// Left side bearing in 26.6.
    pub lsb: i32,
    /// Advance width in 26.6.
    pub advance_width: i32,
    /// Bounding box in pixels (px units, not 26.6).
    pub xmin: i32,
    pub ymin: i32,
    pub xmax: i32,
    pub ymax: i32,
}

/// Fixed-point scaling factors.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScaleMetrics {
    /// X scale: ppem * 64 / units_per_em (26.6 factor).
    pub x_scale: i32,
    /// Y scale: ppem * 64 / units_per_em (26.6 factor).
    pub y_scale: i32,
    /// Pixels per em.
    pub ppem: u16,
}

/// FreeType FT_MulFix: multiply a 16.16 fixed-point value by a scale factor.
/// Returns the result in 26.6 format.
///
/// Exact FreeType semantics: makes both operands positive, computes
/// (a * b + 0x8000) >> 16, then negates if needed. This avoids negative
/// rounding issues.
#[inline]
pub(crate) fn mul_fix(a: i32, b: i32) -> i32 {
    let neg = (a ^ b) < 0;
    let ua = a.unsigned_abs() as u64;
    let ub = b.unsigned_abs() as u64;
    let c = ((ua * ub) + 0x8000) >> 16;
    if neg {
        -(c as i32)
    } else {
        c as i32
    }
}

/// FreeType FT_DivFix: compute (a << 16) / b with rounding bias.
/// Used for scale factor computation.
#[inline]
pub(crate) fn div_fix(a: i32, b: i32) -> i32 {
    if b == 0 {
        return 0;
    }
    // FT_DivFix: ((a << 16) + (b >> 1)) / b
    // The (b >> 1) adds 0.5 rounding to match FreeType's behavior
    let a64 = a as i64;
    let b64 = b as i64;
    (((a64 << 16) + (b64 >> 1)) / b64) as i32
}

/// Round 26.6 to nearest integer pixel (standard rounding).
#[inline]
pub(crate) fn pixel_round(x: i32) -> i32 {
    (x + 32) >> 6 // add 0.5 in 26.6, then truncate
}

/// Ceil 26.6 to integer pixel (round up, matching FreeType's ascender/descender).
#[inline]
pub(crate) fn pixel_ceil(x: i32) -> i32 {
    (x + 63) >> 6 // add (1.0 - epsilon) in 26.6, truncate = ceil
}

impl ScaleMetrics {
    /// Compute scale metrics from point size and font units_per_em.
    pub fn new(size_pt: f32, units_per_em: u16) -> Self {
        // ppem = size_pt (assume 72 DPI)
        let ppem = size_pt.ceil() as u16;
        let ppem_26dot6 = (ppem as i32) << 6; // ppem in 26.6
        let upe = units_per_em as i32;
        let x_scale = div_fix(ppem_26dot6, upe);
        let y_scale = div_fix(ppem_26dot6, upe);
        ScaleMetrics {
            x_scale,
            y_scale,
            ppem,
        }
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_x(&self, fu_x: i16) -> i32 {
        mul_fix(fu_x as i32, self.x_scale)
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_y(&self, fu_y: i16) -> i32 {
        mul_fix(fu_y as i32, self.y_scale)
    }
}

/// Scale a glyph outline to 26.6 coordinates.
pub(crate) fn scale_glyph(data: &FontData, glyph_index: u16) -> Result<ScaledGlyph, FontError> {
    let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);

    // Get metrics
    let h_metric = data.hmtx.get(glyph_index);
    let lsb = scale.scale_x(h_metric.lsb);
    let advance_width = scale.scale_x(h_metric.advance_width as i16);

    // Parse and scale the glyph outline
    let outline = parse_glyph(
        &data.glyf_data,
        &data.loca_data,
        data.loca_format,
        glyph_index,
    )?;

    if outline.num_contours == 0 {
        return Ok(ScaledGlyph {
            points: vec![],
            on_curve: vec![],
            end_pts: vec![],
            num_contours: 0,
            lsb,
            advance_width,
            xmin: 0,
            ymin: 0,
            xmax: 0,
            ymax: 0,
        });
    }

    let n = outline.points.len();
    let mut points = Vec::with_capacity(n);
    let mut on_curve = Vec::with_capacity(n);

    for p in &outline.points {
        let sx = scale.scale_x(p.x);
        let sy = scale.scale_y(p.y);
        points.push((sx, sy));
        on_curve.push(p.on_curve);
    }

    // Compute bbox from actual scaled point coordinates, not from the glyf
    // table header (which can be imprecise or use different conventions).
    // This ensures the pixel bbox matches what FreeType computes from the
    // actual outline points.
    let mut xmin_26 = i32::MAX;
    let mut ymin_26 = i32::MAX;
    let mut xmax_26 = i32::MIN;
    let mut ymax_26 = i32::MIN;
    for &(x, y) in &points {
        xmin_26 = xmin_26.min(x);
        ymin_26 = ymin_26.min(y);
        xmax_26 = xmax_26.max(x);
        ymax_26 = ymax_26.max(y);
    }
    if xmin_26 == i32::MAX {
        xmin_26 = 0;
        ymin_26 = 0;
        xmax_26 = 0;
        ymax_26 = 0;
    }

    Ok(ScaledGlyph {
        points,
        on_curve,
        end_pts: outline.end_pts_of_contours,
        num_contours: outline.num_contours,
        lsb,
        advance_width,
        // Use FreeType PIX_FLOOR for min, PIX_CEIL for max to match FT_BBox
        xmin: xmin_26 >> 6,        // FT_PIX_FLOOR / 64
        ymin: ymin_26 >> 6,        // FT_PIX_FLOOR / 64
        xmax: (xmax_26 + 63) >> 6, // FT_PIX_CEIL / 64
        ymax: (ymax_26 + 63) >> 6, // FT_PIX_CEIL / 64
    })
}

/// Scale a glyph outline and apply TrueType hinting.
pub(crate) fn scale_and_hint(
    data: &FontData,
    glyph_index: u16,
    engine: &mut HintingEngine,
) -> Result<ScaledGlyph, FontError> {
    let mut glyph = scale_glyph(data, glyph_index)?;
    if glyph.num_contours > 0 {
        engine.hint_glyph(data, glyph_index, &mut glyph);
    }
    Ok(glyph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_fix_basic() {
        let result = mul_fix(0x10000, 0x10000);
        assert_eq!(result, 0x10000);
    }

    #[test]
    fn mul_fix_half() {
        let result = mul_fix(0x8000, 0x20000);
        assert_eq!(result, 0x10000);
    }

    #[test]
    fn div_fix_computes_scale_factor() {
        let ppem_26dot6 = 16i32 << 6;
        let upe = 2048i32;
        let scale = div_fix(ppem_26dot6, upe);
        assert_eq!(scale, 0x8000);
    }

    #[test]
    fn pixel_round_exact() {
        assert_eq!(pixel_round(64), 1);
        assert_eq!(pixel_round(128), 2);
        assert_eq!(pixel_round(96), 2);
    }

    #[test]
    fn scale_metrics_from_16pt_2048upe() {
        let s = ScaleMetrics::new(16.0, 2048);
        assert_eq!(s.ppem, 16);
        assert_eq!(s.x_scale, 0x8000);
        assert_eq!(s.y_scale, 0x8000);
    }
}
