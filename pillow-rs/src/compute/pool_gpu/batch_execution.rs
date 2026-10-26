//! GPU batch chunk scheduling, command encoding, and submission.
//! The caller retains the working set while this method prepares, submits, and selects readback.

use super::*;

mod routing;

pub(super) type GpuBatchExecutionResult = Result<
    (
        bool,
        u32,
        u32,
        ReadbackTarget,
        PipelineResourceTelemetry,
        u64,
        bool,
    ),
    PilError,
>;

pub(super) fn select_gpu_chunk_end(
    chunk_start: usize,
    resource_bytes: &[usize],
    shader_work_items: &[u64],
) -> Option<usize> {
    if resource_bytes.len() != shader_work_items.len() {
        return None;
    }
    let mut chunk_end = chunk_start;
    let mut total_bytes = 0usize;
    let mut total_work = 0u64;
    while chunk_end < resource_bytes.len() && chunk_end - chunk_start < MAX_GPU_OPS_PER_SUBMISSION {
        let op_bytes = resource_bytes[chunk_end];
        let op_work = shader_work_items[chunk_end];
        if chunk_end > chunk_start
            && (total_bytes.saturating_add(op_bytes) > MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION
                || total_work.saturating_add(op_work) > MAX_GPU_SHADER_WORK_ITEMS)
        {
            break;
        }
        total_bytes = total_bytes.saturating_add(op_bytes);
        total_work = total_work.saturating_add(op_work);
        chunk_end += 1;
    }
    (chunk_end > chunk_start).then_some(chunk_end)
}

pub(super) struct GpuBatchResources<'a> {
    pub(super) buf_a: &'a wgpu::Buffer,
    pub(super) buf_b: &'a wgpu::Buffer,
    pub(super) initial_input: Option<&'a wgpu::Buffer>,
    pub(super) fallback_img2: &'a wgpu::Buffer,
    pub(super) fallback_img3: &'a wgpu::Buffer,
    pub(super) fallback_lut: &'a wgpu::Buffer,
    pub(super) histogram: &'a wgpu::Buffer,
    pub(super) params: &'a wgpu::Buffer,
    pub(super) params_ranges: Vec<BufferRange>,
    pub(super) img2: Option<&'a wgpu::Buffer>,
    pub(super) img2_ranges: Vec<Option<BufferRange>>,
    pub(super) resize_coeff_ranges: Vec<Option<(BufferRange, BufferRange)>>,
    pub(super) img3: Option<&'a wgpu::Buffer>,
    pub(super) img3_ranges: Vec<Option<BufferRange>>,
    pub(super) lut: Option<&'a wgpu::Buffer>,
    pub(super) lut_ranges: Vec<Option<BufferRange>>,
    pub(super) native_expand_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_luma_transform_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_rgb_packed_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_la_transform_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
}

pub(super) struct PreparedGpuBatch<'a> {
    pub(super) resources: GpuBatchResources<'a>,
    pub(super) input_dims: Vec<(u32, u32)>,
    pub(super) output_dims: Vec<(u32, u32)>,
    /// Source row range represented by a boxed resize's compact horizontal
    /// intermediate. Other operations retain their complete input height.
    pub(super) resize_source_rows: Vec<Option<(u32, u32)>>,
    /// Contain dimensions used by a public Pad operation before its final
    /// placement pass.  The resize and placement remain in one command
    /// buffer, so this is planner metadata rather than a host-side image
    /// handoff.
    pub(super) pad_resize_dims: Vec<Option<(u32, u32)>>,
    pub(super) final_dims: (u32, u32),
    pub(super) resource_telemetry: PipelineResourceTelemetry,
}

