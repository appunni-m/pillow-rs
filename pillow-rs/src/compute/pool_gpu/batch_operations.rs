use super::*;

#[derive(Clone, Copy)]
pub(super) struct BatchOperationPreparationOptions<'a> {
    pub(super) ops: &'a [PipelineOp],
    pub(super) auxiliary_images: &'a [AuxiliaryImages],
    pub(super) logical_mode: Option<&'a str>,
    pub(super) contrast_mean: Option<u8>,
    pub(super) resize_exactness: BatchResizeParameterExactness,
    pub(super) layouts: BatchParameterLayouts,
    pub(super) pipeline_options: BatchPreparationPipelineOptions<'a>,
    pub(super) packed_luma_putdata: bool,
    pub(super) direct_alpha_composite_rgba: Option<&'a [u8]>,
    pub(super) uniform_alignment: usize,
    pub(super) storage_alignment: usize,
    pub(super) max_workgroups_per_dimension: u32,
    pub(super) max_storage_buffer_binding_size: u32,
    pub(super) max_buffer_size: u64,
    pub(super) auxiliary_cache: &'a GpuAuxiliaryCache,
}

#[derive(Clone, Copy)]
struct BatchOperationIteration<'a> {
    index: usize,
    op: &'a PipelineOp,
    source_dimensions: (u32, u32),
    current_mode: u32,
}

pub(super) struct BatchPreparationScratch {
    params_arena: Vec<u32>,
    params_ranges: Vec<BufferRange>,
    img2_arena: Vec<u32>,
    img2_ranges: Vec<Option<BufferRange>>,
    img3_arena: Vec<u32>,
    img3_ranges: Vec<Option<BufferRange>>,
    lut_arena: Vec<u32>,
    lut_ranges: Vec<Option<BufferRange>>,
    second_cache: HashMap<usize, BufferRange>,
    third_cache: HashMap<usize, BufferRange>,
    lut_cache: HashMap<[u32; 256], BufferRange>,
    resize_coeff_ranges: Vec<Option<(BufferRange, BufferRange)>>,
    input_dims: Vec<(u32, u32)>,
    output_dims: Vec<(u32, u32)>,
    resize_source_rows: Vec<Option<(u32, u32)>>,
    pad_resize_dims: Vec<Option<(u32, u32)>>,
    native_expand_output: Option<NativeExpandOutputDispatch>,
    native_luma_transform_output: Option<NativeExpandOutputDispatch>,
}

pub(super) struct BatchPreparationOutputOptions<'a> {
    pub(super) uniform_alignment: usize,
    pub(super) storage_alignment: usize,
    pub(super) direct_img2_bytes: &'a [u8],
    pub(super) native_rgb_packed_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_la_transform_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
    pub(super) final_dims: (u32, u32),
}

impl BatchPreparationScratch {
    pub(super) fn new(operation_count: usize) -> Self {
        Self {
            params_arena: Vec::new(),
            params_ranges: Vec::with_capacity(operation_count),
            img2_arena: Vec::new(),
            img2_ranges: Vec::with_capacity(operation_count),
            img3_arena: Vec::new(),
            img3_ranges: Vec::with_capacity(operation_count),
            lut_arena: Vec::new(),
            lut_ranges: Vec::with_capacity(operation_count),
            second_cache: HashMap::new(),
            third_cache: HashMap::new(),
            lut_cache: HashMap::new(),
            resize_coeff_ranges: Vec::with_capacity(operation_count),
            input_dims: Vec::with_capacity(operation_count),
            output_dims: Vec::with_capacity(operation_count),
            resize_source_rows: Vec::with_capacity(operation_count),
            pad_resize_dims: Vec::with_capacity(operation_count),
            native_expand_output: None,
            native_luma_transform_output: None,
        }
    }

