use super::*;

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compute::pool_gpu) struct NativeRgbPutAlphaDispatch {
    pub(in crate::compute::pool_gpu) input_bytes: usize,
    pub(in crate::compute::pool_gpu) input_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) output_bytes: usize,
    pub(in crate::compute::pool_gpu) output_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) groups_x: u32,
    pub(in crate::compute::pool_gpu) groups_y: u32,
}

/// Plan native three-byte RGB input and four-byte RGBA output for scalar
/// PutAlpha. Shader byte offsets and the linear pixel index are u32, while
/// both storage bindings and each workgroup axis must fit the active adapter.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn plan_native_rgb_put_alpha_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbPutAlphaDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let input_bytes = pixels.checked_mul(3)?;
    let output_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) || input_bytes > u64::from(u32::MAX) {
        return None;
    }
    let input_transfer_bytes = input_bytes.checked_add(3)? & !3;
    let output_transfer_bytes = output_bytes.checked_add(3)? & !3;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if input_transfer_bytes > storage_limit
        || output_transfer_bytes > storage_limit
        || input_transfer_bytes > max_buffer_size
        || output_transfer_bytes > max_buffer_size
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbPutAlphaDispatch {
        input_bytes: usize::try_from(input_bytes).ok()?,
        input_transfer_bytes,
        output_bytes: usize::try_from(output_bytes).ok()?,
        output_transfer_bytes,
        groups_x,
        groups_y,
    })
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compute::pool_gpu) struct NativeRgbPutAlphaDataDispatch {
    pub(in crate::compute::pool_gpu) rgb_bytes: usize,
    pub(in crate::compute::pool_gpu) rgb_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) mask_bytes: usize,
    pub(in crate::compute::pool_gpu) mask_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) output_bytes: usize,
    pub(in crate::compute::pool_gpu) output_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) groups_x: u32,
    pub(in crate::compute::pool_gpu) groups_y: u32,
}

/// Plan native RGB and L-mask uploads for the required RGBA PutAlphaData
/// result. The shader gathers three packed source bytes per pixel and uses a
/// separately packed one-byte mask, so each binding has its own checked span.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn plan_native_rgb_put_alpha_data_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbPutAlphaDataDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let rgb_bytes = pixels.checked_mul(3)?;
    let output_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) || rgb_bytes > u64::from(u32::MAX) {
        return None;
    }
    let padded = |bytes: u64| bytes.checked_add(3).map(|bytes| bytes & !3);
    let rgb_transfer_bytes = padded(rgb_bytes)?;
    let mask_transfer_bytes = padded(pixels)?;
    let output_transfer_bytes = padded(output_bytes)?;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if [
        rgb_transfer_bytes,
        mask_transfer_bytes,
        output_transfer_bytes,
    ]
    .into_iter()
    .any(|bytes| bytes > storage_limit || bytes > max_buffer_size)
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbPutAlphaDataDispatch {
        rgb_bytes: usize::try_from(rgb_bytes).ok()?,
        rgb_transfer_bytes,
        mask_bytes: usize::try_from(pixels).ok()?,
        mask_transfer_bytes,
        output_bytes: usize::try_from(output_bytes).ok()?,
        output_transfer_bytes,
        groups_x,
        groups_y,
    })
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compute::pool_gpu) struct NativeRgbaPutAlphaDataDispatch {
    pub(in crate::compute::pool_gpu) rgba_bytes: usize,
    pub(in crate::compute::pool_gpu) rgba_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) mask_bytes: usize,
    pub(in crate::compute::pool_gpu) mask_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) output_transfer_bytes: u64,
    pub(in crate::compute::pool_gpu) groups_x: u32,
    pub(in crate::compute::pool_gpu) groups_y: u32,
}

/// Plan native four-byte RGBA and one-byte L-mask uploads for PutAlphaData.
/// Pixel and mask indexing use u32 in WGSL, and each binding must fit the
/// active adapter's storage-buffer and allocation limits.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn plan_native_rgba_put_alpha_data_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> Option<NativeRgbaPutAlphaDataDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let rgba_bytes = pixels.checked_mul(4)?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let padded = |bytes: u64| bytes.checked_add(3).map(|bytes| bytes & !3);
    let rgba_transfer_bytes = padded(rgba_bytes)?;
    let mask_transfer_bytes = padded(pixels)?;
    let output_transfer_bytes = rgba_transfer_bytes;
    let storage_limit = u64::from(max_storage_buffer_binding_size);
    if [
        rgba_transfer_bytes,
        mask_transfer_bytes,
        output_transfer_bytes,
    ]
    .into_iter()
    .any(|bytes| bytes > storage_limit || bytes > max_buffer_size)
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbaPutAlphaDataDispatch {
        rgba_bytes: usize::try_from(rgba_bytes).ok()?,
        rgba_transfer_bytes,
        mask_bytes: usize::try_from(pixels).ok()?,
        mask_transfer_bytes,
        output_transfer_bytes,
        groups_x,
        groups_y,
    })
}

impl GpuInner {
    /// Add scalar alpha to native RGB samples and write the required RGBA
    /// result without first expanding the host upload to four bytes per pixel.
    #[cfg(target_endian = "little")]
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

    /// Add a mask-derived alpha to native RGB samples while writing the
    /// required RGBA result directly, without expanding either upload.
    #[cfg(target_endian = "little")]
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

    /// Replace the alpha byte in native RGBA pixels from a packed L mask.
    /// Both inputs stay in their original host layout; the shader writes the
    /// required RGBA output without constructing a widened mask image.
    #[cfg(target_endian = "little")]
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
