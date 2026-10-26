use super::*;

/// `F` stores one little-endian `f32` sample in each four-byte word. The
/// order-statistic shaders have a separate mode-8 path that compares those
/// samples as floats instead of independently comparing their four bytes.
/// Require finite source and raw `PutData(F)` words for now: this gives WGSL
/// ordering the same total behavior as Pillow's finite-value path while NaN
/// ordering remains an explicit future contract rather than an accidental
/// driver-dependent result.
pub(super) fn gpu_float_filter_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .and_then(|count| count.checked_mul(4));
    let finite_words = |bytes: &[u8]| {
        expected == Some(bytes.len())
            && bytes.chunks_exact(4).remainder().is_empty()
            && bytes
                .chunks_exact(4)
                .all(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]).is_finite())
    };
    expected == Some(pixels.as_raw().len())
        && !ops.is_empty()
        && finite_words(pixels.as_raw())
        && ops.iter().all(|op| match op {
            PipelineOp::Mirror => true,
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => finite_words(data),
            PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::RankFilter { .. } => gpu_operation_is_safe(op),
            _ => false,
        })
}

/// `I` stores one signed little-endian i32 sample in each four-byte word.
/// Its convolution contract is different from the byte filter: coefficients
/// are applied to the decoded sample, negative results clamp to zero, and
/// positive results retain the full signed-integer range. The WGSL path uses
/// the same f32 accumulation as Pillow's I-mode implementation, including
/// the reversed rows and +0.5 bias.
pub(super) fn gpu_int_filter_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .and_then(|count| count.checked_mul(4));
    if expected != Some(pixels.as_raw().len()) || ops.is_empty() {
        return false;
    }
    let kernel_is_safe = |kernel: &[f32], scale: f32, offset: f32| {
        let Some(offset) = registry::filter_offset_i32(offset) else {
            return false;
        };
        if !scale.is_finite() || scale == 0.0 {
            return false;
        }
        let normalized_scale = scale;
        if !normalized_scale.is_finite() || normalized_scale == 0.0 {
            return false;
        }
        let coefficient_bound = kernel.iter().try_fold(0.0f64, |total, value| {
            let normalized = f64::from(*value) / f64::from(normalized_scale);
            normalized.is_finite().then_some(total + normalized.abs())
        });
        let Some(coefficient_bound) = coefficient_bound else {
            return false;
        };
        // Bound the f32 accumulation for every possible i32 source sample.
        // The output helper also clamps the positive conversion explicitly,
        // matching Rust's saturating float-to-i32 cast at the core boundary.
        let worst_case = f64::from(i32::MAX) * coefficient_bound + f64::from(offset).abs() + 1.0;
        worst_case.is_finite() && worst_case < f64::from(f32::MAX)
    };
    let mut has_filter = false;
    for op in ops {
        match op {
            PipelineOp::Filter3x3 {
                kernel,
                scale,
                offset,
            } => {
                has_filter = true;
                if !kernel_is_safe(kernel, *scale, *offset) {
                    return false;
                }
            }
            PipelineOp::Filter5x5 {
                kernel,
                scale,
                offset,
            } => {
                has_filter = true;
                if !kernel_is_safe(kernel, *scale, *offset) {
                    return false;
                }
            }
            PipelineOp::PutData {
                mode: PixelMode::I, ..
            } => continue,
            _ => return false,
        }
    }
    has_filter
}

/// Return whether an I-mode convolution followed by a nearest resize can
/// remain in one device batch.
///
/// The 3x3 filter shader is the typed I32 path described by
/// `pool_cpu/ops/filter.rs`; the following nearest resize only relocates the
/// resulting four-byte signed words through host-generated one-tap tables.
/// Keep the composition deliberately narrow: a 5x5 filter, filtered resize,
/// another arithmetic operation, or a mode-changing node would consume the
/// words with a different contract and must retain exact host semantic
/// control until its own composition proof is recorded.
pub(super) fn gpu_int_filter_resize_chain_is_supported(
    ops: &[PipelineOp],
    image: &DynamicImage,
) -> bool {
    let [
        filter,
        PipelineOp::Resize {
            w,
            h,
            filter: resize_filter,
        },
    ] = ops
    else {
        return false;
    };
    if !matches!(resize_filter, ResampleFilter::Nearest)
        || *w == 0
        || *h == 0
        || !matches!(filter, PipelineOp::Filter3x3 { .. })
        || !gpu_int_filter_is_supported(std::slice::from_ref(filter), image)
    {
        return false;
    }
    op_output_dims(
        &PipelineOp::Resize {
            w: *w,
            h: *h,
            filter: *resize_filter,
        },
        image.width(),
        image.height(),
    )
    .is_some_and(|(out_w, out_h)| out_w == *w && out_h == *h)
}

