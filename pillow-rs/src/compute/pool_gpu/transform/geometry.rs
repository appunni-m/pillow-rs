use super::super::*;

/// Return whether the current packed geometry path still needs the exact
/// host implementation for this concrete logical sample contract. The GPU
/// geometry kernels are intentionally retained for ordinary byte-mode work,
/// while Pillow's Thumbnail/fit reducing-gap and typed F/I convolution paths
/// have additional rounding/storage rules. Proven typed I;16 filtered
/// resizes are admitted by their two-pass reducer proof; every other typed
/// geometry remains on exact host semantic control until its device plan is
/// proven.
/// Return whether a rotate node can use the exact byte-relocation lowering.
///
/// Pillow's right-angle fast paths are precisely the byte-relocation
/// transpose contract, regardless of the requested resampling filter. A
/// 180-degree rotation is a fast path with either expansion setting; 90/270
/// are fast paths when expansion is requested or the source is square.
/// Keeping this predicate deliberately narrow is important: the general
/// affine shader still has different pixel-center and fill semantics for
/// fractional angles, rectangular non-expanded rotations, and custom
/// geometry.
pub(in crate::compute::pool_gpu) fn gpu_rotate_has_exact_transpose_lowering(
    op: &PipelineOp,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Rotate {
        angle,
        expand,
        center,
        translate,
        ..
    } = op
    else {
        return false;
    };
    if center.is_some() || translate.is_some() {
        return false;
    }
    if mode.is_some_and(|value| {
        !matches!(
            value,
            "1" | "P"
                | "PA"
                | "L"
                | "LA"
                | "RGB"
                | "RGBA"
                | "RGBX"
                | "RGBa"
                | "CMYK"
                | "HSV"
                | "YCbCr"
                | "I"
                | "F"
                | "I;16"
                | "I;16L"
                | "I;16B"
                | "I;16N"
        )
    }) {
        return false;
    }
    // Transpose is a complete-word relocation.  The shader's mode-7/8
    // branches preserve all four bytes of I/F samples, while mode 5 keeps the
    // native two-byte I;16 payload in the low word (the typed readback drops
    // the transport padding).  These modes therefore have the same exact
    // right-angle contract as packed byte layouts; filtered/fractional
    // rotations remain on their typed semantic paths.
    let normalized = angle.rem_euclid(360.0);
    if (normalized - 180.0).abs() <= f64::EPSILON {
        return true;
    }
    ((normalized - 90.0).abs() <= f64::EPSILON || (normalized - 270.0).abs() <= f64::EPSILON)
        && (*expand || source_dimensions.0 == source_dimensions.1)
}

pub(in crate::compute::pool_gpu) fn gpu_rotate_requires_exact_host_control(
    image: &DynamicImage,
    mode: Option<&str>,
) -> bool {
    // The affine shader is byte-exact for the ordinary packed L/LA/RGB/RGBA
    // layouts. Pillow's typed, indexed, and palette-alpha modes use a
    // different sample contract (or a palette lookup) even when their
    // temporary storage happens to be four bytes per pixel.
    mode.is_some_and(|value| !matches!(value, "L" | "LA" | "RGB" | "RGBA"))
        || !gpu_image_layout_is_supported(image)
}

