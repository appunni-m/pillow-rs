//! GPU getprojection admission and native terminal execution.
//! Keep source scanning, bind-group reuse, dispatch, and readback together.

use super::*;

fn projection_source_is_zero(source: &[u8]) -> bool {
    let zero = u8x16::splat(0);
    let mut blocks = source.chunks_exact(64);
    for block in blocks.by_ref() {
        let mut combined = zero;
        for samples in block.chunks_exact(16) {
            let lanes: [u8; 16] = samples
                .try_into()
                .expect("projection SIMD zero check has sixteen-byte lanes");
            combined |= u8x16::new(lanes);
        }
        if combined != zero {
            return false;
        }
    }

    let remainder = blocks.remainder();
    let mut tail = remainder.chunks_exact(16);
    let mut combined = zero;
    for samples in tail.by_ref() {
        let lanes: [u8; 16] = samples
            .try_into()
            .expect("projection SIMD zero check has sixteen-byte lanes");
        combined |= u8x16::new(lanes);
    }
    combined == zero && tail.remainder().iter().all(|&byte| byte == 0)
}

impl GpuPool {
    pub(crate) fn cmyk_projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
    ) -> bool {
        !ops.is_empty() && Self::projection_terminal_supported(image, ops, 4)
    }

    pub(crate) fn rgb_projection_supported(image: &DynamicImage) -> bool {
        Self::projection_terminal_supported(image, &[], 3)
    }

    pub(crate) fn rgb_projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
    ) -> bool {
        !ops.is_empty() && Self::projection_terminal_supported(image, ops, 3)
    }

    fn projection_terminal_supported(
        image: &DynamicImage,
        ops: &[PipelineOp],
        channels: u8,
    ) -> bool {
        let source = match (channels, image) {
            (3, DynamicImage::ImageRgb8(pixels)) => pixels.as_raw().as_slice(),
            (4, DynamicImage::ImageRgba8(pixels)) => pixels.as_raw().as_slice(),
            _ => return false,
        };
        let Ok(dimensions) = CheckedDims::new(image.width(), image.height(), channels) else {
            return false;
        };
        if image.width() == 0
            || image.height() == 0
            || ops.len() > 128
            || dimensions.total_bytes() % 4 != 0
            || !ops
                .iter()
                .all(|op| matches!(op, PipelineOp::PutPixel { .. }))
        {
            return false;
        }
        if dimensions.total_bytes() != source.len() {
            return false;
        }
        let Ok(gpu) = Self::ensure_init() else {
            return false;
        };
        let limits = gpu.device.limits();
        let width = u64::from(image.width());
        let height = u64::from(image.height());
        let pixels = dimensions.total_pixels() as u64;
        let output_words = u64::from(image.width()).saturating_add(u64::from(image.height()));
        let Some(output_bytes) = output_words.checked_mul(4) else {
            return false;
        };
        let capacity_words = pixels.max(output_words).max(1);
        let Some(capacity_bytes) = capacity_words.checked_mul(4) else {
            return false;
        };
        let groups_x = width.div_ceil(16);
        let groups_y = height
            .div_ceil(16)
            .checked_add(u64::from(ops.iter().any(|op| match op {
                PipelineOp::PutPixel { color, .. } => {
                    color.0 != 0 || color.1 != 0 || color.2 != 0 || (channels == 4 && color.3 != 0)
                }
                _ => false,
            })))
            .unwrap_or(u64::MAX);
        if pixels > u64::from(GPU_BUFFER_CAPACITY)
            || capacity_words > u64::from(GPU_BUFFER_CAPACITY)
            || capacity_bytes > limits.max_buffer_size
            || capacity_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || output_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || groups_x > u64::from(limits.max_compute_workgroups_per_dimension)
            || groups_y > u64::from(limits.max_compute_workgroups_per_dimension)
        {
            return false;
        }
        ops.iter().all(|op| match op {
            PipelineOp::PutPixel { x, y, .. } => *x < image.width() && *y < image.height(),
            _ => false,
        })
    }

    pub(crate) fn execute_cmyk_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        if !Self::cmyk_projection_terminal_supported(image, ops) {
            return Err(PilError::NotImplementedError(
                "GPU does not support this CMYK projection terminal".into(),
            ));
        }
        Self::ensure_init()?.execute_projection_terminal(image, ops, 4, source_is_known_zero)
    }

    pub(crate) fn execute_rgb_projection(
        &self,
        image: &DynamicImage,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        self.execute_rgb_projection_terminal(image, &[], false)
    }

    pub(crate) fn execute_rgb_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        let supported = if ops.is_empty() {
            Self::rgb_projection_supported(image)
        } else {
            Self::rgb_projection_terminal_supported(image, ops)
        };
        if !supported {
            return Err(PilError::NotImplementedError(
                "GPU does not support this RGB projection terminal".into(),
            ));
        }
        Self::ensure_init()?.execute_projection_terminal(image, ops, 3, source_is_known_zero)
    }
}

