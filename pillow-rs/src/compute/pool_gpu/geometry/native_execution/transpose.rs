use super::{
    BufferPool, CachedPipeline, CheckedDims, DynamicImage, GpuInner, NativeGeometryDispatchOutcome,
    NativeTransposeInput, PackedRgbTransposeLayout, PilError, PipelineOp,
    PipelineResourceTelemetry, ReadbackTarget, RgbImage, RgbaImage, TransposeMethod,
    mapped_rgba_transpose_is_supported, packed_rgb_transpose_layout,
    packed_rgb_transpose_tiled_dispatch, ranged_binding, registry, transpose_output_dimensions,
};
use crate::compute::pool_gpu::BufferRange;

#[cfg(target_endian = "little")]
struct NativeTransposeUpload {
    width: u32,
    height: u32,
    mode: u32,
    transfer_bytes: u64,
    mapped_input: bool,
}

impl GpuInner {
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_packed_rgb_transpose(
        &self,
        op: &PipelineOp,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        self.execute_packed_rgb_transpose_with_upload(
            op,
            image,
            layout,
            buffers,
            self.direct_primary_readback,
        )
    }

    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_packed_rgb_transpose_with_upload(
        &self,
        op: &PipelineOp,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
        buffers: &mut BufferPool,
        mapped_input: bool,
    ) -> Result<DynamicImage, PilError> {
        self.execute_native_transpose_with_upload(
            op,
            NativeTransposeInput::Rgb(image, layout),
            buffers,
            mapped_input,
        )
    }

    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_rgba_transpose_with_upload(
        &self,
        op: &PipelineOp,
        image: &RgbaImage,
        buffers: &mut BufferPool,
        mapped_input: bool,
    ) -> Result<DynamicImage, PilError> {
        self.execute_native_transpose_with_upload(
            op,
            NativeTransposeInput::Rgba(image),
            buffers,
            mapped_input,
        )
    }

    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn try_execute_native_transpose_paths(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        limits: &wgpu::Limits,
        capacity: u32,
    ) -> Result<NativeGeometryDispatchOutcome, PilError> {
        if let Some(result) = self.try_execute_native_rgb_mirror(ops, image, mode, limits)? {
            return Ok(NativeGeometryDispatchOutcome::Executed(result));
        }
        if let Some(result) =
            self.try_execute_native_rgb_transpose(ops, image, mode, limits, capacity)?
        {
            return Ok(NativeGeometryDispatchOutcome::Executed(result));
        }
        if let Some(result) = self.try_execute_mapped_rgba_transpose(ops, image, mode, capacity)? {
            return Ok(NativeGeometryDispatchOutcome::Executed(result));
        }
        Ok(NativeGeometryDispatchOutcome::NotApplicable)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_rgb_mirror(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if !matches!(mode, None | Some("RGB")) || !matches!(ops, [PipelineOp::Mirror]) {
            return Ok(None);
        }
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Ok(None);
        };
        let Some(layout) = packed_rgb_transpose_layout(
            rgb.width(),
            rgb.height(),
            limits.max_compute_workgroups_per_dimension,
        ) else {
            return Ok(None);
        };
        // Mirror is the horizontal RGB transpose method. Keep its packed
        // three-byte input and output instead of expanding RGB to RGBA.
        let geometry_as_transpose = PipelineOp::Transpose {
            method: TransposeMethod::FlipLeftRight,
        };
        let words = u32::try_from(layout.transfer_bytes / 4)
            .map_err(|_| PilError::ValueError("GPU packed RGB mirror is too large".into()))?;
        let mut buffers = self.acquire_buffers(words)?;
        let result =
            self.execute_packed_rgb_transpose(&geometry_as_transpose, rgb, &layout, &mut buffers)?;
        self.recycle_buffers(buffers);
        Ok(Some(result))
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_rgb_transpose(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        limits: &wgpu::Limits,
        capacity: u32,
    ) -> Result<Option<DynamicImage>, PilError> {
        if !matches!(mode, None | Some("RGB" | "HSV")) {
            return Ok(None);
        }
        let ([op @ PipelineOp::Transpose { .. }], DynamicImage::ImageRgb8(rgb)) = (ops, image)
        else {
            return Ok(None);
        };
        let Some(layout) = packed_rgb_transpose_layout(
            rgb.width(),
            rgb.height(),
            limits.max_compute_workgroups_per_dimension,
        ) else {
            return Ok(None);
        };
        // Fusion and ordinary preflight have already validated the full
        // batch. RGB and HSV share physical RGB8 triplets here; the singleton
        // transpose only relocates bytes, while mixed operations keep the
        // ordinary packed RGBA path.
        let mut buffers = self.acquire_buffers(capacity)?;
        let result = self.execute_packed_rgb_transpose(op, rgb, &layout, &mut buffers)?;
        self.recycle_buffers(buffers);
        Ok(Some(result))
    }

    #[cfg(target_endian = "little")]
    fn try_execute_mapped_rgba_transpose(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        capacity: u32,
    ) -> Result<Option<DynamicImage>, PilError> {
        if !mapped_rgba_transpose_is_supported(ops, image, mode, self.direct_primary_readback) {
            return Ok(None);
        }
        let ([op], DynamicImage::ImageRgba8(rgba)) = (ops, image) else {
            return Ok(None);
        };
        // Logical mode matters: RGBX, premultiplied samples, and packed I/F
        // storage also use four-byte containers, but keep their mode-specific
        // execution paths.
        let mut buffers = self.acquire_buffers(capacity)?;
        let result = self.execute_rgba_transpose_with_upload(op, rgba, &mut buffers, true)?;
        self.recycle_buffers(buffers);
        Ok(Some(result))
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn resolve_native_transpose_pipeline(
        &self,
        input: &NativeTransposeInput<'_>,
        dimensions: CheckedDims,
        method: &TransposeMethod,
        width: u32,
        height: u32,
    ) -> Result<(std::sync::Arc<CachedPipeline>, u32, u32, u32, u64), PilError> {
        match input {
            NativeTransposeInput::Rgba(_) => Ok((
                self.resolve_pipeline(
                    "Transpose",
                    "transpose.wgsl",
                    include_str!("../../shaders/transpose.wgsl"),
                )?,
                width.div_ceil(16),
                height.div_ceil(16),
                3,
                dimensions.total_bytes() as u64,
            )),
            NativeTransposeInput::Rgb(_, layout) => {
                if dimensions.total_bytes() != layout.native_bytes {
                    return Err(PilError::InternalError(
                        "GPU packed RGB output differs from its checked transfer size".into(),
                    ));
                }
                let (cached, groups_x, groups_y) = if let Some((groups_x, groups_y)) =
                    packed_rgb_transpose_tiled_dispatch(
                        method,
                        width,
                        height,
                        self.device.limits().max_compute_workgroups_per_dimension,
                    ) {
                    (
                        self.resolve_pipeline(
                            "__internal_transpose_rgb_tiled",
                            "transpose_rgb_tiled.wgsl",
                            include_str!("../../shaders/transpose_rgb_tiled.wgsl"),
                        )?,
                        groups_x,
                        groups_y,
                    )
                } else {
                    (
                        self.resolve_pipeline(
                            "__internal_transpose_rgb",
                            "transpose_rgb.wgsl",
                            include_str!("../../shaders/transpose_rgb.wgsl"),
                        )?,
                        layout.workgroups,
                        1,
                    )
                };
                Ok((cached, groups_x, groups_y, 2, layout.transfer_bytes))
            }
        }
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn validate_native_transpose_transfer(
        &self,
        buffers: &BufferPool,
        dimensions: CheckedDims,
        groups: (u32, u32),
        transfer_bytes: u64,
    ) -> Result<(), PilError> {
        let (groups_x, groups_y) = groups;
        let limit = self.device.limits().max_compute_workgroups_per_dimension;
        if groups_x > limit || groups_y > limit {
            return Err(PilError::ValueError(
                "GPU native transpose exceeds adapter workgroup limit".into(),
            ));
        }
        let limits = self.device.limits();
        let buffer_capacity = u64::from(buffers.capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native transpose is too large".into()))?;
        let native_bytes = u64::try_from(dimensions.total_bytes())
            .map_err(|_| PilError::ValueError("GPU native transpose is too large".into()))?;
        if transfer_bytes < native_bytes
            || transfer_bytes > buffer_capacity
            || transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || transfer_bytes > limits.max_buffer_size
        {
            return Err(PilError::ValueError(
                "GPU native transpose exceeds adapter buffer limits".into(),
            ));
        }
        Ok(())
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn prepare_native_transpose_upload(
        &self,
        op: &PipelineOp,
        input: &NativeTransposeInput<'_>,
        dimensions: CheckedDims,
        upload: NativeTransposeUpload,
        buffers: &mut BufferPool,
    ) -> Result<usize, PilError> {
        let operation_params = registry::extract_params(op);
        let opcode = operation_params.first().copied().ok_or_else(|| {
            PilError::InternalError("GPU transpose is missing its method parameter".into())
        })?;
        let parameters = [upload.width, upload.height, upload.mode, 0, opcode, 0, 0, 0];
        let parameter_bytes = std::mem::size_of_val(&parameters);
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            parameter_bytes,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        if upload.mapped_input {
            match input {
                NativeTransposeInput::Rgb(image, layout) => {
                    self.upload_packed_rgb_mapped(image, layout, buffers)?
                }
                NativeTransposeInput::Rgba(_) => self.upload_native_bytes_mapped(
                    input.bytes(),
                    dimensions.total_bytes(),
                    upload.transfer_bytes,
                    buffers,
                )?,
            }
        } else {
            match input {
                NativeTransposeInput::Rgb(image, layout) => {
                    buffers.upload_packed_rgb(&self.queue, image, layout)?
                }
                NativeTransposeInput::Rgba(image) => buffers.upload_rgba(&self.queue, image)?,
            }
        }
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        Ok(parameter_bytes)
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn native_transpose_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buffer: &wgpu::Buffer,
        buffers: &BufferPool,
        transfer_bytes: u64,
        parameter_bytes: usize,
    ) -> Result<wgpu::BindGroup, PilError> {
        let image_range = Some(BufferRange {
            offset: 0,
            size: transfer_bytes,
        });
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_transpose"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(input_buffer, image_range)?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(&buffers.buf_b, image_range)?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: parameter_bytes as u64,
                        }),
                    )?,
                },
            ],
        }))
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn record_native_transpose_dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        cached: &CachedPipeline,
        bind_group: &wgpu::BindGroup,
        groups: (u32, u32),
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_native_transpose"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&cached.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(groups.0) * u64::from(groups.1),
        );
        pass.dispatch_workgroups(groups.0, groups.1, 1);
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn record_native_transpose_telemetry(
        &self,
        buffers: &BufferPool,
        transfer_bytes: u64,
        parameter_bytes: usize,
        mapped_input: bool,
        readback: &ReadbackTarget,
    ) {
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: transfer_bytes,
            readback_bytes: transfer_bytes,
            parameter_bytes: parameter_bytes as u64,
            retained_cache_bytes: buffers.retained_bytes(),
            // Host writes/readback remain visible in the byte counters.
            // Mapping input avoids only the staging-to-storage device copy.
            full_frame_copy_count: u64::from(!mapped_input)
                + u64::from(matches!(readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
    }

    #[cfg(target_endian = "little")]
    fn execute_native_transpose_with_upload(
        &self,
        op: &PipelineOp,
        input: NativeTransposeInput<'_>,
        buffers: &mut BufferPool,
        mapped_input: bool,
    ) -> Result<DynamicImage, PilError> {
        let PipelineOp::Transpose { method } = op else {
            return Err(PilError::InternalError(
                "GPU native dispatch requires one transpose".into(),
            ));
        };
        let (source_width, source_height) = input.dimensions();
        let (width, height) = transpose_output_dimensions(method, source_width, source_height);
        let dimensions = CheckedDims::new(width, height, input.channels())?;
        if dimensions.total_bytes() != input.bytes().len() {
            return Err(PilError::InternalError(
                "GPU native transpose output differs from its checked source size".into(),
            ));
        }
        let (cached, groups_x, groups_y, mode, transfer_bytes) =
            self.resolve_native_transpose_pipeline(&input, dimensions, method, width, height)?;
        self.validate_native_transpose_transfer(
            buffers,
            dimensions,
            (groups_x, groups_y),
            transfer_bytes,
        )?;
        let parameter_bytes = self.prepare_native_transpose_upload(
            op,
            &input,
            dimensions,
            NativeTransposeUpload {
                width,
                height,
                mode,
                transfer_bytes,
                mapped_input,
            },
            buffers,
        )?;
        let input_buffer = if mapped_input {
            &buffers
                .mapped_native_input
                .as_ref()
                .expect("successful mapped upload owns an input buffer")
                .buffer
        } else {
            &buffers.buf_a
        };
        let bind_group = self.native_transpose_bind_group(
            &cached,
            input_buffer,
            buffers,
            transfer_bytes,
            parameter_bytes,
        )?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_transpose"),
            });
        self.record_native_transpose_dispatch(
            &mut encoder,
            &cached,
            &bind_group,
            (groups_x, groups_y),
        );
        let readback = self.prepare_readback(&buffers.buf_b, transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, transfer_bytes);
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native transpose submission")?;
        let result =
            self.readback_with(transfer_bytes, readback.buffer(buffers, false), |mapped| {
                if mapped.len() != transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native transpose readback has an unexpected byte length".into(),
                    ));
                }
                // CheckedDims validated the native length before allocating.
                // Copy only image bytes; exclude any RGB alignment padding.
                let bytes = mapped[..dimensions.total_bytes()].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                match input {
                    NativeTransposeInput::Rgb(..) => {
                        RgbImage::from_raw(width, height, bytes).map(DynamicImage::ImageRgb8)
                    }
                    NativeTransposeInput::Rgba(_) => {
                        RgbaImage::from_raw(width, height, bytes).map(DynamicImage::ImageRgba8)
                    }
                }
                .ok_or_else(|| {
                    PilError::InternalError("bad native transpose readback shape".into())
                })
            })?;
        self.record_native_transpose_telemetry(
            buffers,
            transfer_bytes,
            parameter_bytes,
            mapped_input,
            &readback,
        );
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }
}