/// Return whether a typed, indexed, palette-alpha, or raw packed nearest
/// affine transform can use the fixed-point relocation shader without changing
/// Pillow's source selection.
///
/// `Geometry.c` computes the six affine values as signed 16.16 integers and
/// then walks each output row with integer additions.  The GPU shader carries
/// those same integers, but its arithmetic is i32 while the scalar path uses
/// i64.  Keep the admission proof deliberately explicit: every coefficient,
/// row origin, and absolute output-coordinate sum must fit i32, and the
/// source dimensions must fit the shader's signed bounds checks.  The typed
/// sample itself remains an opaque word (or an index/alpha pair for `PA`), so
/// no integer/float conversion or approximation is introduced.  I;16* uses
/// Pillow's separate nearest coordinate contract (rounding
/// `a*dx+b*dy+c` with `floor(+0.5)`), so its coefficients must be exactly
/// representable in the uploaded 16.16 plan. F uses the ordinary affine
/// center/truncation contract; it is admitted only after every f64 source
/// selection agrees with that uploaded fixed-point walk.
pub(in crate::compute::pool_gpu) fn gpu_nearest_affine_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform { w, h, data, .. } = op else {
        return false;
    };
    if !gpu_nearest_affine_request_is_supported(op, image, mode, source_dimensions) {
        return false;
    }
    let float_mode = mode == Some("F");
    let luma16_mode = matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"));

    // Keep this expression byte-for-byte equivalent to the fixed-point
    // encoding in `prepare_batch` and `affine_nearest_fixed`.
    let fixed = |value: f64| {
        let rounded = value.mul_add(65_536.0, 0.5).floor();
        rounded.is_finite().then_some(rounded as i64)
    };
    let [a, b, c, d, e, f] = [data[0], data[1], data[2], data[3], data[4], data[5]];
    let Some(step_x_x) = fixed(a) else {
        return false;
    };
    let Some(step_y_x) = fixed(b) else {
        return false;
    };
    let origin_x_value = if luma16_mode {
        c + 0.5
    } else {
        c + a * 0.5 + b * 0.5
    };
    let Some(origin_x) = fixed(origin_x_value) else {
        return false;
    };
    let Some(step_x_y) = fixed(d) else {
        return false;
    };
    let Some(step_y_y) = fixed(e) else {
        return false;
    };
    let origin_y_value = if luma16_mode {
        f + 0.5
    } else {
        f + d * 0.5 + e * 0.5
    };
    let Some(origin_y) = fixed(origin_y_value) else {
        return false;
    };
    if luma16_mode {
        // The I;16 CPU path evaluates its f64 affine expression directly and
        // rounds the resulting source coordinate with `floor(value + 0.5)`.
        // Admit only coefficients already exact in the shader's 16.16 plan;
        // otherwise quantizing a fractional coefficient can change the
        // selected source word at a boundary even when no i32 overflow is
        // possible.
        let exactly_fixed = |value: f64| {
            let scaled = value * 65_536.0;
            scaled.is_finite()
                && scaled.fract() == 0.0
                && scaled >= i32::MIN as f64
                && scaled <= i32::MAX as f64
        };
        if ![a, b, c, d, e, f].into_iter().all(exactly_fixed) {
            return false;
        }
    }
    if [step_x_x, step_y_x, origin_x, step_x_y, step_y_y, origin_y]
        .into_iter()
        .any(|value| i32::try_from(value).is_err())
    {
        return false;
    }

    // Bound both the final coordinate and the intermediate addends.  The
    // absolute-sum form is conservative, but lets the shader use ordinary
    // i32 arithmetic with no wraparound while preserving the scalar order.
    let max_x = i128::from(w.saturating_sub(1));
    let max_y = i128::from(h.saturating_sub(1));
    let fits_i32 = |origin: i64, step_x: i64, step_y: i64| {
        i128::from(origin).abs()
            + i128::from(step_x).abs() * max_x
            + i128::from(step_y).abs() * max_y
            <= i128::from(i32::MAX)
    };
    if !fits_i32(origin_x, step_x_x, step_y_x) || !fits_i32(origin_y, step_x_y, step_y_y) {
        return false;
    }

    if !float_mode {
        return true;
    }

    gpu_float_affine_source_selection_matches(
        source_dimensions,
        (*w, *h),
        [a, b, c, d, e, f],
        [origin_x, step_x_x, step_y_x, origin_y, step_x_y, step_y_y],
    )
}