impl GpuInner {
    fn create_projection_bind_group(
        &self,
        layout: &wgpu::BindGroupLayout,
        source: &wgpu::Buffer,
        source_bytes: u64,
        output: &wgpu::Buffer,
        output_bytes: u64,
        params: &wgpu::Buffer,
        overrides: &wgpu::Buffer,
        override_bytes: u64,
    ) -> Result<wgpu::BindGroup, PilError> {
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_getprojection"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ranged_binding(
                        source,
                        Some(BufferRange {
                            offset: 0,
                            size: source_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ranged_binding(
                        output,
                        Some(BufferRange {
                            offset: 0,
                            size: output_bytes,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ranged_binding(
                        params,
                        Some(BufferRange {
                            offset: 0,
                            size: 32,
                        }),
                    )?,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ranged_binding(
                        overrides,
                        Some(BufferRange {
                            offset: 0,
                            size: override_bytes,
                        }),
                    )?,
                },
            ],
        }))
    }

    fn execute_projection_terminal(
        &self,
        image: &DynamicImage,
        ops: &[PipelineOp],
        channels: u8,
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>, PipelineResourceTelemetry, u64), PilError> {
        let source: &[u8] = match (channels, image) {
            (3, DynamicImage::ImageRgb8(pixels)) => pixels.as_raw(),
            (4, DynamicImage::ImageRgba8(pixels)) => pixels.as_raw(),
            _ => {
                return Err(PilError::InternalError(
                    "GPU projection received an unsupported native carrier".into(),
                ));
            }
        };
        let (width, height) = image.dimensions();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let source_bytes = dimensions.total_bytes();
        if source.len() != source_bytes {
            return Err(PilError::InternalError(
                "GPU projection source is not tightly packed".into(),
            ));
        }
        // A zero-filled source contributes no projected pixels. The sparse
        // override kernel can handle this case without transferring or
        // dispatching across the full frame.
        // A constructor-proven zero fill avoids the CPU preflight scan. Other
        // sources still require an exact scan before the zero-upload shader
        // route can be admitted.
        let source_is_zero =
            !ops.is_empty() && (source_is_known_zero || projection_source_is_zero(source));

        let mut pending = Vec::with_capacity(ops.len());
        for (order, op) in ops.iter().enumerate() {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                return Err(PilError::InternalError(
                    "GPU projection received a non-pixel operation".into(),
                ));
            };
            if *x >= width || *y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = y
                .checked_mul(width)
                .and_then(|row| row.checked_add(*x))
                .ok_or_else(|| PilError::ValueError("GPU projection index overflow".into()))?;
            let nonzero = u32::from(
                color.0 != 0 || color.1 != 0 || color.2 != 0 || (channels == 4 && color.3 != 0),
            );
            pending.push((index, order, nonzero));
        }
        pending.sort_unstable_by_key(|(index, order, _)| (*index, *order));
        let mut overrides: Vec<(u32, u32)> = Vec::with_capacity(pending.len());
        for (index, _, nonzero) in pending {
            if let Some((previous_index, previous_nonzero)) = overrides.last_mut()
                && *previous_index == index
            {
                *previous_nonzero = nonzero;
            } else {
                overrides.push((index, nonzero));
            }
        }
        let has_nonzero_override = overrides.iter().any(|(_, nonzero)| *nonzero != 0);

