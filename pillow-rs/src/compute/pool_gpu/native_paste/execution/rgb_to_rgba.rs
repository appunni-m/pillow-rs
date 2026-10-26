use super::super::super::pipeline_cache::CachedPipeline;
use super::super::super::{
    BufferPool, BufferRange, GpuInner, PipelineResourceTelemetry, ReadbackTarget, ranged_binding,
};
use super::super::NativeRgbRgbaPasteDispatch;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage, RgbaImage};
use std::num::NonZeroU64;
use std::sync::Arc;

impl GpuInner {
    /// Paste packed RGB bytes into RGBA without expanding the source on CPU.
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
        validate_native_rgb_rgba_paste_layout(
            self,
            destination,
            source,
            dispatch,
            buffers,
            (width, height),
            (source_width, source_height),
        )?;
        upload_native_rgb_rgba_paste(
            self,
            buffers,
            destination.as_raw(),
            source.as_raw(),
            dispatch,
        )?;
        let (cached, bind_group) = native_rgb_rgba_paste_bind_group(
            self,
            buffers,
            dispatch,
            (width, height),
            (source_width, source_height),
            offset,
        )?;
        submit_native_rgb_rgba_paste(
            self,
            buffers,
            &cached,
            &bind_group,
            dispatch,
            (width, height),
        )
    }
}

fn validate_native_rgb_rgba_paste_layout(
    gpu: &GpuInner,
    destination: &RgbaImage,
    source: &RgbImage,
    dispatch: NativeRgbRgbaPasteDispatch,
    buffers: &BufferPool,
    dimensions: (u32, u32),
    source_dimensions: (u32, u32),
) -> Result<(), PilError> {
    let destination_dims = CheckedDims::new(dimensions.0, dimensions.1, 4)?;
    let source_dims = CheckedDims::new(source_dimensions.0, source_dimensions.1, 3)?;
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
    let limits = gpu.device.limits();
    if dispatch.destination_transfer_bytes > buffer_capacity
        || dispatch.destination_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
        || dispatch.source_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
        || dispatch.destination_transfer_bytes > limits.max_buffer_size
        || dispatch.source_transfer_bytes > limits.max_buffer_size
    {
        return Err(PilError::ValueError(
            "GPU native RGB-to-RGBA Paste exceeds adapter buffer limits".into(),
        ));
    }
    Ok(())
}

fn upload_native_rgb_rgba_paste(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    destination: &[u8],
    source: &[u8],
    dispatch: NativeRgbRgbaPasteDispatch,
) -> Result<(), PilError> {
    let limits = gpu.device.limits();
    buffers.img2_arena.ensure_capacity(
        &gpu.device,
        "gpu_paste_native_rgb_to_rgba_source",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        dispatch.source_transfer_bytes as usize,
        4,
    );
    buffers.params_arena.ensure_capacity(
        &gpu.device,
        "gpu_paste_native_rgb_to_rgba_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        48,
        limits.min_uniform_buffer_offset_alignment as usize,
    );
    write_padded_native_paste_bytes(
        gpu,
        &buffers.buf_a,
        destination,
        dispatch.destination_transfer_bytes,
    )?;
    write_padded_native_paste_bytes(
        gpu,
        &buffers.img2_arena.buffer,
        source,
        dispatch.source_transfer_bytes,
    )
}

fn write_padded_native_paste_bytes(
    gpu: &GpuInner,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    transfer_bytes: u64,
) -> Result<(), PilError> {
    let size = NonZeroU64::new(transfer_bytes)
        .ok_or_else(|| PilError::InternalError("GPU native Paste upload is empty".into()))?;
    let mut view = gpu
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
    Ok(())
}

fn native_rgb_rgba_paste_bind_group(
    gpu: &GpuInner,
    buffers: &BufferPool,
    dispatch: NativeRgbRgbaPasteDispatch,
    dimensions: (u32, u32),
    source_dimensions: (u32, u32),
    offset: (i32, i32),
) -> Result<(Arc<CachedPipeline>, wgpu::BindGroup), PilError> {
    let parameters = [
        dimensions.0,
        dimensions.1,
        source_dimensions.0,
        source_dimensions.1,
        dispatch.destination_pixels,
        dispatch.workgroups_x,
        offset.0 as u32,
        offset.1 as u32,
        0,
        0,
        0,
        0,
    ];
    gpu.queue.write_buffer(
        &buffers.params_arena.buffer,
        0,
        bytemuck::cast_slice(&parameters),
    );
    let cached = gpu.resolve_pipeline(
        "__internal_paste_native_rgb_to_rgba",
        "paste_native_rgb_to_rgba.wgsl",
        include_str!("../../shaders/paste_native_rgb_to_rgba.wgsl"),
    )?;
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
    Ok((cached, bind_group))
}

fn submit_native_rgb_rgba_paste(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    cached: &CachedPipeline,
    bind_group: &wgpu::BindGroup,
    dispatch: NativeRgbRgbaPasteDispatch,
    dimensions: (u32, u32),
) -> Result<DynamicImage, PilError> {
    let mut encoder = gpu
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
        pass.set_bind_group(0, bind_group, &[]);
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(dispatch.workgroups_x) * u64::from(dispatch.workgroups_y),
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
    gpu.poll_device("GPU native RGB-to-RGBA Paste submission")?;
    let result = gpu.readback_with(
        dispatch.destination_transfer_bytes,
        readback.buffer(buffers, false),
        |mapped| decode_native_rgb_rgba_paste(mapped, dimensions, dispatch),
    )?;
    record_native_rgb_rgba_paste_telemetry(buffers, dispatch, &readback);
    if let ReadbackTarget::Staging(staging) = readback {
        gpu.recycle_staging(staging);
    }
    Ok(result)
}

fn decode_native_rgb_rgba_paste(
    mapped: &[u8],
    dimensions: (u32, u32),
    dispatch: NativeRgbRgbaPasteDispatch,
) -> Result<DynamicImage, PilError> {
    if mapped.len() != dispatch.destination_transfer_bytes as usize {
        return Err(PilError::InternalError(
            "GPU native RGB-to-RGBA Paste readback length mismatch".into(),
        ));
    }
    let bytes = mapped[..dispatch.destination_bytes].to_vec();
    crate::compute::record_pipeline_allocation(bytes.len());
    RgbaImage::from_raw(dimensions.0, dimensions.1, bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| {
            PilError::InternalError("GPU native RGB-to-RGBA Paste output shape mismatch".into())
        })
}

fn record_native_rgb_rgba_paste_telemetry(
    buffers: &BufferPool,
    dispatch: NativeRgbRgbaPasteDispatch,
    readback: &ReadbackTarget,
) {
    crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        upload_bytes: dispatch.input_transfer_bytes,
        auxiliary_bytes: dispatch.source_transfer_bytes,
        readback_bytes: dispatch.destination_transfer_bytes,
        parameter_bytes: 48,
        retained_cache_bytes: buffers.retained_bytes(),
        full_frame_copy_count: 2 + u64::from(matches!(readback, ReadbackTarget::Staging(_))),
        mode_conversion_count: 0,
        ..PipelineResourceTelemetry::default()
    });
    crate::compute::record_pipeline_dispatch_count(1);
}