    pub(super) fn into_output<'a>(
        self,
        options: BatchPreparationOutputOptions<'a>,
    ) -> GpuBatchPreparationOutput<'a> {
        GpuBatchPreparationOutput {
            uniform_alignment: options.uniform_alignment,
            storage_alignment: options.storage_alignment,
            direct_img2_bytes: options.direct_img2_bytes,
            params_arena: self.params_arena,
            params_ranges: self.params_ranges,
            img2_arena: self.img2_arena,
            img2_ranges: self.img2_ranges,
            img3_arena: self.img3_arena,
            img3_ranges: self.img3_ranges,
            lut_arena: self.lut_arena,
            lut_ranges: self.lut_ranges,
            resize_coeff_ranges: self.resize_coeff_ranges,
            input_dims: self.input_dims,
            output_dims: self.output_dims,
            resize_source_rows: self.resize_source_rows,
            pad_resize_dims: self.pad_resize_dims,
            native_expand_output: self.native_expand_output,
            native_luma_transform_output: self.native_luma_transform_output,
            native_rgb_packed_output: options.native_rgb_packed_output,
            native_la_transform_output: options.native_la_transform_output,
            native_cover_packed_output: options.native_cover_packed_output,
            final_dims: options.final_dims,
        }
    }
}

impl GpuInner {
    pub(super) fn prepare_batch_operations(
        &self,
        options: BatchOperationPreparationOptions<'_>,
        buffers: &BufferPool,
        scratch: &mut BatchPreparationScratch,
        initial_state: (u32, u32, u32),
    ) -> Result<(u32, u32), PilError> {
        let (mut cur_w, mut cur_h, mut current_mode) = initial_state;
        for (index, op) in options.ops.iter().enumerate() {
            let next = self.prepare_batch_operation(
                options,
                BatchOperationIteration {
                    index,
                    op,
                    source_dimensions: (cur_w, cur_h),
                    current_mode,
                },
                buffers,
                scratch,
            )?;
            cur_w = next.0;
            cur_h = next.1;
            current_mode = gpu_mode_after_op(current_mode, op);
        }
        Ok((cur_w, cur_h))
    }

    fn prepare_batch_operation(
        &self,
        options: BatchOperationPreparationOptions<'_>,
        iteration: BatchOperationIteration<'_>,
        buffers: &BufferPool,
        scratch: &mut BatchPreparationScratch,
    ) -> Result<(u32, u32), PilError> {
        let index = iteration.index;
        let op = iteration.op;
        let (cur_w, cur_h) = iteration.source_dimensions;
        let cached = self.resolve_preparation_pipeline(
            options.ops,
            index,
            (cur_w, cur_h),
            &options.pipeline_options,
        )?;
        let plan = self.prepare_batch_operation_plan(
            BatchOperationPlanningContext {
                ops: options.ops,
                op,
                source_dimensions: (cur_w, cur_h),
                current_mode: iteration.current_mode,
                logical_mode: options.logical_mode,
                output_only_pipeline: cached.is_output_only,
                native_expand_channels: options.layouts.native_expand_channels,
                packed_luma_transform_output: options.layouts.packed_luma_transform_output,
                max_workgroups_per_dimension: options.max_workgroups_per_dimension,
                max_storage_buffer_binding_size: options.max_storage_buffer_binding_size,
                max_buffer_size: options.max_buffer_size,
            },
            buffers,
        )?;
        if let Some(dispatch) = plan.native_expand_output {
            scratch.native_expand_output = Some(dispatch);
        }
        if let Some(dispatch) = plan.native_luma_transform_output {
            scratch.native_luma_transform_output = Some(dispatch);
        }
        self.prepare_batch_operation_parameters(options, iteration, &plan, scratch)?;
        self.prepare_batch_operation_auxiliaries(
            options, iteration, &cached, &plan, buffers, scratch,
        )?;
        Ok(plan.output_dimensions)
    }

