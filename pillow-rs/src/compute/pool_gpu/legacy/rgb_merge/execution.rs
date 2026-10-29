use super::super::pipeline_cache::CachedPipeline;
use super::super::{
    BufferPool, BufferRange, GPU_NATIVE_RGB_LUMA_MERGE_MODE, GpuInner, PipelineResourceTelemetry,
    ReadbackTarget, ReusableGpuBuffer, ranged_binding,
};
use super::NativeRgbMergeDispatch;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::traits::view::GenericImageView;
use std::num::NonZeroU64;
use std::sync::Arc;

impl GpuInner {
    /// Merge three native L planes into packed RGB bytes without RGBA staging.
    #[cfg(target_endian = "little")]
    fn upload_native_rgb_luma_merge_mapped(
        &self,
        red: &[u8],
        green: &[u8],
        blue: &[u8],
        dispatch: &NativeRgbMergeDispatch,
        auxiliary_offset: u64,
        transfer_bytes: u64,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback
            || red.len() != dispatch.input_bytes
            || green.len().checked_add(blue.len()) != Some(dispatch.auxiliary_bytes)
            || auxiliary_offset < dispatch.input_transfer_bytes
            || auxiliary_offset.checked_add(dispatch.auxiliary_transfer_bytes)
                != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(
                "GPU mapped RGB Merge input layout mismatch".into(),
            ));
        }

        let transfer_len = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU mapped RGB Merge input is too large".into()))?;
        let auxiliary_start = usize::try_from(auxiliary_offset)
            .map_err(|_| PilError::ValueError("GPU mapped RGB Merge offset is too large".into()))?;
        let green_end = auxiliary_start
            .checked_add(green.len())
            .ok_or_else(|| PilError::ValueError("GPU mapped RGB Merge input overflow".into()))?;
        let blue_end = green_end
            .checked_add(blue.len())
            .ok_or_else(|| PilError::ValueError("GPU mapped RGB Merge input overflow".into()))?;
        if blue_end > transfer_len {
            return Err(PilError::InternalError(
                "GPU mapped RGB Merge bands exceed their checked buffer".into(),
            ));
        }

        let usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::STORAGE;
        let input = buffers.mapped_native_input.get_or_insert_with(|| {
            ReusableGpuBuffer::new(
                &self.device,
                "gpu_mapped_native_rgb_merge_input",
                usage,
                transfer_len,
                4,
            )
        });
        input.ensure_capacity(
            &self.device,
            "gpu_mapped_native_rgb_merge_input",
            usage,
            transfer_len,
            4,
        );
        self.wait_for_buffer_mapping(
            transfer_bytes,
            &input.buffer,
            wgpu::MapMode::Write,
            "GPU mapped RGB Merge input",
        )?;
        let result = {
            let mut view = input.buffer.slice(..transfer_bytes).get_mapped_range_mut();
            let mapped = view.as_mut();
            if mapped.len() != transfer_len {
                Err(PilError::InternalError(
                    "GPU mapped RGB Merge input has an unexpected length".into(),
                ))
            } else {
                mapped.fill(0);
                mapped[..red.len()].copy_from_slice(red);
                mapped[auxiliary_start..auxiliary_start + green.len()].copy_from_slice(green);
                mapped[green_end..blue_end].copy_from_slice(blue);
                Ok(())
            }
        };
        input.buffer.unmap();
        result
    }
    #[cfg(target_endian = "little")]
    pub(in crate::compute::pool_gpu) fn execute_native_rgb_luma_merge(
        &self,
        image: &DynamicImage,
        bands: &[crate::image::Image],
        dispatch: NativeRgbMergeDispatch,
        buffers: &mut BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let inputs = validate_native_rgb_merge_inputs(image, bands, dispatch)?;
        let limits = self.device.limits();
        validate_native_rgb_merge_limits(buffers, dispatch, &limits)?;
        let mapped_input_layout = mapped_rgb_merge_input_layout(dispatch, &limits);
        let mapped_input = self.direct_primary_readback && mapped_input_layout.is_some();
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_merge_native_rgb_luma_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            limits.min_uniform_buffer_offset_alignment as usize,
        );
        let mapped_auxiliary_offset = upload_native_rgb_merge_inputs(
            self,
            buffers,
            &inputs,
            dispatch,
            &limits,
            mapped_input_layout,
            mapped_input,
        )?;
        let (cached, bind_group) =
            prepare_native_rgb_merge_bind_group(self, buffers, dispatch, mapped_auxiliary_offset)?;
        submit_native_rgb_merge(self, buffers, dispatch, mapped_input, &cached, &bind_group)
    }
}

struct ValidatedNativeRgbMergeInputs<'a> {
    red: &'a [u8],
    green_owner: Arc<DynamicImage>,
    blue_owner: Arc<DynamicImage>,
}

