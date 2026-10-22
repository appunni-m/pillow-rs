use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeHsvPadDispatch {
    pub(super) output: NativePackedChannelOutputDispatch,
    pub(super) input_bytes: usize,
    pub(super) input_transfer_bytes: u64,
    pub(super) output_width: u32,
    pub(super) output_height: u32,
    pub(super) offset_y: u32,
    pub(super) fill_pixel: u32,
}

/// Plan native HSV padding for identity contain geometry. RGB triples use
/// three packed output words per four pixels; requiring complete four-pixel
/// groups per row keeps both source and output word addresses aligned.
#[cfg(target_endian = "little")]
pub(super) fn plan_native_hsv_pad_dispatch(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeHsvPadDispatch> {
    let [
        op @ PipelineOp::Pad {
            w: dst_w, h: dst_h, ..
        },
    ] = ops
    else {
        return None;
    };
    if logical_mode != Some("HSV") {
        return None;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return None;
    };
    if *dst_w != image.width() || *dst_w % 4 != 0 {
        return None;
    }
    let source_dimensions = image.dimensions();
    let (resize_dimensions, (offset_x, offset_y)) =
        gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)?;
    if resize_dimensions != source_dimensions || offset_x != 0 {
        return None;
    }
    let source_end_y = offset_y.checked_add(resize_dimensions.1)?;
    if source_end_y > *dst_h {
        return None;
    }

    let input_layout = CheckedDims::new(image.width(), image.height(), 3).ok()?;
    let input_bytes = input_layout.total_bytes();
    if input_bytes == 0 || rgb.as_raw().len() != input_bytes || input_bytes > u32::MAX as usize {
        return None;
    }
    let input_transfer_bytes =
        u64::try_from(input_bytes.checked_add(3)?.checked_div(4)?.checked_mul(4)?).ok()?;
    let buffer_capacity_bytes = u64::from(GPU_BUFFER_CAPACITY).checked_mul(4)?;
    if input_transfer_bytes > buffer_capacity_bytes
        || input_transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || input_transfer_bytes > max_buffer_size
    {
        return None;
    }

    let output = plan_native_rgb_point_output(
        *dst_w,
        *dst_h,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        GPU_BUFFER_CAPACITY,
    )?;
    if !output.row_tiled {
        return None;
    }

    Some(NativeHsvPadDispatch {
        output,
        input_bytes,
        input_transfer_bytes,
        output_width: *dst_w,
        output_height: *dst_h,
        offset_y,
        fill_pixel: gpu_pad_fill(op, logical_mode, execution_mode_code(image, logical_mode)),
    })
}

impl GpuInner {
    /// Keep identity-contain HSV Pad in its native three-byte representation.
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_hsv_pad(
        &self,
        image: &DynamicImage,
        dispatch: NativeHsvPadDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Err(PilError::InternalError(
                "GPU native HSV Pad requires ImageRgb8 storage".into(),
            ));
        };
        if rgb.as_raw().len() != dispatch.input_bytes
            || CheckedDims::new(image.width(), image.height(), 3)?.total_bytes()
                != dispatch.input_bytes
            || dispatch.output.transfer_bytes
                > u64::from(buffers.capacity)
                    .checked_mul(4)
                    .ok_or_else(|| PilError::ValueError("GPU HSV Pad buffer is too large".into()))?
            || dispatch.input_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native HSV Pad layout exceeds its checked buffers".into(),
            ));
        }

        buffers.upload_native_channel_bytes(
            &self.queue,
            image.width(),
            image.height(),
            3,
            rgb.as_raw(),
        )?;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_pad_hsv_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            64,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            image.width(),
            image.height(),
            2,
            0,
            image.width(),
            image.height(),
            3,
            0,
            dispatch.output_width,
            dispatch.output_height,
            dispatch.fill_pixel,
            0,
            dispatch.offset_y,
            0,
            0,
            0,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_pad_hsv_native_rgb",
            "pad_hsv_native_rgb.wgsl",
            include_str!("shaders/pad_hsv_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_pad_hsv_native_rgb"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.input_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output.transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 64,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_pad_hsv_native_rgb_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_pad_hsv_native_rgb_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.output.groups_x)
                    .saturating_mul(u64::from(dispatch.output.groups_y)),
            );
            pass.dispatch_workgroups(dispatch.output.groups_x, dispatch.output.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output.transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output.transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        let result = self.readback_to_native_channels(
            dispatch.output_width,
            dispatch.output_height,
            3,
            readback.buffer(buffers, false),
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes,
            readback_bytes: dispatch.output.transfer_bytes,
            parameter_bytes: 64,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 2 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }
}
