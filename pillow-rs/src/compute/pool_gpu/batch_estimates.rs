use super::*;

pub(super) fn putdata_auxiliary_words(
    data_len: usize,
    channels: usize,
    packed_luma: bool,
) -> usize {
    if packed_luma {
        data_len.div_ceil(std::mem::size_of::<u32>())
    } else {
        data_len.div_ceil(channels)
    }
}

#[derive(Clone, Copy)]
pub(super) struct BatchEstimateOptions<'a> {
    pub(super) uniform_alignment: usize,
    pub(super) storage_alignment: usize,
    pub(super) packed_luma_putdata: bool,
    pub(super) logical_mode: Option<&'a str>,
    pub(super) f_resize_f64_is_exact: bool,
    pub(super) f_resize_f64_ordered_is_exact: bool,
}

pub(super) struct BatchExecutionEstimates {
    pub(super) resource_bytes: Vec<usize>,
    pub(super) shader_work_items: Vec<u64>,
}

impl GpuInner {
    pub(super) fn estimate_batch_resources(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        dimensions: (u32, u32),
        mode: u32,
        options: BatchEstimateOptions<'_>,
        buffers: &BufferPool,
    ) -> Result<BatchExecutionEstimates, PilError> {
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }

        let mut resource_source_dims = Vec::with_capacity(ops.len());
        let mut resource_w = dimensions.0;
        let mut resource_h = dimensions.1;
        for op in ops {
            resource_source_dims.push((resource_w, resource_h));
            let next = match op_output_dims(op, resource_w, resource_h) {
                Some(dimensions) => dimensions,
                None if op_has_explicit_output_dimensions(op) => {
                    return Err(PilError::ValueError(format!(
                        "GPU operation '{}' has no safe source dimensions",
                        registry::variant_key(op)
                    )));
                }
                None => (resource_w, resource_h),
            };
            (resource_w, resource_h) = next;
        }
        let resource_bytes = ops
            .iter()
            .zip(auxiliary_images.iter())
            .enumerate()
            .map(|(index, (op, auxiliary))| {
                self.estimate_resource_bytes(
                    op,
                    auxiliary,
                    buffers,
                    options.uniform_alignment,
                    options.storage_alignment,
                    resource_source_dims[index],
                    op_modes[index],
                    options.packed_luma_putdata,
                    options.logical_mode,
                    options.f_resize_f64_is_exact,
                    options.f_resize_f64_ordered_is_exact,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut work_w = dimensions.0;
        let mut work_h = dimensions.1;
        let shader_work_items = ops
            .iter()
            .map(|op| {
                let next = match op_output_dims(op, work_w, work_h) {
                    Some(dimensions) => dimensions,
                    None if op_has_explicit_output_dimensions(op) => {
                        return Err(PilError::ValueError(format!(
                            "GPU operation '{}' has no safe work dimensions",
                            registry::variant_key(op)
                        )));
                    }
                    None => (work_w, work_h),
                };
                let work = gpu_shader_work_items(op, (work_w, work_h), next, options.logical_mode)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{}' has no bounded shader work estimate",
                            registry::variant_key(op)
                        ))
                    })?;
                (work_w, work_h) = next;
                Ok(work)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BatchExecutionEstimates {
            resource_bytes,
            shader_work_items,
        })
    }

    pub(super) fn estimate_resource_bytes(
        &self,
        op: &PipelineOp,
        auxiliary: &AuxiliaryImages,
        buffers: &BufferPool,
        uniform_alignment: usize,
        storage_alignment: usize,
        source_dimensions: (u32, u32),
        mode: u32,
        packed_luma_putdata: bool,
        logical_mode: Option<&str>,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
    ) -> Result<usize, PilError> {
        let mut total = estimate_parameter_arena_bytes(op, uniform_alignment)?;
        add_auxiliary_arena_bytes(
            &mut total,
            op,
            auxiliary,
            buffers,
            storage_alignment,
            packed_luma_putdata,
        )?;
        add_histogram_arena_bytes(&mut total, op)?;

        if let Some(words) = resize_coefficient_word_counts(
            op,
            source_dimensions,
            mode,
            f_resize_f64_is_exact,
            f_resize_f64_ordered_is_exact,
        )? {
            add_coefficient_pair_resource_bytes(
                &mut total,
                words,
                storage_alignment,
                "GPU resize coefficient arena size overflow",
            )?;
        }
        if let Some(words) = fit_coefficient_word_counts(op, source_dimensions)? {
            add_coefficient_pair_resource_bytes(
                &mut total,
                words,
                storage_alignment,
                "GPU Fit coefficient arena size overflow",
            )?;
        }
        add_transform_geometry_resource_bytes(
            &mut total,
            op,
            source_dimensions,
            logical_mode,
            storage_alignment,
        )?;
        add_lut_resource_bytes(&mut total, op, mode, storage_alignment)?;
        Ok(total)
    }
}

