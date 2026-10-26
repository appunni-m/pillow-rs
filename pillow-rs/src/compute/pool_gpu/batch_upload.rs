use super::*;

mod packing;
pub(super) use packing::*;

#[cfg(target_endian = "little")]
#[inline]
fn padded_mapped_input_bytes(bytes: &[u8]) -> Option<u64> {
    u64::try_from(bytes.len())
        .ok()?
        .checked_add(3)
        .map(|length| length & !3)
}

#[derive(Clone, Copy)]
pub(super) struct GpuBatchUploadOptions<'a> {
    pub(super) mode: Option<&'a str>,
    pub(super) native_luma16: bool,
    pub(super) native_luma16_convert: bool,
    pub(super) native_luma16_paste: bool,
    pub(super) full_luma_putdata: bool,
    pub(super) output_only_constant: bool,
    pub(super) packed_la_pad: bool,
    pub(super) packed_native_byte_filter: bool,
    pub(super) packed_luma_colorize: bool,
    pub(super) packed_luma_convert: bool,
    pub(super) native_alpha_convert_input: bool,
    pub(super) packed_luma_transform_input: bool,
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) native_la_transform_output: bool,
    pub(super) native_rgb_compact_input: bool,
    pub(super) native_extract_band_channels: Option<u8>,
    pub(super) native_grayscale_rgb_input: bool,
    pub(super) native_grayscale_cmyk_input: bool,
    pub(super) native_sharpness_l_input: bool,
    pub(super) native_sharpness_la_input: bool,
    pub(super) native_sharpness_la_patch: Option<(usize, [u8; 2])>,
    pub(super) native_sharpness_rgb_input: bool,
    pub(super) native_reduce_rgb_input: bool,
    pub(super) native_expand_channels: Option<u8>,
    pub(super) native_cover_input_channels: Option<u8>,
    pub(super) mapped_native_box_blur_input: bool,
    pub(super) mapped_native_grayscale_input: bool,
}

impl GpuInner {
    pub(super) fn upload_batch_input(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        options: GpuBatchUploadOptions<'_>,
    ) -> Result<(), PilError> {
        if self.upload_batch_source_override(image, buffers, options)?
            || self.upload_batch_packed_input(image, buffers, options)?
            || self.upload_batch_grayscale_input(image, buffers, options)?
        {
            return Ok(());
        }
        self.upload_batch_native_or_standard_input(image, buffers, options)
    }

