use super::*;

impl GpuInner {
    /// Execute exact masked L/LA/PA/RGB/HSV/RGBA/CMYK Paste with compact buffers.
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_masked_byte_paste(
        &self,
        destination: &DynamicImage,
        source: &DynamicImage,
        mask: &DynamicImage,
        logical_mode: &str,
        offset: (i32, i32),
        dispatch: NativeMaskedBytePasteDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (width, height) = destination.dimensions();
        let (source_width, source_height) = source.dimensions();
        let (region_width, region_height) = mask.dimensions();
        let storage_matches = match (logical_mode, dispatch.bytes_per_pixel) {
            ("L", 1) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageLuma8(_),
                    DynamicImage::ImageLuma8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            ("LA", 2) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageLumaA8(_),
                    DynamicImage::ImageLumaA8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            ("PA", 2) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageLumaA8(_),
                    DynamicImage::ImageLumaA8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            ("RGB" | "HSV", 3) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageRgb8(_),
                    DynamicImage::ImageRgb8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            ("RGBA", 4) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageRgba8(_),
                    DynamicImage::ImageRgba8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            // The carrier is ImageRgba8, but CMYK's fourth byte is K and all
            // four channels must be blended as ordinary stored bytes.
            ("CMYK", 4) => matches!(
                (destination, source, mask),
                (
                    DynamicImage::ImageRgba8(_),
                    DynamicImage::ImageRgba8(_),
                    DynamicImage::ImageLuma8(_)
                )
            ),
            _ => false,
        };
        if !storage_matches {
            return Err(PilError::InternalError(
                "GPU native masked Paste storage variant mismatch".into(),
            ));
        }
        let destination_bytes = destination.as_bytes();
        let source_bytes = source.as_bytes();
        let mask_bytes = mask.as_bytes();
        let destination_dims = CheckedDims::new(width, height, dispatch.bytes_per_pixel)?;
        let source_dims = CheckedDims::new(source_width, source_height, dispatch.bytes_per_pixel)?;
        let mask_dims = CheckedDims::new(region_width, region_height, 1)?;
        let source_is_broadcast =
            (source_width, source_height) == (1, 1) && (region_width, region_height) != (1, 1);
        if dispatch.source_is_broadcast != source_is_broadcast
            || (!source_is_broadcast
                && (region_width, region_height) != (source_width, source_height))
            || destination_bytes.len() != dispatch.destination_bytes
            || source_bytes.len() != dispatch.source_bytes
            || mask_bytes.len() != dispatch.mask_bytes
            || destination_dims.total_bytes() != dispatch.destination_bytes
            || source_dims.total_bytes() != dispatch.source_bytes
            || mask_dims.total_bytes() != dispatch.mask_bytes
            || destination_dims.total_pixels() != dispatch.destination_pixels as usize
            || source_dims.total_pixels() != dispatch.source_pixels as usize
        {
            return Err(PilError::InternalError(
                "GPU native masked Paste layout mismatch".into(),
            ));
        }
        let buffer_capacity = u64::from(buffers.capacity)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU native masked Paste is too large".into()))?;
        let binding_limit = u64::from(self.device.limits().max_storage_buffer_binding_size);
        let max_buffer_size = self.device.limits().max_buffer_size;
        if dispatch.destination_transfer_bytes > buffer_capacity
            || [
                dispatch.destination_transfer_bytes,
                dispatch.input_transfer_bytes,
            ]
            .into_iter()
            .any(|size| size > binding_limit || size > max_buffer_size)
        {
            return Err(PilError::ValueError(
                "GPU native masked Paste exceeds adapter buffer limits".into(),
            ));
        }

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_paste_native_masked_l_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            48,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );

