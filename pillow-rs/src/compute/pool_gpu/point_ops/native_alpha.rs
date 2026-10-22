use super::*;

impl GpuInner {
    pub(in crate::compute::pool_gpu) fn execute_native_rgb_put_alpha(
        &self,
        image: &DynamicImage,
        alpha: u8,
        dispatch: NativeRgbPutAlphaDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgb8(rgb) = image else {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlpha requires ImageRgb8 storage".into(),
            ));
        };
        let (width, height) = rgb.dimensions();
        let dimensions = CheckedDims::new(width, height, 3)?;
        if dimensions.total_bytes() != dispatch.input_bytes
            || rgb.as_raw().len() != dispatch.input_bytes
            || CheckedDims::new(width, height, 4)?.total_bytes() != dispatch.output_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlpha layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 3, rgb.as_raw())?;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::from(alpha),
            u32::try_from(dispatch.input_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlpha input is too large".into())
            })?,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaNativeRGB",
            "put_alpha_native_rgb.wgsl",
            include_str!("../shaders/put_alpha_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_native_rgb"),
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
                            size: dispatch.output_transfer_bytes,
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
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_native_rgb"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_native_rgb"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGB PutAlpha submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB PutAlpha readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.output_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 1 + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(result)
    }

    pub(in crate::compute::pool_gpu) fn execute_native_rgb_put_alpha_data(
        &self,
        image: &DynamicImage,
        mask: &DynamicImage,
        dispatch: NativeRgbPutAlphaDataDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (DynamicImage::ImageRgb8(rgb), DynamicImage::ImageLuma8(alpha)) = (image, mask) else {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlphaData requires RGB and L storage".into(),
            ));
        };
        let (width, height) = rgb.dimensions();
        let dimensions = CheckedDims::new(width, height, 3)?;
        let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
        if rgb.dimensions() != alpha.dimensions()
            || dimensions.total_bytes() != dispatch.rgb_bytes
            || rgb.as_raw().len() != dispatch.rgb_bytes
            || alpha.as_raw().len() != dispatch.mask_bytes
            || pixel_count != dispatch.mask_bytes
            || CheckedDims::new(width, height, 4)?.total_bytes() != dispatch.output_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGB PutAlphaData layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 3, rgb.as_raw())?;
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgb_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.mask_transfer_bytes as usize,
            4,
        );
        let mask_size = NonZeroU64::new(dispatch.mask_transfer_bytes).ok_or_else(|| {
            PilError::ValueError("GPU native RGB PutAlphaData mask is empty".into())
        })?;
        let mut mask_upload = self
            .queue
            .write_buffer_with(&buffers.img3_arena.buffer, 0, mask_size)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native RGB PutAlphaData mask staging allocation failed".into(),
                )
            })?;
        mask_upload.fill(0);
        mask_upload[..dispatch.mask_bytes].copy_from_slice(alpha.as_raw());
        drop(mask_upload);

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::try_from(dispatch.rgb_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData input is too large".into())
            })?,
            u32::try_from(dispatch.mask_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData mask is too large".into())
            })?,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaDataNativeRGB",
            "put_alpha_data_native_rgb.wgsl",
            include_str!("../shaders/put_alpha_data_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_data_native_rgb"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.rgb_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.mask_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_data_native_rgb"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_data_native_rgb"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGB PutAlphaData submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB PutAlphaData readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.output_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.rgb_transfer_bytes,
            auxiliary_bytes: dispatch.mask_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
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

    pub(in crate::compute::pool_gpu) fn execute_native_rgba_put_alpha_data(
        &self,
        image: &DynamicImage,
        mask: &DynamicImage,
        dispatch: NativeRgbaPutAlphaDataDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (DynamicImage::ImageRgba8(rgba), DynamicImage::ImageLuma8(alpha)) = (image, mask)
        else {
            return Err(PilError::InternalError(
                "GPU native RGBA PutAlphaData requires RGBA and L storage".into(),
            ));
        };
        let (width, height) = rgba.dimensions();
        let dimensions = CheckedDims::new(width, height, 4)?;
        let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
        if rgba.dimensions() != alpha.dimensions()
            || dimensions.total_bytes() != dispatch.rgba_bytes
            || rgba.as_raw().len() != dispatch.rgba_bytes
            || alpha.as_raw().len() != dispatch.mask_bytes
            || pixel_count != dispatch.mask_bytes
            || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU native RGBA PutAlphaData layout mismatch".into(),
            ));
        }

        buffers.upload_native_channel_bytes(&self.queue, width, height, 4, rgba.as_raw())?;
        buffers.img3_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgba_mask",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.mask_transfer_bytes as usize,
            4,
        );
        let mask_size = NonZeroU64::new(dispatch.mask_transfer_bytes).ok_or_else(|| {
            PilError::ValueError("GPU native RGBA PutAlphaData mask is empty".into())
        })?;
        let mut mask_upload = self
            .queue
            .write_buffer_with(&buffers.img3_arena.buffer, 0, mask_size)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native RGBA PutAlphaData mask staging allocation failed".into(),
                )
            })?;
        mask_upload.fill(0);
        mask_upload[..dispatch.mask_bytes].copy_from_slice(alpha.as_raw());
        drop(mask_upload);

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_put_alpha_data_native_rgba_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            16,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let parameters = [
            width,
            height,
            u32::try_from(dispatch.mask_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGBA PutAlphaData mask is too large".into())
            })?,
            0,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "PutAlphaDataNativeRGBA",
            "put_alpha_data_native_rgba.wgsl",
            include_str!("../shaders/put_alpha_data_native_rgba.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_put_alpha_data_native_rgba"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.rgba_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img3_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.mask_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.output_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 16,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_put_alpha_data_native_rgba"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_put_alpha_data_native_rgba"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
            );
            pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
        }
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.output_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGBA PutAlphaData submission")?;
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGBA PutAlphaData readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.rgba_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, 4)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.rgba_transfer_bytes,
            auxiliary_bytes: dispatch.mask_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 16,
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
