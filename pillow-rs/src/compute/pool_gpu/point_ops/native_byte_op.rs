use super::*;

type NativeByteOpShader = (&'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct NativeByteOpLayout {
    width: u32,
    height: u32,
    length: usize,
    transfer_bytes: u64,
    words: u32,
    columns: u32,
    rows: u32,
    groups_x: u32,
    groups_y: u32,
    in_place: bool,
    separate_secondary: bool,
}

#[derive(Clone, Copy)]
struct NativeByteOpParameters {
    words: [u32; 8],
    word_count: usize,
    byte_count: usize,
}

fn plan_native_byte_op_layout(
    gpu: &GpuInner,
    op: &PipelineOp,
    image: &DynamicImage,
    other: Option<&DynamicImage>,
    channels: u8,
    fused_screen: bool,
    buffers: &BufferPool,
) -> Result<NativeByteOpLayout, PilError> {
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
    let limit = gpu.device.limits().max_compute_workgroups_per_dimension;
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
    Ok(NativeByteOpLayout {
        width,
        height,
        length,
        transfer_bytes,
        words,
        columns,
        rows,
        groups_x,
        groups_y,
        in_place,
        separate_secondary,
    })
}

fn native_byte_op_blend_shader(op: &PipelineOp, fused_screen: bool) -> Option<NativeByteOpShader> {
    Some(match op {
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
        PipelineOp::BlendModule { .. } => (
            "BlendModule",
            "blend_module.wgsl",
            include_str!("../shaders/blend_module.wgsl"),
        ),
        _ => return None,
    })
}

fn native_byte_op_shader(
    op: &PipelineOp,
    fused_screen: bool,
) -> Result<NativeByteOpShader, PilError> {
    if let Some(shader) = native_byte_op_blend_shader(op, fused_screen) {
        return Ok(shader);
    }
    Ok(match op {
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
        PipelineOp::AlphaComposite { .. } => (
            "AlphaComposite",
            "alpha_composite.wgsl",
            include_str!("../shaders/alpha_composite.wgsl"),
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
        // Threshold zero makes Solarize invert each stored byte, including
        // alpha and a partial final word.
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
    })
}

fn prepare_native_byte_op_parameters(
    gpu: &GpuInner,
    op: &PipelineOp,
    channels: u8,
    logical_mode: Option<&str>,
    layout: NativeByteOpLayout,
    buffers: &mut BufferPool,
) -> Result<NativeByteOpParameters, PilError> {
    // Brightness uses native samples-per-pixel. AlphaComposite mode 9 packs
    // two complete LA pixels; RGBA keeps its ordinary mode code.
    let mode = if matches!(op, PipelineOp::Brightness { .. }) {
        u32::from(channels)
    } else if layout.in_place && channels == 4 {
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
        u32::try_from(layout.length)
            .map_err(|_| PilError::ValueError("GPU native brightness input is too large".into()))?
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
    let words = [
        layout.columns,
        layout.rows,
        mode,
        layout.words,
        parameter,
        byte_len,
        active_channels,
        0,
    ];
    let word_count = if matches!(
        op,
        PipelineOp::Brightness { .. } | PipelineOp::BlendModule { .. }
    ) {
        8
    } else if layout.in_place {
        6
    } else if layout.separate_secondary {
        4
    } else {
        5
    };
    let byte_count = std::mem::size_of_val(&words[..word_count]);
    buffers.params_arena.ensure_capacity(
        &gpu.device,
        "gpu_batch_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        byte_count,
        gpu.device.limits().min_uniform_buffer_offset_alignment as usize,
    );
    if layout.separate_secondary {
        buffers.img2_arena.ensure_capacity(
            &gpu.device,
            "gpu_batch_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            layout.transfer_bytes as usize,
            4,
        );
    }
    Ok(NativeByteOpParameters {
        words,
        word_count,
        byte_count,
    })
}

fn upload_native_byte_op_inputs(
    gpu: &GpuInner,
    image: &DynamicImage,
    other: Option<&DynamicImage>,
    layout: NativeByteOpLayout,
    buffers: &mut BufferPool,
) -> Result<(), PilError> {
    let write_bytes = |buffer: &wgpu::Buffer, bytes: &[u8]| -> Result<(), PilError> {
        let size = NonZeroU64::new(layout.transfer_bytes).ok_or_else(|| {
            PilError::InternalError("GPU native byte operation upload is empty".into())
        })?;
        let mut view = gpu
            .queue
            .write_buffer_with(buffer, 0, size)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native byte operation staging allocation failed".into(),
                )
            })?;
        let mapped = view.as_mut();
        mapped[..layout.length].copy_from_slice(bytes);
        mapped[layout.length..].fill(0);
        Ok(())
    };
    // The in-place shader reads the overlay and updates the destination;
    // initialize that output buffer directly instead of a device copy.
    let (input_image, second_image) = if layout.in_place {
        (
            other.ok_or_else(|| PilError::InternalError("missing composite source".into()))?,
            Some(image),
        )
    } else {
        (image, other)
    };
    if gpu.direct_primary_readback {
        gpu.upload_native_bytes_mapped(
            input_image.as_bytes(),
            layout.length,
            layout.transfer_bytes,
            buffers,
        )?;
    } else {
        write_bytes(&buffers.buf_a, input_image.as_bytes())?;
    }
    if let Some(second_image) = second_image {
        write_bytes(
            if layout.in_place {
                &buffers.buf_b
            } else {
                &buffers.img2_arena.buffer
            },
            second_image.as_bytes(),
        )?;
    }
    Ok(())
}