    fn prepare_batch_operation_parameters(
        &self,
        options: BatchOperationPreparationOptions<'_>,
        iteration: BatchOperationIteration<'_>,
        plan: &BatchOperationPreparationPlan,
        scratch: &mut BatchPreparationScratch,
    ) -> Result<(), PilError> {
        let op = iteration.op;
        let (cur_w, cur_h) = iteration.source_dimensions;
        let (out_w, out_h) = plan.output_dimensions;
        scratch.input_dims.push((cur_w, cur_h));
        scratch.output_dims.push((out_w, out_h));
        scratch
            .resize_source_rows
            .push(plan.resize_source_row_range);
        let params = prepare_batch_operation_params(BatchOperationParameterContext {
            op,
            auxiliary: &options.auxiliary_images[iteration.index],
            source_dimensions: (cur_w, cur_h),
            output_dimensions: (out_w, out_h),
            shader_dimensions: plan.shader_dimensions,
            op_mode: plan.op_mode,
            logical_mode: options.logical_mode,
            contrast_mean: options.contrast_mean,
            source_row_base: plan.source_row_base,
            max_workgroups_per_dimension: options.max_workgroups_per_dimension,
            resize_exactness: options.resize_exactness,
            layouts: BatchParameterLayouts {
                native_expand_output: plan.native_expand_output,
                ..options.layouts
            },
        })?;
        scratch.params_ranges.push(append_arena_slice(
            &mut scratch.params_arena,
            &params,
            options.uniform_alignment,
        ));
        Ok(())
    }

    fn prepare_batch_operation_auxiliaries(
        &self,
        options: BatchOperationPreparationOptions<'_>,
        iteration: BatchOperationIteration<'_>,
        cached: &CachedPipeline,
        plan: &BatchOperationPreparationPlan,
        buffers: &BufferPool,
        scratch: &mut BatchPreparationScratch,
    ) -> Result<(), PilError> {
        let op = iteration.op;
        let (cur_w, cur_h) = iteration.source_dimensions;
        let (out_w, out_h) = plan.output_dimensions;
        let resize_coeff_range = self.prepare_batch_resize_coefficients(
            BatchResizeCoefficientContext {
                op,
                input_dimensions: (cur_w, cur_h),
                output_dimensions: (out_w, out_h),
                logical_mode: options.logical_mode,
                op_mode: plan.op_mode,
                f_resize_f64_is_exact: options.resize_exactness.f64_is_exact,
                f_resize_f64_ordered_is_exact: options.resize_exactness.f64_ordered_is_exact,
                boxed_resize_coefficients: plan.boxed_resize_coefficients.as_ref(),
                storage_alignment: options.storage_alignment,
            },
            &mut scratch.img2_arena,
            options.auxiliary_cache,
        )?;
        scratch.resize_coeff_ranges.push(resize_coeff_range);
        scratch.pad_resize_dims.push(
            matches!(op, PipelineOp::Pad { .. })
                .then(|| gpu_pad_geometry(op, cur_w, cur_h).map(|(dims, _)| dims))
                .flatten(),
        );
        let auxiliary_ranges = GpuBatchAuxiliaryPreparation {
            op,
            auxiliary: &options.auxiliary_images[iteration.index],
            index: iteration.index,
            op_mode: plan.op_mode,
            logical_mode: options.logical_mode,
            source_dimensions: (cur_w, cur_h),
            cached_pipeline: cached,
            packed_luma_putdata: options.packed_luma_putdata,
            direct_alpha_composite_rgba: options.direct_alpha_composite_rgba,
            capacity: buffers.capacity,
            storage_alignment: options.storage_alignment,
            auxiliary_cache: options.auxiliary_cache,
            img2_arena: &mut scratch.img2_arena,
            img3_arena: &mut scratch.img3_arena,
            lut_arena: &mut scratch.lut_arena,
            second_cache: &mut scratch.second_cache,
            third_cache: &mut scratch.third_cache,
            lut_cache: &mut scratch.lut_cache,
        }
        .prepare()?;
        scratch.img2_ranges.push(auxiliary_ranges.second);
        scratch.img3_ranges.push(auxiliary_ranges.third);
        scratch.lut_ranges.push(auxiliary_ranges.lut);
        Ok(())
    }
}
