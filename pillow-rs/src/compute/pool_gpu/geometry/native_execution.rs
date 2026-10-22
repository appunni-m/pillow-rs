use super::*;

impl GpuInner {
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
        let upload_size = NonZeroU64::new(dispatch.transfer_bytes)
            .ok_or_else(|| PilError::ValueError("GPU native Flip input is empty".into()))?;
        let mut upload = self
            .queue
            .write_buffer_with(&buffers.buf_a, 0, upload_size)
            .ok_or_else(|| {
                PilError::InternalError("GPU native Flip staging allocation failed".into())
            })?;
        upload.fill(0);
        if padded_row_bytes == dispatch.row_bytes {
            upload[..raw.len()].copy_from_slice(raw);
        } else {
            for row in 0..height as usize {
                let source_start = row * dispatch.row_bytes;
                let target_start = row * padded_row_bytes;
                upload[target_start..target_start + dispatch.row_bytes]
                    .copy_from_slice(&raw[source_start..source_start + dispatch.row_bytes]);
            }
        }
        drop(upload);

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
            include_str!("../shaders/flip_native_byte_rows.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_byte_flip"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        &buffers.buf_a,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.transfer_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        &buffers.buf_b,
                        Some(BufferRange {
                            offset: 0,
                            size: dispatch.transfer_bytes,
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
                label: Some("gpu_native_byte_flip"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_byte_flip"),
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
        let readback = self.prepare_readback(&buffers.buf_b, dispatch.transfer_bytes)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(
                &buffers.buf_b,
                0,
                &staging.buffer,
                0,
                dispatch.transfer_bytes,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU native Flip submission")?;
        let result = self.readback_with(
            dispatch.transfer_bytes,
            readback.buffer(buffers, false),
            |mapped| {
                if mapped.len() != dispatch.transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native Flip readback length mismatch".into(),
                    ));
                }
                let mut bytes = Vec::with_capacity(dimensions.total_bytes());
                for row in mapped.chunks_exact(padded_row_bytes).take(height as usize) {
                    bytes.extend_from_slice(&row[..dispatch.row_bytes]);
                }
                if bytes.len() != dimensions.total_bytes() {
                    return Err(PilError::InternalError(
                        "GPU native Flip omitted a source row".into(),
                    ));
                }
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, usize::from(channels))
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: dispatch.transfer_bytes,
            readback_bytes: dispatch.transfer_bytes,
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
        Ok(crate::image::preserve_mode(image, result))
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
        let (cached, groups_x, groups_y, mode, transfer_bytes) = match &input {
            NativeTransposeInput::Rgba(_) => (
                self.resolve_pipeline(
                    "Transpose",
                    "transpose.wgsl",
                    include_str!("../shaders/transpose.wgsl"),
                )?,
                width.div_ceil(16),
                height.div_ceil(16),
                3,
                dimensions.total_bytes() as u64,
            ),
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
                            include_str!("../shaders/transpose_rgb_tiled.wgsl"),
                        )?,
                        groups_x,
                        groups_y,
                    )
                } else {
                    (
                        self.resolve_pipeline(
                            "__internal_transpose_rgb",
                            "transpose_rgb.wgsl",
                            include_str!("../shaders/transpose_rgb.wgsl"),
                        )?,
                        layout.workgroups,
                        1,
                    )
                };
                (cached, groups_x, groups_y, 2, layout.transfer_bytes)
            }
        };
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
        let operation_params = registry::extract_params(op);
        let opcode = operation_params.first().copied().ok_or_else(|| {
            PilError::InternalError("GPU transpose is missing its method parameter".into())
        })?;
        let parameters = [width, height, mode, 0, opcode, 0, 0, 0];
        let parameter_bytes = std::mem::size_of_val(&parameters);
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            parameter_bytes,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        if mapped_input {
            match &input {
                NativeTransposeInput::Rgb(image, layout) => {
                    self.upload_packed_rgb_mapped(image, layout, buffers)?
                }
                NativeTransposeInput::Rgba(_) => self.upload_native_bytes_mapped(
                    input.bytes(),
                    dimensions.total_bytes(),
                    transfer_bytes,
                    buffers,
                )?,
            }
        } else {
            match &input {
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
        let image_range = Some(BufferRange {
            offset: 0,
            size: transfer_bytes,
        });
        let input_buffer = if mapped_input {
            &buffers
                .mapped_native_input
                .as_ref()
                .expect("successful mapped upload owns an input buffer")
                .buffer
        } else {
            &buffers.buf_a
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_transpose"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_transpose"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(groups_x) * u64::from(groups_y),
            );
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
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
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: transfer_bytes,
            readback_bytes: transfer_bytes,
            parameter_bytes: parameter_bytes as u64,
            retained_cache_bytes: buffers.retained_bytes(),
            // Host writes/readback remain visible in the byte counters.
            // Mapping input avoids only the staging-to-storage device copy.
            full_frame_copy_count: u64::from(!mapped_input)
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
