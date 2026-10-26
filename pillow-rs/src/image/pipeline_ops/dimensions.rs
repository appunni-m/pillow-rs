use crate::pipeline::{PipelineOp, TransposeMethod};

fn round_dimension(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return None;
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    (rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
}

fn known_rotated_dimensions(
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    (width, height): (u32, u32),
) -> Option<(u32, u32)> {
    if !expand {
        return Some((width, height));
    }
    // Pillow takes an exact transpose fast path for right-angle rotations
    // before constructing the affine matrix, regardless of the requested
    // resampling filter. Reflect that public shape here so lazy `size`
    // agrees with the materialized result.
    let normalized = angle.rem_euclid(360.0);
    if center.is_none()
        && translate.is_none()
        && [90.0, 270.0]
            .into_iter()
            .any(|right_angle| (normalized - right_angle).abs() <= f64::EPSILON)
    {
        return Some((height, width));
    }
    let sw = f64::from(width);
    let sh = f64::from(height);
    let radians = -angle.to_radians();
    let affine_a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
    let affine_b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
    let affine_d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
    let affine_e = affine_a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let affine_c =
        affine_a * (-center_x - translate_x) + affine_b * (-center_y - translate_y) + center_x;
    let affine_f =
        affine_d * (-center_x - translate_x) + affine_e * (-center_y - translate_y) + center_y;
    let transform = |x: f64, y: f64| {
        (
            affine_a * x + affine_b * y + affine_c,
            affine_d * x + affine_e * y + affine_f,
        )
    };
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in &corners {
        let (rotated_x, rotated_y) = transform(x, y);
        min_x = min_x.min(rotated_x);
        max_x = max_x.max(rotated_x);
        min_y = min_y.min(rotated_y);
        max_y = max_y.max(rotated_y);
    }
    Some((
        round_dimension(max_x.ceil() - min_x.floor())?,
        round_dimension(max_y.ceil() - min_y.floor())?,
    ))
}

fn known_contain_cover_dimensions(
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
    cover: bool,
) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    let image_ratio = f64::from(width) / f64::from(height);
    let destination_ratio = f64::from(target_width) / f64::from(target_height);
    if (image_ratio - destination_ratio).abs() < 1e-10 {
        return Some((target_width, target_height));
    }
    if (image_ratio > destination_ratio) != cover {
        let new_height =
            round_dimension(f64::from(height) / f64::from(width) * f64::from(target_width))?;
        Some((
            target_width,
            if cover { new_height.max(1) } else { new_height },
        ))
    } else {
        let new_width =
            round_dimension(f64::from(width) / f64::from(height) * f64::from(target_height))?;
        Some((
            if cover { new_width.max(1) } else { new_width },
            target_height,
        ))
    }
}

fn known_transposed_dimensions(method: &TransposeMethod, width: u32, height: u32) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        TransposeMethod::FlipLeftRight
        | TransposeMethod::FlipTopBottom
        | TransposeMethod::Rotate180 => (width, height),
    }
}

/// Computes the output shape for a lazy operation without touching pixels.
///
/// The match is intentionally conservative: operations with secondary images,
/// validation that currently occurs during execution, or mode-specific storage
/// transitions return `None` and retain the existing materialization fallback.
pub fn known_pipeline_op_dimensions(
    op: &PipelineOp,
    (width, height): (u32, u32),
) -> Option<(u32, u32)> {
    if is_known_dimension_preserving_op(op) {
        return Some((width, height));
    }

    match op {
        PipelineOp::Resize { w, h, .. }
        | PipelineOp::ResizeBoxed { w, h, .. }
        | PipelineOp::Thumbnail { w, h, .. }
        | PipelineOp::Pad { w, h, .. }
        | PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_add(border)?, height.checked_add(border)?))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_sub(border)?, height.checked_sub(border)?))
        }
        PipelineOp::Transpose { method } => {
            Some(known_transposed_dimensions(method, width, height))
        }
        PipelineOp::Rotate {
            angle,
            expand,
            center,
            translate,
            ..
        } => known_rotated_dimensions(*angle, *expand, *center, *translate, (width, height)),
        PipelineOp::Reduce { x_factor, y_factor } => Some((
            width.div_ceil((*x_factor).max(1)),
            height.div_ceil((*y_factor).max(1)),
        )),
        PipelineOp::Scale { factor, .. } => Some((
            round_dimension(f64::from(width) * *factor)?.max(1),
            round_dimension(f64::from(height) * *factor)?.max(1),
        )),
        PipelineOp::Contain { w, h, .. } | PipelineOp::Cover { w, h, .. } => {
            known_contain_cover_dimensions(
                width,
                height,
                *w,
                *h,
                matches!(op, PipelineOp::Cover { .. }),
            )
        }
        _ => None,
    }
}

#[inline]
fn is_known_dimension_preserving_op(op: &PipelineOp) -> bool {
    // These unary operations have no deferred dimension or secondary-image
    // validation. Keeping their shape known lets point/filter/draw chains
    // answer `size()` without replaying pixels.
    matches!(
        op,
        PipelineOp::Convert { .. }
            | PipelineOp::ConvertLab { .. }
            | PipelineOp::RemapPalette { .. }
            | PipelineOp::Color3DLut { .. }
            | PipelineOp::ExtractBand { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Autocontrast { .. }
            | PipelineOp::Equalize
            | PipelineOp::EqualizeMasked { .. }
            | PipelineOp::Invert
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Posterize { .. }
            | PipelineOp::Solarize { .. }
            | PipelineOp::Grayscale
            | PipelineOp::Colorize { .. }
            | PipelineOp::Constant { .. }
            // Public putpixel validates coordinates before queuing and cannot
            // change dimensions. Knowing the shape avoids materializing prior writes.
            | PipelineOp::PutPixel { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Duplicate
            | PipelineOp::InvertChops
            | PipelineOp::Brightness { .. }
            | PipelineOp::Contrast { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Sharpness { .. }
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::Eval { .. }
            | PipelineOp::EffectNoise { .. }
            | PipelineOp::DrawLine { .. }
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