struct BatchExecutionConfig<'a> {
    ops: &'a [PipelineOp],
    auxiliary_images: &'a [AuxiliaryImages],
    dimensions: (u32, u32),
    mode: u32,
    packed_luma_colorize: bool,
    packed_luma_convert: bool,
    packed_luma_transform_input: bool,
    packed_luma_transform_output: bool,
    native_rgb_compact_input: bool,
    native_rgb_packed_output: bool,
    native_la_transform_output: bool,
    packed_luma_point: bool,
    packed_luma_putdata: bool,
    packed_luma_order_statistic: bool,
    packed_native_byte_filter: bool,
    packed_luma_pad: bool,
    packed_la_pad: bool,
    native_extract_band: bool,
    native_grayscale_rgb_input: bool,
    native_sharpness_l_input: bool,
    native_sharpness_la_input: bool,
    native_sharpness_rgb_input: bool,
    native_reduce_rgb_input: bool,
    native_expand_channels: Option<u8>,
    native_cover_input_channels: Option<u8>,
    native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
    logical_mode: Option<&'a str>,
    contrast_mean: Option<u8>,
    f_resize_constant_bits: Option<u32>,
    f_resize_box_copy_is_exact: bool,
    f_resize_identity_is_exact: bool,
    f_resize_box_average_is_exact: bool,
    f_resize_dyadic_is_exact: bool,
    f_resize_f64_is_exact: bool,
    f_resize_f64_ordered_is_exact: bool,
    initial_input: Option<&'a wgpu::Buffer>,
}

struct BatchExecutionState {
    current_is_a: bool,
    dimensions: (u32, u32),
    current_mode: u32,
    dispatch_count: u64,
    native_expand_output: bool,
    readback: Option<ReadbackTarget>,
    resource_telemetry: PipelineResourceTelemetry,
    submission_index: usize,
}

impl GpuInner {
    #[cfg(test)]
    pub(super) fn execute_batch_impl(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        packed_luma_colorize: bool,
        packed_luma_convert: bool,
        packed_luma_transform_input: bool,
        packed_luma_transform_output: bool,
        native_rgb_compact_input: bool,
        native_rgb_packed_output: bool,
        native_la_transform_output: bool,
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        packed_la_pad: bool,
        native_extract_band: bool,
        native_grayscale_rgb_input: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_reduce_rgb_input: bool,
        native_expand_channels: Option<u8>,
        native_cover_input_channels: Option<u8>,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        initial_input: Option<&wgpu::Buffer>,
        buffers: &mut BufferPool,
    ) -> GpuBatchExecutionResult {
        let config = BatchExecutionConfig {
            ops,
            auxiliary_images,
            dimensions: (w, h),
            mode,
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_transform_input,
            packed_luma_transform_output,
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            native_extract_band,
            native_grayscale_rgb_input,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
            native_reduce_rgb_input,
            native_expand_channels,
            native_cover_input_channels,
            native_cover_packed_output,
            logical_mode,
            contrast_mean,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact,
            f_resize_f64_ordered_is_exact,
            initial_input,
        };
        self.execute_batch_config(&config, buffers)
    }