fn gpu_nearest_affine_request_is_supported(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        ..
    } = op
    else {
        return false;
    };
    let typed_mode = mode == Some("I");
    let float_mode = mode == Some("F");
    let luma16_mode = matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"));
    let indexed_mode = matches!(mode, Some("1" | "P"));
    let palette_alpha_mode = mode == Some("PA");
    // CMYK is four raw channel bytes in the same Rgba8 transport. Unlike
    // RGBA, its fourth byte is K rather than alpha, and the mode-4 nearest
    // shader branch copies all four bytes without color conversion.
    let raw_packed_mode = mode == Some("CMYK");
    if (!typed_mode
        && !float_mode
        && !luma16_mode
        && !indexed_mode
        && !palette_alpha_mode
        && !raw_packed_mode)
        || (typed_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        || (float_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        // The mode-8 shader's affine branch copies complete float words only
        // for an explicit nearest request. Pillow's filtered F transform
        // contract interpolates scalar values; retaining those rows on the
        // exact host path avoids silently treating a bilinear request as a
        // nearest relocation.
        || (float_mode && !matches!(filter, ResampleFilter::Nearest))
        || (luma16_mode && !matches!(image, DynamicImage::ImageI16(_)))
        || (indexed_mode && !matches!(image, DynamicImage::ImageLuma8(_)))
        || (palette_alpha_mode && !matches!(image, DynamicImage::ImageLumaA8(_)))
        || (raw_packed_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        || !matches!(method, TransformMethod::Affine)
        || (!gpu_transform_uses_nearest(mode, *filter) && !luma16_mode)
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_dimensions.0 > i32::MAX as u32
        || source_dimensions.1 > i32::MAX as u32
        || *w > i32::MAX as u32
        || *h > i32::MAX as u32
        || *w == 0
        || *h == 0
        || (float_mode
            && u64::from(*w).saturating_mul(u64::from(*h))
                > GPU_F_AFFINE_PROOF_MAX_PIXELS as u64)
        || data.len() < 6
        || data[..6]
            .iter()
            .any(|value| !value.is_finite() || (*value as f32).is_infinite())
    {
        return false;
    }
    true
}

fn gpu_float_affine_source_selection_matches(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    matrix: [f64; 6],
    fixed_coordinates: [i64; 6],
) -> bool {
    let [a, b, c, d, e, f] = matrix;
    let [origin_x, step_x_x, step_y_x, origin_y, step_x_y, step_y_y] = fixed_coordinates;
    // F affine nearest uses the same opaque-word sampler as I, but its CPU
    // reference evaluates the original f64 matrix at each pixel before
    // truncating non-negative coordinates. The GPU path instead walks the
    // quantized signed-16.16 plan above. Compare the resulting in-bounds
    // source selection for every output pixel; equal fill classifications are
    // sufficient when both coordinates are outside the source rectangle.
    let host_index = |value: f64| -> Option<i64> {
        if !value.is_finite() || value < 0.0 {
            return Some(-1);
        }
        (value <= i64::MAX as f64).then_some(value as i64)
    };
    let fixed_index = |value: i64| -> i64 { if value < 0 { -1 } else { value >> 16 } };
    let source_width = i64::from(source_dimensions.0);
    let source_height = i64::from(source_dimensions.1);
    for dy in 0..output_dimensions.1 {
        for dx in 0..output_dimensions.0 {
            let dx_f = f64::from(dx);
            let dy_f = f64::from(dy);
            let host_x = a * (dx_f + 0.5) + b * (dy_f + 0.5) + c;
            let host_y = d * (dx_f + 0.5) + e * (dy_f + 0.5) + f;
            let Some(host_x) = host_index(host_x) else {
                return false;
            };
            let Some(host_y) = host_index(host_y) else {
                return false;
            };
            let fixed_x = i128::from(origin_x)
                + i128::from(step_x_x) * i128::from(dx)
                + i128::from(step_y_x) * i128::from(dy);
            let fixed_y = i128::from(origin_y)
                + i128::from(step_x_y) * i128::from(dx)
                + i128::from(step_y_y) * i128::from(dy);
            let fixed_x = i64::try_from(fixed_x).ok();
            let fixed_y = i64::try_from(fixed_y).ok();
            let (Some(fixed_x), Some(fixed_y)) = (fixed_x, fixed_y) else {
                return false;
            };
            let device_x = fixed_index(fixed_x);
            let device_y = fixed_index(fixed_y);
            let host_sample =
                (host_x >= 0 && host_x < source_width && host_y >= 0 && host_y < source_height)
                    .then_some((host_x, host_y));
            let device_sample = (device_x >= 0
                && device_x < source_width
                && device_y >= 0
                && device_y < source_height)
                .then_some((device_x, device_y));
            if host_sample != device_sample {
                return false;
            }
        }
    }
    true
}
