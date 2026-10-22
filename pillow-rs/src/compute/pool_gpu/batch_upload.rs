use super::*;

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
        if options.native_luma16_convert || options.native_luma16_paste {
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
