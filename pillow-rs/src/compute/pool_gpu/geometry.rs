use super::*;

/// Compute output dimensions for a size-changing op given current input dimensions.
/// Returns `None` if the op does not change the image dimensions.
pub(super) fn round_positive_ties_even(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let lower = value.floor();
    let fraction = value - lower;
    if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    }
}

/// Reproduce Pillow's reverse affine planner for `Image.rotate`.
///
/// The native implementation rounds the trigonometric coefficients to
/// fifteen decimal places, computes the expanded bounds from the transformed
/// image edges, and then applies the center-preserving expansion shift.  Keep
/// this control-plane calculation identical to the CPU/SIMD geometry helpers;
/// the resulting coefficients are consumed by the real GPU Transform shader.
pub(super) fn gpu_rotate_affine(
    angle: f64,
    expand: bool,
    _fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    width: u32,
    height: u32,
) -> Option<([f64; 6], (u32, u32))> {
    let sw = f64::from(width);
    let sh = f64::from(height);
    let rad = -angle.to_radians();
    let a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let e = a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut c = a * (-center_x - translate_x) + b * (-center_y - translate_y) + center_x;
    let mut f = d * (-center_x - translate_x) + e * (-center_y - translate_y) + center_y;
    let transform = |x: f64, y: f64, c: f64, f: f64| (a * x + b * y + c, d * x + e * y + f);
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in &corners {
        let (rx, ry) = transform(x, y, c, f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (out_w, out_h) = if expand {
        let w = max_x.ceil() - min_x.floor();
        let h = max_y.ceil() - min_y.floor();
        if !w.is_finite()
            || !h.is_finite()
            || w < 0.0
            || h < 0.0
            || w > f64::from(u32::MAX)
            || h > f64::from(u32::MAX)
        {
            return None;
        }
        (w as u32, h as u32)
    } else {
        (width, height)
    };
    if expand {
        let shift_x = -(f64::from(out_w) - sw) / 2.0;
        let shift_y = -(f64::from(out_h) - sh) / 2.0;
        (c, f) = transform(shift_x, shift_y, c, f);
    }
    Some(([a, b, c, d, e, f], (out_w, out_h)))
}

/// Return the floating-point source crop box used by ImageOps.fit.
///
/// Fit's public contract resamples this box directly; rounding it to integer
/// crop coordinates before resizing changes edge pixels for non-centred or
/// bleed-enabled calls. The exact device path uses these values to build the
/// same boxed coefficient tables as the scalar implementation.
pub(super) fn gpu_fit_box(
    source_w: u32,
    source_h: u32,
    output_w: u32,
    output_h: u32,
    bleed: f64,
    centering: (f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    if source_w == 0 || source_h == 0 || output_w == 0 || output_h == 0 {
        return None;
    }
    let b = if (0.0..0.5).contains(&bleed) {
        bleed
    } else {
        0.0
    };
    let source_w = f64::from(source_w);
    let source_h = f64::from(source_h);
    let output_w = f64::from(output_w);
    let output_h = f64::from(output_h);
    let bleed_w = b * source_w;
    let bleed_h = b * source_h;
    // Pillow keeps the exact positive live dimensions here.  Clamping either
    // axis to one changes the fractional box for small images/high bleed and
    // sends NEAREST to the wrong source sample.
    let live_w = source_w - 2.0 * bleed_w;
    let live_h = source_h - 2.0 * bleed_h;
    let live_ratio = live_w / live_h;
    let output_ratio = output_w / output_h;
    let (crop_w, crop_h) = if (live_ratio - output_ratio).abs() < 1e-10 {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        (output_ratio * live_h, live_h)
    } else {
        (live_w, live_w / output_ratio)
    };
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let left = bleed_w + (live_w - crop_w) * cx;
    let top = bleed_h + (live_h - crop_h) * cy;
    let values = (left, top, crop_w, crop_h);
    [values.0, values.1, values.2, values.3]
        .into_iter()
        .all(|value| value.is_finite() && (value as f32).is_finite())
        .then_some(values)
}

pub(super) fn op_output_dims(op: &PipelineOp, cur_w: u32, cur_h: u32) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. } => Some((*w, *h)),
        PipelineOp::ResizeBoxed { w, h, .. } => Some((*w, *h)),
        PipelineOp::Contain { .. } | PipelineOp::Cover { .. } => {
            gpu_contain_cover_output_dims(op, cur_w, cur_h)
        }
        PipelineOp::Pad { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_add(border)?;
            let new_h = cur_h.checked_add(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_sub(border)?;
            let new_h = cur_h.checked_sub(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::Rotate {
            angle,
            expand,
            fill,
            center,
            translate,
            ..
        } => gpu_rotate_affine(*angle, *expand, *fill, *center, *translate, cur_w, cur_h)
            .map(|(_, dimensions)| dimensions),
        PipelineOp::Thumbnail { w, h, .. } => gpu_thumbnail_output_dims(cur_w, cur_h, *w, *h),
        PipelineOp::Fit { w, h, .. } => Some((*w, *h)),
        PipelineOp::Transpose { method } => {
            if matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            ) {
                Some((cur_h, cur_w))
            } else {
                Some((cur_w, cur_h))
            }
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            let fx = (*x_factor).max(1);
            let fy = (*y_factor).max(1);
            Some((cur_w.div_ceil(fx), cur_h.div_ceil(fy)))
        }
        PipelineOp::Scale { factor, .. } => {
            // ImageOps.scale uses Python's round(width * factor), including
            // ties-to-even at half-pixel products.
            let new_w = round_positive_ties_even(f64::from(cur_w) * factor);
            let new_h = round_positive_ties_even(f64::from(cur_h) * factor);
            if !new_w.is_finite()
                || !new_h.is_finite()
                || new_w < 0.0
                || new_h < 0.0
                || new_w > f64::from(u32::MAX)
                || new_h > f64::from(u32::MAX)
            {
                return None;
            }
            let new_w = new_w.max(1.0) as u32;
            let new_h = new_h.max(1.0) as u32;
            Some((new_w, new_h))
        }
        PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::CompositeModule { other, .. } => other.size().ok(),
        _ => None,
    }
}

/// Compute the scalar aspect-ratio result used by ImageOps.contain/cover.
///
/// These public operations do not have a distinct pixel algorithm: after
/// Pillow chooses the rounded output size they call the ordinary resize path.
/// Keep the calculation in the GPU planner identical to the CPU ImageOps
/// implementation, including ties-to-even rounding and Cover's minimum size
/// clamp. Returning `None` for zero/invalid geometry leaves the original public
/// operation on its normal CPU/error path.
pub(super) fn gpu_contain_cover_output_dims(
    op: &PipelineOp,
    source_w: u32,
    source_h: u32,
) -> Option<(u32, u32)> {
    let (target_w, target_h, is_cover) = match op {
        PipelineOp::Contain { w, h, .. } => (*w, *h, false),
        PipelineOp::Cover { w, h, .. } => (*w, *h, true),
        _ => return None,
    };
    if source_w == 0 || source_h == 0 || target_w == 0 || target_h == 0 {
        return None;
    }

    let image_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(target_w) / f64::from(target_h);
    if (image_ratio - destination_ratio).abs() < 1e-10 {
        return Some((target_w, target_h));
    }

    let adjust_height = (image_ratio > destination_ratio) != is_cover;
    if adjust_height {
        let height = round_positive_ties_even(
            f64::from(source_h) / f64::from(source_w) * f64::from(target_w),
        ) as u32;
        Some((target_w, if is_cover { height.max(1) } else { height }))
    } else {
        let width = round_positive_ties_even(
            f64::from(source_w) / f64::from(source_h) * f64::from(target_h),
        ) as u32;
        Some((if is_cover { width.max(1) } else { width }, target_h))
    }
}

/// Return the exact intermediate resize size and integer placement used by
/// ImageOps.pad.  Pillow performs contain sizing with ties-to-even rounding,
/// then chooses the axis-dependent rounded paste offset.  The values are
/// scalar control-plane decisions; the resize and final fill/copy stay on the
/// device.
pub(super) fn gpu_pad_geometry(
    op: &PipelineOp,
    source_w: u32,
    source_h: u32,
) -> Option<((u32, u32), (u32, u32))> {
    let PipelineOp::Pad {
        w, h, centering, ..
    } = op
    else {
        return None;
    };
    if source_w == 0 || source_h == 0 || *w == 0 || *h == 0 {
        return None;
    }

    let image_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(*w) / f64::from(*h);
    let (raw_w, raw_h) = if (image_ratio - destination_ratio).abs() < 1e-10 {
        (f64::from(*w), f64::from(*h))
    } else if image_ratio > destination_ratio {
        (
            f64::from(*w),
            round_positive_ties_even(f64::from(source_h) / f64::from(source_w) * f64::from(*w)),
        )
    } else {
        (
            round_positive_ties_even(f64::from(source_w) / f64::from(source_h) * f64::from(*h)),
            f64::from(*h),
        )
    };
    if !raw_w.is_finite()
        || !raw_h.is_finite()
        || raw_w < 0.0
        || raw_h < 0.0
        || raw_w > f64::from(u32::MAX)
        || raw_h > f64::from(u32::MAX)
    {
        return None;
    }
    let raw_w_u32 = raw_w as u32;
    let raw_h_u32 = raw_h as u32;
    // Pillow passes the rounded contain dimensions directly to Image.resize;
    // a zero axis raises ValueError instead of being clamped to one. Keep the
    // GPU geometry helper out of that invalid domain so strict dispatch uses
    // the same public error path.
    if raw_w_u32 == 0 || raw_h_u32 == 0 {
        return None;
    }
    let resize_w = raw_w_u32;
    let resize_h = raw_h_u32;
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let (offset_x, offset_y) = if raw_w_u32 != *w {
        (
            round_positive_ties_even(f64::from(*w - raw_w_u32) * cx) as u32,
            0,
        )
    } else {
        (
            0,
            round_positive_ties_even(f64::from(*h - raw_h_u32) * cy) as u32,
        )
    };
    Some(((resize_w, resize_h), (offset_x, offset_y)))
}

/// Return Pad geometry admitted by the RGBX fused vertical-resize path.
/// Horizontal resize remains the existing exact kernel; the second dispatch
/// writes both the resized rows and the pad fill directly into the destination
/// canvas. Keep this narrow to non-nearest resizes whose contain image spans
/// the canvas width, leaving identity and horizontally padded cases unchanged.
pub(super) fn gpu_pad_rgbx_fused_vertical_geometry(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Option<((u32, u32), (u32, u32))> {
    if logical_mode != Some("RGBX") {
        return None;
    }
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return None;
    };
    if matches!(filter, ResampleFilter::Nearest) {
        return None;
    }

    let (resize_dimensions, offsets) =
        gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)?;
    let (resize_w, resize_h) = resize_dimensions;
    let (offset_x, offset_y) = offsets;
    if resize_dimensions == source_dimensions || resize_w != *w || offset_x != 0 {
        return None;
    }
    if offset_y
        .checked_add(resize_h)
        .is_none_or(|end_y| end_y > *h)
    {
        return None;
    }

    Some((resize_dimensions, offsets))
}

pub(super) fn gpu_pad_fill(op: &PipelineOp, logical_mode: Option<&str>, mode: u32) -> u32 {
    let PipelineOp::Pad { color, .. } = op else {
        return 0;
    };
    // I and F store one scalar sample in the packed four-byte transport. The
    // public color resolver has already converted scalar/named colors to
    // that representation; preserve the complete word instead of
    // interpreting the bytes as RGB channels. An omitted fill is scalar
    // +0.0/+0, not an opaque RGBA black.
    if matches!(logical_mode, Some("I" | "F")) {
        let (a, b, c, d) = color.unwrap_or((0, 0, 0, 0));
        return u32::from(a) | (u32::from(b) << 8) | (u32::from(c) << 16) | (u32::from(d) << 24);
    }
    // Pillow creates the destination in the source mode before pasting, so
    // omitted color uses zero for packed four-byte modes. RGBX's X and CMYK's
    // K are not alpha, but their fourth stored samples still default to zero.
    let zero_fourth_default = matches!(
        logical_mode,
        Some("LA" | "PA" | "RGBA" | "RGBa" | "RGBX" | "CMYK")
    ) || matches!(mode, 1 | 3);
    let (r, g, b, a) = color.unwrap_or((0, 0, 0, if zero_fourth_default { 0 } else { 255 }));
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16) | (u32::from(a) << 24)
}

/// Return true for operations whose output dimensions are not simply the
/// current input dimensions. `op_output_dims` returns `None` for an overflow or
/// an unavailable nested-image size, so callers must distinguish that from a
/// genuinely dimension-preserving operation before dispatching.
pub(super) fn op_has_explicit_output_dimensions(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Resize { .. }
            | PipelineOp::ResizeBoxed { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::CompositeModule { .. }
    )
}

pub(super) fn auxiliary_dimensions(image: Option<&DynamicImage>) -> Option<(u32, u32)> {
    image.map(DynamicImage::dimensions)
}
