use super::super::*;
use super::*;

/// Return whether every destination pixel of an affine transform is outside
/// the source rectangle, so the device can emit the already-resolved fill word
/// without sampling or interpolating any source value.
///
/// This is a deliberately narrow identity proof for typed/raw packed modes.
/// For ordinary floating-point affine transforms, prove both the host f64
/// coordinates and the shader's f32 coordinates are outside by at least one
/// source pixel. Typed nearest modes use the signed-16.16 coordinate walk, so
/// their integer bounds are checked against the exact values uploaded to the
/// shader instead. No source bytes are inspected because the result is wholly
/// determined by Pillow's validated fill contract.
pub(in crate::compute::pool_gpu) fn gpu_transform_all_fill_is_exact(
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
        palette_fill,
        ..
    } = op
    else {
        return false;
    };
    if !matches!(method, TransformMethod::Affine)
        || *w == 0
        || *h == 0
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || data.len() < 6
        || palette_fill.is_some()
        || !matches!(
            mode,
            Some(
                "L" | "LA"
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
        )
    {
        return false;
    }

    let source_layout_is_valid = match mode {
        Some("I" | "F") => matches!(image, DynamicImage::ImageRgba8(_)),
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => {
            matches!(image, DynamicImage::ImageLuma16(_))
        }
        _ => gpu_image_layout_is_supported(image),
    };
    if !source_layout_is_valid {
        return false;
    }

    let coefficients = &data[..6];
    if coefficients
        .iter()
        .any(|value| !value.is_finite() || (*value as f32).is_infinite())
    {
        return false;
    }
    // Keep destination and source extents exactly representable in the
    // shader's f32 coordinate and dimension expressions. The actual buffer
    // limits are much smaller, but making the mathematical bound explicit
    // prevents a rounded endpoint from weakening the interval proof.
    const MAX_F32_EXACT_INTEGER: u32 = 1 << 24;
    if *w > MAX_F32_EXACT_INTEGER
        || *h > MAX_F32_EXACT_INTEGER
        || source_dimensions.0 > MAX_F32_EXACT_INTEGER
        || source_dimensions.1 > MAX_F32_EXACT_INTEGER
    {
        return false;
    }

    let outside = |minimum: f64, maximum: f64, extent: u32| {
        maximum <= -1.0 || minimum >= f64::from(extent) + 1.0
    };
    let bounds_f64 = |a: f64, b: f64, c: f64, center: f64| -> Option<(f64, f64)> {
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for dx in [0.0, f64::from(w.saturating_sub(1))] {
            for dy in [0.0, f64::from(h.saturating_sub(1))] {
                let value = a * (dx + center) + b * (dy + center) + c;
                if !value.is_finite() {
                    return None;
                }
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
        }
        Some((minimum, maximum))
    };

    let [a, b, c, d, e, f] = coefficients else {
        return false;
    };
    let center = if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        0.0
    } else {
        0.5
    };

    if gpu_transform_uses_nearest(mode, *filter) {
        // Keep this encoding byte-for-byte identical to `prepare_batch` and
        // `sample_nearest_fixed`; the integer bounds cover all four corners
        // and therefore every affine destination coordinate.
        let fixed = |value: f64| {
            let rounded = value.mul_add(65_536.0, 0.5);
            (rounded.is_finite() && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64)
                .then_some(rounded as i64)
        };
        let Some(step_x_x) = fixed(*a) else {
            return false;
        };
        let Some(step_y_x) = fixed(*b) else {
            return false;
        };
        let luma16_mode = matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"));
        if luma16_mode {
            // Keep the fill-only proof on the same exact coordinate plan as
            // the in-bounds mode-5 admission.  A fractional coefficient that
            // rounds differently in 16.16 could move one edge sample across
            // the nearest fill boundary even though the bulk of the canvas
            // is outside the source.
            let exactly_fixed = |value: f64| {
                let scaled = value * 65_536.0;
                scaled.is_finite()
                    && scaled.fract() == 0.0
                    && scaled >= i32::MIN as f64
                    && scaled <= i32::MAX as f64
            };
            if ![*a, *b, *c, *d, *e, *f].into_iter().all(exactly_fixed) {
                return false;
            }
        }
        let origin_x_value = if luma16_mode {
            *c + 0.5
        } else {
            *c + *a * 0.5 + *b * 0.5
        };
        let Some(origin_x) = fixed(origin_x_value) else {
            return false;
        };
        let Some(step_x_y) = fixed(*d) else {
            return false;
        };
        let Some(step_y_y) = fixed(*e) else {
            return false;
        };
        let origin_y_value = if luma16_mode {
            *f + 0.5
        } else {
            *f + *d * 0.5 + *e * 0.5
        };
        let Some(origin_y) = fixed(origin_y_value) else {
            return false;
        };
        if [step_x_x, step_y_x, origin_x, step_x_y, step_y_y, origin_y]
            .into_iter()
            .any(|value| i32::try_from(value).is_err())
        {
            return false;
        }
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
        let bounds_i128 = |origin: i64, step_x: i64, step_y: i64| {
            let x = i128::from(step_x) * i128::from(w.saturating_sub(1));
            let y = i128::from(step_y) * i128::from(h.saturating_sub(1));
            let values = [
                i128::from(origin),
                i128::from(origin) + x,
                i128::from(origin) + y,
                i128::from(origin) + x + y,
            ];
            let minimum = *values.iter().min()?;
            let maximum = *values.iter().max()?;
            Some((minimum, maximum))
        };
        let Some((min_x, max_x)) = bounds_i128(origin_x, step_x_x, step_y_x) else {
            return false;
        };
        let Some((min_y, max_y)) = bounds_i128(origin_y, step_x_y, step_y_y) else {
            return false;
        };
        let source_w = i128::from(source_dimensions.0) * 65_536;
        let source_h = i128::from(source_dimensions.1) * 65_536;
        if luma16_mode {
            // The native I;16 nearest sampler maps a fixed coordinate to
            // floor(source + 0.5), and the shader rejects negative fixed
            // coordinates before shifting.  These are the exact fill-only
            // boundaries for the mode-5 word path.
            return max_x < 0 || min_x >= source_w || max_y < 0 || min_y >= source_h;
        }
        return max_x <= -65_536
            || min_x >= source_w + 65_536
            || max_y <= -65_536
            || min_y >= source_h + 65_536;
    }

    let Some((min_x, max_x)) = bounds_f64(*a, *b, *c, center) else {
        return false;
    };
    let Some((min_y, max_y)) = bounds_f64(*d, *e, *f, center) else {
        return false;
    };
    let f32_coefficients = [
        *a as f32, *b as f32, *c as f32, *d as f32, *e as f32, *f as f32,
    ];
    // Evaluate the shader's actual f32 operation order at each affine
    // corner. Relying on the exact f64 polynomial here would miss a rounded
    // multiply/add that moves a coordinate back across the source edge.
    // Every individual term and the final sum must remain finite; otherwise
    // an intermediate infinity/NaN could defeat the shader's bounds check.
    let bounds_f32_shader = |a: f32, b: f32, c: f32, center: f32| -> Option<(f64, f64)> {
        let mut minimum = f32::INFINITY;
        let mut maximum = f32::NEG_INFINITY;
        for dx in [0.0f32, (w.saturating_sub(1)) as f32] {
            for dy in [0.0f32, (h.saturating_sub(1)) as f32] {
                let ax = dx + center;
                let ay = dy + center;
                let x_term = a * ax;
                let y_term = b * ay;
                // Check the unfused WGSL expression plus the possible
                // multiply-add contractions permitted by device compilers.
                // Requiring every form to stay on the same out-of-bounds
                // side keeps this proof independent of contraction choice.
                let values = [
                    (x_term + y_term) + c,
                    a.mul_add(ax, y_term) + c,
                    x_term + b.mul_add(ay, c),
                    a.mul_add(ax, b.mul_add(ay, c)),
                ];
                if !ax.is_finite()
                    || !ay.is_finite()
                    || !x_term.is_finite()
                    || !y_term.is_finite()
                    || values.iter().any(|value| !value.is_finite())
                {
                    return None;
                }
                for value in values {
                    minimum = minimum.min(value);
                    maximum = maximum.max(value);
                }
            }
        }
        Some((f64::from(minimum), f64::from(maximum)))
    };
    let Some((min_x_f32, max_x_f32)) = bounds_f32_shader(
        f32_coefficients[0],
        f32_coefficients[1],
        f32_coefficients[2],
        center as f32,
    ) else {
        return false;
    };
    let Some((min_y_f32, max_y_f32)) = bounds_f32_shader(
        f32_coefficients[3],
        f32_coefficients[4],
        f32_coefficients[5],
        center as f32,
    ) else {
        return false;
    };
    (outside(min_x, max_x, source_dimensions.0) || outside(min_y, max_y, source_dimensions.1))
        && (outside(min_x_f32, max_x_f32, source_dimensions.0)
            || outside(min_y_f32, max_y_f32, source_dimensions.1))
}

