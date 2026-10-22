use super::*;

/// Return whether a nearest resize uses the packed byte convolution plan.
///
/// The convolution plan receives host-generated one-tap tables so its source
/// index follows Pillow's cumulative f64 stepping exactly. I- and F-mode need
/// that plan too: their four-byte samples are copied opaquely by the typed
/// branch in the resize shaders, rather than being interpreted as four byte
/// channels. I;16 retains its established relocation shader until its own
/// nearest-coordinate receipts are expanded with the same proof.
pub(super) fn gpu_resize_nearest_uses_coefficients(logical_mode: Option<&str>) -> bool {
    !matches!(
        logical_mode,
        Some("1" | "I;16" | "I;16L" | "I;16B" | "I;16N")
    )
}

/// Build a resize coefficient table for the exact GPU byte path.
///
/// Non-nearest filters reuse the shared Pillow fixed-point table. Nearest is
/// different: Pillow's affine implementation advances one f64 accumulator
/// per output sample, so computing `(x + 0.5) * scale` independently in WGSL
/// can select a neighboring source pixel at a representable boundary. A
/// one-tap table preserves that scalar control-plane decision while leaving
/// every pixel read/write on the device.
pub(super) fn gpu_resize_coefficients(
    out_size: u32,
    in_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    if !matches!(filter, ResampleFilter::Nearest) {
        return precompute_coeffs(out_size, in_size, filter);
    }
    if out_size == 0 || in_size == 0 {
        return Arc::new(FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        });
    }

    let scale = f64::from(in_size) / f64::from(out_size);
    let mut source_position = scale * 0.5;
    let mut xmin = Vec::with_capacity(out_size as usize);
    let mut count = Vec::with_capacity(out_size as usize);
    let mut offsets = Vec::with_capacity(out_size as usize);
    let mut weights = Vec::with_capacity(out_size as usize);
    for _ in 0..out_size {
        let source = if source_position >= f64::from(in_size) {
            in_size - 1
        } else {
            source_position as u32
        };
        xmin.push(i64::from(source));
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
        source_position += scale;
    }
    Arc::new(FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    })
}

/// Validate and size the fixed-point coefficient table consumed by the
/// separable resize shaders.
///
/// `FilterCoeffs` is deliberately shared with the CPU/SIMD implementations so
/// the GPU receives the same Pillow-rounded coefficients.  The shader uses an
/// i32 accumulator; prove the complete byte-domain accumulation before a
/// resize is admitted rather than allowing device arithmetic to wrap.
pub(super) fn resize_coeff_word_count(coeffs: &FilterCoeffs) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.offsets.len()
        || coeffs
            .offsets
            .iter()
            .zip(&coeffs.count)
            .any(|(&offset, &count)| {
                offset
                    .checked_add(count)
                    .is_none_or(|end| end > coeffs.weights.len())
            })
    {
        return Err(PilError::InternalError(
            "GPU resize coefficient table shape is invalid".into(),
        ));
    }

    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        if xmin < 0
            || u32::try_from(xmin).is_err()
            || u32::try_from(count).is_err()
            || u32::try_from(offset).is_err()
        {
            return Err(PilError::ValueError(
                "GPU resize coefficient metadata exceeds shader limits".into(),
            ));
        }
    }

    for &weight in &coeffs.weights {
        let weight = i32::try_from(weight).map_err(|_| {
            PilError::ValueError("GPU resize coefficient exceeds signed shader limits".into())
        })?;
        let abs_weight = i64::from(weight).checked_abs().ok_or_else(|| {
            PilError::ValueError("GPU resize coefficient magnitude overflow".into())
        })?;
        // Every intermediate partial sum is bounded by the absolute sum of
        // all taps.  The largest byte sample is 255 and the shader adds the
        // fixed-point half-unit before shifting.
        let _ = abs_weight;
    }
    for (&offset, &count) in coeffs.offsets.iter().zip(&coeffs.count) {
        let mut abs_sum = 0i64;
        for &weight in coeffs.weights.get(offset..offset + count).ok_or_else(|| {
            PilError::InternalError("GPU resize coefficient slice is invalid".into())
        })? {
            let abs_weight = weight.checked_abs().ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient magnitude overflow".into())
            })?;
            abs_sum = abs_sum.checked_add(abs_weight).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient sum overflow".into())
            })?;
        }
        let max_accumulator = abs_sum
            .checked_mul(255)
            .and_then(|value| value.checked_add(1i64 << 21))
            .ok_or_else(|| PilError::ValueError("GPU resize accumulator overflow".into()))?;
        if max_accumulator > i64::from(i32::MAX) {
            return Err(PilError::ValueError(
                "GPU resize accumulator exceeds signed shader limits".into(),
            ));
        }
    }

    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(coeffs.weights.len()))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Encode one Pillow-compatible coefficient table as `[xmin,count,offset]`
