//! SIMD native-layout Draw family adapters.

mod kernels;

pub(super) use self::kernels::draw_line_vector_block;
use self::kernels::{
    simd_draw_arc_native, simd_draw_chord_native, simd_draw_ellipse_native, simd_draw_line_native,
    simd_draw_pieslice_native, simd_draw_points_native, simd_draw_polygon_native,
    simd_draw_rectangle_native, simd_draw_rounded_rect_native,
};
use super::simd_unsupported;
use super::{SimdImageShape, SimdLayout};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

/// Return the native byte layout accepted by drawing operations.
///
/// Drawing writes samples, rather than interpreting them as colors. That
/// allows the RGB-family logical modes backed by `Rgb8` and the CMYK-family
/// mode backed by `Rgba8` to stay in their native interleaved storage. An
/// explicit RGBA drawing context over an RGB image is also valid: the RGB
/// kernel blends the supplied alpha into the three destination samples.
pub(super) fn native_draw_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "RGBA" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn has_visible_draw_rectangle(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    outline_width: u32,
) -> bool {
    if !valid_draw_rectangle(width, height, x0, y0, x1, y1) {
        return false;
    }
    let has_ink = fill.is_some() || (outline.is_some() && outline_width != 0);
    has_ink
        && i64::from(x1) >= 0
        && i64::from(y1) >= 0
        && i64::from(x0) < i64::from(width)
        && i64::from(y0) < i64::from(height)
}

fn valid_draw_rectangle(width: u32, height: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let _ = (width, height);
    x1 >= x0 && y1 >= y0
}

fn line_bounds_intersect(width: u32, height: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let min_x = i64::from(x0.min(x1));
    let max_x = i64::from(x0.max(x1));
    let min_y = i64::from(y0.min(y1));
    let max_y = i64::from(y0.max(y1));
    width != 0
        && height != 0
        && max_x >= 0
        && max_y >= 0
        && min_x < i64::from(width)
        && min_y < i64::from(height)
}

fn has_visible_draw_point(width: u32, height: u32, points: &[(i32, i32)]) -> bool {
    points.iter().any(|&(x, y)| {
        x >= 0 && y >= 0 && i64::from(x) < i64::from(width) && i64::from(y) < i64::from(height)
    })
}

pub(super) fn shape_draw_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "RGBA" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        SimdLayout::Rgba8
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            Some(4)
        }
        SimdLayout::Luma16 | SimdLayout::Unsupported => None,
        _ => None,
    }
}

/// Check whether the native Draw adapters can execute this operation for an
/// already materialized image. The dispatcher routes one Draw family group
/// here so mode, vector-row, and geometry admission stay beside the adapters.
pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::DrawLine { x0, y0, x1, y1, .. } => {
            native_draw_layout(img, mode).is_some_and(|channels| {
                super::has_vectorized_byte_rows(img, channels)
                    && line_bounds_intersect(img.width(), img.height(), *x0, *y0, *x1, *y1)
                    && channels != 0
            })
        }
        PipelineOp::DrawPoint { points, .. } => {
            native_draw_layout(img, mode).is_some_and(|channels| {
                (points.is_empty() || super::has_vectorized_byte_rows(img, channels))
                    && (!has_visible_draw_point(img.width(), img.height(), points)
                        || super::has_vectorized_byte_rows(img, channels))
            })
        }
        PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. } => native_draw_layout(img, mode)
            .is_some_and(|channels| super::has_vectorized_byte_rows(img, channels)),
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            ..
        } => native_draw_layout(img, mode).is_some_and(|channels| {
            super::has_vectorized_byte_rows(img, channels)
                && *x1 >= *x0
                && *y1 >= *y0
                && radius.is_finite()
                && *radius >= 0.0
        }),
        PipelineOp::DrawPolygon { .. } => native_draw_layout(img, mode)
            .is_some_and(|channels| super::has_vectorized_byte_rows(img, channels)),
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => native_draw_layout(img, mode).is_some_and(|channels| {
            (img.width() == 0
                || img.height() == 0
                || super::has_vectorized_byte_rows(img, channels))
                && valid_draw_rectangle(img.width(), img.height(), *x0, *y0, *x1, *y1)
                && (has_visible_draw_rectangle(
                    img.width(),
                    img.height(),
                    *x0,
                    *y0,
                    *x1,
                    *y1,
                    *fill,
                    *outline,
                    *width,
                ) || fill.is_none() && (outline.is_none() || *width == 0)
                    || i64::from(*x1) < 0
                    || i64::from(*y1) < 0
                    || i64::from(*x0) >= i64::from(img.width())
                    || i64::from(*y0) >= i64::from(img.height()))
        }),
        _ => false,
    }
}