fn validate_native_rgb_merge_inputs<'a>(
    image: &'a DynamicImage,
    bands: &[crate::image::Image],
    dispatch: NativeRgbMergeDispatch,
) -> Result<ValidatedNativeRgbMergeInputs<'a>, PilError> {
    let DynamicImage::ImageLuma8(red) = image else {
        return Err(PilError::InternalError(
            "GPU native RGB Merge requires a native L first band".into(),
        ));
    };
    let dimensions = CheckedDims::new(dispatch.width, dispatch.height, 1)?;
    if bands.len() != 3
        || image.dimensions() != (dispatch.width, dispatch.height)
        || dimensions.total_pixels() != dispatch.pixels as usize
        || red.as_raw().len() != dispatch.input_bytes
        || CheckedDims::new(dispatch.width, dispatch.height, 3)?.total_bytes()
            != dispatch.output_bytes
        || dispatch.output_word_count as u64 * 4 != dispatch.output_transfer_bytes
    {
        return Err(PilError::InternalError(
            "GPU native RGB Merge input layout mismatch".into(),
        ));
    }

    let green_owner = bands[1].materialized_shared_for_ops()?;
    let blue_owner = bands[2].materialized_shared_for_ops()?;
    let (DynamicImage::ImageLuma8(green), DynamicImage::ImageLuma8(blue)) =
        (green_owner.as_ref(), blue_owner.as_ref())
    else {
        return Err(PilError::InternalError(
            "GPU native RGB Merge auxiliary bands must stay in L mode".into(),
        ));
    };
    if green.dimensions() != image.dimensions()
        || blue.dimensions() != image.dimensions()
        || green.as_raw().len().checked_add(blue.as_raw().len()) != Some(dispatch.auxiliary_bytes)
    {
        return Err(PilError::InternalError(
            "GPU native RGB Merge auxiliary layout mismatch".into(),
        ));
    }
    Ok(ValidatedNativeRgbMergeInputs {
        red: red.as_raw(),
        green_owner,
        blue_owner,
    })
}

fn validate_native_rgb_merge_limits(
    buffers: &BufferPool,
    dispatch: NativeRgbMergeDispatch,
    limits: &wgpu::Limits,
) -> Result<(), PilError> {
    if dispatch.input_transfer_bytes > u64::from(buffers.capacity) * 4
        || dispatch.output_transfer_bytes > u64::from(buffers.capacity) * 4
        || dispatch.auxiliary_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
        || [
            dispatch.input_transfer_bytes,
            dispatch.auxiliary_transfer_bytes,
            dispatch.output_transfer_bytes,
        ]
        .into_iter()
        .any(|bytes| {
            bytes > u64::from(limits.max_storage_buffer_binding_size)
                || bytes > limits.max_buffer_size
        })
    {
        return Err(PilError::ValueError(
            "GPU native RGB Merge exceeds adapter buffer limits".into(),
        ));
    }
    Ok(())
}

fn mapped_rgb_merge_input_layout(
    dispatch: NativeRgbMergeDispatch,
    limits: &wgpu::Limits,
) -> Option<(u64, u64)> {
    let alignment = u64::from(limits.min_storage_buffer_offset_alignment.max(1));
    dispatch
        .input_transfer_bytes
        .checked_add(alignment - 1)
        .map(|aligned| aligned / alignment * alignment)
        .and_then(|offset| {
            offset
                .checked_add(dispatch.auxiliary_transfer_bytes)
                .map(|end| (offset, end))
        })
        .filter(|(_, total)| *total <= limits.max_buffer_size)
        .filter(|(_, total)| usize::try_from(*total).is_ok())
}

fn upload_native_rgb_merge_inputs(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    inputs: &ValidatedNativeRgbMergeInputs<'_>,
    dispatch: NativeRgbMergeDispatch,
    limits: &wgpu::Limits,
    mapped_input_layout: Option<(u64, u64)>,
    mapped_input: bool,
) -> Result<Option<u64>, PilError> {
    let (DynamicImage::ImageLuma8(green), DynamicImage::ImageLuma8(blue)) =
        (inputs.green_owner.as_ref(), inputs.blue_owner.as_ref())
    else {
        unreachable!("validated RGB Merge planes retain L mode")
    };
    if mapped_input {
        let (offset, total) = mapped_input_layout.expect("mapped layout was admitted");
        gpu.upload_native_rgb_luma_merge_mapped(
            inputs.red,
            green.as_raw(),
            blue.as_raw(),
            &dispatch,
            offset,
            total,
            buffers,
        )?;
        return Ok(Some(offset));
    }

    buffers.img2_arena.ensure_capacity(
        &gpu.device,
        "gpu_merge_native_rgb_luma_auxiliary",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        dispatch.auxiliary_transfer_bytes as usize,
        limits.min_storage_buffer_offset_alignment as usize,
    );
    let write_luma = |buffer: &wgpu::Buffer,
                      transfer_bytes: u64,
                      copy_samples: &mut dyn FnMut(&mut [u8])|
     -> Result<(), PilError> {
        let size = NonZeroU64::new(transfer_bytes).ok_or_else(|| {
            PilError::InternalError("GPU native RGB Merge upload is empty".into())
        })?;
        let mut upload = gpu
            .queue
            .write_buffer_with(buffer, 0, size)
            .ok_or_else(|| {
                PilError::InternalError("GPU native RGB Merge staging allocation failed".into())
            })?;
        let mapped = upload.as_mut();
        mapped.fill(0);
        copy_samples(mapped);
        Ok(())
    };
    write_luma(
        &buffers.buf_a,
        dispatch.input_transfer_bytes,
        &mut |mapped| {
            mapped[..dispatch.input_bytes].copy_from_slice(inputs.red);
        },
    )?;
    write_luma(
        &buffers.img2_arena.buffer,
        dispatch.auxiliary_transfer_bytes,
        &mut |mapped| {
            let green_len = green.as_raw().len();
            mapped[..green_len].copy_from_slice(green.as_raw());
            mapped[green_len..green_len + blue.as_raw().len()].copy_from_slice(blue.as_raw());
        },
    )?;
    Ok(None)
}

