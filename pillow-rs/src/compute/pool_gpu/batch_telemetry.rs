use super::*;

#[derive(Clone, Copy)]
pub(super) struct GpuBatchResourceTelemetryOptions<'a> {
    pub(super) input_dimensions: (u32, u32),
    pub(super) output_dimensions: (u32, u32),
    pub(super) mode: Option<&'a str>,
    pub(super) output_layout: &'a GpuBatchOutputLayout,
    pub(super) output_only_constant: bool,
    pub(super) full_luma_putdata: bool,
    pub(super) native_extract_band_channels: Option<u8>,
    pub(super) native_grayscale_rgb_input: bool,
    pub(super) native_rgb_compact_input: bool,
    pub(super) native_sharpness_l_input: bool,
    pub(super) packed_native_byte_filter: bool,
    pub(super) packed_la_pad: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) native_sharpness_la_input: bool,
    pub(super) native_la_transform_output: bool,
    pub(super) native_sharpness_rgb_input: bool,
    pub(super) native_reduce_rgb_input: bool,
    pub(super) native_expand_channels: Option<u8>,
    pub(super) native_cover_input_channels: Option<u8>,
    pub(super) packed_luma_colorize: bool,
    pub(super) packed_luma_convert: bool,
    pub(super) packed_luma_transform_input: bool,
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) native_luma16_convert: bool,
    pub(super) native_luma16_paste: bool,
    pub(super) native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
    pub(super) native_rgb_packed_output: bool,
    pub(super) packed_luma_transform_output: bool,
    pub(super) native_expand_output: bool,
}