fn estimate_parameter_arena_bytes(op: &PipelineOp, alignment: usize) -> Result<usize, PilError> {
    let op_param_words = if matches!(op, PipelineOp::Pad { .. }) {
        // Header + resize controls + placement controls. The final
        // output dimensions appended below are included separately.
        9
    } else if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. }) {
        // Fit and ResizeBoxed are lowered to the exact separable resize
        // kernels. Their fractional bounds live in coefficient tables,
        // leaving the same four resize control words as public Resize.
        4
    } else if let PipelineOp::Transform { method, .. } = op {
        // extract_params contributes ten public words; the executor then
        // adds premultiply, method, two projective values, and (for one
        // mesh record) four remaining source-corner values.
        if matches!(method, TransformMethod::Mesh) {
            18
        } else {
            14
        }
    } else if matches!(op, PipelineOp::Color3DLut { .. }) {
        7
    } else {
        registry::extract_params(op).len()
    };
    let param_words = 4usize
        .checked_add(op_param_words)
        .and_then(|words| words.checked_add(usize::from(matches!(op, PipelineOp::Contrast { .. }))))
        .and_then(|words| words.checked_add(2))
        .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?;
    let param_bytes = param_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?;
    Ok(aligned_bytes(param_bytes, alignment))
}

fn add_auxiliary_arena_bytes(
    total: &mut usize,
    op: &PipelineOp,
    auxiliary: &AuxiliaryImages,
    buffers: &BufferPool,
    storage_alignment: usize,
    packed_luma_putdata: bool,
) -> Result<(), PilError> {
    if let PipelineOp::PutData { data, mode } = op {
        let words = putdata_auxiliary_words(
            data.len(),
            mode.channels(),
            packed_luma_putdata && *mode == PixelMode::L,
        );
        if words > buffers.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < putdata auxiliary size {}",
                buffers.capacity, words
            )));
        }
        if words > 0 {
            let bytes = words
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU putdata arena size overflow".into()))?;
            add_aligned_resource_bytes(
                total,
                bytes,
                storage_alignment,
                "GPU auxiliary arena size overflow",
            )?;
        }
    } else if let Some(second) = auxiliary.second.as_ref() {
        let bytes = aligned_image_resource_bytes(second, storage_alignment)?;
        add_resource_bytes(total, bytes, "GPU auxiliary arena size overflow")?;
    }
    if let Some(third) = auxiliary.third.as_ref() {
        let bytes = aligned_image_resource_bytes(third, storage_alignment)?;
        add_resource_bytes(total, bytes, "GPU auxiliary arena size overflow")?;
    }
    Ok(())
}

fn aligned_image_resource_bytes(
    image: &DynamicImage,
    storage_alignment: usize,
) -> Result<usize, PilError> {
    let (width, height) = image.dimensions();
    Ok(aligned_bytes(
        CheckedDims::new(width, height, 4)?.total_bytes(),
        storage_alignment,
    ))
}

fn add_histogram_arena_bytes(total: &mut usize, op: &PipelineOp) -> Result<(), PilError> {
    if matches!(
        op,
        PipelineOp::Autocontrast { .. } | PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
    ) {
        add_resource_bytes(
            total,
            GPU_HISTOGRAM_BYTES,
            "GPU histogram arena size overflow",
        )?;
    }
    Ok(())
}

fn resize_coefficient_word_counts(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    mode: u32,
    f_resize_f64_is_exact: bool,
    f_resize_f64_ordered_is_exact: bool,
) -> Result<Option<(usize, usize)>, PilError> {
    let resize_coefficients = match op {
        PipelineOp::Resize { w, h, filter } => Some((*w, *h, *filter, None)),
        PipelineOp::ResizeBoxed {
            w,
            h,
            filter,
            box_coords,
        } => Some((*w, *h, *filter, Some(*box_coords))),
        PipelineOp::Pad { filter, .. } => {
            gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
                .map(|((resize_w, resize_h), _)| (resize_w, resize_h, *filter, None))
        }
        _ => None,
    };
    let Some((resize_w, resize_h, filter, box_coords)) = resize_coefficients else {
        return Ok(None);
    };
    let (source_w, source_h) = source_dimensions;
    let counts = if let Some(box_coords) = box_coords {
        let (horizontal, vertical) = gpu_resize_boxed_coefficients(
            source_dimensions,
            (resize_w, resize_h),
            box_coords,
            filter,
        );
        (
            resize_coeff_word_count(&horizontal)?,
            resize_coeff_word_count(&vertical)?,
        )
    } else if (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact)
        && matches!(mode, 5 | 7 | 8)
        && !matches!(filter, ResampleFilter::Nearest)
        && matches!(op, PipelineOp::Resize { .. } | PipelineOp::Pad { .. })
    {
        f_resize_coefficient_word_counts(
            op,
            (source_w, source_h),
            (resize_w, resize_h),
            filter,
            mode,
            f_resize_f64_ordered_is_exact,
        )?
    } else {
        let horizontal = gpu_resize_coefficients(resize_w, source_w, filter);
        let vertical = gpu_resize_coefficients(resize_h, source_h, filter);
        (
            resize_coeff_word_count(&horizontal)?,
            resize_coeff_word_count(&vertical)?,
        )
    };
    Ok(Some(counts))
}