    fn execute_batch_config(
        &self,
        config: &BatchExecutionConfig<'_>,
        buffers: &mut BufferPool,
    ) -> GpuBatchExecutionResult {
        if let Some(result) = self.try_execute_f_resize_tiled(
            config.ops,
            config.dimensions,
            config.logical_mode,
            config.f_resize_f64_is_exact,
            config.f_resize_f64_ordered_is_exact,
            buffers,
        ) {
            return result;
        }
        gpu_log!(
            "[GPU] batch_impl: {} ops, start dims {}x{}",
            config.ops.len(),
            config.dimensions.0,
            config.dimensions.1
        );
        if config.ops.len() != config.auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }
        let (estimates, auxiliary_cache) = self.prepare_batch_execution(config, buffers)?;
        self.execute_batch_chunks(config, &estimates, &auxiliary_cache, buffers)
    }

    fn try_execute_f_resize_tiled(
        &self,
        ops: &[PipelineOp],
        dimensions: (u32, u32),
        logical_mode: Option<&str>,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        buffers: &mut BufferPool,
    ) -> Option<GpuBatchExecutionResult> {
        if logical_mode != Some("F") || !(f_resize_f64_is_exact || f_resize_f64_ordered_is_exact) {
            return None;
        }
        let [
            PipelineOp::Resize {
                w: out_w,
                h: out_h,
                filter,
            },
        ] = ops
        else {
            return None;
        };
        if matches!(filter, ResampleFilter::Nearest)
            || (matches!(filter, ResampleFilter::Box)
                && gpu_f_resize_compact_box_any_axis(dimensions, (*out_w, *out_h)))
        {
            return None;
        }
        let (kernel, support) = filter_from_resample(*filter);
        let horizontal = precompute_coeffs_f64(*out_w, dimensions.0, kernel, support);
        let vertical = precompute_coeffs_f64(*out_h, dimensions.1, kernel, support);
        if !gpu_f_resize_coefficients_need_tiles(&horizontal)
            && !gpu_f_resize_coefficients_need_tiles(&vertical)
        {
            return None;
        }
        Some(
            self.execute_f_resize_tiled(
                dimensions,
                (*out_w, *out_h),
                &horizontal,
                &vertical,
                buffers,
            )
            .map(
                |(current_is_a, width, height, readback, telemetry, dispatches)| {
                    (
                        current_is_a,
                        width,
                        height,
                        readback,
                        telemetry,
                        dispatches,
                        false,
                    )
                },
            ),
        )
    }

    fn prepare_batch_execution(
        &self,
        config: &BatchExecutionConfig<'_>,
        buffers: &mut BufferPool,
    ) -> Result<(BatchExecutionEstimates, GpuAuxiliaryCache), PilError> {
        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let estimates = self.estimate_batch_resources(
            config.ops,
            config.auxiliary_images,
            config.dimensions,
            config.mode,
            BatchEstimateOptions {
                uniform_alignment,
                storage_alignment,
                packed_luma_putdata: config.packed_luma_putdata,
                logical_mode: config.logical_mode,
                f_resize_f64_is_exact: config.f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact: config.f_resize_f64_ordered_is_exact,
            },
            buffers,
        )?;
        let auxiliary_cache = GpuAuxiliaryCache::from_batch(
            config.ops,
            config.auxiliary_images,
            config.mode,
            buffers.capacity,
            storage_alignment,
        )?;
        self.upload_auxiliary_cache(&auxiliary_cache, buffers, storage_alignment);
        Ok((estimates, auxiliary_cache))
    }

    fn execute_batch_chunks(
        &self,
        config: &BatchExecutionConfig<'_>,
        estimates: &BatchExecutionEstimates,
        auxiliary_cache: &GpuAuxiliaryCache,
        buffers: &mut BufferPool,
    ) -> GpuBatchExecutionResult {
        let mut state = BatchExecutionState {
            current_is_a: true,
            dimensions: config.dimensions,
            current_mode: config.mode,
            dispatch_count: 0,
            native_expand_output: false,
            readback: None,
            resource_telemetry: PipelineResourceTelemetry {
                auxiliary_bytes: auxiliary_cache.total_bytes() as u64,
                ..PipelineResourceTelemetry::default()
            },
            submission_index: 0,
        };
        let mut chunk_start = 0usize;
        while chunk_start < config.ops.len() {
            self.ensure_healthy("GPU batch submission")?;
            let chunk_end = select_gpu_chunk_end(
                chunk_start,
                &estimates.resource_bytes,
                &estimates.shader_work_items,
            )
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU chunk scheduling made no progress because its estimates are inconsistent"
                        .into(),
                )
            })?;
            self.execute_batch_chunk(
                config,
                estimates,
                auxiliary_cache,
                chunk_start,
                chunk_end,
                &mut state,
                buffers,
            )?;
            chunk_start = chunk_end;
        }
        let readback = state.readback.ok_or_else(|| {
            PilError::InternalError("GPU batch produced no readback target".into())
        })?;
        Ok((
            state.current_is_a,
            state.dimensions.0,
            state.dimensions.1,
            readback,
            state.resource_telemetry,
            state.dispatch_count,
            state.native_expand_output,
        ))
    }

    fn execute_batch_chunk(
        &self,
        config: &BatchExecutionConfig<'_>,
        estimates: &BatchExecutionEstimates,
        auxiliary_cache: &GpuAuxiliaryCache,
        chunk_start: usize,
        chunk_end: usize,
        state: &mut BatchExecutionState,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        let metrics = batch_chunk_metrics(config, estimates, chunk_start, chunk_end, state);
        state.dispatch_count = state.dispatch_count.saturating_add(metrics.dispatch_count);
        state.resource_telemetry.fused_operation_count = state
            .resource_telemetry
            .fused_operation_count
            .saturating_add(metrics.fused_operation_count);
        let mut prepared = self.prepare_execution_chunk(
            config,
            state,
            auxiliary_cache,
            chunk_start,
            chunk_end,
            buffers,
        )?;
        state.native_expand_output |= prepared.resources.native_expand_output.is_some();
        state.resource_telemetry.parameter_bytes = state
            .resource_telemetry
            .parameter_bytes
            .saturating_add(prepared.resource_telemetry.parameter_bytes);
        state.resource_telemetry.auxiliary_bytes = state
            .resource_telemetry
            .auxiliary_bytes
            .saturating_add(prepared.resource_telemetry.auxiliary_bytes);
        let (final_dims, readback) =
            self.encode_prepared_batch_chunk(config, &mut prepared, state, chunk_start, chunk_end)?;
        drop(prepared);
        self.poll_device("GPU batch submission")?;
        state.submission_index += 1;
        gpu_log!(
            "[GPU] submitted chunk={} ops={}..{} resources={} bytes work={}",
            state.submission_index,
            chunk_start,
            chunk_end,
            metrics.estimated_bytes,
            metrics.estimated_work
        );
        state.dimensions = final_dims;
        state.current_mode =
            gpu_mode_after_ops(state.current_mode, &config.ops[chunk_start..chunk_end]);
        if let Some(readback) = readback {
            state.readback = Some(readback);
        }
        Ok(())
    }

    fn prepare_execution_chunk<'a>(
        &self,
        config: &'a BatchExecutionConfig<'a>,
        state: &BatchExecutionState,
        auxiliary_cache: &GpuAuxiliaryCache,
        chunk_start: usize,
        chunk_end: usize,
        buffers: &'a mut BufferPool,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        let mut prepared = self.prepare_batch(
            &config.ops[chunk_start..chunk_end],
            &config.auxiliary_images[chunk_start..chunk_end],
            state.dimensions.0,
            state.dimensions.1,
            state.current_mode,
            config.logical_mode,
            config.contrast_mean,
            config.f_resize_constant_bits,
            config.f_resize_box_copy_is_exact,
            config.f_resize_identity_is_exact,
            config.f_resize_box_average_is_exact,
            config.f_resize_dyadic_is_exact,
            config.f_resize_f64_is_exact,
            config.f_resize_f64_ordered_is_exact,
            config.packed_luma_colorize,
            config.packed_luma_convert,
            config.packed_luma_transform_input,
            config.packed_luma_transform_output,
            config.native_rgb_compact_input,
            config.native_rgb_packed_output,
            config.native_la_transform_output,
            config.packed_luma_point,
            config.packed_luma_putdata,
            config.packed_luma_order_statistic,
            config.packed_native_byte_filter,
            config.packed_luma_pad,
            config.native_extract_band,
            config.native_grayscale_rgb_input,
            config.native_sharpness_l_input,
            config.native_sharpness_la_input,
            config.native_sharpness_rgb_input,
            config.native_reduce_rgb_input,
            config.native_expand_channels,
            config.native_cover_input_channels,
            config.native_cover_packed_output,
            buffers,
            auxiliary_cache,
        )?;
        if chunk_start == 0 {
            prepared.resources.initial_input = config.initial_input;
        }
        Ok(prepared)
    }

    fn encode_prepared_batch_chunk(
        &self,
        config: &BatchExecutionConfig<'_>,
        prepared: &mut PreparedGpuBatch<'_>,
        state: &mut BatchExecutionState,
        chunk_start: usize,
        chunk_end: usize,
    ) -> Result<((u32, u32), Option<ReadbackTarget>), PilError> {
        let final_dims = prepared.final_dims;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_batch_encoder"),
            });
        state.current_is_a = self.encode_batch(
            &mut encoder,
            &config.ops[chunk_start..chunk_end],
            prepared,
            state.current_is_a,
            config.logical_mode,
            config.packed_luma_point,
            config.packed_luma_putdata,
            config.packed_luma_order_statistic,
            config.packed_native_byte_filter,
            config.packed_luma_pad,
            config.packed_la_pad,
            config.packed_luma_transform_output,
            config.native_sharpness_l_input,
            config.native_sharpness_la_input,
            config.native_sharpness_rgb_input,
        )?;
        let readback = if chunk_end == config.ops.len() {
            let size = final_batch_readback_size(prepared, config)?;
            let source = if state.current_is_a {
                prepared.resources.buf_a
            } else {
                prepared.resources.buf_b
            };
            let target = self.prepare_readback(source, size)?;
            if let ReadbackTarget::Staging(staging) = &target {
                // Keep the staged copy after compute in this command buffer.
                // Direct mapping waits on the final primary-buffer use.
                encoder.copy_buffer_to_buffer(source, 0, &staging.buffer, 0, size);
            }
            Some(target)
        } else {
            None
        };
        self.queue.submit(Some(encoder.finish()));
        Ok((final_dims, readback))
    }
}