/// metadata followed by signed fixed-point weights.  The returned words are
/// uploaded as an auxiliary storage range; no image pixels are materialized on
/// the host for this control table.
pub(super) fn encode_resize_coeffs(coeffs: &FilterCoeffs) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
    }
    words.extend(coeffs.weights.iter().map(|&weight| {
        // resize_coeff_word_count already checked this conversion.
        i32::try_from(weight).expect("validated GPU resize coefficient") as u32
    }));
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Validate and size an f64 coefficient table transported to marker-9's
/// integer reducer.  Each dyadic coefficient occupies four words: a 53-bit
/// mantissa, its binary exponent, and its sign.  Metadata offsets therefore
/// count groups of four rather than individual weights.
pub(super) fn resize_coeff_word_count_f64(coeffs: &FilterCoeffsF64) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.weights.len()
        || coeffs
            .count
            .iter()
            .zip(&coeffs.weights)
            .any(|(&count, weights)| count != weights.len())
    {
        return Err(PilError::InternalError(
            "GPU f64 resize coefficient table shape is invalid".into(),
        ));
    }
    for &xmin in &coeffs.xmin {
        if xmin < 0 || u32::try_from(xmin).is_err() {
            return Err(PilError::ValueError(
                "GPU resize coefficient xmin exceeds shader limits".into(),
            ));
        }
    }
    let mut weight_words = 0usize;
    for row in &coeffs.weights {
        for &weight in row {
            if gpu_f64_integer_parts(weight).is_none() {
                return Err(PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                ));
            }
        }
        weight_words = weight_words
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(weight_words))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Return whether one f64 coefficient table can be bound as one storage range.
///
/// The host proof may certify every row's arithmetic while the encoded table
/// still exceeds the device binding limit when multiple output rows are
/// present.  Keep this check next to the encoder's word-count contract so
/// marker 9's special-value prepass and marker 12's ordered reducer make the
/// same adapter-safe admission decision before any bind group is created.
pub(super) fn gpu_f_resize_f64_coefficients_fit_binding(coeffs: &FilterCoeffsF64) -> bool {
    let Ok(word_count) = resize_coeff_word_count_f64(coeffs) else {
        return false;
    };
    let Some(bytes) = word_count.checked_mul(std::mem::size_of::<u32>()) else {
        return false;
    };
    aligned_bytes(bytes, GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES)
        <= GPU_F_RESIZE_MAX_COEFFICIENT_BINDING_BYTES
}