fn native_byte_op_bind_group(
    gpu: &GpuInner,
    cached: &CachedPipeline,
    buffers: &BufferPool,
    layout: NativeByteOpLayout,
    parameters: NativeByteOpParameters,
) -> Result<wgpu::BindGroup, PilError> {
    let input = if gpu.direct_primary_readback {
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
        size: layout.transfer_bytes,
    });
    let parameter_range = Some(BufferRange {
        offset: 0,
        size: parameters.byte_count as u64,
    });
    let mut entries = Vec::with_capacity(if layout.separate_secondary { 4 } else { 3 });
    entries.push(wgpu::BindGroupEntry {
        binding: 0,
        resource: ranged_binding(input, image_range)?,
    });
    if layout.separate_secondary {
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: ranged_binding(&buffers.img2_arena.buffer, image_range)?,
        });
    }
    entries.push(wgpu::BindGroupEntry {
        binding: if layout.separate_secondary { 2 } else { 1 },
        resource: ranged_binding(&buffers.buf_b, image_range)?,
    });
    entries.push(wgpu::BindGroupEntry {
        binding: if layout.separate_secondary { 3 } else { 2 },
        resource: ranged_binding(&buffers.params_arena.buffer, parameter_range)?,
    });
    Ok(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_native_byte_op"),
        layout: &cached.bind_group_layout,
        entries: &entries,
    }))
}

fn submit_native_byte_op(
    gpu: &GpuInner,
    cached: &CachedPipeline,
    bind_group: &wgpu::BindGroup,
    buffers: &mut BufferPool,
    layout: NativeByteOpLayout,
    parameters: NativeByteOpParameters,
    channels: u8,
    has_auxiliary: bool,
    fused_screen: bool,
) -> Result<DynamicImage, PilError> {
    let mut encoder = gpu
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
        pass.set_bind_group(0, bind_group, &[]);
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(layout.groups_x) * u64::from(layout.groups_y),
        );
        pass.dispatch_workgroups(layout.groups_x, layout.groups_y, 1);
    }
    let readback = gpu.prepare_readback(&buffers.buf_b, layout.transfer_bytes)?;
    if let ReadbackTarget::Staging(staging) = &readback {
        encoder.copy_buffer_to_buffer(&buffers.buf_b, 0, &staging.buffer, 0, layout.transfer_bytes);
    }
    gpu.queue.submit(Some(encoder.finish()));
    gpu.poll_device("GPU native byte operation submission")?;
    let result = gpu.readback_with(
        layout.transfer_bytes,
        readback.buffer(buffers, false),
        |mapped| {
            if mapped.len() != layout.transfer_bytes as usize {
                return Err(PilError::InternalError(
                    "GPU native byte operation readback length mismatch".into(),
                ));
            }
            let bytes = mapped[..layout.length].to_vec();
            crate::compute::record_pipeline_allocation(bytes.len());
            crate::image_utils::raw_bytes_to_image(
                layout.width,
                layout.height,
                bytes,
                usize::from(channels),
            )
        },
    )?;
    crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        upload_bytes: layout.transfer_bytes,
        auxiliary_bytes: if has_auxiliary {
            layout.transfer_bytes
        } else {
            0
        },
        readback_bytes: layout.transfer_bytes,
        parameter_bytes: parameters.byte_count as u64,
        retained_cache_bytes: buffers.retained_bytes(),
        full_frame_copy_count: 1
            + u64::from(!gpu.direct_primary_readback)
            + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
        mode_conversion_count: 0,
        fused_operation_count: if fused_screen { 2 } else { 0 },
        ..PipelineResourceTelemetry::default()
    });
    crate::compute::record_pipeline_dispatch_count(1);
    if let ReadbackTarget::Staging(staging) = readback {
        gpu.recycle_staging(staging);
    }
    Ok(result)
}

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
        let layout =
            plan_native_byte_op_layout(self, op, image, other, channels, fused_screen, buffers)?;
        let (variant, shader_file, shader_source) = native_byte_op_shader(op, fused_screen)?;
        let cached = self.resolve_pipeline(variant, shader_file, shader_source)?;
        let parameters =
            prepare_native_byte_op_parameters(self, op, channels, logical_mode, layout, buffers)?;
        upload_native_byte_op_inputs(self, image, other, layout, buffers)?;
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters.words[..parameters.word_count]),
        );
        let bind_group = native_byte_op_bind_group(self, &cached, buffers, layout, parameters)?;
        let result = submit_native_byte_op(
            self,
            &cached,
            &bind_group,
            buffers,
            layout,
            parameters,
            channels,
            other.is_some(),
            fused_screen,
        )?;
        Ok(crate::image::preserve_mode(image, result))
    }
}
