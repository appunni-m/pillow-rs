use super::super::super::pipeline_cache::CachedPipeline;
use super::super::super::{
    BufferPool, BufferRange, GpuInner, PipelineResourceTelemetry, ReadbackTarget, ranged_binding,
};
use super::super::NativeMaskedBytePasteDispatch;
use super::super::gpu_native_masked_byte_paste_shader;
use super::{ValidatedMaskedPasteLayout, validate_masked_paste_layout};
use crate::error::PilError;
use crate::raster::DynamicImage;
use std::num::NonZeroU64;
use std::sync::Arc;

struct MaskedPasteInputs<'a> {
    destination: &'a wgpu::Buffer,
    auxiliary: &'a wgpu::Buffer,
    auxiliary_offset: u64,
}

struct PreparedMaskedPaste {
    cached: Arc<CachedPipeline>,
    bind_group: wgpu::BindGroup,
    label: &'static str,
}

impl GpuInner {
    /// Execute exact masked byte Paste with compact buffers.
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
        let layout =
            validate_masked_paste_layout(destination, source, mask, logical_mode, dispatch)?;
        validate_masked_paste_limits(self, buffers, dispatch)?;
        let auxiliary_offset = upload_masked_paste_inputs(self, buffers, &layout, dispatch)?;
        let prepared = prepare_masked_paste_bindings(
            self,
            buffers,
            &layout,
            logical_mode,
            offset,
            dispatch,
            auxiliary_offset,
        )?;
        submit_masked_paste(self, buffers, &layout, dispatch, prepared)
    }
}

fn validate_masked_paste_limits(
    gpu: &GpuInner,
    buffers: &BufferPool,
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<(), PilError> {
    let buffer_capacity = u64::from(buffers.capacity)
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("GPU native masked Paste is too large".into()))?;
    let binding_limit = u64::from(gpu.device.limits().max_storage_buffer_binding_size);
    let max_buffer_size = gpu.device.limits().max_buffer_size;
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
    Ok(())
}