pub(super) fn encode_resize_coeffs_f64(coeffs: &FilterCoeffsF64) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count_f64(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    let mut weight_offset = 0usize;
    for ((&xmin, &count), row) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.weights) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(weight_offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
        weight_offset = weight_offset
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    for row in &coeffs.weights {
        for &weight in row {
            let parts = gpu_f64_integer_parts(weight).ok_or_else(|| {
                PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                )
            })?;
            words.push(parts.mantissa as u32);
            words.push((parts.mantissa >> 32) as u32);
            words.push(parts.exponent as u32);
            words.push(u32::from(parts.negative && parts.mantissa != 0));
        }
    }
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Encode compact Box rows whose normalized coefficient is repeated for every
/// tap.  This is used only for integer-ratio downscales beyond the full f64
/// coefficient binding envelope; ordinary rows continue to use the complete
/// per-tap table above.
pub(super) fn encode_resize_compact_box_axis(
    source_size: u32,
    output_size: u32,
) -> Result<Vec<u32>, PilError> {
    if output_size == 0
        || source_size % output_size != 0
        || source_size / output_size <= GPU_F_RESIZE_ORDERED_MAX_TAPS as u32
    {
        return Err(PilError::InternalError(
            "GPU compact Box coefficient geometry is invalid".into(),
        ));
    }
    let tap_count = source_size / output_size;
    let coefficient = 1.0 / f64::from(tap_count);
    let parts = gpu_f64_integer_parts(coefficient).ok_or_else(|| {
        PilError::ValueError("GPU compact Box coefficient is not a finite normal value".into())
    })?;
    let output_count = usize::try_from(output_size)
        .map_err(|_| PilError::ValueError("GPU compact Box output size is too large".into()))?;
    let mut words = Vec::with_capacity(
        output_count
            .checked_mul(3)
            .and_then(|count| count.checked_add(4))
            .ok_or_else(|| PilError::ValueError("GPU compact Box arena size overflow".into()))?,
    );
    for output_index in 0..output_count {
        let xmin = u32::try_from(
            output_index
                .checked_mul(usize::try_from(tap_count).map_err(|_| {
                    PilError::ValueError("GPU compact Box tap count is too large".into())
                })?)
                .ok_or_else(|| PilError::ValueError("GPU compact Box offset overflow".into()))?,
        )
        .map_err(|_| PilError::ValueError("GPU compact Box offset exceeds shader limits".into()))?;
        words.extend([xmin, tap_count, 0]);
    }
    words.extend([
        parts.mantissa as u32,
        (parts.mantissa >> 32) as u32,
        parts.exponent as u32,
        u32::from(parts.negative && parts.mantissa != 0),
    ]);
    Ok(words)
}

