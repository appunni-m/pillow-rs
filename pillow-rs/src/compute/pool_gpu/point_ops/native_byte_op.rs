use super::*;

impl GpuInner {
    /// Execute admitted byte operations without transport mode expansion.
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_byte_op(
        &self,
        op: &PipelineOp,
        image: &DynamicImage,
        other: Option<&DynamicImage>,
        channels: u8,
        logical_mode: Option<&str>,
        fused_screen: bool,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let (width, height) = image.dimensions();
        let dimensions = CheckedDims::new(width, height, channels)?;
        let length = dimensions.total_bytes();
        if image.as_bytes().len() != length
            || other.is_some_and(|other| {
                image.dimensions() != other.dimensions()
                    || image.color() != other.color()
                    || other.as_bytes().len() != length
            })
            || other.is_none()
                != matches!(
                    op,
                    PipelineOp::Duplicate
                        | PipelineOp::Invert
                        | PipelineOp::InvertChops
                        | PipelineOp::Solarize { .. }
                        | PipelineOp::Posterize { .. }
                        | PipelineOp::Brightness { .. }
                        | PipelineOp::PutAlpha {
                            mode: PixelMode::LA | PixelMode::PA,
                            ..
                        }
                )
            || (fused_screen && !matches!(op, PipelineOp::Multiply { .. }))
        {
            return Err(PilError::InternalError(
                "GPU native byte operation layout mismatch".into(),
            ));
        }
        let transfer_bytes = (length as u64 + 3) & !3;
        if transfer_bytes > u64::from(buffers.capacity) * 4 {
            return Err(PilError::InternalError(
                "GPU native byte operation exceeds its working set".into(),
            ));
        }
        let words = u32::try_from(transfer_bytes / 4)
            .map_err(|_| PilError::ValueError("GPU native byte operation is too large".into()))?;
        let columns = words.min(1024);
        let rows = words.div_ceil(columns);
        let groups_x = columns.div_ceil(16);
        let groups_y = rows.div_ceil(16);
        let limit = self.device.limits().max_compute_workgroups_per_dimension;
        if groups_x > limit || groups_y > limit {
            return Err(PilError::ValueError(
                "GPU native byte operation exceeds adapter workgroup limit".into(),
            ));
        }
        let in_place = matches!(op, PipelineOp::AlphaComposite { .. });
        let separate_secondary = matches!(
            op,
            PipelineOp::Multiply { .. }
                | PipelineOp::Screen { .. }
                | PipelineOp::Overlay { .. }
                | PipelineOp::HardLight { .. }
                | PipelineOp::SoftLight { .. }
                | PipelineOp::Difference { .. }
                | PipelineOp::Darker { .. }
                | PipelineOp::Lighter { .. }
                | PipelineOp::AddModulo { .. }
                | PipelineOp::SubtractModulo { .. }
                | PipelineOp::LogicalAnd { .. }
                | PipelineOp::LogicalOr { .. }
                | PipelineOp::LogicalXor { .. }
                | PipelineOp::BlendModule { .. }
        );
        let (variant, shader_file, shader_source) = match op {
            PipelineOp::Duplicate => (
                "DuplicateNativeBytes",
                "duplicate.wgsl",
                include_str!("../shaders/duplicate.wgsl"),
            ),
            PipelineOp::Brightness { .. } => (
                "BrightnessNativeBytes",
                "brightness_native.wgsl",
                include_str!("../shaders/brightness_native.wgsl"),
            ),
            PipelineOp::PutAlpha {
                mode: PixelMode::LA,
                ..
            } => (
                "PutAlphaNativeLA",
                "put_alpha_native_la.wgsl",
                include_str!("../shaders/put_alpha_native_la.wgsl"),
            ),
            PipelineOp::PutAlpha {
                mode: PixelMode::PA,
                ..
            } => (
                "PutAlphaNativePA",
                "put_alpha_native_la.wgsl",
                include_str!("../shaders/put_alpha_native_la.wgsl"),
            ),
            PipelineOp::Multiply { .. } if fused_screen => (
                "__internal_multiply_screen",
                "multiply_screen.wgsl",
                include_str!("../shaders/multiply_screen.wgsl"),
            ),
            PipelineOp::Multiply { .. } => (
                "Multiply",
                "multiply.wgsl",
                include_str!("../shaders/multiply.wgsl"),
            ),
            PipelineOp::Screen { .. } => (
                "Screen",
                "screen.wgsl",
                include_str!("../shaders/screen.wgsl"),
            ),
            PipelineOp::Overlay { .. } => (
                "Overlay",
                "overlay.wgsl",
                include_str!("../shaders/overlay.wgsl"),
            ),
            PipelineOp::HardLight { .. } => (
                "HardLight",
                "hard_light.wgsl",
                include_str!("../shaders/hard_light.wgsl"),
            ),
            PipelineOp::SoftLight { .. } => (
                "SoftLight",
                "soft_light.wgsl",
                include_str!("../shaders/soft_light.wgsl"),
            ),
            PipelineOp::Difference { .. } => (
                "Difference",
                "difference.wgsl",
                include_str!("../shaders/difference.wgsl"),
            ),
            PipelineOp::Darker { .. } => (
                "Darker",
                "darker.wgsl",
                include_str!("../shaders/darker.wgsl"),
            ),
            PipelineOp::Lighter { .. } => (
                "Lighter",
                "lighter.wgsl",
                include_str!("../shaders/lighter.wgsl"),
            ),
            PipelineOp::AddModulo { .. } => (
                "AddModulo",
                "add_modulo.wgsl",
                include_str!("../shaders/add_modulo.wgsl"),
            ),
            PipelineOp::SubtractModulo { .. } => (
                "SubtractModulo",
                "subtract_modulo.wgsl",
                include_str!("../shaders/subtract_modulo.wgsl"),
            ),
            PipelineOp::LogicalAnd { .. } => (
                "LogicalAnd",
                "logical_and.wgsl",
                include_str!("../shaders/logical_and.wgsl"),
            ),
            PipelineOp::LogicalOr { .. } => (
                "LogicalOr",
                "logical_or.wgsl",
                include_str!("../shaders/logical_or.wgsl"),
            ),
            PipelineOp::LogicalXor { .. } => (
                "LogicalXor",
                "logical_xor.wgsl",
                include_str!("../shaders/logical_xor.wgsl"),
            ),
            PipelineOp::AlphaComposite { .. } => (
                "AlphaComposite",
                "alpha_composite.wgsl",
                include_str!("../shaders/alpha_composite.wgsl"),
            ),
            PipelineOp::BlendModule { .. } => (
                "BlendModule",
                "blend_module.wgsl",
                include_str!("../shaders/blend_module.wgsl"),
            ),
            PipelineOp::Solarize { .. } => (
                "Solarize",
                "solarize.wgsl",
                include_str!("../shaders/solarize.wgsl"),
            ),
            PipelineOp::Posterize { .. } => (
                "PosterizeNativeBytes",
                "posterize.wgsl",
                include_str!("../shaders/posterize.wgsl"),
            ),
            // The native bytewise shader treats a u32 as four independent
            // samples (mode 9); threshold zero makes Solarize exactly invert
            // every stored byte, including alpha and a partial final word.
            PipelineOp::Invert | PipelineOp::InvertChops => (
                "InvertNativeBytes",
                "solarize.wgsl",
                include_str!("../shaders/solarize.wgsl"),
            ),
            _ => {
                return Err(PilError::InternalError(
                    "unsupported native binary GPU operation".into(),
                ));
            }
        };
        let cached = self.resolve_pipeline(variant, shader_file, shader_source)?;
        // Bytewise mode 9 packs four independent samples. Brightness uses
        // native samples-per-pixel. AlphaComposite mode 9 packs two complete
        // LA pixels; RGBA keeps its ordinary mode code.
        let mode = if matches!(op, PipelineOp::Brightness { .. }) {
            u32::from(channels)
        } else if in_place && channels == 4 {
            3
        } else {
            9
        };
        let parameter = match op {
            PipelineOp::Brightness { factor } => registry::gpu_brightness_factor_int(*factor)
                .ok_or_else(|| {
                    PilError::ValueError("GPU native brightness factor is not exact".into())
                })?,
            PipelineOp::PutAlpha {
                alpha,
                mode: PixelMode::LA | PixelMode::PA,
            } => u32::from(*alpha),
            PipelineOp::Solarize { threshold } => u32::from(*threshold),
            PipelineOp::Posterize { bits } => u32::from(*bits),
            PipelineOp::Invert | PipelineOp::InvertChops => 0,
            PipelineOp::BlendModule { alpha, .. } => (*alpha as f32).to_bits(),
            _ => 0,
        };
        let byte_len = if matches!(op, PipelineOp::Brightness { .. }) {
            u32::try_from(length).map_err(|_| {
                PilError::ValueError("GPU native brightness input is too large".into())
            })?
        } else {
            0
        };
        let active_channels = if matches!(op, PipelineOp::Brightness { .. }) {
            match (logical_mode, channels) {
                (None | Some("LA"), 2) => 1,
                _ => u32::from(channels),
            }
        } else {
            0
        };
        let parameters = [
            columns,
            rows,
            mode,
            words,
            parameter,
            byte_len,
            active_channels,
            0,
        ];
        let parameter_words = if matches!(
            op,
            PipelineOp::Brightness { .. } | PipelineOp::BlendModule { .. }
        ) {
            8
        } else if in_place {
            6
        } else if separate_secondary {
            4
        } else {
            5
        };
        let parameters = &parameters[..parameter_words];
        let parameter_bytes = std::mem::size_of_val(parameters);
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            parameter_bytes,
            self.device.limits().min_uniform_buffer_offset_alignment as usize,
        );
        if separate_secondary {
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                transfer_bytes as usize,
                4,
            );
        }
        let write_bytes = |buffer: &wgpu::Buffer, bytes: &[u8]| -> Result<(), PilError> {
            let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
                PilError::InternalError("GPU native byte operation upload is empty".into())
            })?;
            let mut view = self
                .queue
                .write_buffer_with(buffer, 0, size)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native byte operation staging allocation failed".into(),
                    )
                })?;
            let mapped = view.as_mut();
            mapped[..length].copy_from_slice(bytes);
            mapped[length..].fill(0);
            Ok(())
        };
        // The in-place shader reads the overlay and updates the destination;
        // initialize that output buffer directly instead of a device copy.
        let (input_image, second_image) = if in_place {
            (
                other.ok_or_else(|| PilError::InternalError("missing composite source".into()))?,
                Some(image),
            )
        } else {
            (image, other)
        };
        if self.direct_primary_readback {
            self.upload_native_bytes_mapped(
                input_image.as_bytes(),
                length,
                transfer_bytes,
                buffers,
            )?;
        } else {
            write_bytes(&buffers.buf_a, input_image.as_bytes())?;
        }
        if let Some(second_image) = second_image {
            write_bytes(
                if in_place {
                    &buffers.buf_b
                } else {
                    &buffers.img2_arena.buffer
                },
                second_image.as_bytes(),
            )?;
        }
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(parameters),
        );
        let input = if self.direct_primary_readback {
            &buffers
                .mapped_native_input
                .as_ref()
                .expect("successful mapped upload owns an input buffer")
                .buffer
        } else {
            &buffers.buf_a
        };
        let image_range = Some(BufferRange {
            offset: 0,
            size: transfer_bytes,
        });
        let parameter_range = Some(BufferRange {
            offset: 0,
            size: parameter_bytes as u64,
        });
        let mut entries = Vec::with_capacity(if separate_secondary { 4 } else { 3 });
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: ranged_binding(input, image_range)?,
        });
        if separate_secondary {
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: ranged_binding(&buffers.img2_arena.buffer, image_range)?,
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: if separate_secondary { 2 } else { 1 },
            resource: ranged_binding(&buffers.buf_b, image_range)?,
        });
        entries.push(wgpu::BindGroupEntry {
            binding: if separate_secondary { 3 } else { 2 },
            resource: ranged_binding(&buffers.params_arena.buffer, parameter_range)?,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_native_byte_op"),
            layout: &cached.bind_group_layout,
            entries: &entries,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_native_byte_op"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_native_byte_op"),
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
        self.poll_device("GPU native byte operation submission")?;
        let result =
            self.readback_with(transfer_bytes, readback.buffer(buffers, false), |mapped| {
                if mapped.len() != transfer_bytes as usize {
                    return Err(PilError::InternalError(
                        "GPU native byte operation readback length mismatch".into(),
                    ));
                }
                let bytes = mapped[..length].to_vec();
                crate::compute::record_pipeline_allocation(bytes.len());
                crate::image_utils::raw_bytes_to_image(width, height, bytes, usize::from(channels))
            })?;
        crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
            upload_bytes: transfer_bytes,
            auxiliary_bytes: if other.is_some() { transfer_bytes } else { 0 },
            readback_bytes: transfer_bytes,
            parameter_bytes: parameter_bytes as u64,
            retained_cache_bytes: buffers.retained_bytes(),
            full_frame_copy_count: 1
                + u64::from(!self.direct_primary_readback)
                + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
            mode_conversion_count: 0,
            fused_operation_count: if fused_screen { 2 } else { 0 },
            ..PipelineResourceTelemetry::default()
        });
        crate::compute::record_pipeline_dispatch_count(1);
        if let ReadbackTarget::Staging(staging) = readback {
            self.recycle_staging(staging);
        }
        Ok(crate::image::preserve_mode(image, result))
    }
}
