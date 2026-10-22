//! GPU batch chunk scheduling, command encoding, and submission.
//! The caller retains the working set while this method prepares, submits, and selects readback.

use super::*;

impl GpuInner {
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
    ) -> Result<
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
    > {
        if logical_mode == Some("F") && (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact) {
            if let [
                PipelineOp::Resize {
                    w: out_w,
                    h: out_h,
                    filter,
                },
            ] = ops
            {
                if !matches!(filter, ResampleFilter::Nearest)
                    && !(matches!(filter, ResampleFilter::Box)
                        && gpu_f_resize_compact_box_any_axis((w, h), (*out_w, *out_h)))
                {
                    let (kernel, support) = filter_from_resample(*filter);
                    let horizontal = precompute_coeffs_f64(*out_w, w, kernel, support);
                    let vertical = precompute_coeffs_f64(*out_h, h, kernel, support);
                    if gpu_f_resize_coefficients_need_tiles(&horizontal)
                        || gpu_f_resize_coefficients_need_tiles(&vertical)
                    {
                        return self
                            .execute_f_resize_tiled(
                                (w, h),
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
                            );
                    }
                }
            }
        }
        let mut current_is_a = true;
        let mut cur_w = w;
        let mut cur_h = h;
        let mut dispatch_count = 0u64;
        let mut native_expand_output = false;
        gpu_log!(
            "[GPU] batch_impl: {} ops, start dims {}x{}",
            ops.len(),
            cur_w,
            cur_h
        );
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let BatchExecutionEstimates {
            resource_bytes,
            shader_work_items,
        } = self.estimate_batch_resources(
            ops,
            auxiliary_images,
            (w, h),
            mode,
            BatchEstimateOptions {
                uniform_alignment,
                storage_alignment,
                packed_luma_putdata,
                logical_mode,
                f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact,
            },
            buffers,
        )?;
        let auxiliary_cache = GpuAuxiliaryCache::from_batch(
            ops,
            auxiliary_images,
            mode,
            buffers.capacity,
            storage_alignment,
        )?;
        self.upload_auxiliary_cache(&auxiliary_cache, buffers, storage_alignment);
        let mut chunk_start = 0usize;
        let mut submission_index = 0usize;
        let mut readback = None;
        let mut resource_telemetry = PipelineResourceTelemetry {
            auxiliary_bytes: auxiliary_cache.total_bytes() as u64,
            ..PipelineResourceTelemetry::default()
        };
        let mut current_mode = mode;
        while chunk_start < ops.len() {
            self.ensure_healthy("GPU batch submission")?;
            let chunk_end = select_gpu_chunk_end(chunk_start, &resource_bytes, &shader_work_items)
                .ok_or_else(|| {
                    PilError::InternalError(
                    "GPU chunk scheduling made no progress because its estimates are inconsistent"
                        .into(),
                )
                })?;
            let chunk_ops = &ops[chunk_start..chunk_end];
            let chunk_dispatch_count = if (packed_native_byte_filter
                && matches!(logical_mode, None | Some("L"))
                || native_rgb_packed_output && matches!(logical_mode, None | Some("RGB")))
                && matches!(chunk_ops, [PipelineOp::BoxBlur { radius: 1 }])
            {
                1
            } else {
                gpu_dispatch_count(chunk_ops, logical_mode, (cur_w, cur_h))
            };
            dispatch_count = dispatch_count.saturating_add(chunk_dispatch_count);
            resource_telemetry.fused_operation_count = resource_telemetry
                .fused_operation_count
                .saturating_add(gpu_fused_operation_count(
                    chunk_ops,
                    logical_mode,
                    (cur_w, cur_h),
                ));
            let estimated_bytes = resource_bytes[chunk_start..chunk_end]
                .iter()
                .fold(0usize, |total, bytes| total.saturating_add(*bytes));
            let estimated_work = shader_work_items[chunk_start..chunk_end]
                .iter()
                .fold(0u64, |total, work| total.saturating_add(*work));

            let mut prepared = self.prepare_batch(
                &ops[chunk_start..chunk_end],
                &auxiliary_images[chunk_start..chunk_end],
                cur_w,
                cur_h,
                current_mode,
                logical_mode,
                contrast_mean,
                f_resize_constant_bits,
                f_resize_box_copy_is_exact,
                f_resize_identity_is_exact,
                f_resize_box_average_is_exact,
                f_resize_dyadic_is_exact,
                f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact,
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
                native_extract_band,
                native_grayscale_rgb_input,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                native_cover_packed_output,
                buffers,
                &auxiliary_cache,
            )?;
            if chunk_start == 0 {
                prepared.resources.initial_input = initial_input;
            }
            native_expand_output |= prepared.resources.native_expand_output.is_some();
            resource_telemetry.parameter_bytes = resource_telemetry
                .parameter_bytes
                .saturating_add(prepared.resource_telemetry.parameter_bytes);
            resource_telemetry.auxiliary_bytes = resource_telemetry
                .auxiliary_bytes
                .saturating_add(prepared.resource_telemetry.auxiliary_bytes);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_batch_encoder"),
                });
            current_is_a = self.encode_batch(
                &mut encoder,
                &ops[chunk_start..chunk_end],
                &prepared,
                current_is_a,
                logical_mode,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                packed_native_byte_filter,
                packed_luma_pad,
                packed_la_pad,
                packed_luma_transform_output,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_rgb_input,
            )?;

            let final_dims = prepared.final_dims;
            if chunk_end == ops.len() {
                let size = if let Some(dispatch) = prepared.resources.native_cover_packed_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_expand_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_rgb_packed_output {
                    dispatch.transfer_bytes
                } else if let Some(dispatch) = prepared.resources.native_la_transform_output {
                    dispatch.transfer_bytes
                } else if packed_luma_transform_output {
                    compact_luma8_transfer_bytes(final_dims.0, final_dims.1)?
                } else if packed_la_pad {
                    compact_native_channel_transfer_bytes(final_dims.0, final_dims.1, 2)?
                } else if packed_luma_point
                    || packed_luma_putdata
                    || packed_luma_order_statistic
                    || packed_native_byte_filter
                    || packed_luma_pad
                    || native_sharpness_l_input
                    || matches!(
                        ops.last(),
                        Some(PipelineOp::ExtractBand { .. } | PipelineOp::Grayscale)
                    )
                {
                    compact_luma8_transfer_bytes(final_dims.0, final_dims.1)?
                } else {
                    CheckedDims::new(final_dims.0, final_dims.1, 4)?.total_bytes() as u64
                };
                let src = if current_is_a {
                    prepared.resources.buf_a
                } else {
                    prepared.resources.buf_b
                };
                let target = self.prepare_readback(src, size)?;
                if let ReadbackTarget::Staging(staging) = &target {
                    // Keep the staged copy after compute in the same command
                    // buffer. Direct mapping instead waits on this submission's
                    // final primary-buffer use, without an extra device copy.
                    encoder.copy_buffer_to_buffer(src, 0, &staging.buffer, 0, size);
                }
                readback = Some(target);
            }
            self.queue.submit(Some(encoder.finish()));
            // The queue preserves submission order: the next chunk's writes
            // to these arenas are ordered after this chunk's reads. Release
            // the borrow only after the command buffer has captured it.
            drop(prepared);
            self.poll_device("GPU batch submission")?;
            submission_index += 1;
            gpu_log!(
                "[GPU] submitted chunk={} ops={}..{} resources={} bytes work={}",
                submission_index,
                chunk_start,
                chunk_end,
                estimated_bytes,
                estimated_work
            );
            (cur_w, cur_h) = final_dims;
            current_mode = gpu_mode_after_ops(current_mode, &ops[chunk_start..chunk_end]);
            chunk_start = chunk_end;
        }
        // After all chunks, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b. Queue ordering
        // keeps chunk submissions dependent without a blocking poll between them.
        let readback = readback.ok_or_else(|| {
            PilError::InternalError("GPU batch produced no readback target".into())
        })?;
        Ok((
            current_is_a,
            cur_w,
            cur_h,
            readback,
            resource_telemetry,
            dispatch_count,
            native_expand_output,
        ))
    }
}