        let output_words = width
            .checked_add(height)
            .ok_or_else(|| PilError::ValueError("GPU projection size overflow".into()))?;
        let output_bytes = usize::try_from(output_words)
            .ok()
            .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
            .ok_or_else(|| PilError::ValueError("GPU projection is too large".into()))?;
        // A known-zero source contributes only the sparse overrides. Pack the
        // two axis vectors into bitsets while they are on the GPU, then expand
        // the public Vec<u32> results after the smaller readback completes.
        let packed_output = source_is_zero;
        let packed_column_words = width.div_ceil(32) as usize;
        let packed_row_words = height.div_ceil(32) as usize;
        let transfer_bytes_len = if packed_output {
            packed_column_words
                .checked_add(packed_row_words)
                .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
                .ok_or_else(|| PilError::ValueError("GPU projection is too large".into()))?
        } else {
            output_bytes
        };
        let transfer_bytes = u64::try_from(transfer_bytes_len)
            .map_err(|_| PilError::ValueError("GPU projection is too large".into()))?;
        // The zero-source shader branch never reads the frame binding. Give
        // it one valid storage word and size the working set for the packed
        // result instead of allocating two full-frame buffers.
        let capacity_words = u32::try_from(if packed_output {
            (transfer_bytes_len / std::mem::size_of::<u32>()).max(1)
        } else {
            dimensions.total_pixels().max(output_words as usize).max(1)
        })
        .map_err(|_| PilError::ValueError("GPU projection working set is too large".into()))?;
        let mut buffers = self.acquire_buffers(capacity_words)?;