/// Check Draw admission for a planned image shape before materialization.
pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::DrawLine { x0, y0, x1, y1, .. } => shape_draw_channels(shape, mode)
            .is_some_and(|channels| {
                super::shape_has_vector_rows(shape, channels)
                    && line_bounds_intersect(shape.width, shape.height, *x0, *y0, *x1, *y1)
                    && channels != 0
            }),
        PipelineOp::DrawPoint { points, .. } => {
            shape_draw_channels(shape, mode).is_some_and(|channels| {
                (points.is_empty() || super::shape_has_vector_rows(shape, channels))
                    && (!has_visible_draw_point(shape.width, shape.height, points)
                        || super::shape_has_vector_rows(shape, channels))
            })
        }
        PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. } => shape_draw_channels(shape, mode)
            .is_some_and(|channels| super::shape_has_vector_rows(shape, channels)),
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            ..
        } => shape_draw_channels(shape, mode).is_some_and(|channels| {
            super::shape_has_vector_rows(shape, channels)
                && *x1 >= *x0
                && *y1 >= *y0
                && radius.is_finite()
                && *radius >= 0.0
        }),
        PipelineOp::DrawPolygon { .. } => shape_draw_channels(shape, mode)
            .is_some_and(|channels| super::shape_has_vector_rows(shape, channels)),
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => shape_draw_channels(shape, mode).is_some_and(|channels| {
            (shape.width == 0 || shape.height == 0 || super::shape_has_vector_rows(shape, channels))
                && valid_draw_rectangle(shape.width, shape.height, *x0, *y0, *x1, *y1)
                && (has_visible_draw_rectangle(
                    shape.width,
                    shape.height,
                    *x0,
                    *y0,
                    *x1,
                    *y1,
                    *fill,
                    *outline,
                    *width,
                ) || fill.is_none() && (outline.is_none() || *width == 0)
                    || i64::from(*x1) < 0
                    || i64::from(*y1) < 0
                    || i64::from(*x0) >= i64::from(shape.width)
                    || i64::from(*y0) >= i64::from(shape.height))
        }),
        _ => false,
    }
}

/// Draw returns native sample buffers for every supported operation.
pub(super) fn preserves_native_contract(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
    )
}

/// Draw a width-one line directly in the destination's native byte layout.
/// Geometry, clipping, and mode admission are scalar control work; contiguous
/// raster runs are filled/blended by the masked `u8x16` kernel.  Wide and typed
/// lines are rejected by preflight so they cannot silently enter the CPU draw
/// adapter.
pub(crate) fn simd_draw_line(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawLine {
        x0,
        y0,
        x1,
        y1,
        fill,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawLine op".into()));
    };
    simd_draw_line_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawLine"))
}

/// Draw one or more points through the destination's native byte layout.
pub(crate) fn simd_draw_point(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPoint {
        points,
        fill,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPoint op".into()));
    };
    let Some(channels) = native_draw_layout(img, mode) else {
        return Err(simd_unsupported("DrawPoint"));
    };
    simd_draw_points_native(img, points, *fill, *alpha_blend_rgb, channels)?
        .ok_or_else(|| simd_unsupported("DrawPoint"))
}