/// Return whether the native merge shader can consume Pillow's indexed first
/// band without expanding its palette.
///
/// `ImagingMerge` accepts a `P` image only in the first position of a
/// multi-band byte merge and treats that image as raw index bytes.  The
/// ordinary merge shader has the same byte-interleave contract for canonical
/// RGB, but it does not implement LAB's signed-channel bias or any typed
/// scalar destination.  Keep this admission to one validated RGB merge so a
/// palette lookup can never be silently replaced by an unrelated channel
/// conversion.
pub(super) fn gpu_palette_first_rgb_merge_is_supported(
    ops: &[PipelineOp],
    mode: Option<&str>,
) -> bool {
    if mode != Some("P") {
        return false;
    }
    let [
        PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode,
            bands,
        },
    ] = ops
    else {
        return false;
    };
    logical_mode == "RGB"
        && bands.len() == 3
        && bands
            .first()
            .is_some_and(|band| band.mode().ok().as_deref() == Some("P"))
        && bands
            .iter()
            .skip(1)
            .all(|band| band.mode().ok().as_deref() == Some("L"))
}

/// Validate the index space assumptions made by multi-input shaders. A
/// storage-array read is not bounds-checked by WGSL, so an image that is
/// smaller than the coordinates used by a shader must never be uploaded for a
/// GPU dispatch. Operations with intentionally different canvases (Paste and
/// CompositeModule) are checked against the dimensions encoded in their
/// parameter contract instead of being forced to match the destination.
fn gpu_paste_compact_solid_source_supported(
    source: &crate::image::Image,
    mask: Option<&crate::image::Image>,
    mask_alpha: bool,
) -> bool {
    if mask_alpha {
        return false;
    }
    let source_mode = source.mode().ok();
    let mask_mode = mask.and_then(|image| image.mode().ok());
    matches!(
        (source_mode.as_deref(), mask_mode.as_deref()),
        (
            Some("1" | "P" | "L" | "LA" | "PA" | "RGB" | "HSV" | "RGBA" | "CMYK"),
            Some("1")
        ) | (Some("L" | "RGB" | "HSV" | "RGBA" | "CMYK"), Some("L"))
    )
}

