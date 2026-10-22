use super::*;

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeRgbMergeDispatch {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: u32,
    pub(super) input_bytes: usize,
    pub(super) auxiliary_bytes: usize,
    pub(super) output_bytes: usize,
    pub(super) input_transfer_bytes: u64,
    pub(super) auxiliary_transfer_bytes: u64,
    pub(super) output_transfer_bytes: u64,
    pub(super) output_word_count: u32,
    pub(super) output_pixel_groups: u32,
    pub(super) invocation_row_width: u32,
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) buffer_words: u32,
}

/// Plan a compact three-L-to-RGB merge. Native L planes and RGB output use
/// four-byte padded transfers; 2D dispatch tiling keeps the per-axis device
/// limit valid even when output groups exceed one axis.
#[cfg(target_endian = "little")]
pub(super) fn plan_gpu_native_rgb_merge(
    width: u32,
    height: u32,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeRgbMergeDispatch> {
    if width == 0 || height == 0 || max_workgroups == 0 {
        return None;
    }
    let dimensions = CheckedDims::new(width, height, 1).ok()?;
    let pixels = dimensions.total_pixels();
    let input_bytes = pixels;
    let auxiliary_bytes = pixels.checked_mul(2)?;
    let output_bytes = pixels.checked_mul(3)?;
    if pixels > GPU_BUFFER_CAPACITY as usize
        || u32::try_from(pixels).is_err()
        || u32::try_from(auxiliary_bytes).is_err()
        || u32::try_from(output_bytes).is_err()
    {
        return None;
    }
    let transfer = |bytes: usize| {
        u64::try_from(bytes)
            .ok()?
            .checked_add(3)
            .map(|aligned| aligned & !3)
    };
    let input_transfer_bytes = transfer(input_bytes)?;
    let auxiliary_transfer_bytes = transfer(auxiliary_bytes)?;
    let output_transfer_bytes = transfer(output_bytes)?;
    let input_words = u32::try_from(input_transfer_bytes / 4).ok()?;
    let auxiliary_words = u32::try_from(auxiliary_transfer_bytes / 4).ok()?;
    let output_word_count = u32::try_from(output_transfer_bytes / 4).ok()?;
    let output_pixels = u32::try_from(pixels).ok()?;
    let output_pixel_groups = output_pixels.div_ceil(4);
    let required_workgroups = output_pixel_groups.div_ceil(256);
    if required_workgroups == 0 {
        return None;
    }
    let groups_x = required_workgroups.min(max_workgroups);
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > max_workgroups {
        return None;
    }
    let invocation_row_width = groups_x.checked_mul(16)?;
    let buffer_words = input_words.max(output_word_count);
    let buffer_bytes = u64::from(buffer_words).checked_mul(4)?;
    let binding_limit = u64::from(max_storage_binding_bytes);
    if buffer_words == 0
        || buffer_words > GPU_BUFFER_CAPACITY
        || auxiliary_words > GPU_BUFFER_CAPACITY
        || buffer_bytes > max_buffer_bytes
        || [
            input_transfer_bytes,
            auxiliary_transfer_bytes,
            output_transfer_bytes,
        ]
        .into_iter()
        .any(|bytes| bytes > binding_limit || bytes > max_buffer_bytes)
    {
        return None;
    }
    Some(NativeRgbMergeDispatch {
        width,
        height,
        pixels: output_pixels,
        input_bytes,
        auxiliary_bytes,
        output_bytes,
        input_transfer_bytes,
        auxiliary_transfer_bytes,
        output_transfer_bytes,
        output_word_count,
        output_pixel_groups,
        invocation_row_width,
        groups_x,
        groups_y,
        buffer_words,
    })
}

/// Admit the exact native L,L,L -> RGB merge layout. The compact kernel reads
/// three independent L planes and writes three-byte RGB, so other band modes,
/// logical modes, and operation sequences keep the established shader route.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_rgb_luma_merge_bands<'a>(
    ops: &'a [PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Option<&'a [crate::image::Image]> {
    if !matches!(mode, None | Some("L")) || !matches!(image, DynamicImage::ImageLuma8(_)) {
        return None;
    }
    let [
        PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode,
            bands,
        },
    ] = ops
    else {
        return None;
    };
    let dimensions = image.dimensions();
    (logical_mode == "RGB"
        && bands.len() == 3
        && bands.iter().all(|band| {
            band.size().ok() == Some(dimensions) && band.mode().ok().as_deref() == Some("L")
        }))
    .then_some(bands.as_ref())
}

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
            || green.as_raw().len().checked_add(blue.as_raw().len())
                != Some(dispatch.auxiliary_bytes)
        {
            return Err(PilError::InternalError(
                "GPU native RGB Merge auxiliary layout mismatch".into(),
            ));
        }

        let limits = self.device.limits();
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

        let auxiliary_offset_alignment =
            u64::from(limits.min_storage_buffer_offset_alignment.max(1));
        let mapped_auxiliary_offset = dispatch
            .input_transfer_bytes
            .checked_add(auxiliary_offset_alignment - 1)
            .map(|aligned| aligned / auxiliary_offset_alignment * auxiliary_offset_alignment);
        let mapped_input_layout = mapped_auxiliary_offset
            .and_then(|offset| {
                offset
                    .checked_add(dispatch.auxiliary_transfer_bytes)
                    .map(|end| (offset, end))
            })
            .filter(|(_, total)| *total <= limits.max_buffer_size)
            .filter(|(_, total)| usize::try_from(*total).is_ok());
        let mapped_input = self.direct_primary_readback && mapped_input_layout.is_some();
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_merge_native_rgb_luma_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            32,
            limits.min_uniform_buffer_offset_alignment as usize,
        );

        let mapped_auxiliary_offset = if mapped_input {
            let (offset, total) = mapped_input_layout.expect("mapped layout was admitted");
            self.upload_native_rgb_luma_merge_mapped(
                red.as_raw(),
                green.as_raw(),
                blue.as_raw(),
                &dispatch,
                offset,
                total,
                buffers,
            )?;
            Some(offset)
        } else {
            buffers.img2_arena.ensure_capacity(
                &self.device,
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
                let mut upload =
                    self.queue
                        .write_buffer_with(buffer, 0, size)
                        .ok_or_else(|| {
                            PilError::InternalError(
                                "GPU native RGB Merge staging allocation failed".into(),
                            )
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
                    mapped[..dispatch.input_bytes].copy_from_slice(red.as_raw());
                },
            )?;
            write_luma(
                &buffers.img2_arena.buffer,
                dispatch.auxiliary_transfer_bytes,
                &mut |mapped| {
                    let green_len = green.as_raw().len();
                    mapped[..green_len].copy_from_slice(green.as_raw());
                    mapped[green_len..green_len + blue.as_raw().len()]
                        .copy_from_slice(blue.as_raw());
                },
            )?;
            None
        };

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
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&parameters),
        );
        let cached = self.resolve_pipeline(
            "__internal_merge_native_rgb_luma",
            "merge.wgsl",
            include_str!("shaders/merge.wgsl"),
        )?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        let mut encoder = self
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
        let result = self.readback_to_native_channels(
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
            self.recycle_staging(staging);
        }
        Ok(result)
    }
}