        let uniform_alignment = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_getprojection_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            uniform_alignment,
        );
        let mut params = [0u8; 32];
        params[0..4].copy_from_slice(&width.to_le_bytes());
        params[4..8].copy_from_slice(&height.to_le_bytes());
        params[8..12].copy_from_slice(&(overrides.len() as u32).to_le_bytes());
        let has_zero_override = overrides.iter().any(|(_, nonzero)| *nonzero == 0);
        params[12..16].copy_from_slice(&u32::from(has_zero_override).to_le_bytes());
        params[16..20].copy_from_slice(&(channels as u32).to_le_bytes());
        params[20..24].copy_from_slice(&u32::from(source_is_zero).to_le_bytes());
        params[24..28].copy_from_slice(&u32::from(packed_output).to_le_bytes());

        let mut override_bytes = Vec::with_capacity(overrides.len() * 8);
        for (index, nonzero) in &overrides {
            override_bytes.extend_from_slice(&index.to_le_bytes());
            override_bytes.extend_from_slice(&nonzero.to_le_bytes());
        }
        #[cfg(target_endian = "little")]
        if !source_is_zero {
            if self.direct_primary_readback {
                let source_transfer_bytes = u64::try_from(source_bytes).map_err(|_| {
                    PilError::ValueError("GPU projection source is too large".into())
                })?;
                self.upload_native_bytes_mapped(
                    source,
                    source_bytes,
                    source_transfer_bytes,
                    &mut buffers,
                )?;
            } else {
                self.queue.write_buffer(&buffers.buf_a, 0, source);
            }
        }
        #[cfg(not(target_endian = "little"))]
        if !source_is_zero {
            self.queue.write_buffer(&buffers.buf_a, 0, source);
        }
        self.queue
            .write_buffer(&buffers.params_arena.buffer, 0, &params);
        if !override_bytes.is_empty() {
            self.queue
                .write_buffer(&buffers.lut_buf, 0, &override_bytes);
        }

        let cached = self.resolve_pipeline(
            "__internal_getprojection_atomic",
            "getprojection_atomic.wgsl",
            include_str!("shaders/getprojection_atomic.wgsl"),
        )?;
        let transient_bind_group;
        let bind_group = if source_is_zero {
            let params_capacity_bytes = buffers.params_arena.capacity_bytes;
            if buffers
                .projection_zero_bind_group
                .as_ref()
                .is_none_or(|(capacity, _)| *capacity != params_capacity_bytes)
            {
                let output_binding_bytes = buffers.buf_b.size().min(u64::from(
                    self.device.limits().max_storage_buffer_binding_size,
                ));
                if output_binding_bytes < transfer_bytes {
                    return Err(PilError::InternalError(
                        "GPU projection pooled output binding is too small".into(),
                    ));
                }
                let bind_group = self.create_projection_bind_group(
                    &cached.bind_group_layout,
                    &buffers.buf_a,
                    std::mem::size_of::<u32>() as u64,
                    &buffers.buf_b,
                    output_binding_bytes,
                    &buffers.params_arena.buffer,
                    &buffers.lut_buf,
                    buffers.lut_buf.size(),
                )?;
                buffers.projection_zero_bind_group = Some((params_capacity_bytes, bind_group));
            }
            buffers
                .projection_zero_bind_group
                .as_ref()
                .map(|(_, bind_group)| bind_group)
                .expect("zero-source projection bind group was created")
        } else {
            #[cfg(target_endian = "little")]
            let source_buffer = if self.direct_primary_readback {
                &buffers
                    .mapped_native_input
                    .as_ref()
                    .expect("successful mapped projection upload retains its source")
                    .buffer
            } else {
                &buffers.buf_a
            };
            #[cfg(not(target_endian = "little"))]
            let source_buffer = &buffers.buf_a;
            transient_bind_group = self.create_projection_bind_group(
                &cached.bind_group_layout,
                source_buffer,
                source_bytes as u64,
                &buffers.buf_b,
                transfer_bytes,
                &buffers.params_arena.buffer,
                &buffers.lut_buf,
                override_bytes.len().max(8) as u64,
            )?;
            &transient_bind_group
        };
        let readback = self.prepare_readback(&buffers.buf_b, transfer_bytes)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_getprojection"),
            });
        encoder.clear_buffer(&buffers.buf_b, 0, Some(transfer_bytes));
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_getprojection"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            let (groups_x, groups_y) = if source_is_zero {
                (1, 1)
            } else {
                (
                    width.div_ceil(16),
                    height.div_ceil(16) + u32::from(has_nonzero_override),
                )
            };
            pass.dispatch_workgroups(groups_x, groups_y, 1);
            crate::compute::record_gpu_shader_dispatch(
                cached.variant_name,
                cached.shader_file,
                u64::from(groups_x) * u64::from(groups_y),
            );
        }
        if let ReadbackTarget::Staging(staging) = &readback {
            encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, transfer_bytes);
        }
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU projection submission")?;

        // Mapping the result polls the submitted work until readback is ready.
        let (horizontal, vertical) =
            self.readback_with(transfer_bytes, readback.buffer(&buffers, false), |mapped| {
                if mapped.len() != transfer_bytes_len {
                    return Err(PilError::InternalError(
                        "GPU projection readback length mismatch".into(),
                    ));
                }
                let (horizontal, vertical) = if packed_output {
                    let packed = |index: usize| {
                        let start = index * std::mem::size_of::<u32>();
                        u32::from_le_bytes([
                            mapped[start],
                            mapped[start + 1],
                            mapped[start + 2],
                            mapped[start + 3],
                        ])
                    };
                    let horizontal = (0..width as usize)
                        .map(|index| (packed(index / 32) >> (index % 32)) & 1)
                        .collect::<Vec<_>>();
                    let vertical = (0..height as usize)
                        .map(|index| (packed(packed_column_words + index / 32) >> (index % 32)) & 1)
                        .collect::<Vec<_>>();
                    (horizontal, vertical)
                } else {
                    let horizontal = (0..width as usize)
                        .map(|index| {
                            let word_offset = index * 4;
                            u32::from_le_bytes([
                                mapped[word_offset],
                                mapped[word_offset + 1],
                                mapped[word_offset + 2],
                                mapped[word_offset + 3],
                            ])
                        })
                        .collect::<Vec<_>>();
                    let vertical_base = width as usize * 4;
                    let vertical = (0..height as usize)
                        .map(|index| {
                            let word_offset = vertical_base + index * 4;
                            u32::from_le_bytes([
                                mapped[word_offset],
                                mapped[word_offset + 1],
                                mapped[word_offset + 2],
                                mapped[word_offset + 3],
                            ])
                        })
                        .collect::<Vec<_>>();
                    (horizontal, vertical)
                };
                crate::compute::record_pipeline_allocation(width as usize * 4);
                crate::compute::record_pipeline_allocation(height as usize * 4);
                Ok((horizontal, vertical))
            })?;
        let dispatches = 1;
        let resource = PipelineResourceTelemetry {
            upload_bytes: if source_is_zero {
                0
            } else {
                source_bytes as u64
            },
            readback_bytes: transfer_bytes,
            auxiliary_bytes: override_bytes.len() as u64,
            parameter_bytes: params.len() as u64,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: u64::from(!source_is_zero)
                + u64::from(
                    transfer_bytes >= source_bytes as u64
                        && matches!(&readback, ReadbackTarget::Staging(_)),
                ),
            fused_operation_count: ops.len() as u64,
            ..PipelineResourceTelemetry::default()
        };
        crate::compute::record_pipeline_dispatch_count(dispatches);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        self.recycle_buffers(buffers);
        Ok((horizontal, vertical, resource, dispatches))
    }
}