    fn upload_batch_source_override(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        options: GpuBatchUploadOptions<'_>,
    ) -> Result<bool, PilError> {
        if options.output_only_constant {
            return Ok(true);
        }
        if options.native_luma16 {
            let DynamicImage::ImageLuma16(image) = image else {
                return Err(PilError::InternalError(
                    "GPU typed layout was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16(&self.queue, image, options.mode)?;
            return Ok(true);
        }
        if (options.native_luma16_convert && matches!(image, DynamicImage::ImageLuma16(_)))
            || options.native_luma16_paste
        {
            let DynamicImage::ImageLuma16(image) = image else {
                return Err(PilError::InternalError(
                    "GPU typed operation was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16_numeric(&self.queue, image)?;
            return Ok(true);
        }
        if options.full_luma_putdata {
            return Ok(true);
        }
        Ok(false)
    }

    fn upload_batch_packed_input(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        options: GpuBatchUploadOptions<'_>,
    ) -> Result<bool, PilError> {
        let (width, height) = image.dimensions();
        if options.native_alpha_convert_input {
            buffers.upload_native_channel_bytes(&self.queue, width, height, 2, image.as_bytes())?;
            return Ok(true);
        }
        if options.packed_la_pad {
            buffers.upload_native_channel_bytes(&self.queue, width, height, 2, image.as_bytes())?;
            return Ok(true);
        }
        if options.packed_native_byte_filter && matches!(options.mode, Some("LA")) {
            buffers.upload_native_channel_bytes(&self.queue, width, height, 2, image.as_bytes())?;
            return Ok(true);
        }
        if options.packed_luma_colorize
            || options.packed_luma_convert
            || options.packed_luma_transform_input
            || options.packed_luma_point
            || options.packed_luma_putdata
            || options.packed_luma_order_statistic
            || options.packed_native_byte_filter
            || options.packed_luma_pad
        {
            let DynamicImage::ImageLuma8(image) = image else {
                return Err(PilError::InternalError(
                    "packed-luma GPU input requires native L samples".into(),
                ));
            };
            if options.mapped_native_box_blur_input {
                self.upload_packed_luma8_mapped(image, buffers)?;
            } else {
                buffers.upload_packed_luma8(&self.queue, image)?;
            }
            return Ok(true);
        }
        if options.native_la_transform_output {
            buffers.upload_native_channel_bytes(&self.queue, width, height, 2, image.as_bytes())?;
            return Ok(true);
        }
        if options.native_rgb_compact_input {
            self.upload_batch_native_channels(
                image,
                buffers,
                width,
                height,
                3,
                options.mapped_native_box_blur_input,
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    fn upload_batch_grayscale_input(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        options: GpuBatchUploadOptions<'_>,
    ) -> Result<bool, PilError> {
        let (width, height) = image.dimensions();
        if let Some(channels) = options.native_extract_band_channels {
            buffers.upload_native_channel_bytes(
                &self.queue,
                width,
                height,
                channels,
                image.as_bytes(),
            )?;
            return Ok(true);
        }
        if options.native_grayscale_rgb_input {
            self.upload_batch_native_channels(
                image,
                buffers,
                width,
                height,
                3,
                options.mapped_native_grayscale_input,
            )?;
            return Ok(true);
        }
        if options.native_grayscale_cmyk_input {
            if options.mapped_native_grayscale_input {
                let transfer_bytes = compact_native_channel_transfer_bytes(width, height, 4)?;
                self.upload_native_bytes_mapped(
                    image.as_bytes(),
                    image.as_bytes().len(),
                    transfer_bytes,
                    buffers,
                )?;
            } else {
                buffers.upload_standard_image(&self.queue, image)?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn upload_batch_native_or_standard_input(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        options: GpuBatchUploadOptions<'_>,
    ) -> Result<(), PilError> {
        let (width, height) = image.dimensions();
        if options.native_sharpness_l_input {
            let DynamicImage::ImageLuma8(image) = image else {
                return Err(PilError::InternalError(
                    "native-L Sharpness input requires an ImageLuma8 source".into(),
                ));
            };
            buffers.upload_packed_luma8(&self.queue, image)?;
        } else if options.native_sharpness_la_input {
            buffers.upload_native_channel_bytes_with_la_patch(
                &self.queue,
                width,
                height,
                2,
                image.as_bytes(),
                options.native_sharpness_la_patch,
            )?;
        } else if options.native_sharpness_rgb_input || options.native_reduce_rgb_input {
            buffers.upload_native_channel_bytes(&self.queue, width, height, 3, image.as_bytes())?;
        } else if let Some(channels) = options.native_expand_channels {
            buffers.upload_native_channel_bytes(
                &self.queue,
                width,
                height,
                channels,
                image.as_bytes(),
            )?;
        } else if let Some(channels) = options.native_cover_input_channels {
            buffers.upload_native_channel_bytes(
                &self.queue,
                width,
                height,
                channels,
                image.as_bytes(),
            )?;
        } else {
            buffers.upload_standard_image(&self.queue, image)?;
        }
        Ok(())
    }

    fn upload_batch_native_channels(
        &self,
        image: &DynamicImage,
        buffers: &mut BufferPool,
        width: u32,
        height: u32,
        channels: u8,
        mapped_input: bool,
    ) -> Result<(), PilError> {
        if mapped_input {
            let transfer_bytes = compact_native_channel_transfer_bytes(width, height, channels)?;
            self.upload_native_bytes_mapped(
                image.as_bytes(),
                image.as_bytes().len(),
                transfer_bytes,
                buffers,
            )?;
        } else {
            buffers.upload_native_channel_bytes(
                &self.queue,
                width,
                height,
                channels,
                image.as_bytes(),
            )?;
        }
        Ok(())
    }
}

#[cfg(target_endian = "little")]
impl GpuInner {
    #[cfg(target_endian = "little")]
    pub(super) fn upload_packed_rgb_mapped(
        &self,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        self.upload_native_bytes_mapped(
            image.as_raw(),
            layout.native_bytes,
            layout.transfer_bytes,
            buffers,
        )
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_packed_luma8_mapped(
        &self,
        image: &crate::raster::GrayImage,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        let (width, height) = image.dimensions();
        let native_bytes = CheckedDims::new(width, height, 1)?.total_bytes();
        let transfer_bytes = compact_luma8_transfer_bytes(width, height)?;
        self.upload_native_bytes_mapped(image.as_raw(), native_bytes, transfer_bytes, buffers)
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_native_bytes_mapped(
        &self,
        bytes: &[u8],
        native_bytes: usize,
        transfer_bytes: u64,
        buffers: &mut BufferPool,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback {
            return Err(PilError::InternalError(
                "GPU mapped native upload requires supported shared Metal storage".into(),
            ));
        }
        if bytes.len() != native_bytes
            || transfer_bytes > u64::from(buffers.capacity) * 4
            || (native_bytes as u64).checked_add(3).map(|bytes| bytes & !3) != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(
                "GPU mapped native upload does not fit its checked working set".into(),
            ));
        }
        let size = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::InternalError("GPU mapped input size overflow".into()))?;
        let usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::STORAGE;
        let input = buffers.mapped_native_input.get_or_insert_with(|| {
            ReusableGpuBuffer::new(&self.device, "gpu_mapped_native_input", usage, size, 4)
        });
        input.ensure_capacity(&self.device, "gpu_mapped_native_input", usage, size, 4);
        self.upload_bytes_to_mapped_buffer(
            &input.buffer,
            bytes,
            native_bytes,
            transfer_bytes,
            "GPU mapped native upload",
        )
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_masked_paste_inputs_mapped(
        &self,
        destination: &[u8],
        destination_transfer_bytes: u64,
        source: &[u8],
        source_transfer_bytes: u64,
        mask: &[u8],
        mask_transfer_bytes: u64,
        buffers: &mut BufferPool,
    ) -> Result<u64, PilError> {
        if !self.direct_primary_readback {
            return Err(PilError::InternalError(
                "GPU mapped masked Paste input requires shared Metal storage".into(),
            ));
        }
        if padded_mapped_input_bytes(destination) != Some(destination_transfer_bytes)
            || padded_mapped_input_bytes(source) != Some(source_transfer_bytes)
            || padded_mapped_input_bytes(mask) != Some(mask_transfer_bytes)
        {
            return Err(PilError::InternalError(
                "GPU mapped masked Paste inputs do not match their checked layout".into(),
            ));
        }
        let auxiliary_transfer_bytes = source_transfer_bytes
            .checked_add(mask_transfer_bytes)
            .ok_or_else(|| {
                PilError::ValueError("GPU mapped masked Paste input size overflow".into())
            })?;
        let limits = self.device.limits();
        let binding_alignment = u64::from(limits.min_storage_buffer_offset_alignment.max(4));
        let remainder = destination_transfer_bytes % binding_alignment;
        let alignment_padding = if remainder == 0 {
            0
        } else {
            binding_alignment - remainder
        };
        let auxiliary_offset = destination_transfer_bytes
            .checked_add(alignment_padding)
            .ok_or_else(|| {
                PilError::ValueError("GPU mapped masked Paste offset overflow".into())
            })?;
        let total_transfer_bytes = auxiliary_offset
            .checked_add(auxiliary_transfer_bytes)
            .ok_or_else(|| {
                PilError::ValueError("GPU mapped masked Paste input size overflow".into())
            })?;
        if auxiliary_transfer_bytes == 0
            || destination_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || auxiliary_transfer_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || total_transfer_bytes > limits.max_buffer_size
        {
            return Err(PilError::InternalError(
                "GPU mapped masked Paste inputs exceed adapter limits".into(),
            ));
        }
        let size = usize::try_from(total_transfer_bytes).map_err(|_| {
            PilError::InternalError("GPU mapped masked Paste input size overflow".into())
        })?;
        let usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::STORAGE;
        let input = buffers.mapped_native_input.get_or_insert_with(|| {
            ReusableGpuBuffer::new(
                &self.device,
                "gpu_mapped_native_masked_paste_inputs",
                usage,
                size,
                4,
            )
        });
        input.ensure_capacity(
            &self.device,
            "gpu_mapped_native_masked_paste_inputs",
            usage,
            size,
            4,
        );
        let buffer = &input.buffer;
        self.wait_for_buffer_mapping(
            total_transfer_bytes,
            buffer,
            wgpu::MapMode::Write,
            "GPU mapped masked Paste inputs",
        )?;
        let result = {
            let mut view = buffer.slice(..total_transfer_bytes).get_mapped_range_mut();
            let mapped = view.as_mut();
            if mapped.len() != size {
                Err(PilError::InternalError(
                    "GPU mapped masked Paste inputs have an unexpected size".into(),
                ))
            } else {
                let destination_end = destination_transfer_bytes as usize;
                let auxiliary_start = auxiliary_offset as usize;
                let source_end = auxiliary_start + source_transfer_bytes as usize;
                mapped[..destination.len()].copy_from_slice(destination);
                mapped[destination.len()..destination_end].fill(0);
                mapped[destination_end..auxiliary_start].fill(0);
                mapped[auxiliary_start..auxiliary_start + source.len()].copy_from_slice(source);
                mapped[auxiliary_start + source.len()..source_end].fill(0);
                let mask_start = source_end;
                let mask_data_end = mask_start + mask.len();
                mapped[mask_start..mask_data_end].copy_from_slice(mask);
                mapped[mask_data_end..].fill(0);
                Ok(())
            }
        };
        buffer.unmap();
        result.map(|()| auxiliary_offset)
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_bytes_to_mapped_buffer(
        &self,
        buffer: &wgpu::Buffer,
        bytes: &[u8],
        native_bytes: usize,
        transfer_bytes: u64,
        stage: &str,
    ) -> Result<(), PilError> {
        if !self.direct_primary_readback {
            return Err(PilError::InternalError(
                "GPU mapped native upload requires supported shared Metal storage".into(),
            ));
        }
        if bytes.len() != native_bytes
            || transfer_bytes > buffer.size()
            || (native_bytes as u64).checked_add(3).map(|bytes| bytes & !3) != Some(transfer_bytes)
        {
            return Err(PilError::InternalError(format!(
                "{stage} does not fit its checked mapped buffer"
            )));
        }
        let size = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::InternalError("GPU mapped input size overflow".into()))?;
        // The batch owns this buffer exclusively. Recycled batches reached
        // output map completion, so their input has no outstanding GPU use.
        self.wait_for_buffer_mapping(transfer_bytes, buffer, wgpu::MapMode::Write, stage)?;
        let result = {
            let mut view = buffer.slice(..transfer_bytes).get_mapped_range_mut();
            // BufferViewMut's read dereference logs a slow-read warning even
            // for len(). We only write: obtain the writable slice explicitly
            // while preserving the checked mapping length and view lifetime.
            let mapped = view.as_mut();
            if mapped.len() != size {
                Err(PilError::InternalError(format!(
                    "{stage} has an unexpected mapped byte length"
                )))
            } else {
                mapped[..native_bytes].copy_from_slice(bytes);
                mapped[native_bytes..].fill(0);
                Ok(())
            }
        };
        // Drop the host view and unmap before any shader can reference it,
        // including the defensive error path above.
        buffer.unmap();
        result
    }
}

impl GpuInner {
    pub(super) fn upload_auxiliary_cache(
        &self,
        cache: &GpuAuxiliaryCache,
        buffers: &mut BufferPool,
        storage_alignment: usize,
    ) {
        if !cache.img2_values.is_empty() {
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img2_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img2_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img2_values),
            );
        }
        if !cache.img3_values.is_empty() {
            buffers.img3_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img3",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img3_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img3_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img3_values),
            );
        }
        if !cache.lut_values.is_empty() {
            buffers.lut_arena.ensure_capacity(
                &self.device,
                "gpu_batch_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.lut_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.lut_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.lut_values),
            );
        }
    }
}