/// Draw a rectangle with scalar edge geometry and native-byte SIMD spans.
///
/// The outline's placement and clipping follow the CPU Pillow-compatible
/// primitive. Horizontal and one-pixel vertical spans use masked vector
/// stores, so the adapter never delegates a supported rectangle to the CPU
/// draw canvas or widens its samples through RGBA.
pub(crate) fn simd_draw_rectangle(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawRectangle {
        x0,
        y0,
        x1,
        y1,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawRectangle op".into()));
    };
    if native_draw_layout(img, mode).is_some()
        && valid_draw_rectangle(img.width(), img.height(), *x0, *y0, *x1, *y1)
        && !has_visible_draw_rectangle(
            img.width(),
            img.height(),
            *x0,
            *y0,
            *x1,
            *y1,
            *fill,
            *outline,
            *width,
        )
    {
        // A valid native-mode rectangle can be a public no-op (for example,
        // ImageDraw on L with both inks omitted) or can lie completely outside
        // the canvas. No pixel data needs a vector kernel in that case, but it
        // is still a SIMD control-plane result rather than a CPU fallback.
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(img.clone());
    }
    simd_draw_rectangle_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawRectangle"))
}

/// Draw a polygon through Pillow's scalar scanline geometry and native SIMD
/// span writes.  The current admission contract covers fills and the public
/// no-op cases where the outline is absent, equal to the fill, or zero-width.
/// A distinct outline is deliberately rejected during preflight because
/// Pillow masks wide polygon strokes against the fill; advertising that path
/// before a native masked-stroke kernel exists would reintroduce a hidden CPU
/// data path.
pub(crate) fn simd_draw_polygon(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPolygon {
        points,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPolygon op".into()));
    };
    // Pillow's floating-point ImagingDraw polygon path stores the fill sample
    // but ignores the outline. Keep the SIMD adapter aligned with
    // pool_cpu::ops::draw::op_draw_polygon instead of painting an extra stroke
    // that is observable only after materialization.
    let outline = if mode == Some("F") { None } else { *outline };
    simd_draw_polygon_native(img, points, *fill, outline, *width, *alpha_blend_rgb, mode)?
        .ok_or_else(|| simd_unsupported("DrawPolygon"))
}

/// Draw a rounded rectangle using scalar corner geometry and native SIMD
/// spans for fills, arcs, and straight edge sections.
pub(crate) fn simd_draw_rounded_rect(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawRoundedRect {
        x0,
        y0,
        x1,
        y1,
        radius,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawRoundedRect op".into()));
    };
    simd_draw_rounded_rect_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *radius,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawRoundedRect"))
}

/// Draw an ellipse through the scalar geometry/SIMD span split.
pub(crate) fn simd_draw_ellipse(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawEllipse {
        x0,
        y0,
        x1,
        y1,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawEllipse op".into()));
    };
    simd_draw_ellipse_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawEllipse"))
}

/// Draw a circle through the ellipse scan converter and native SIMD spans.
pub(crate) fn simd_draw_circle(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawCircle {
        cx,
        cy,
        radius,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawCircle op".into()));
    };
    simd_draw_ellipse_native(
        img,
        cx.saturating_sub(*radius),
        cy.saturating_sub(*radius),
        cx.saturating_add(*radius),
        cy.saturating_add(*radius),
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawCircle"))
}

/// Draw an arc through scalar clipping and native SIMD span writes.
pub(crate) fn simd_draw_arc(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawArc {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawArc op".into()));
    };
    simd_draw_arc_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawArc"))
}

/// Draw a chord through scalar clipping and native SIMD span writes.
pub(crate) fn simd_draw_chord(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawChord {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawChord op".into()));
    };
    simd_draw_chord_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawChord"))
}

/// Draw a pieslice through scalar clipping and native SIMD span writes.
pub(crate) fn simd_draw_pieslice(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPieslice {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPieslice op".into()));
    };
    simd_draw_pieslice_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawPieslice"))
}

#[cfg(test)]
mod tests;
