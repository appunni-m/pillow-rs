use super::*;

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