/// Lower a raw-word nearest rotate through the same fixed-point affine proof
/// used by typed/indexed transforms. The public rotate node is expanded to an
/// affine `Transform` only after Pillow's angle/center/translation geometry
/// has been materialized, so construct that exact intermediate here for the
/// admission check rather than widening every scalar-word rotate. PA carries
/// an opaque index/alpha pair just like its already-admitted affine path;
/// filtered floating-point rotation is intentionally excluded because it
/// interpolates sample values and therefore needs the ordered-f64 arithmetic
/// proof instead of a relocation proof.
pub(in crate::compute::pool_gpu) fn gpu_rotate_nearest_affine_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    if !matches!(mode, Some("CMYK" | "F" | "PA")) {
        return false;
    }
    let PipelineOp::Rotate {
        nearest,
        angle,
        expand,
        fill,
        center,
        translate,
        ..
    } = op
    else {
        return false;
    };
    if !*nearest {
        return false;
    }
    let Some((affine, (w, h))) = gpu_rotate_affine(
        *angle,
        *expand,
        *fill,
        *center,
        *translate,
        source_dimensions.0,
        source_dimensions.1,
    ) else {
        return false;
    };
    let transformed = PipelineOp::Transform {
        w,
        h,
        method: TransformMethod::Affine,
        data: Arc::from(affine.to_vec()),
        filter: ResampleFilter::Nearest,
        fill: *fill,
        fill_is_none: fill.is_none(),
        palette_fill: None,
    };
    gpu_nearest_affine_is_exact(&transformed, image, mode, source_dimensions)
}

