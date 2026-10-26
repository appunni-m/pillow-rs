use super::*;

#[cfg(target_endian = "little")]
struct NativeCompositeInput<'a> {
    image: &'a DynamicImage,
    other: &'a DynamicImage,
    mask: &'a DynamicImage,
    channels: u8,
    mask_channels: u8,
    mask_channel: u8,
}

#[cfg(target_endian = "little")]
struct NativeCompositePlan {
    width: u32,
    height: u32,
    channels: u8,
    mask_channel: u8,
    output_length: usize,
    input_transfer: u64,
    other_transfer: u64,
    mask_transfer: u64,
    output_transfer: u64,
    word_count: u32,
    workgroups: u32,
    groups_x: u32,
    groups_y: u32,
}

#[cfg(target_endian = "little")]
#[inline(always)]
fn native_composite_transfer_size(length: usize) -> Result<u64, PilError> {
    let length = u64::try_from(length)
        .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
    Ok(length
        .checked_add(3)
        .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?
        & !3)
}

impl GpuInner {
    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn plan_native_composite(
        &self,
        input: &NativeCompositeInput<'_>,
        buffers: &BufferPool,
    ) -> Result<NativeCompositePlan, PilError> {
        let (width, height) = input.image.dimensions();
        let dimensions = CheckedDims::new(width, height, input.channels)?;
        let pixels = dimensions.total_pixels();
        let output_length = dimensions.total_bytes();
        let mask_length = pixels
            .checked_mul(usize::from(input.mask_channels))
            .ok_or_else(|| PilError::ValueError("GPU native composite is too large".into()))?;
        if input.image.as_bytes().len() != output_length
            || input.other.dimensions() != (width, height)
            || input.other.as_bytes().len() != output_length
            || input.mask.dimensions() != (width, height)
            || input.mask.as_bytes().len() != mask_length
        {
            return Err(PilError::InternalError(
                "GPU native composite layout mismatch".into(),
            ));
        }

        let input_transfer = native_composite_transfer_size(output_length)?;
        let output_transfer = input_transfer;
        let other_transfer = input_transfer;
        let mask_transfer = native_composite_transfer_size(mask_length)?;
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

        Ok(NativeCompositePlan {
            width,
            height,
            channels: input.channels,
            mask_channel: input.mask_channel,
            output_length,
            input_transfer,
            other_transfer,
            mask_transfer,
            output_transfer,
            word_count,
            workgroups,
            groups_x,
            groups_y,
        })
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn prepare_native_composite_buffers(
        &self,
        input: &NativeCompositeInput<'_>,
        plan: &NativeCompositePlan,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        buffers.img2_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            plan.other_transfer as usize,
            4,
        );
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_composite_native_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            plan.mask_transfer as usize,
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
        parameters[0] = u32::from(input.channels);
        parameters[1] = u32::from(input.mask_channels);
        parameters[2] = u32::from(plan.mask_channel);
        parameters[3] = u32::try_from(plan.output_length)
            .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
        parameters[4] = plan.word_count;
        parameters[5] = plan.groups_x;
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
        write_bytes(&buffers.buf_a, input.image.as_bytes(), plan.input_transfer)?;
        write_bytes(
            &buffers.img2_arena.buffer,
            input.other.as_bytes(),
            plan.other_transfer,
        )?;
        write_bytes(
            &buffers.img3_arena.buffer,
            input.mask.as_bytes(),
            plan.mask_transfer,
        )?;
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        Ok(())
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn native_composite_bind_group(
        &self,
        cached: &CachedPipeline,
        plan: &NativeCompositePlan,
        buffers: &BufferPool,
    ) -> Result<wgpu::BindGroup, PilError> {
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_composite"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: plan.input_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img2_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: plan.other_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: plan.mask_transfer,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: plan.output_transfer,
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
        }))
    }

    #[cfg(target_endian = "little")]
    #[inline(always)]
    fn submit_native_composite(
        &self,
        cached: &CachedPipeline,
        bind_group: &wgpu::BindGroup,
        plan: &NativeCompositePlan,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
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
            pass.set_bind_group(0, bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(plan.workgroups),
            );
            pass.dispatch_workgroups(plan.groups_x, plan.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, plan.output_transfer)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                plan.output_transfer,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native composite submission")?;
        let result = self.readback_with(
            plan.output_transfer,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != plan.output_transfer as usize {
                    return Err(PilError::InternalError(
                        "GPU native composite readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..plan.output_length].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(
                    plan.width,
                    plan.height,
                    bytes,
                    usize::from(plan.channels),
                )
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: plan.input_transfer + plan.other_transfer + plan.mask_transfer,
            auxiliary_bytes: plan.other_transfer + plan.mask_transfer,
            readback_bytes: plan.output_transfer,
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
        let input = NativeCompositeInput {
            image,
            other,
            mask,
            channels,
            mask_channels,
            mask_channel,
        };
        let plan = self.plan_native_composite(&input, buffers)?;
        self.prepare_native_composite_buffers(&input, &plan, buffers)?;
        let cached = self.resolve_pipeline(
            "__internal_composite_native",
            "composite_native.wgsl",
            include_str!("shaders/composite_native.wgsl"),
        )?;
        let bind_group = self.native_composite_bind_group(&cached, &plan, buffers)?;
        self.submit_native_composite(&cached, &bind_group, &plan, buffers)
    }
}
