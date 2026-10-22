use super::*;

impl GpuInner {
    pub(in crate::compute::pool_gpu) fn execute_native_rgb_crop(
        &self,
        source: &DynamicImage,
        left: u32,
        top: u32,
        dispatch: NativeRgbCropDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgb8(source) = source else {
            return Err(PilError::InternalError(
                "GPU native RGB Crop storage variant mismatch".into(),
            ));
        };
        let (source_width, source_height) = source.dimensions();
        let source_dims = CheckedDims::new(source_width, source_height, 3)?;
        let output_dims = CheckedDims::new(dispatch.output_width, dispatch.output_height, 3)?;
        let right = left.checked_add(dispatch.output_width).ok_or_else(|| {
            PilError::ValueError("GPU native RGB Crop coordinate overflow".into())
        })?;
        let bottom = top.checked_add(dispatch.output_height).ok_or_else(|| {
            PilError::ValueError("GPU native RGB Crop coordinate overflow".into())
        })?;
        if source_dims.total_bytes() != dispatch.source_bytes
            || source.as_raw().len() != dispatch.source_bytes
            || output_dims.total_bytes() != dispatch.output_bytes
            || left >= right
            || top >= bottom
            || right > source_width
            || bottom > source_height
        {
            return Err(PilError::InternalError(
                "GPU native RGB Crop layout mismatch".into(),
            ));
        }
        let buffer_capacity = u64::from(buffers.capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native RGB Crop is too large".into()))?;
        if dispatch.source_transfer_bytes > buffer_capacity
            || dispatch.output_transfer_bytes > buffer_capacity
            || [
                dispatch.source_transfer_bytes,
                dispatch.output_transfer_bytes,
            ]
            .into_iter()
            .any(|size| {
                size > u64::from(self.device.limits().max_storage_buffer_binding_size)
                    || size > self.device.limits().max_buffer_size
            })
        {
            return Err(PilError::ValueError(
                "GPU native RGB Crop exceeds adapter buffer limits".into(),
            ));
        }

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_crop_native_rgb_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            48,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        let upload_size = NonZeroU64::new(dispatch.source_transfer_bytes)
            .ok_or_else(|| PilError::InternalError("GPU native RGB Crop upload is empty".into()))?;
        let mut upload = self
            .queue
            .write_buffer_with(&buffers.buf_a, 0, upload_size)
            .ok_or_else(|| {
                PilError::InternalError("GPU native RGB Crop staging allocation failed".into())
            })?;
        let mapped = upload.as_mut();
        if mapped.len() != dispatch.source_transfer_bytes as usize
            || source.as_raw().len() > mapped.len()
        {
            return Err(PilError::InternalError(
                "GPU native RGB Crop upload length mismatch".into(),
            ));
        }
        mapped[..source.as_raw().len()].copy_from_slice(source.as_raw());
        mapped[source.as_raw().len()..].fill(0);
        drop(upload);

        let parameters = [
            source_width,
            source_height,
            left,
            top,
            dispatch.output_width,
            dispatch.output_height,
            u32::try_from(dispatch.source_transfer_bytes).map_err(|_| {
                PilError::ValueError("GPU native RGB Crop source buffer is too large".into())
            })?,
            dispatch.output_word_count,
            dispatch.output_pixel_groups,
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
            "__internal_crop_native_rgb",
            "crop_native_rgb.wgsl",
            include_str!("../shaders/crop_native_rgb.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_rgb_crop"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.source_transfer_bytes,
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
                            size: 48,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_rgb_crop"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_rgb_crop"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.workgroups),
            );
            pass.dispatch_workgroups(dispatch.workgroups, 1, 1);
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
        let result = self.readback_with(
            dispatch.output_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.output_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB Crop readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.output_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(
                    dispatch.output_width,
                    dispatch.output_height,
                    bytes,
                    3,
                )
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.source_transfer_bytes,
            readback_bytes: dispatch.output_transfer_bytes,
            parameter_bytes: 48,
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

    pub(in crate::compute::pool_gpu) fn execute_native_byte_paste(
        &self,
        destination: &DynamicImage,
        source: &DynamicImage,
        offset: (i32, i32),
        dispatch: NativeBytePasteDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (width, height) = destination.dimensions();
        let (source_width, source_height) = source.dimensions();
        let bytes_per_pixel = usize::from(dispatch.bytes_per_pixel);
        if !matches!(
            (dispatch.bytes_per_pixel, destination, source),
            (1, DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_))
                | (
                    2,
                    DynamicImage::ImageLumaA8(_),
                    DynamicImage::ImageLumaA8(_)
                )
                | (3, DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_))
        ) {
            return Err(PilError::InternalError(
                "GPU native Paste storage variant mismatch".into(),
            ));
        }
        let destination_dims = CheckedDims::new(width, height, dispatch.bytes_per_pixel)?;
        let source_dims = CheckedDims::new(source_width, source_height, dispatch.bytes_per_pixel)?;
        if destination_dims.total_bytes() != dispatch.destination_bytes
            || source_dims.total_bytes() != dispatch.source_bytes
            || destination.as_bytes().len() != dispatch.destination_bytes
            || source.as_bytes().len() != dispatch.source_bytes
        {
            return Err(PilError::InternalError(
                "GPU native Paste layout mismatch".into(),
            ));
        }
        let buffer_capacity = u64::from(buffers.capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native Paste is too large".into()))?;
        if dispatch.destination_transfer_bytes > buffer_capacity
            || dispatch.source_transfer_bytes
                > u64::from(self.device.limits().max_storage_buffer_binding_size)
            || dispatch.source_transfer_bytes > self.device.limits().max_buffer_size
        {
            return Err(PilError::ValueError(
                "GPU native Paste exceeds adapter buffer limits".into(),
            ));
        }

        buffers.img2_arena.ensure_capacity(
            &self.device,
            "gpu_paste_native_byte_source",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.source_transfer_bytes as usize,
            4,
        );
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_paste_native_byte_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            48,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );

        let write_padded = |buffer: &wgpu::Buffer, bytes: &[u8], transfer_bytes: u64| {
            let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
                PilError::InternalError("GPU native Paste upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError("GPU native Paste staging allocation failed".into())
                })?;
            let mapped = view.as_mut();
            if mapped.len() != transfer_bytes as usize || bytes.len() > mapped.len() {
                return Err(PilError::InternalError(
                    "GPU native Paste upload length mismatch".into(),
                ));
            }
            mapped[..bytes.len()].copy_from_slice(bytes);
            mapped[bytes.len()..].fill(0);
            Ok::<(), PilError>(())
        };
        write_padded(
            &buffers.buf_a,
            destination.as_bytes(),
            dispatch.destination_transfer_bytes,
        )?;
        write_padded(
            &buffers.img2_arena.buffer,
            source.as_bytes(),
            dispatch.source_transfer_bytes,
        )?;

        let parameters = [
            width,
            height,
            source_width,
            source_height,
            dispatch.destination_pixels,
            dispatch.word_count,
            u32::from(dispatch.bytes_per_pixel),
            0,
            offset.0 as u32,
            offset.1 as u32,
            0,
            0,
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_paste_native_bytes",
            "paste_native_bytes.wgsl",
            include_str!("../shaders/paste_native_bytes.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_byte_paste"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.destination_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img2_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.source_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.destination_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 48,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_byte_paste"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_byte_paste"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.workgroups),
            );
            pass.dispatch_workgroups(dispatch.workgroups, 1, 1);
        }
        let readback =
            self.prepare_readback(&buffers.buf_b, dispatch.destination_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.destination_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native Paste submission")?;
        let result = self.readback_with(
            dispatch.destination_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.destination_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native Paste readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.destination_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, bytes_per_pixel)
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.destination_transfer_bytes,
            auxiliary_bytes: dispatch.source_transfer_bytes,
            readback_bytes: dispatch.destination_transfer_bytes,
            parameter_bytes: 48,
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

    pub(in crate::compute::pool_gpu) fn execute_native_rgb_to_rgba_paste(
        &self,
        destination: &DynamicImage,
        source: &DynamicImage,
        offset: (i32, i32),
        dispatch: NativeRgbRgbaPasteDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (DynamicImage::ImageRgba8(destination), DynamicImage::ImageRgb8(source)) =
            (destination, source)
        else {
            return Err(PilError::InternalError(
                "GPU native RGB-to-RGBA Paste storage variant mismatch".into(),
            ));
        };
        let (width, height) = destination.dimensions();
        let (source_width, source_height) = source.dimensions();
        let destination_dims = CheckedDims::new(width, height, 4)?;
        let source_dims = CheckedDims::new(source_width, source_height, 3)?;
        if destination_dims.total_bytes() != dispatch.destination_bytes
            || source_dims.total_bytes() != dispatch.source_bytes
            || destination.as_raw().len() != dispatch.destination_bytes
            || source.as_raw().len() != dispatch.source_bytes
        {
            return Err(PilError::InternalError(
                "GPU native RGB-to-RGBA Paste layout mismatch".into(),
            ));
        }
        let buffer_capacity = u64::from(buffers.capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native Paste is too large".into()))?;
        let limits = self.device.limits();
        if dispatch.destination_transfer_bytes > buffer_capacity
            || dispatch.destination_transfer_bytes
                > u64::from(limits.max_storage_buffer_binding_size)
            || dispatch.source_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || dispatch.destination_transfer_bytes > limits.max_buffer_size
            || dispatch.source_transfer_bytes > limits.max_buffer_size
        {
            return Err(PilError::ValueError(
                "GPU native RGB-to-RGBA Paste exceeds adapter buffer limits".into(),
            ));
        }

        buffers.img2_arena.ensure_capacity(
            &self.device,
            "gpu_paste_native_rgb_to_rgba_source",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            dispatch.source_transfer_bytes as usize,
            4,
        );
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_paste_native_rgb_to_rgba_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            48,
            limits.min_uniform_buffer_offset_alignment as usize,
        );

        let write_padded = |buffer: &wgpu::Buffer, bytes: &[u8], transfer_bytes: u64| {
            let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
                PilError::InternalError("GPU native Paste upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError("GPU native Paste staging allocation failed".into())
                })?;
            let mapped = view.as_mut();
            if mapped.len() != transfer_bytes as usize || bytes.len() > mapped.len() {
                return Err(PilError::InternalError(
                    "GPU native Paste upload length mismatch".into(),
                ));
            }
            mapped[..bytes.len()].copy_from_slice(bytes);
            mapped[bytes.len()..].fill(0);
            Ok::<(), PilError>(())
        };
        write_padded(
            &buffers.buf_a,
            destination.as_raw(),
            dispatch.destination_transfer_bytes,
        )?;
        write_padded(
            &buffers.img2_arena.buffer,
            source.as_raw(),
            dispatch.source_transfer_bytes,
        )?;

        let parameters = [
            width,
            height,
            source_width,
            source_height,
            dispatch.destination_pixels,
            dispatch.workgroups_x,
            offset.0 as u32,
            offset.1 as u32,
            0,
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
            "__internal_paste_native_rgb_to_rgba",
            "paste_native_rgb_to_rgba.wgsl",
            include_str!("../shaders/paste_native_rgb_to_rgba.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_rgb_to_rgba_paste"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.destination_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.img2_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.source_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.destination_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        &buffers.params_arena.buffer,
                        Some(BufferRange {
                            offset: 0,
                            size: 48,
                        }),
                    )?,
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_rgb_to_rgba_paste"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_rgb_to_rgba_paste"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.workgroups_x) * u64::from(dispatch.workgroups_y),
            );
            pass.dispatch_workgroups(dispatch.workgroups_x, dispatch.workgroups_y, 1);
        }
        let readback =
            self.prepare_readback(&buffers.buf_b, dispatch.destination_transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.destination_transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native RGB-to-RGBA Paste submission")?;
        let result = self.readback_with(
            dispatch.destination_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.destination_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native RGB-to-RGBA Paste readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.destination_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::raster::RgbaImage::from_raw(width, height, bytes)
                    .map(DynamicImage::ImageRgba8)
                    .ok_or_else(|| {
                        PilError::InternalError(
                            "GPU native RGB-to-RGBA Paste output shape mismatch".into(),
                        )
                    })
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.input_transfer_bytes,
            auxiliary_bytes: dispatch.source_transfer_bytes,
            readback_bytes: dispatch.destination_transfer_bytes,
            parameter_bytes: 48,
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