pub(super) fn gpu_auxiliary_shapes_are_safe(
    op: &PipelineOp,
    auxiliary: &AuxiliaryImages,
    cur_w: u32,
    cur_h: u32,
) -> bool {
    let second = auxiliary_dimensions(auxiliary.second.as_deref());
    let third = auxiliary_dimensions(auxiliary.third.as_deref());
    let current = (cur_w, cur_h);

    match op {
        PipelineOp::EffectSpread { .. } => {
            let expected = CheckedDims::new(cur_w, cur_h, 1)
                .ok()
                .and_then(|dims| u32::try_from(dims.total_pixels()).ok());
            auxiliary.third.is_none() && second == expected.map(|width| (width, 1))
        }
        PipelineOp::EffectNoise { .. } => auxiliary.third.is_none() && second == Some(current),
        PipelineOp::Merge { bands, .. } => {
            let expected = CheckedDims::new(cur_w, cur_h, 1)
                .ok()
                .and_then(|dims| {
                    dims.total_pixels()
                        .checked_mul(bands.len().saturating_sub(1))
                })
                .and_then(|pixels| u32::try_from(pixels).ok());
            auxiliary.third.is_none()
                && match (second, expected) {
                    (None, Some(0)) => true,
                    (Some((w, h)), Some(pixels)) => h == 1 && w == pixels,
                    _ => false,
                }
        }
        PipelineOp::Color3DLut { table, .. } => {
            auxiliary.third.is_none()
                && second
                    == color3dlut_table_words(table)
                        .ok()
                        .and_then(|words| u32::try_from(words.len()).ok())
                        .map(|width| (width, 1))
        }
        PipelineOp::ConvertLab { .. } => {
            auxiliary.third.is_none()
                && second
                    == u32::try_from(crate::lab::GPU_TABLE_WORDS)
                        .ok()
                        .map(|width| (width, 1))
        }
        PipelineOp::Paste {
            source,
            w,
            h,
            mask,
            mask_alpha,
            ..
        } => {
            if *w <= 0 || *h <= 0 {
                return false;
            }
            let region = (*w as u32, *h as u32);
            let source_dimensions = source.size().ok();
            // ImageDraw.bitmap's internal solid-mask route stores one source
            // pixel and keeps the full region in its L or binary mask. Native masked
            // Paste broadcasts that sample; CPU fallback expands it first.
            let compact_solid_source = source_dimensions == Some((1, 1))
                && gpu_paste_compact_solid_source_supported(source, mask.as_deref(), *mask_alpha);
            let source_matches_region = source_dimensions == Some(region);
            let source_matches_compact = compact_solid_source && region != (1, 1) && mask.is_some();
            second == source_dimensions
                && (source_matches_region || source_matches_compact)
                && match mask {
                    Some(_) => third == Some(region),
                    None => third.is_none(),
                }
        }
        PipelineOp::CompositeModule { .. } => third == Some(current),
        PipelineOp::PutAlphaData { .. } => third == Some(current),
        PipelineOp::AlphaComposite { .. } => second == Some(current),
        _ => match (second, third) {
            (None, None) => true,
            (second, third) => {
                second
                    .map(|dimensions| dimensions == current)
                    .unwrap_or(true)
                    && third
                        .map(|dimensions| dimensions == current)
                        .unwrap_or(true)
            }
        },
    }
}

/// A zero-area two-input operation has no pixel work to dispatch. Treat it as a
/// successful GPU no-op after validating its auxiliary image contract; this
/// keeps empty Pillow images out of the CPU fallback without pretending that a
/// zero-work storage-buffer dispatch occurred.
pub(super) fn gpu_empty_two_input_batch_is_noop(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    (image.width() == 0 || image.height() == 0)
        && !ops.is_empty()
        && ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::AlphaComposite { .. }
                    | PipelineOp::Multiply { .. }
                    | PipelineOp::Screen { .. }
                    | PipelineOp::Darker { .. }
                    | PipelineOp::Lighter { .. }
                    | PipelineOp::Difference { .. }
                    | PipelineOp::Overlay { .. }
                    | PipelineOp::HardLight { .. }
                    | PipelineOp::SoftLight { .. }
                    | PipelineOp::AddModulo { .. }
                    | PipelineOp::SubtractModulo { .. }
                    | PipelineOp::Add { .. }
                    | PipelineOp::Subtract { .. }
                    | PipelineOp::BlendModule { .. }
                    | PipelineOp::LogicalAnd { .. }
                    | PipelineOp::LogicalOr { .. }
                    | PipelineOp::LogicalXor { .. }
            )
        })
}

pub(super) fn gpu_empty_two_input_inputs_are_safe(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
) -> bool {
    ops.len() == auxiliary_images.len()
        && auxiliary_images
            .iter()
            .enumerate()
            .all(|(index, auxiliary)| {
                auxiliary.second.as_ref().is_some_and(|second| {
                    gpu_image_layout_is_supported(second)
                        && second.dimensions() == image.dimensions()
                }) && auxiliary.third.is_none()
                    && gpu_auxiliary_shapes_are_safe(
                        &ops[index],
                        auxiliary,
                        image.width(),
                        image.height(),
                    )
                    && gpu_auxiliary_modes_are_safe(&ops[index], image, auxiliary)
            })
}

/// A Reduce by `(1, 1)` is Pillow's identity operation. It must return an
/// independent image, but it has no reduction work for a GPU to perform. Such
/// operations can be removed from a GPU batch after validation, allowing the
/// remaining real pixel operations to stay on the GPU as well.
pub(super) fn gpu_reduce_is_identity(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Reduce {
            x_factor: 1,
            y_factor: 1
        }
    )
}

