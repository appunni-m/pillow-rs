use super::*;

impl GpuInner {
    /// Execute a full-size ImageChops composite using native byte buffers.
    /// The shader treats each packed word as four independent result bytes,
    /// so L/LA/RGB channels and an L/alpha mask stay compact from upload to
    /// readback instead of expanding every input to packed RGBA pixels.
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_composite(
        &self,
        image: &DynamicImage,
        other: &DynamicImage,
        mask: &DynamicImage,
        channels: u8,
        mask_channels: u8,
        mask_channel: u8,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (width, height) = image.dimensions();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let pixels = dimensions.total_pixels();
        let output_length = dimensions.total_bytes();
        let mask_length = pixels
            .checked_mul(usize::from(mask_channels))
            .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?;
        if image.as_bytes().len() != output_length
            || other.dimensions() != (width, height)
            || other.as_bytes().len() != output_length
            || mask.dimensions() != (width, height)
            || mask.as_bytes().len() != mask_length
        {
            return Err(PilError::InternalError(
                "GPU native composite layout mismatch".into(),
            ));
        }

        let transfer = |length: usize| -> Result<u64, PilError> {
            let length = u64::try_from(length)
                .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
            Ok(length
                .checked_add(3)
                .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?
                & !3)
        };
        let input_transfer = transfer(output_length)?;
        let output_transfer = input_transfer;
        let other_transfer = input_transfer;
        let mask_transfer = transfer(mask_length)?;
        if output_transfer > u64::from(buffers.capacity) * 4
            || mask_transfer > u64::from(self.device.limits().max_storage_buffer_binding_size)
        {
            return Err(PilError::ValueError(
                "GPU native composite exceeds adapter buffer limits".into(),
            ));
        }
        let word_count = u32::try_from(output_transfer / 4)
            .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
        let workgroups = word_count.div_ceil(64);
        let (groups_x, groups_y) = plan_native_composite_dispatch(
            word_count,
            self.device.limits().max_compute_workgroups_per_dimension,
        )
        .ok_or_else(|| {
            PilError::ValueError("GPU native composite exceeds adapter workgroup grid limit".into())
        })?;

        buffers.img2_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            other_transfer as usize,
            4,
        );
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mask_transfer as usize,
            4,
        );
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );

        let mut parameters = [0u32; 8];
        parameters[0] = u32::from(channels);
        parameters[1] = u32::from(mask_channels);
        parameters[2] = u32::from(mask_channel);
        parameters[3] = u32::try_from(output_length)
            .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
        parameters[4] = word_count;
        parameters[5] = groups_x;
        let write_bytes = |buffer: &wgpu::Buffer, bytes: &[u8], size: u64| {
            let size = NonZeroU64::new(size).ok_or_else(|| {
                PilError::InternalError("GPU native composite upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError("GPU native composite staging allocation failed".into())
                })?;
            let mapped = view.as_mut();
            mapped[..bytes.len()].copy_from_slice(bytes);
            mapped[bytes.len()..].fill(0);
            Ok::<(), PilError>(())
        };
        write_bytes(&buffers.buf_a, image.as_bytes(), input_transfer)?;
        write_bytes(&buffers.img2_arena.buffer, other.as_bytes(), other_transfer)?;
        write_bytes(&buffers.img3_arena.buffer, mask.as_bytes(), mask_transfer)?;
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_composite_native",
            "composite_native.wgsl",
            include_str!("shaders/composite_native.wgsl"),
        )?;

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_composite"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: input_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img2_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: other_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: mask_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: output_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 32,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_composite"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_composite"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(workgroups),
            );
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, output_transfer)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, output_transfer);
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native composite submission")?;
        let result =
            self.readback_with(output_transfer, readback.buffer(buffers, false), |mapped| {
                if mapped.len() != output_transfer as usize {
                    return Err(PilError::InternalError(
                        "GPU native composite readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..output_length].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, usize::from(channels))
            })?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: input_transfer + other_transfer + mask_transfer,
            auxiliary_bytes: other_transfer + mask_transfer,
            readback_bytes: output_transfer,
            parameter_bytes: 32,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 3 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
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