struct BatchChunkMetrics {
    dispatch_count: u64,
    fused_operation_count: u64,
    estimated_bytes: usize,
    estimated_work: u64,
}

fn batch_chunk_metrics(
    config: &BatchExecutionConfig<'_>,
    estimates: &BatchExecutionEstimates,
    chunk_start: usize,
    chunk_end: usize,
    state: &BatchExecutionState,
) -> BatchChunkMetrics {
    let chunk_ops = &config.ops[chunk_start..chunk_end];
    let dispatch_count = if (config.packed_native_byte_filter
        && matches!(config.logical_mode, None | Some("L"))
        || config.native_rgb_packed_output && matches!(config.logical_mode, None | Some("RGB")))
        && matches!(chunk_ops, [PipelineOp::BoxBlur { radius: 1 }])
    {
        1
    } else {
        gpu_dispatch_count(chunk_ops, config.logical_mode, state.dimensions)
    };
    let fused_operation_count =
        gpu_fused_operation_count(chunk_ops, config.logical_mode, state.dimensions);
    let estimated_bytes = estimates.resource_bytes[chunk_start..chunk_end]
        .iter()
        .fold(0usize, |total, bytes| total.saturating_add(*bytes));
    let estimated_work = estimates.shader_work_items[chunk_start..chunk_end]
        .iter()
        .fold(0u64, |total, work| total.saturating_add(*work));
    BatchChunkMetrics {
        dispatch_count,
        fused_operation_count,
        estimated_bytes,
        estimated_work,
    }
}