fn upload_masked_paste_inputs(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    layout: &ValidatedMaskedPasteLayout<'_>,
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<u64, PilError> {
    buffers.params_arena.ensure_capacity(
        &gpu.device,
        "gpu_paste_native_masked_l_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        48,
        gpu.device.limits().min_uniform_buffer_offset_alignment as usize,
    );
    if gpu.direct_primary_readback {
        return gpu.upload_masked_paste_inputs_mapped(
            layout.destination_bytes,
            dispatch.destination_transfer_bytes,
            layout.source_bytes,
            dispatch.source_transfer_bytes,
            layout.mask_bytes,
            dispatch.mask_transfer_bytes,
            buffers,
        );
    }
    write_padded_masked_paste_bytes(
        gpu,
        &buffers.buf_a,
        layout.destination_bytes,
        dispatch.destination_transfer_bytes,
    )?;
    buffers.img2_arena.ensure_capacity(
        &gpu.device,
        "gpu_paste_native_masked_source_mask",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        dispatch.input_transfer_bytes as usize,
        4,
    );
    upload_masked_paste_source_mask(gpu, buffers, layout, dispatch)
}

fn write_padded_masked_paste_bytes(
    gpu: &GpuInner,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    transfer_bytes: u64,
) -> Result<(), PilError> {
    let size = NonZeroU64::new(transfer_bytes)
        .ok_or_else(|| PilError::InternalError("GPU native masked Paste upload is empty".into()))?;
    let mut view = gpu
        .queue
        .write_buffer_with(buffer, 0, size)
        .ok_or_else(|| {
            PilError::InternalError("GPU native masked Paste staging allocation failed".into())
        })?;
    let mapped = view.as_mut();
    if mapped.len() != transfer_bytes as usize || bytes.len() > mapped.len() {
        return Err(PilError::InternalError(
            "GPU native masked Paste upload length mismatch".into(),
        ));
    }
    mapped[..bytes.len()].copy_from_slice(bytes);
    mapped[bytes.len()..].fill(0);
    Ok(())
}

fn upload_masked_paste_source_mask(
    gpu: &GpuInner,
    buffers: &BufferPool,
    layout: &ValidatedMaskedPasteLayout<'_>,
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<u64, PilError> {
    let input_upload_size = NonZeroU64::new(dispatch.input_transfer_bytes)
        .ok_or_else(|| PilError::InternalError("GPU native masked Paste upload is empty".into()))?;
    let mut input_upload = gpu
        .queue
        .write_buffer_with(&buffers.img2_arena.buffer, 0, input_upload_size)
        .ok_or_else(|| {
            PilError::InternalError("GPU native masked Paste staging allocation failed".into())
        })?;
    let input_mapped = input_upload.as_mut();
    let source_transfer = dispatch.source_transfer_bytes as usize;
    let mask_transfer = dispatch.mask_transfer_bytes as usize;
    let mask_start = source_transfer;
    let mask_end = mask_start.checked_add(mask_transfer).ok_or_else(|| {
        PilError::InternalError("GPU native masked Paste input length overflow".into())
    })?;
    if input_mapped.len() != dispatch.input_transfer_bytes as usize
        || layout.source_bytes.len() > source_transfer
        || layout.mask_bytes.len() > mask_transfer
        || mask_end != input_mapped.len()
    {
        return Err(PilError::InternalError(
            "GPU native masked Paste upload length mismatch".into(),
        ));
    }
    input_mapped[..layout.source_bytes.len()].copy_from_slice(layout.source_bytes);
    input_mapped[layout.source_bytes.len()..mask_start].fill(0);
    input_mapped[mask_start..mask_start + layout.mask_bytes.len()]
        .copy_from_slice(layout.mask_bytes);
    input_mapped[mask_start + layout.mask_bytes.len()..mask_end].fill(0);
    // QueueWriteBufferView submits the upload before the compute command buffer.
    drop(input_upload);
    Ok(0)
}

fn prepare_masked_paste_bindings(
    gpu: &GpuInner,
    buffers: &BufferPool,
    layout: &ValidatedMaskedPasteLayout<'_>,
    logical_mode: &str,
    offset: (i32, i32),
    dispatch: NativeMaskedBytePasteDispatch,
    auxiliary_offset: u64,
) -> Result<PreparedMaskedPaste, PilError> {
    let (destination_input, auxiliary_input) = if gpu.direct_primary_readback {
        let input = &buffers
            .mapped_native_input
            .as_ref()
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU native masked Paste mapped input was not retained".into(),
                )
            })?
            .buffer;
        (input, input)
    } else {
        (&buffers.buf_a, &buffers.img2_arena.buffer)
    };
    let parameters = masked_paste_parameters(layout, logical_mode, offset, dispatch)?;
    gpu.queue.write_buffer(
        &buffers.params_arena.buffer,
        0,
        bytemuck::cast_slice(&parameters),
    );
    let (pipeline_variant, shader_file, shader_source) = gpu_native_masked_byte_paste_shader(
        logical_mode,
        dispatch.bytes_per_pixel,
    )
    .ok_or_else(|| {
        PilError::InternalError("GPU native masked Paste has an unsupported logical mode".into())
    })?;
    let cached = gpu.resolve_pipeline(pipeline_variant, shader_file, shader_source)?;
    let label = masked_paste_label(logical_mode);
    let bind_group = create_masked_paste_bind_group(
        gpu,
        buffers,
        dispatch,
        MaskedPasteInputs {
            destination: destination_input,
            auxiliary: auxiliary_input,
            auxiliary_offset,
        },
        &cached,
        label,
    )?;
    Ok(PreparedMaskedPaste {
        cached,
        bind_group,
        label,
    })
}

fn masked_paste_parameters(
    layout: &ValidatedMaskedPasteLayout<'_>,
    logical_mode: &str,
    offset: (i32, i32),
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<[u32; 12], PilError> {
    Ok([
        layout.width,
        layout.height,
        layout.region_width,
        layout.region_height,
        dispatch.destination_pixels,
        dispatch.word_count,
        offset.0 as u32,
        offset.1 as u32,
        u32::try_from(dispatch.source_transfer_bytes / 4).map_err(|_| {
            PilError::ValueError("GPU native masked Paste mask offset is too large".into())
        })?,
        if dispatch.bytes_per_pixel == 4 {
            dispatch.workgroups_x
        } else {
            u32::from(dispatch.binary_mask)
        },
        if dispatch.source_is_broadcast { 1 } else { 0 },
        if dispatch.bytes_per_pixel == 4 {
            u32::from(dispatch.binary_mask)
        } else {
            u32::from(
                matches!(logical_mode, "RGB" | "HSV")
                    && offset == (0, 0)
                    && layout.region_width == layout.width
                    && layout.region_height == layout.height,
            )
        },
    ])
}

fn masked_paste_label(logical_mode: &str) -> &'static str {
    match logical_mode {
        "1" | "L" | "P" => "gpu_native_masked_l_paste",
        "LA" => "gpu_native_masked_la_paste",
        "PA" => "gpu_native_masked_pa_paste",
        "RGB" | "HSV" => "gpu_native_masked_rgb_paste",
        "RGBA" => "gpu_native_masked_rgba_paste",
        "CMYK" => "gpu_native_masked_cmyk_paste",
        _ => unreachable!("storage variant validated above"),
    }
}