pub(super) fn gpu_resize_coefficients_are_safe(
    filter: ResampleFilter,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    let (source_w, source_h) = source_dimensions;
    let (output_w, output_h) = output_dimensions;
    if source_w == 0 || source_h == 0 || output_w == 0 || output_h == 0 {
        return false;
    }
    let horizontal = gpu_resize_coefficients(output_w, source_w, filter);
    let vertical = gpu_resize_coefficients(output_h, source_h, filter);
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

/// Build the exact two-pass coefficient tables for ImageOps.fit's fractional
/// source box.  The boxed builder performs Pillow's required f32 boundary
/// conversion before calculating f64 centers and 22-bit weights; reusing it
/// here keeps the GPU's control table identical to `pil_resize_boxed` while
/// leaving all sample reads, accumulation, and writes on the device.
/// This follows Pillow's `libImaging/Resample.c::precompute_coeffs` contract:
/// `ImageOps.fit` passes its crop box through the float ABI before those
/// coefficients are calculated, so retaining the fractional box is essential
/// at pixel boundaries.
pub(super) fn gpu_fit_coefficients(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> Option<(FilterCoeffs, FilterCoeffs)> {
    let (crop_left, crop_top, crop_w, crop_h) = gpu_fit_box(
        source_dimensions.0,
        source_dimensions.1,
        output_dimensions.0,
        output_dimensions.1,
        bleed,
        centering,
    )?;
    if matches!(filter, ResampleFilter::Nearest) {
        return Some((
            gpu_fit_nearest_coefficients(
                output_dimensions.0,
                source_dimensions.0,
                crop_left,
                crop_left + crop_w,
            ),
            gpu_fit_nearest_coefficients(
                output_dimensions.1,
                source_dimensions.1,
                crop_top,
                crop_top + crop_h,
            ),
        ));
    }
    Some((
        precompute_coeffs_boxed_for_filter(
            output_dimensions.0,
            source_dimensions.0,
            crop_left,
            crop_left + crop_w,
            filter,
        ),
        precompute_coeffs_boxed_for_filter(
            output_dimensions.1,
            source_dimensions.1,
            crop_top,
            crop_top + crop_h,
            filter,
        ),
    ))
}

/// Build one-tap tables for Pillow's boxed nearest affine mapping.
///
/// `Image.resize(box=...)` does not use the convolution coefficients for
/// `NEAREST`: `ImagingScaleAffine` evaluates the source coordinate from the
/// float box bounds for each output pixel.  Keep that mapping in the host
/// control table (after the same f32 boundary conversion) so the exact
/// resize kernels can still own every byte read and write.
pub(super) fn gpu_fit_nearest_coefficients(
    output_size: u32,
    source_size: u32,
    box_start: f64,
    box_end: f64,
) -> FilterCoeffs {
    if output_size == 0 || source_size == 0 {
        return FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        };
    }
    let box_start_f32 = box_start as f32;
    let box_end_f32 = box_end as f32;
    let box_start = box_start_f32 as f64;
    let scale = f64::from(box_end_f32 - box_start_f32) / f64::from(output_size);
    let last_source = i64::from(source_size - 1);
    let mut xmin = Vec::with_capacity(output_size as usize);
    let mut count = Vec::with_capacity(output_size as usize);
    let mut offsets = Vec::with_capacity(output_size as usize);
    let mut weights = Vec::with_capacity(output_size as usize);
    // ImagingScaleAffine initializes one center coordinate and advances it
    // cumulatively. Recomputing each `(index + 0.5) * scale` can cross an
    // upsample boundary differently after f64 rounding.
    let mut coordinate = box_start + scale * 0.5;
    for _ in 0..output_size {
        let source = (coordinate.floor() as i64).clamp(0, last_source);
        xmin.push(source);
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
        coordinate += scale;
    }
    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

/// Build the exact boxed coefficient pair consumed by the device resize
/// passes. Boxed nearest uses Pillow's cumulative affine coordinate walk;
/// filtered modes use the shared float32-boundary fixed-point table builder.
pub(super) fn gpu_resize_boxed_coefficients(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> (FilterCoeffs, FilterCoeffs) {
    let (left, top, right, bottom) = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if matches!(filter, ResampleFilter::Nearest) {
        return (
            gpu_fit_nearest_coefficients(output_dimensions.0, source_dimensions.0, left, right),
            gpu_fit_nearest_coefficients(output_dimensions.1, source_dimensions.1, top, bottom),
        );
    }
    (
        precompute_coeffs_boxed_for_filter(
            output_dimensions.0,
            source_dimensions.0,
            left,
            right,
            filter,
        ),
        precompute_coeffs_boxed_for_filter(
            output_dimensions.1,
            source_dimensions.1,
            top,
            bottom,
            filter,
        ),
    )
}

pub(super) fn gpu_resize_boxed_coefficients_are_safe(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> bool {
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || output_dimensions.0 == 0
        || output_dimensions.1 == 0
    {
        return false;
    }
    let Some(source_pixels) = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_pixels())
    else {
        return false;
    };
    let Some(output_pixels) = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_pixels())
    else {
        return false;
    };
    if source_pixels > GPU_BUFFER_CAPACITY as usize || output_pixels > GPU_BUFFER_CAPACITY as usize
    {
        return false;
    }
    let (horizontal, vertical) =
        gpu_resize_boxed_coefficients(source_dimensions, output_dimensions, bounds, filter);
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

pub(super) fn gpu_fit_coefficients_are_safe(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> bool {
    let Some((horizontal, vertical)) = gpu_fit_coefficients(
        source_dimensions,
        output_dimensions,
        bleed,
        centering,
        filter,
    ) else {
        return false;
    };
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

#[derive(Clone, Copy)]
pub(super) struct BatchResizeCoefficientContext<'a> {
    pub(super) op: &'a PipelineOp,
    pub(super) input_dimensions: (u32, u32),
    pub(super) output_dimensions: (u32, u32),
    pub(super) logical_mode: Option<&'a str>,
    pub(super) op_mode: u32,
    pub(super) f_resize_f64_is_exact: bool,
    pub(super) f_resize_f64_ordered_is_exact: bool,
    pub(super) boxed_resize_coefficients: Option<&'a (FilterCoeffs, FilterCoeffs)>,
    pub(super) storage_alignment: usize,
}

impl GpuInner {
    pub(super) fn prepare_batch_resize_coefficients(
        &self,
        context: BatchResizeCoefficientContext<'_>,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<Option<(BufferRange, BufferRange)>, PilError> {
        match context.op {
            PipelineOp::Resize { filter, .. } => {
                self.prepare_resize_coefficients(context, *filter, img2_arena, auxiliary_cache)
            }
            PipelineOp::ResizeBoxed { .. } => {
                let (horizontal, vertical) =
                    context.boxed_resize_coefficients.ok_or_else(|| {
                        PilError::InternalError(
                            "GPU ResizeBoxed coefficients were not prepared".into(),
                        )
                    })?;
                self.append_resize_coeff_ranges(
                    img2_arena,
                    auxiliary_cache,
                    horizontal,
                    vertical,
                    context.storage_alignment,
                )
                .map(Some)
            }
            PipelineOp::Pad { filter, .. } => {
                self.prepare_pad_coefficients(context, *filter, img2_arena, auxiliary_cache)
            }
            PipelineOp::Fit {
                bleed,
                centering,
                filter,
                ..
            } => self.prepare_fit_coefficients(
                context,
                *bleed,
                *centering,
                *filter,
                img2_arena,
                auxiliary_cache,
            ),
            _ => Ok(None),
        }
    }

    fn prepare_resize_coefficients(
        &self,
        context: BatchResizeCoefficientContext<'_>,
        filter: ResampleFilter,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<Option<(BufferRange, BufferRange)>, PilError> {
        let (source_w, source_h) = context.input_dimensions;
        let (output_w, output_h) = context.output_dimensions;
        if matches!(filter, ResampleFilter::Nearest)
            && !gpu_resize_nearest_uses_coefficients(context.logical_mode)
        {
            return Ok(None);
        }
        let horizontal = gpu_resize_coefficients(output_w, source_w, filter);
        let vertical = gpu_resize_coefficients(output_h, source_h, filter);
        let exact_f64 = (context.f_resize_f64_is_exact || context.f_resize_f64_ordered_is_exact)
            && !matches!(filter, ResampleFilter::Nearest)
            && (context.logical_mode == Some("F")
                || (context.op_mode == 5
                    && context
                        .logical_mode
                        .is_none_or(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")))
                || (context.op_mode == 7 && context.logical_mode == Some("I")));
        if !exact_f64 {
            return self
                .append_resize_coeff_ranges(
                    img2_arena,
                    auxiliary_cache,
                    &horizontal,
                    &vertical,
                    context.storage_alignment,
                )
                .map(Some);
        }
        self.prepare_exact_f64_resize_coefficients(context, filter, img2_arena, auxiliary_cache)
    }

    fn prepare_exact_f64_resize_coefficients(
        &self,
        context: BatchResizeCoefficientContext<'_>,
        filter: ResampleFilter,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<Option<(BufferRange, BufferRange)>, PilError> {
        let (source_w, source_h) = context.input_dimensions;
        let (output_w, output_h) = context.output_dimensions;
        let (kernel, support) = filter_from_resample(filter);
        let compact_box = context.f_resize_f64_ordered_is_exact
            && context.logical_mode == Some("F")
            && matches!(filter, ResampleFilter::Box)
            && gpu_f_resize_compact_box_any_axis((source_w, source_h), (output_w, output_h));
        if compact_box {
            let horizontal_words = if gpu_f_resize_compact_box_axis(source_w, output_w) {
                encode_resize_compact_box_axis(source_w, output_w)?
            } else {
                encode_resize_coeffs_f64(&precompute_coeffs_f64(
                    output_w, source_w, kernel, support,
                ))?
            };
            let vertical_words = if gpu_f_resize_compact_box_axis(source_h, output_h) {
                encode_resize_compact_box_axis(source_h, output_h)?
            } else {
                encode_resize_coeffs_f64(&precompute_coeffs_f64(
                    output_h, source_h, kernel, support,
                ))?
            };
            return self
                .append_resize_coeff_word_ranges(
                    img2_arena,
                    auxiliary_cache,
                    &horizontal_words,
                    &vertical_words,
                    context.storage_alignment,
                )
                .map(Some);
        }
        let horizontal = precompute_coeffs_f64(output_w, source_w, kernel, support);
        let vertical = precompute_coeffs_f64(output_h, source_h, kernel, support);
        self.append_resize_f64_coeff_ranges(
            img2_arena,
            auxiliary_cache,
            &horizontal,
            &vertical,
            context.storage_alignment,
        )
        .map(Some)
    }

    fn prepare_pad_coefficients(
        &self,
        context: BatchResizeCoefficientContext<'_>,
        filter: ResampleFilter,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<Option<(BufferRange, BufferRange)>, PilError> {
        let (source_w, source_h) = context.input_dimensions;
        let ((resize_w, resize_h), _) = gpu_pad_geometry(context.op, source_w, source_h)
            .ok_or_else(|| PilError::ValueError("GPU Pad has no safe geometry".into()))?;
        // An identity contain resize is a byte-preserving copy in Pillow.
        // The placement shader reads the original image directly.
        if (resize_w, resize_h) == (source_w, source_h) {
            return Ok(None);
        }
        let filter = if context.logical_mode == Some("P") {
            ResampleFilter::Nearest
        } else {
            filter
        };
        if context.f_resize_f64_is_exact
            && context.logical_mode == Some("F")
            && !matches!(filter, ResampleFilter::Nearest)
        {
            let (kernel, support) = filter_from_resample(filter);
            let horizontal = precompute_coeffs_f64(resize_w, source_w, kernel, support);
            let vertical = precompute_coeffs_f64(resize_h, source_h, kernel, support);
            return self
                .append_resize_f64_coeff_ranges(
                    img2_arena,
                    auxiliary_cache,
                    &horizontal,
                    &vertical,
                    context.storage_alignment,
                )
                .map(Some);
        }
        let horizontal = gpu_resize_coefficients(resize_w, source_w, filter);
        let vertical = gpu_resize_coefficients(resize_h, source_h, filter);
        self.append_resize_coeff_ranges(
            img2_arena,
            auxiliary_cache,
            &horizontal,
            &vertical,
            context.storage_alignment,
        )
        .map(Some)
    }

    fn prepare_fit_coefficients(
        &self,
        context: BatchResizeCoefficientContext<'_>,
        bleed: f64,
        centering: (f64, f64),
        filter: ResampleFilter,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<Option<(BufferRange, BufferRange)>, PilError> {
        let (horizontal, vertical) = gpu_fit_coefficients(
            context.input_dimensions,
            context.output_dimensions,
            bleed,
            centering,
            filter,
        )
        .ok_or_else(|| PilError::ValueError("GPU Fit has no safe crop geometry".into()))?;
        self.append_resize_coeff_ranges(
            img2_arena,
            auxiliary_cache,
            &horizontal,
            &vertical,
            context.storage_alignment,
        )
        .map(Some)
    }
}