/// Sharpness at factor one is exactly its original sample buffer. Skip it in
/// the GPU executor before packing/uploading a frame that the shader would
/// only copy back unchanged.
pub(super) fn gpu_sharpness_is_identity(op: &PipelineOp) -> bool {
    matches!(op, PipelineOp::Sharpness { factor } if *factor == 1.0)
}

/// Return whether `ImageOps.fit` is the exact identity for this GPU segment.
///
/// With no bleed and equal source/target dimensions, the fit planner's crop
/// box is the complete source image.  Pillow's boxed resize is then an exact
/// sample-preserving copy for every byte resampling filter; use the native
/// Duplicate shader instead of building redundant fractional-box coefficient
/// tables for this case.
/// Keep the lowering limited to ordinary packed byte modes so a logical mode
/// whose sample contract is not represented by the RGBA transport cannot be
/// silently narrowed by the optimization.
pub(super) fn gpu_fit_is_exact_identity(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let PipelineOp::Fit { w, h, bleed, .. } = op else {
        return false;
    };
    source_dimensions.0 > 0
        && source_dimensions.1 > 0
        && (*w, *h) == source_dimensions
        && *bleed == 0.0
        && gpu_image_layout_is_supported(image)
        && logical_mode.is_none_or(|mode| matches!(mode, "L" | "LA" | "RGB" | "RGBA"))
}

// Preserve Pillow's vertical-first FLOAT32 store for tall F inputs by
// separating the resize into GPU segments.
fn expand_tall_f_resize(
    op: &PipelineOp,
    f_resize_chain: bool,
    dimensions: (u32, u32),
) -> Option<(Vec<PipelineOp>, (u32, u32))> {
    let PipelineOp::Resize { w, h, filter } = op else {
        return None;
    };
    if !f_resize_chain
        || *w == 0
        || *h == 0
        || matches!(filter, ResampleFilter::Nearest)
        || !gpu_f_resize_uses_pillow_tall_order(dimensions, (*w, *h))
    {
        return None;
    }
    // PIL.Image.resize's tall-image branch stores FLOAT32 after the vertical
    // pass. Separate GPU segments retain those words and prove the horizontal
    // pass from that actual result.
    Some((
        vec![
            PipelineOp::Resize {
                w: dimensions.0,
                h: *h,
                filter: *filter,
            },
            op.clone(),
        ],
        (*w, *h),
    ))
}

