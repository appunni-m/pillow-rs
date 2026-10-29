use super::{
    BufferPool, CachedPipeline, CheckedDims, DynamicImage, GPU_BUFFER_CAPACITY, GpuInner,
    NativeByteFlipDispatch, NativeFlipFormat, NativeGeometryDispatchOutcome, NonZeroU64, PilError,
    PipelineOp, PipelineResourceTelemetry, ReadbackTarget, TransposeMethod, gpu_native_flip_format,
    packed_rgb_transpose_layout, plan_native_byte_flip_dispatch, ranged_binding,
};
use crate::compute::pool_gpu::BufferRange;

#[cfg(target_endian = "little")]
struct NativeByteFlipSubmission<'a> {
    cached: &'a CachedPipeline,
    bind_group: &'a wgpu::BindGroup,
    width: u32,
    height: u32,
    channels: u8,
    dispatch: NativeByteFlipDispatch,
    dimensions: CheckedDims,
    padded_row_bytes: usize,
}

impl GpuInner {
    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn upload_native_byte_flip_rows(
        &self,
        image: &DynamicImage,
        dispatch: NativeByteFlipDispatch,
        padded_row_bytes: usize,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        let upload_size = NonZeroU64::new(dispatch.transfer_bytes)
            .ok_or_else(|| PilError::ValueError("GPU native Flip input is empty".into()))?;
        let mut upload = self
            .queue
            .write_buffer_with(&buffers.buf_a, 0, upload_size)
            .ok_or_else(|| {
                PilError::InternalError("GPU native Flip staging allocation failed".into())
            })?;
        upload.fill(0);
        let raw = image.as_bytes();
        if padded_row_bytes == dispatch.row_bytes {
            upload[..raw.len()].copy_from_slice(raw);
        } else {
            for row in 0..image.height() as usize {
                let source_start = row * dispatch.row_bytes;
                let target_start = row * padded_row_bytes;
                upload[target_start..target_start + dispatch.row_bytes]
                    .copy_from_slice(&raw[source_start..source_start + dispatch.row_bytes]);
            }
        }
        drop(upload);
        Ok(())
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn native_byte_flip_bind_group(
        &self,
        cached: &CachedPipeline,
        buffers: &BufferPool,
        transfer_bytes: u64,
    ) -> Result<wgpu::BindGroup, PilError> {
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_byte_flip"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        }))
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn submit_native_byte_flip(
        &self,
        buffers: &mut BufferPool,
        submission: NativeByteFlipSubmission<'_>,
    ) -> Result<DynamicImage, PilError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_byte_flip"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_byte_flip"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&submission.cached.pipeline);
            pass.set_bind_group(0, submission.bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                submission.cached.variant_name,
                submission.cached.shader_file,
                u64::from(submission.dispatch.groups_x) * u64::from(submission.dispatch.groups_y),
            );
            pass.dispatch_workgroups(
                submission.dispatch.groups_x,
                submission.dispatch.groups_y,
                1,
            );
        }
        let readback = self.prepare_readback(&buffers.buf_b, submission.dispatch.transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                submission.dispatch.transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native Flip submission")?;
        let result = self.readback_with(
            submission.dispatch.transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != submission.dispatch.transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native Flip readback length mismatch".into(),
                    ));
                }
                let mut bytes = Vec::with_capacity(submission.dimensions.total_bytes());
                for row in mapped
                    .chunks_exact(submission.padded_row_bytes)
                    .take(submission.height as usize)
                {
                    bytes.extend_from_slice(&row[..submission.dispatch.row_bytes]);
                }
                if bytes.len() != submission.dimensions.total_bytes() {
                    return Err(PilError::InternalError(
                        "GPU native Flip omitted a source row".into(),
                    ));
                }
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(
                    submission.width,
                    submission.height,
                    bytes,
                    usize::from(submission.channels),
                )
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: submission.dispatch.transfer_bytes,
            readback_bytes: submission.dispatch.transfer_bytes,
            parameter_bytes: 16,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 1
                + u64::from(!self.direct_primary_readback)
                + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_byte_flip(
        &self,
        image: &DynamicImage,
        format: NativeFlipFormat,
        dispatch: NativeByteFlipDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let width = image.width();
        let height = image.height();
        let channels = format.channels();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let raw = image.as_bytes();
        let padded_row_bytes = usize::try_from(dispatch.words_per_row)
            .ok()
            .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
            .ok_or_else(|| PilError::ValueError("GPU native Flip row is too large".into()))?;
        let expected_transfer_bytes = u64::try_from(padded_row_bytes)
            .ok()
            .and_then(|row| row.checked_mul(u64::from(height)))
            .ok_or_else(|| PilError::ValueError("GPU native Flip transfer is too large".into()))?;
        let buffer_capacity = u64::from(buffers.capacity) * 4;
        let limits = self.device.limits();
        if raw.len() != dimensions.total_bytes()
            || dispatch.row_bytes.checked_mul(height as usize) != Some(raw.len())
            || dispatch.transfer_bytes != expected_transfer_bytes
            || dispatch.word_count as u64 * 4 != dispatch.transfer_bytes
            || dispatch.groups_x > limits.max_compute_workgroups_per_dimension
            || dispatch.groups_y > limits.max_compute_workgroups_per_dimension
            || dispatch.transfer_bytes > buffer_capacity
            || dispatch.transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || dispatch.transfer_bytes > limits.max_buffer_size
        {
            return Err(PilError::InternalError(
                "GPU native Flip does not match its checked row layout".into(),
            ));
        }

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_native_flip_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            limits.min_uniform_buffer_offset_alignment as usize,
        );
        self.upload_native_byte_flip_rows(image, dispatch, padded_row_bytes, buffers)?;

        let parameters = [
            height,
            dispatch.words_per_row,
            dispatch.word_count,
            dispatch.groups_x,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_flip_native_byte_rows",
            "flip_native_byte_rows.wgsl",
            include_str!("../../shaders/flip_native_byte_rows.wgsl"),
        )?;
        let bind_group =
            self.native_byte_flip_bind_group(&cached, buffers, dispatch.transfer_bytes)?;
        let result = self.submit_native_byte_flip(
            buffers,
            NativeByteFlipSubmission {
                cached: &cached,
                bind_group: &bind_group,
                width,
                height,
                channels,
                dispatch,
                dimensions,
                padded_row_bytes,
            },
        )?;
        Ok(crate::image::preserve_mode(image, result))
    }

    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn try_execute_native_flip(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        limits: &wgpu::Limits,
        capacity: u32,
    ) -> Result<NativeGeometryDispatchOutcome, PilError> {
        let Some(format) = gpu_native_flip_format(ops, image, mode) else {
            return Ok(NativeGeometryDispatchOutcome::NotApplicable);
        };
        match format {
            NativeFlipFormat::Mode1
            | NativeFlipFormat::L
            | NativeFlipFormat::Palette
            | NativeFlipFormat::LA
            | NativeFlipFormat::PA => self.execute_native_byte_flip_format(image, format, limits),
            NativeFlipFormat::Rgb => self.execute_native_rgb_flip(image, limits),
            NativeFlipFormat::Rgba => self.execute_native_rgba_flip(image, limits, capacity),
        }
    }

    #[cfg(target_endian = "little")]
    fn execute_native_byte_flip_format(
        &self,
        image: &DynamicImage,
        format: NativeFlipFormat,
        limits: &wgpu::Limits,
    ) -> Result<NativeGeometryDispatchOutcome, PilError> {
        let (channels, limit_reason) = match format {
            NativeFlipFormat::Mode1 => (1, "native byte Flip exceeds adapter or shader limits"),
            NativeFlipFormat::L => (1, "native L Flip exceeds adapter or shader limits"),
            NativeFlipFormat::Palette => (
                1,
                "native palette-index Flip exceeds adapter or shader limits",
            ),
            NativeFlipFormat::LA => (2, "native LA Flip exceeds adapter or shader limits"),
            NativeFlipFormat::PA => (2, "native PA Flip exceeds adapter or shader limits"),
            NativeFlipFormat::Rgb | NativeFlipFormat::Rgba => {
                return Err(PilError::InternalError(
                    "native byte Flip received a multiword layout".into(),
                ));
            }
        };
        let Some(dispatch) = plan_native_byte_flip_dispatch(
            image.width(),
            image.height(),
            channels,
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
            GPU_BUFFER_CAPACITY,
        ) else {
            return Ok(NativeGeometryDispatchOutcome::ExceedsLimits(limit_reason));
        };
        let mut buffers = self.acquire_buffers(dispatch.word_count)?;
        let result = self.execute_native_byte_flip(image, format, dispatch, &mut buffers)?;
        self.recycle_buffers(buffers);
        Ok(NativeGeometryDispatchOutcome::Executed(result))
    }

    #[cfg(target_endian = "little")]
    fn execute_native_rgb_flip(
        &self,
        image: &DynamicImage,
        limits: &wgpu::Limits,
    ) -> Result<NativeGeometryDispatchOutcome, PilError> {
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Err(PilError::InternalError(
                "native RGB Flip admission did not match RGB storage".into(),
            ));
        };
        let Some(layout) = packed_rgb_transpose_layout(
            rgb.width(),
            rgb.height(),
            limits.max_compute_workgroups_per_dimension,
        ) else {
            return Ok(NativeGeometryDispatchOutcome::ExceedsLimits(
                "native RGB Flip exceeds adapter or shader limits",
            ));
        };
        let words = u32::try_from(layout.transfer_bytes / 4)
            .map_err(|_| PilError::ValueError("GPU packed RGB Flip is too large".into()))?;
        let mut buffers = self.acquire_buffers(words)?;
        let geometry_as_transpose = PipelineOp::Transpose {
            method: TransposeMethod::FlipTopBottom,
        };
        let result =
            self.execute_packed_rgb_transpose(&geometry_as_transpose, rgb, &layout, &mut buffers)?;
        self.recycle_buffers(buffers);
        Ok(NativeGeometryDispatchOutcome::Executed(result))
    }

    #[cfg(target_endian = "little")]
    fn execute_native_rgba_flip(
        &self,
        image: &DynamicImage,
        limits: &wgpu::Limits,
        capacity: u32,
    ) -> Result<NativeGeometryDispatchOutcome, PilError> {
        let DynamicImage::ImageRgba8(rgba) = image else {
            return Err(PilError::InternalError(
                "native RGBA Flip admission did not match RGBA storage".into(),
            ));
        };
        let dimensions = CheckedDims::new(image.width(), image.height(), 4)?;
        let transfer_bytes = u64::try_from(dimensions.total_bytes())
            .map_err(|_| PilError::ValueError("GPU native RGBA Flip is too large".into()))?;
        let workgroup_count = u64::from(capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native RGBA Flip is too large".into()))?;
        let groups_x = image.width().div_ceil(16);
        let groups_y = image.height().div_ceil(16);
        if transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || transfer_bytes > limits.max_buffer_size
            || transfer_bytes > workgroup_count
            || groups_x > limits.max_compute_workgroups_per_dimension
            || groups_y > limits.max_compute_workgroups_per_dimension
        {
            return Ok(NativeGeometryDispatchOutcome::ExceedsLimits(
                "native RGBA Flip exceeds adapter or shader limits",
            ));
        }
        let mut buffers = self.acquire_buffers(capacity)?;
        let geometry_as_transpose = PipelineOp::Transpose {
            method: TransposeMethod::FlipTopBottom,
        };
        let result = self.execute_rgba_transpose_with_upload(
            &geometry_as_transpose,
            rgba,
            &mut buffers,
            self.direct_primary_readback,
        )?;
        self.recycle_buffers(buffers);
        Ok(NativeGeometryDispatchOutcome::Executed(result))
    }
}