impl GpuBatchResourceTelemetryOptions<'_> {
    fn upload_bytes(self) -> Result<u64, PilError> {
        let (width, height) = self.input_dimensions;
        if self.output_only_constant || self.full_luma_putdata {
            Ok(0)
        } else if let Some(channels) = self.native_extract_band_channels {
            let raw_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU channel input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU channel input is too large".into()))
        } else if self.native_grayscale_rgb_input || self.native_rgb_compact_input {
            let raw_bytes = CheckedDims::new(width, height, 3)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU native RGB input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native RGB input is too large".into()))
        } else if self.native_sharpness_l_input {
            compact_luma8_transfer_bytes(width, height)
        } else if self.packed_native_byte_filter && matches!(self.mode, Some("LA")) {
            let raw_bytes = CheckedDims::new(width, height, 2)?.total_bytes();
            padded_transfer_bytes(raw_bytes)
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or_else(|| PilError::ValueError("GPU native LA input is too large".into()))
        } else if self.packed_native_byte_filter {
            compact_luma8_transfer_bytes(width, height)
        } else if self.packed_la_pad {
            compact_native_channel_transfer_bytes(width, height, 2)
        } else if self.packed_luma_pad {
            compact_luma8_transfer_bytes(width, height)
        } else if self.native_sharpness_la_input || self.native_la_transform_output {
            let raw_bytes = CheckedDims::new(width, height, 2)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU native LA input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native LA input is too large".into()))
        } else if self.native_sharpness_rgb_input || self.native_reduce_rgb_input {
            let raw_bytes = CheckedDims::new(width, height, 3)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU native RGB input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU native RGB input is too large".into()))
        } else if let Some(channels) = self.native_expand_channels {
            let raw_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU Expand input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU Expand input is too large".into()))
        } else if let Some(channels) = self.native_cover_input_channels {
            let raw_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
            let transfer_bytes = padded_transfer_bytes(raw_bytes)
                .ok_or_else(|| PilError::ValueError("GPU Cover input is too large".into()))?;
            u64::try_from(transfer_bytes)
                .map_err(|_| PilError::ValueError("GPU Cover input is too large".into()))
        } else if self.packed_luma_colorize
            || self.packed_luma_convert
            || self.packed_luma_transform_input
            || self.packed_luma_point
            || self.packed_luma_putdata
            || self.packed_luma_order_statistic
            || self.packed_native_byte_filter
            || self.packed_luma_pad
            || self.native_sharpness_l_input
        {
            compact_luma8_transfer_bytes(width, height)
        } else {
            Ok(CheckedDims::new(width, height, 4)?.total_bytes() as u64)
        }
    }

    fn readback_bytes(self) -> Result<u64, PilError> {
        let (width, height) = self.output_dimensions;
        if let Some(dispatch) = self.native_cover_packed_output {
            Ok(dispatch.transfer_bytes)
        } else if self.native_rgb_packed_output {
            u64::try_from(CheckedDims::new(width, height, 3)?.total_bytes())
                .map_err(|_| PilError::ValueError("GPU native RGB readback is too large".into()))?
                .div_ceil(4)
                .checked_mul(4)
                .ok_or_else(|| PilError::ValueError("GPU native RGB readback size overflow".into()))
        } else if self.packed_la_pad || self.native_la_transform_output {
            u64::try_from(CheckedDims::new(width, height, 2)?.total_bytes())
                .map_err(|_| PilError::ValueError("GPU native LA readback is too large".into()))?
                .div_ceil(4)
                .checked_mul(4)
                .ok_or_else(|| PilError::ValueError("GPU native LA readback size overflow".into()))
        } else if self.packed_native_byte_filter && matches!(self.mode, Some("LA")) {
            let raw_bytes = CheckedDims::new(width, height, 2)?.total_bytes();
            u64::try_from(
                raw_bytes
                    .div_ceil(std::mem::size_of::<u32>())
                    .checked_mul(std::mem::size_of::<u32>())
                    .ok_or_else(|| {
                        PilError::ValueError("GPU native LA readback size overflow".into())
                    })?,
            )
            .map_err(|_| PilError::ValueError("GPU native LA readback size overflow".into()))
        } else if self.native_expand_output {
            let channels = self.native_expand_channels.ok_or_else(|| {
                PilError::InternalError("GPU native Expand output is missing its channels".into())
            })?;
            let native_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
            u64::try_from(
                native_bytes
                    .div_ceil(std::mem::size_of::<u32>())
                    .checked_mul(std::mem::size_of::<u32>())
                    .ok_or_else(|| {
                        PilError::ValueError("GPU native image readback size overflow".into())
                    })?,
            )
            .map_err(|_| PilError::ValueError("GPU native image readback size overflow".into()))
        } else if self.output_layout.native_luma8_extract
            || self.output_layout.native_luma8_grayscale
            || self.output_layout.native_luma8_constant
            || self.packed_luma_transform_output
            || self.packed_luma_point
            || self.packed_luma_putdata
            || self.packed_luma_order_statistic
            || self.packed_native_byte_filter
            || self.packed_luma_pad
            || self.native_sharpness_l_input
        {
            compact_luma8_transfer_bytes(width, height)
        } else {
            Ok(CheckedDims::new(width, height, 4)?.total_bytes() as u64)
        }
    }

    fn mode_conversion_count(self, image: &DynamicImage) -> u64 {
        if self.output_only_constant
            || self.output_layout.native_luma8_constant
            || self.packed_luma_convert
            || self.packed_luma_transform_input
            || self.packed_luma_order_statistic
            || self.packed_native_byte_filter
            || self.packed_luma_pad
            || self.packed_la_pad
            || self.native_rgb_compact_input
            || self.native_la_transform_output
            || self.native_grayscale_rgb_input
            || self.native_sharpness_l_input
            || self.native_sharpness_la_input
            || self.native_sharpness_rgb_input
            || self.native_reduce_rgb_input
            || self.native_expand_channels.is_some()
            || self.native_cover_input_channels.is_some()
        {
            // Native upload avoids source widening; RGBA readback narrowing is
            // accounted at the output boundary.
            0
        } else {
            u64::from(
                self.native_extract_band_channels.is_none()
                    && !self.native_grayscale_rgb_input
                    && !self.native_sharpness_l_input
                    && !self.native_sharpness_la_input
                    && !self.native_sharpness_rgb_input
                    && !self.native_reduce_rgb_input
                    && !self.packed_luma_point
                    && !self.packed_luma_putdata
                    && !self.packed_luma_order_statistic
                    && !self.packed_native_byte_filter
                    && !self.packed_luma_pad
                    && !self.packed_la_pad
                    && (self.native_luma16_convert
                        || self.native_luma16_paste
                        || !matches!(
                            image,
                            DynamicImage::ImageRgba8(_) | DynamicImage::ImageLuma16(_)
                        )),
            )
        }
    }
}

pub(super) fn gpu_batch_resource_telemetry(
    mut telemetry: PipelineResourceTelemetry,
    buffers: &BufferPool,
    readback: &ReadbackTarget,
    image: &DynamicImage,
    options: GpuBatchResourceTelemetryOptions<'_>,
) -> Result<PipelineResourceTelemetry, PilError> {
    telemetry.upload_bytes = options.upload_bytes()?;
    telemetry.readback_bytes = options.readback_bytes()?;
    telemetry.retained_cache_bytes = buffers.retained_bytes();
    telemetry.full_frame_copy_count = readback.full_frame_copy_count(!options.output_only_constant);
    telemetry.mode_conversion_count = options.mode_conversion_count(image);
    Ok(telemetry)
}

fn padded_transfer_bytes(raw_bytes: usize) -> Option<usize> {
    raw_bytes
        .div_ceil(std::mem::size_of::<u32>())
        .checked_mul(std::mem::size_of::<u32>())
}