        let write_padded = |buffer: &wgpu::Buffer, bytes: &[u8], transfer_bytes: u64| {
            let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
                PilError::InternalError("GPU native masked Paste upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native masked Paste staging allocation failed".into(),
                    )
                })?;
            let mapped = view.as_mut();
            if mapped.len() != transfer_bytes as usize || bytes.len() > mapped.len() {
                return Err(PilError::InternalError(
                    "GPU native masked Paste upload length mismatch".into(),
                ));
            }
            mapped[..bytes.len()].copy_from_slice(bytes);
            mapped[bytes.len()..].fill(0);
            Ok::<(), PilError>(())
        };
        let auxiliary_offset = if self.direct_primary_readback {
            self.upload_masked_paste_inputs_mapped(
                destination_bytes,
                dispatch.destination_transfer_bytes,
                source_bytes,
                dispatch.source_transfer_bytes,
                mask_bytes,
                dispatch.mask_transfer_bytes,
                buffers,
            )?
        } else {
            write_padded(
                &buffers.buf_a,
                destination_bytes,
                dispatch.destination_transfer_bytes,
            )?;
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_paste_native_masked_source_mask",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                dispatch.input_transfer_bytes as usize,
                4,
            );
            let input_upload_size =
                NonZeroU64::new(dispatch.input_transfer_bytes).ok_or_else(|| {
                    PilError::InternalError("GPU native masked Paste upload is empty".into())
                })?;
            let mut input_upload = self
                .queue
                .write_buffer_with(&buffers.img2_arena.buffer, 0, input_upload_size)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native masked Paste staging allocation failed".into(),
                    )
                })?;
            let input_mapped = input_upload.as_mut();
            let source_transfer = dispatch.source_transfer_bytes as usize;
            let mask_transfer = dispatch.mask_transfer_bytes as usize;
            let mask_start = source_transfer;
            let mask_end = mask_start.checked_add(mask_transfer).ok_or_else(|| {
                PilError::InternalError("GPU native masked Paste input length overflow".into())
            })?;
            if input_mapped.len() != dispatch.input_transfer_bytes as usize
                || source_bytes.len() > source_transfer
                || mask_bytes.len() > mask_transfer
                || mask_end != input_mapped.len()
            {
                return Err(PilError::InternalError(
                    "GPU native masked Paste upload length mismatch".into(),
                ));
            }
            input_mapped[..source_bytes.len()].copy_from_slice(source_bytes);
            input_mapped[source_bytes.len()..mask_start].fill(0);
            input_mapped[mask_start..mask_start + mask_bytes.len()].copy_from_slice(mask_bytes);
            input_mapped[mask_start + mask_bytes.len()..mask_end].fill(0);
            // QueueWriteBufferView submits the upload when dropped, before the
            // compute command buffer below.
            drop(input_upload);
            0
        };
        let destination_input = if self.direct_primary_readback {
            &buffers
                .mapped_native_input
                .as_ref()
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native masked Paste mapped input was not retained".into(),
                    )
                })?
                .buffer
        } else {
            &buffers.buf_a
        };
        let auxiliary_input = if self.direct_primary_readback {
            &buffers
                .mapped_native_input
                .as_ref()
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native masked Paste mapped input was not retained".into(),
                    )
                })?
                .buffer
        } else {
            &buffers.img2_arena.buffer
        };

        let parameters = [
            width,
            height,
            region_width,
            region_height,
            dispatch.destination_pixels,
            dispatch.word_count,
            offset.0 as u32,
            offset.1 as u32,
            u32::try_from(dispatch.source_transfer_bytes / 4).map_err(|_| {
                PilError::ValueError("GPU native masked Paste mask offset is too large".into())
            })?,
            dispatch.workgroups_x,
            if dispatch.source_is_broadcast { 1 } else { 0 },
            u32::from(
                matches!(logical_mode, "RGB" | "HSV")
                    && offset == (0, 0)
                    && region_width == width
                    && region_height == height,
            ),
        ];
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let (pipeline_variant, shader_file, shader_source) =
            gpu_native_masked_byte_paste_shader(logical_mode, dispatch.bytes_per_pixel)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native masked Paste has an unsupported logical mode".into(),
                    )
                })?;
        let cached = self.resolve_pipeline(pipeline_variant, shader_file, shader_source)?;
        let label = match logical_mode {
            "L" => "gpu_native_masked_l_paste",
            "LA" => "gpu_native_masked_la_paste",
            "PA" => "gpu_native_masked_pa_paste",
            "RGB" | "HSV" => "gpu_native_masked_rgb_paste",
            "RGBA" => "gpu_native_masked_rgba_paste",
            "CMYK" => "gpu_native_masked_cmyk_paste",
            _ => unreachable!("storage variant validated above"),
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        destination_input,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.destination_transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        auxiliary_input,
                        Some(BufferRange {
                            offset: auxiliary_offset,
                            size: dispatch.input_transfer_bytes,
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
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(dispatch.workgroups),
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
        let result = self.readback_with(
            dispatch.destination_transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.destination_transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native masked Paste readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..dispatch.destination_bytes].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(
                    width,
                    height,
                    bytes,
                    usize::from(dispatch.bytes_per_pixel),
                )
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.destination_transfer_bytes
                + dispatch.source_transfer_bytes
                + dispatch.mask_transfer_bytes,
            auxiliary_bytes: dispatch.source_transfer_bytes + dispatch.mask_transfer_bytes,
            readback_bytes: dispatch.destination_transfer_bytes,
            parameter_bytes: 48,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 1
                + 2 * u64::from(!self.direct_primary_readback)
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
}