fn create_masked_paste_bind_group(
    gpu: &GpuInner,
    buffers: &BufferPool,
    dispatch: NativeMaskedBytePasteDispatch,
    inputs: MaskedPasteInputs<'_>,
    cached: &CachedPipeline,
    label: &'static str,
) -> Result<wgpu::BindGroup, PilError> {
    Ok(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ranged_binding(
                    inputs.destination,
                    Some(BufferRange {
                        offset: 0,
                        size: dispatch.destination_transfer_bytes,
                    }),
                )?,
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ranged_binding(
                    inputs.auxiliary,
                    Some(BufferRange {
                        offset: inputs.auxiliary_offset,
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
    }))
}

fn submit_masked_paste(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    layout: &ValidatedMaskedPasteLayout<'_>,
    dispatch: NativeMaskedBytePasteDispatch,
    prepared: PreparedMaskedPaste,
) -> Result<DynamicImage, PilError> {
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(prepared.label),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(prepared.label),
            timestamp_writes: None,
        });
        pass.set_pipeline(&prepared.cached.pipeline);
        pass.set_bind_group(0, &prepared.bind_group, &[]);
        crate::compute::record_gpu_shader_dispatch(
            prepared.cached.variant_name,
            prepared.cached.shader_file,
            u64::from(dispatch.workgroups),
        );
        pass.dispatch_workgroups(dispatch.workgroups_x, dispatch.workgroups_y, 1);
    }
    let readback = gpu.prepare_readback(&buffers.buf_b, dispatch.destination_transfer_bytes)?;
    if let ReadbackTarget::Staging(staging) = &readback {
        encoder.copy_buffer_to_buffer(
            &buffers.buf_b,
            0,
            &staging.buffer,
            0,
            dispatch.destination_transfer_bytes,
        );
    }
    gpu.queue.submit(Some(encoder.finish()));
    let result = gpu.readback_with(
        dispatch.destination_transfer_bytes,
        readback.buffer(buffers, false),
        |mapped| decode_masked_paste_readback(mapped, layout, dispatch),
    )?;
    record_masked_paste_telemetry(gpu, buffers, dispatch, &readback);
    if let ReadbackTarget::Staging(staging) = readback {
        gpu.recycle_staging(staging);
    }
    Ok(result)
}

fn decode_masked_paste_readback(
    mapped: &[u8],
    layout: &ValidatedMaskedPasteLayout<'_>,
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<DynamicImage, PilError> {
    if mapped.len() != dispatch.destination_transfer_bytes as usize {
        return Err(PilError::InternalError(
            "GPU native masked Paste readback length mismatch".into(),
        ));
    }
    let bytes = mapped[..dispatch.destination_bytes].to_vec();
    crate::compute::record_pipeline_allocation(bytes.len());
    crate::image_utils::raw_bytes_to_image(
        layout.width,
        layout.height,
        bytes,
        usize::from(dispatch.bytes_per_pixel),
    )
}

fn record_masked_paste_telemetry(
    gpu: &GpuInner,
    buffers: &BufferPool,
    dispatch: NativeMaskedBytePasteDispatch,
    readback: &ReadbackTarget,
) {
    crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        upload_bytes: dispatch.destination_transfer_bytes
            + dispatch.source_transfer_bytes
            + dispatch.mask_transfer_bytes,
        auxiliary_bytes: dispatch.source_transfer_bytes + dispatch.mask_transfer_bytes,
        readback_bytes: dispatch.destination_transfer_bytes,
        parameter_bytes: 48,
        retained_cache_bytes: buffers.retained_bytes(),
        full_frame_copy_count: 1
            + 2 * u64::from(!gpu.direct_primary_readback)
            + u64::from(matches!(readback, ReadbackTarget::Staging(_))),
        mode_conversion_count: 0,
        ..PipelineResourceTelemetry::default()
    });
    crate::compute::record_pipeline_dispatch_count(1);
}