fn f_resize_coefficient_word_counts(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    filter: ResampleFilter,
    mode: u32,
    f_resize_f64_ordered_is_exact: bool,
) -> Result<(usize, usize), PilError> {
    let (source_w, source_h) = source_dimensions;
    let (resize_w, resize_h) = output_dimensions;
    let compact_box = f_resize_f64_ordered_is_exact
        && mode == 8
        && matches!(op, PipelineOp::Resize { .. })
        && matches!(filter, ResampleFilter::Box)
        && gpu_f_resize_compact_box_any_axis((source_w, source_h), (resize_w, resize_h));
    let (kernel, support) = filter_from_resample(filter);
    let axis_words = |source_size: u32, output_size: u32| {
        if gpu_f_resize_compact_box_axis(source_size, output_size) {
            usize::try_from(output_size)
                .ok()
                .and_then(|count| count.checked_mul(3))
                .and_then(|metadata| metadata.checked_add(4))
                .ok_or_else(|| {
                    PilError::ValueError("GPU compact Box coefficient arena size overflow".into())
                })
        } else {
            let coeffs = precompute_coeffs_f64(output_size, source_size, kernel, support);
            resize_coeff_word_count_f64(&coeffs)
        }
    };
    if compact_box {
        Ok((
            axis_words(source_w, resize_w)?,
            axis_words(source_h, resize_h)?,
        ))
    } else {
        let horizontal = precompute_coeffs_f64(resize_w, source_w, kernel, support);
        let vertical = precompute_coeffs_f64(resize_h, source_h, kernel, support);
        Ok((
            resize_coeff_word_count_f64(&horizontal)?,
            resize_coeff_word_count_f64(&vertical)?,
        ))
    }
}

fn fit_coefficient_word_counts(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
) -> Result<Option<(usize, usize)>, PilError> {
    let PipelineOp::Fit {
        w,
        h,
        bleed,
        centering,
        filter,
        ..
    } = op
    else {
        return Ok(None);
    };
    let (horizontal, vertical) =
        gpu_fit_coefficients(source_dimensions, (*w, *h), *bleed, *centering, *filter)
            .ok_or_else(|| PilError::ValueError("GPU Fit has no safe crop geometry".into()))?;
    Ok(Some((
        resize_coeff_word_count(&horizontal)?,
        resize_coeff_word_count(&vertical)?,
    )))
}

fn add_coefficient_pair_resource_bytes(
    total: &mut usize,
    word_counts: (usize, usize),
    alignment: usize,
    error: &'static str,
) -> Result<(), PilError> {
    let horizontal_bytes = word_counts
        .0
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError(error.into()))?;
    let vertical_bytes = word_counts
        .1
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError(error.into()))?;
    let bytes = aligned_bytes(horizontal_bytes, alignment)
        .checked_add(aligned_bytes(vertical_bytes, alignment))
        .ok_or_else(|| PilError::ValueError(error.into()))?;
    add_resource_bytes(total, bytes, error)
}

fn add_transform_geometry_resource_bytes(
    total: &mut usize,
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    logical_mode: Option<&str>,
    storage_alignment: usize,
) -> Result<(), PilError> {
    if gpu_transform_uses_geometry_table(op, logical_mode, source_dimensions) {
        let (width, height) = op_output_dims(op, source_dimensions.0, source_dimensions.1)
            .ok_or_else(|| PilError::InternalError("GPU transform dimensions missing".into()))?;
        let bytes = CheckedDims::new(width, height, (GPU_TRANSFORM_GEOMETRY_WORDS * 4) as u8)?
            .total_bytes();
        add_aligned_resource_bytes(
            total,
            bytes,
            storage_alignment,
            "GPU transform geometry arena size overflow",
        )?;
    }
    Ok(())
}

fn add_lut_resource_bytes(
    total: &mut usize,
    op: &PipelineOp,
    mode: u32,
    storage_alignment: usize,
) -> Result<(), PilError> {
    if extract_lut(op, mode).is_some() {
        add_aligned_resource_bytes(
            total,
            256 * std::mem::size_of::<u32>(),
            storage_alignment,
            "GPU LUT arena size overflow",
        )?;
    }
    Ok(())
}

fn add_aligned_resource_bytes(
    total: &mut usize,
    bytes: usize,
    alignment: usize,
    error: &'static str,
) -> Result<(), PilError> {
    add_resource_bytes(total, aligned_bytes(bytes, alignment), error)
}

fn add_resource_bytes(
    total: &mut usize,
    bytes: usize,
    error: &'static str,
) -> Result<(), PilError> {
    *total = total
        .checked_add(bytes)
        .ok_or_else(|| PilError::ValueError(error.into()))?;
    Ok(())
}