fn prepare_native_rgb_merge_bind_group(
    gpu: &GpuInner,
    buffers: &BufferPool,
    dispatch: NativeRgbMergeDispatch,
    mapped_auxiliary_offset: Option<u64>,
) -> Result<(Arc<CachedPipeline>, wgpu::BindGroup), PilError> {
    let parameters = [
        dispatch.invocation_row_width,
        dispatch.output_pixel_groups,
        GPU_NATIVE_RGB_LUMA_MERGE_MODE,
        dispatch.output_word_count,
        dispatch.pixels,
        0,
        0,
        0,
    ];
    let (input_buffer, auxiliary_buffer, input_range, auxiliary_range) =
        if let Some(auxiliary_offset) = mapped_auxiliary_offset {
            let input = &buffers
                .mapped_native_input
                .as_ref()
                .expect("mapped RGB Merge upload owns its input buffer")
                .buffer;
            (
                input,
                input,
                BufferRange {
                    offset: 0,
                    size: dispatch.input_transfer_bytes,
                },
                BufferRange {
                    offset: auxiliary_offset,
                    size: dispatch.auxiliary_transfer_bytes,
                },
            )
        } else {
            (
                &buffers.buf_a,
                &buffers.img2_arena.buffer,
                BufferRange {
                    offset: 0,
                    size: dispatch.input_transfer_bytes,
                },
                BufferRange {
                    offset: 0,
                    size: dispatch.auxiliary_transfer_bytes,
                },
            )
        };
    gpu.queue.write_buffer(
        &buffers.params_arena.buffer,
        0,
        bytemuck::cast_slice(&parameters),
    );
    let cached = gpu.resolve_pipeline(
        "__internal_merge_native_rgb_luma",
        "merge.wgsl",
        include_str!("../shaders/merge.wgsl"),
    )?;
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_native_rgb_luma_merge"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ranged_binding(input_buffer, Some(input_range))?,
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ranged_binding(auxiliary_buffer, Some(auxiliary_range))?,
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
                        size: 32,
                    }),
                )?,
            },
        ],
    });
    Ok((cached, bind_group))
}

fn submit_native_rgb_merge(
    gpu: &GpuInner,
    buffers: &mut BufferPool,
    dispatch: NativeRgbMergeDispatch,
    mapped_input: bool,
    cached: &CachedPipeline,
    bind_group: &wgpu::BindGroup,
) -> Result<DynamicImage, PilError> {
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_native_rgb_luma_merge"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_native_rgb_luma_merge"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&cached.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(dispatch.groups_x) * u64::from(dispatch.groups_y),
        );
        pass.dispatch_workgroups(dispatch.groups_x, dispatch.groups_y, 1);
    }
    let readback = gpu.prepare_readback(&buffers.buf_b, dispatch.output_transfer_bytes)?;
    if let ReadbackTarget::Staging(staging) = &readback {
        encoder.copy_buffer_to_buffer(
            &buffers.buf_b,
            0,
            &staging.buffer,
            0,
            dispatch.output_transfer_bytes,
        );
    }
    gpu.queue.submit(Some(encoder.finish()));
    let result = gpu.readback_to_native_channels(
        dispatch.width,
        dispatch.height,
        3,
        readback.buffer(buffers, false),
    )?;
    crate::compute::record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        upload_bytes: dispatch.input_transfer_bytes + dispatch.auxiliary_transfer_bytes,
        auxiliary_bytes: dispatch.auxiliary_transfer_bytes,
        readback_bytes: dispatch.output_transfer_bytes,
        parameter_bytes: 32,
        retained_cache_bytes: buffers.retained_bytes(),
        full_frame_copy_count: u64::from(!mapped_input) * 3
            + u64::from(matches!(&readback, ReadbackTarget::Staging(_))),
        mode_conversion_count: 0,
        ..PipelineResourceTelemetry::default()
    });
    crate::compute::record_pipeline_dispatch_count(1);
    if let ReadbackTarget::Staging(staging) = readback {
        gpu.recycle_staging(staging);
    }
    Ok(result)
}