pub(in crate::compute::pool_gpu) fn gpu_geometry_host_control_reason(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
    f_resize_constant_bits: Option<u32>,
    f_resize_box_copy_is_exact: bool,
    f_resize_identity_is_exact: bool,
    f_resize_box_average_is_exact: bool,
    f_resize_dyadic_is_exact: bool,
    f_resize_f64_is_exact: bool,
) -> Option<&'static str> {
    // Uniform transforms retain their existing exact source-selection proof.
    // Arbitrary projective/quad/mesh maps instead carry exact source geometry
    // into a bounded table and perform all sample arithmetic on the device.
    let rotate_needs_typed_control = gpu_rotate_requires_exact_host_control(image, mode);
    let mut dimensions = image.dimensions();
    for op in ops {
        if let PipelineOp::ResizeBoxed {
            w,
            h,
            filter,
            box_coords,
        } = op
        {
            let bounds = (
                f64::from(box_coords.0 as f32),
                f64::from(box_coords.1 as f32),
                f64::from(box_coords.2 as f32),
                f64::from(box_coords.3 as f32),
            );
            if bounds.0 < 0.0
                || bounds.1 < 0.0
                || bounds.2 > f64::from(dimensions.0 as f32)
                || bounds.3 > f64::from(dimensions.1 as f32)
                || bounds.2 < bounds.0
                || bounds.3 < bounds.1
            {
                return Some("ResizeBoxed float32 bounds are outside the source image");
            }
            if !gpu_resize_boxed_coefficients_are_safe(dimensions, (*w, *h), *box_coords, *filter) {
                return Some("ResizeBoxed coefficient tables exceed the GPU contract");
            }
            let horizontal_identity = *w == dimensions.0
                && box_coords.0 == 0.0
                && box_coords.2 == f64::from(dimensions.0);
            if !matches!(filter, ResampleFilter::Nearest)
                && u64::from(dimensions.1) > u64::from(dimensions.0) * 100
                && *h < dimensions.1
                && !horizontal_identity
            {
                return Some("ResizeBoxed requires Pillow vertical-first pass order");
            }
            if !matches!(filter, ResampleFilter::Nearest)
                && matches!(mode, Some("F" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N"))
            {
                return Some("ResizeBoxed typed ordered arithmetic is not proven");
            }
            let raw_alpha_mode = matches!(mode, Some("CMYK" | "La" | "PA" | "RGBa" | "RGBX"));
            if !matches!(filter, ResampleFilter::Nearest)
                && !raw_alpha_mode
                && matches!(
                    image.color(),
                    crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
                )
            {
                return Some("ResizeBoxed straight-alpha pass dependency is not proven");
            }
        }
        if let PipelineOp::Fit { w, h, filter, .. } = op {
            // Fit's fractional crop is lowered to the boxed Resample.c
            // contract. The shared device convolution plan is proven for
            // single-channel L and opaque RGB rows, plus the existing raw
            // channel layouts. Keep straight-alpha/typed rows on the exact
            // host path: tiny spans can expose stale horizontal
            // intermediates on Metal. P forces NEAREST before this decision.
            // Identity Fit is lowered to Duplicate and remains a native copy.
            // F nearest remains native only when both axes are reductions;
            // mixed/upsampling cases retain host control until their
            // two-pass device dependency is proven on all adapters.
            let native_f_nearest_fit = mode == Some("F")
                && matches!(filter, ResampleFilter::Nearest)
                && *w <= dimensions.0
                && *h <= dimensions.1;
            let native_fit_layout = matches!(
                mode,
                Some("L" | "RGB" | "P" | "PA" | "RGBX" | "RGBa" | "CMYK")
            ) || native_f_nearest_fit;
            if !native_fit_layout {
                return Some("Fit boxed-resize layout or pass dependency is not proven");
            }
        }
        if mode == Some("F")
            && let PipelineOp::Resize { w, h, .. } = op
            && gpu_f_resize_uses_pillow_tall_order(dimensions, (*w, *h))
        {
            // Pillow's Python-level tall-image optimization changes the
            // observable pass order to vertical-first. The current GPU
            // reducer is horizontal-first, so the CPU exact path owns these
            // rows until the alternate device plan is proven.
            return Some("F Resize requires Pillow vertical-first pass order");
        }
        let thumbnail_needs_control = matches!(op, PipelineOp::Thumbnail { .. })
            && gpu_thumbnail_requires_exact_host_control(op, dimensions, image, mode);
        let projective_transform_needs_control =
            matches!(
                op,
                PipelineOp::Transform {
                    method: TransformMethod::Perspective
                        | TransformMethod::Quad
                        | TransformMethod::Mesh,
                    ..
                }
            ) && !gpu_transform_geometry_is_admitted(op, mode, dimensions)
                && !gpu_projective_nearest_is_exact(op, image, mode, dimensions);
        let typed_transform_needs_control = rotate_needs_typed_control
            && matches!(op, PipelineOp::Transform { .. })
            && !gpu_nearest_affine_is_exact(op, image, mode, dimensions)
            && !gpu_transform_geometry_is_admitted(op, mode, dimensions)
            && !gpu_projective_nearest_is_exact(op, image, mode, dimensions)
            && !gpu_transform_all_fill_is_exact(op, image, mode, dimensions);
        if thumbnail_needs_control {
            return Some("Thumbnail reducing-gap or typed arithmetic is not proven");
        }
        if projective_transform_needs_control {
            return Some(
                "Transform projective sample layout or geometry storage exceeds the native contract",
            );
        }
        if typed_transform_needs_control {
            return Some("Transform typed-sample arithmetic is not proven");
        }
        if rotate_needs_typed_control && matches!(op, PipelineOp::Rotate { .. }) {
            return Some("Rotate typed-sample arithmetic is not proven");
        }
        if let Some(next) = op_output_dims(op, dimensions.0, dimensions.1) {
            dimensions = next;
        }
    }
    // Nearest resize only relocates complete four-byte samples for F mode;
    // the raw mode-8 shader preserves those bytes and the affine coordinate
    // contract is already represented by its native relocation path. I mode
    // uses the same opaque-word relocation, with host-generated one-tap
    // tables preserving Pillow's cumulative f64 source walk. Keep filtered I
    // resize on the exact host path until a typed convolution shader carries
    // its integer accumulator and rounding rules. F-mode filtered resize is
    // admitted only when one of the narrow F-mode proofs establishes an
    // exact bit-pattern copy, constant, 2:1 Box average, exact integer-
    // emulated/dyadic arithmetic reduction, or the marker-9 f64 reducer. The
    // marker-9 proof may carry complete-word relocation and nearest stages
    // between filtered resizes; every such intermediate is materialized in
    // the proof before the next reducer. Mixed F samples and unproved
    // arithmetic filters remain on this host-controlled path because ordinary
    // f32 convolution cannot reproduce Pillow's f64 rounding.
    let has_filtered_resize = ops.iter().any(|op| {
        matches!(
            op,
            PipelineOp::Resize { filter, .. }
                if !matches!(filter, ResampleFilter::Nearest)
        )
    });
    if (mode == Some("I") && has_filtered_resize && !f_resize_f64_is_exact)
        || (matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
            && has_filtered_resize
            && !gpu_luma16_resize_f64_is_exact(ops, image, mode))
        || (mode == Some("F")
            && has_filtered_resize
            && f_resize_constant_bits.is_none()
            && !f_resize_box_copy_is_exact
            && !f_resize_identity_is_exact
            && !f_resize_box_average_is_exact
            && !f_resize_dyadic_is_exact
            && !f_resize_f64_is_exact)
    {
        Some("Resize typed ordered arithmetic or coefficient storage is not proven")
    } else {
        None
    }
}

pub(in crate::compute::pool_gpu) fn validate_gpu_operations(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Result<(), PilError> {
    for op in ops {
        let i_mode_filter = mode == Some("I")
            && matches!(
                op,
                PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
            )
            && (gpu_int_filter_is_supported(ops, image)
                || gpu_int_filter_resize_chain_is_supported(ops, image));
        if !gpu_operation_is_safe(op) && !i_mode_filter {
            return Err(PilError::ValueError(format!(
                "GPU operation '{}' exceeds the bounded shader safety limits",
                registry::variant_key(op)
            )));
        }
    }
    Ok(())
}