fn expand_thumbnail_integer_reduction(
    op: &PipelineOp,
    dimensions: (u32, u32),
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<(Vec<PipelineOp>, (u32, u32))> {
    let PipelineOp::Thumbnail { filter, .. } = op else {
        return None;
    };
    let effective_filter = if matches!(logical_mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        *filter
    };
    let has_alpha = matches!(
        image,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) && !matches!(logical_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    if gpu_thumbnail_requires_exact_host_control(op, dimensions, image, logical_mode)
        || matches!(effective_filter, ResampleFilter::Nearest)
        || has_alpha
        || matches!(logical_mode, Some("F" | "I"))
    {
        return None;
    }
    let (out_w, out_h) = op_output_dims(op, dimensions.0, dimensions.1)?;
    let factor_x = ((f64::from(dimensions.0) / f64::from(out_w) / 2.0) as u32).max(1);
    let factor_y = ((f64::from(dimensions.1) / f64::from(out_h) / 2.0) as u32).max(1);
    if (factor_x == 1 && factor_y == 1)
        || dimensions.0 % factor_x != 0
        || dimensions.1 % factor_y != 0
    {
        return None;
    }
    // Pillow's default Thumbnail reducing_gap=2.0 performs integer box
    // reduction before resize. Partial edge blocks stay on the exact host path.
    Some((
        vec![
            PipelineOp::Reduce {
                x_factor: factor_x,
                y_factor: factor_y,
            },
            PipelineOp::Resize {
                w: out_w,
                h: out_h,
                filter: effective_filter,
            },
        ],
        (out_w, out_h),
    ))
}

fn expand_gpu_rotate(
    op: &PipelineOp,
    image: &DynamicImage,
    dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> PipelineOp {
    let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        center,
        translate,
        filter,
        nearest,
    } = op
    else {
        return op.clone();
    };
    if gpu_rotate_requires_exact_host_control(image, logical_mode)
        && !matches!(logical_mode, Some("1" | "P"))
        && !matches!(
            (op, logical_mode),
            (
                PipelineOp::Rotate { nearest: false, .. },
                Some("PA" | "RGBa" | "RGBX" | "CMYK" | "HSV" | "YCbCr" | "F")
            )
        )
        && !gpu_rotate_has_exact_transpose_lowering(op, logical_mode, dimensions)
        && !gpu_rotate_nearest_affine_is_exact(op, image, logical_mode, dimensions)
    {
        return op.clone();
    }
    let normalized_angle = angle.rem_euclid(360.0);
    if center.is_none() && translate.is_none() && normalized_angle.abs() <= f64::EPSILON {
        return PipelineOp::Duplicate;
    }
    let right_angle = if gpu_rotate_has_exact_transpose_lowering(op, logical_mode, dimensions) {
        match angle.rem_euclid(360.0) {
            value if (value - 90.0).abs() < f64::EPSILON => Some(TransposeMethod::Rotate90),
            value if (value - 180.0).abs() < f64::EPSILON => Some(TransposeMethod::Rotate180),
            value if (value - 270.0).abs() < f64::EPSILON => Some(TransposeMethod::Rotate270),
            _ => None,
        }
    } else {
        None
    };
    if let Some(method) = right_angle {
        return PipelineOp::Transpose { method };
    }
    let Some((affine, (w, h))) = gpu_rotate_affine(
        *angle,
        *expand,
        *fill,
        *center,
        *translate,
        dimensions.0,
        dimensions.1,
    ) else {
        return op.clone();
    };
    // The public boundary keeps LA/PA rotate colors in the public RGBA-shaped
    // tuple `(gray, gray, gray, alpha)`, while Transform expects
    // `(gray, alpha, 0, 0)`. Normalize only this lowered rotate node.
    let transform_fill = if matches!(logical_mode, Some("LA" | "PA")) {
        (*fill).map(|(gray, _, _, alpha)| (gray, alpha, 0, 0))
    } else {
        *fill
    };
    let effective_nearest = *nearest || matches!(logical_mode, Some("1" | "P"));
    // Filtered rotation uses the binary64 projective kernel. A denominator of
    // one preserves affine center mapping; host-generated tables carry only
    // geometry, and every pixel sample stays on the device.
    let (method, data) = if effective_nearest {
        (TransformMethod::Affine, affine.to_vec())
    } else {
        let mut data = affine.to_vec();
        data.extend([0.0, 0.0]);
        (TransformMethod::Perspective, data)
    };
    PipelineOp::Transform {
        w,
        h,
        method,
        data: Arc::from(data),
        filter: if effective_nearest {
            ResampleFilter::Nearest
        } else {
            *filter
        },
        // Rotate's omitted fill is zero in every stored band, including
        // RGBX's fourth byte. Make that explicit for the GPU and any fallback.
        fill: Some(transform_fill.unwrap_or((0, 0, 0, 0))),
        fill_is_none: fill.is_none(),
        palette_fill: None,
    }
}

/// Replace valid aspect-ratio/Scale nodes with the equivalent exact Resize
/// operation.
///
/// Pillow computes each Scale output axis with ties-to-even rounding before
/// calling its resize implementation. The GPU executor already carries that
/// dimension contract and has exact nearest/fixed-point convolution kernels
/// for Resize, so retaining a separate floating-point Scale shader would
/// duplicate the mapping and risk a one-pixel boundary difference. Invalid
/// or unrepresentable factors are deliberately left unchanged; the normal
/// public-operation preflight then reports the same error or CPU fallback.
pub(super) fn expand_gpu_geometry_ops(
    ops: &[PipelineOp],
    image: &DynamicImage,
    dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Vec<PipelineOp> {
    let mut expanded = Vec::with_capacity(ops.len());
    let (mut cur_w, mut cur_h) = dimensions;
    let i_resize_identity_is_exact = dimensions == image.dimensions()
        && gpu_i_resize_identity_is_exact(ops, image, logical_mode);
    let f_resize_chain =
        logical_mode == Some("F") && ops.iter().all(|op| matches!(op, PipelineOp::Resize { .. }));
    for op in ops {
        if let Some((replacements, dimensions)) =
            expand_tall_f_resize(op, f_resize_chain, (cur_w, cur_h))
        {
            expanded.extend(replacements);
            (cur_w, cur_h) = dimensions;
            continue;
        }
        if i_resize_identity_is_exact {
            // The proof is restricted to the one pure same-size resize, so a
            // raw-word Duplicate is equivalent to Pillow's typed identity
            // result and avoids entering the filtered-I host-control branch.
            expanded.push(PipelineOp::Duplicate);
            continue;
        }
        if gpu_fit_is_exact_identity(op, (cur_w, cur_h), image, logical_mode) {
            // The operation still needs an independent result image and a
            // terminal GPU receipt, so lower to a real copy rather than
            // dropping Fit as a no-op.
            expanded.push(PipelineOp::Duplicate);
            continue;
        }
        if let Some((replacements, dimensions)) =
            expand_thumbnail_integer_reduction(op, (cur_w, cur_h), image, logical_mode)
        {
            expanded.extend(replacements);
            (cur_w, cur_h) = dimensions;
            continue;
        }
        let replacement = match op {
            PipelineOp::Transform {
                method: TransformMethod::Affine,
                data,
                filter,
                ..
            } if data.len() >= 6
                && (!gpu_transform_uses_nearest(logical_mode, *filter)
                    || matches!(logical_mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))) =>
            {
                let mut replacement = op.clone();
                if let PipelineOp::Transform {
                    method,
                    data: coefficients,
                    ..
                } = &mut replacement
                {
                    // Denominator one retains the affine center mapping while
                    // using the exact filtered arithmetic and SPECIAL nearest
                    // coordinates, rather than the old f32/16.16 shader paths.
                    *method = TransformMethod::Perspective;
                    let mut projective = data[..6].to_vec();
                    projective.extend([0.0, 0.0]);
                    *coefficients = Arc::from(projective);
                }
                replacement
            }
            PipelineOp::Scale { filter, .. }
            | PipelineOp::Contain { filter, .. }
            | PipelineOp::Cover { filter, .. } => match op_output_dims(op, cur_w, cur_h) {
                Some((w, h)) => PipelineOp::Resize {
                    w,
                    h,
                    filter: *filter,
                },
                None => op.clone(),
            },
            // Thumbnail has already computed its aspect-preserving output
            // dimensions at the public boundary.  Lower it to the exact
            // separable Resize implementation so every resampling filter
            // (including bicubic/lanczos) uses the same fixed-point kernels
            // as Image.resize instead of the older bilinear-only shader.
            PipelineOp::Thumbnail { filter, .. }
                if gpu_thumbnail_requires_exact_host_control(
                    op,
                    (cur_w, cur_h),
                    image,
                    logical_mode,
                ) =>
            {
                op.clone()
            }
            PipelineOp::Thumbnail { filter, .. } => match op_output_dims(op, cur_w, cur_h) {
                Some((w, h)) => PipelineOp::Resize {
                    w,
                    h,
                    filter: if matches!(logical_mode, Some("1" | "P")) {
                        ResampleFilter::Nearest
                    } else {
                        *filter
                    },
                },
                None => op.clone(),
            },
            // Rotation is an affine mapping after Pillow's scalar geometry
            // planner has selected the expanded canvas.  Feed that mapping
            // into the reviewed Transform shader; exact right-angle
            // rotations use the byte-relocation transpose kernel instead.
            PipelineOp::Rotate { .. } => expand_gpu_rotate(op, image, (cur_w, cur_h), logical_mode),
            _ => op.clone(),
        };
        if let Some(next) = op_output_dims(&replacement, cur_w, cur_h) {
            (cur_w, cur_h) = next;
        }
        expanded.push(replacement);
    }
    expanded
}