fn final_batch_readback_size(
    prepared: &PreparedGpuBatch<'_>,
    config: &BatchExecutionConfig<'_>,
) -> Result<u64, PilError> {
    let final_dims = prepared.final_dims;
    if let Some(dispatch) = prepared.resources.native_cover_packed_output {
        Ok(dispatch.transfer_bytes)
    } else if let Some(dispatch) = prepared.resources.native_expand_output {
        Ok(dispatch.transfer_bytes)
    } else if let Some(dispatch) = prepared.resources.native_rgb_packed_output {
        Ok(dispatch.transfer_bytes)
    } else if let Some(dispatch) = prepared.resources.native_la_transform_output {
        Ok(dispatch.transfer_bytes)
    } else if config.packed_luma_transform_output {
        compact_luma8_transfer_bytes(final_dims.0, final_dims.1)
    } else if config.packed_la_pad {
        compact_native_channel_transfer_bytes(final_dims.0, final_dims.1, 2)
    } else if config.packed_luma_point
        || config.packed_luma_putdata
        || config.packed_luma_order_statistic
        || config.packed_native_byte_filter
        || config.packed_luma_pad
        || config.native_sharpness_l_input
        || matches!(
            config.ops.last(),
            Some(PipelineOp::ExtractBand { .. } | PipelineOp::Grayscale)
        )
    {
        compact_luma8_transfer_bytes(final_dims.0, final_dims.1)
    } else {
        Ok(CheckedDims::new(final_dims.0, final_dims.1, 4)?.total_bytes() as u64)
    }
}
