//! LABV2 byte decoding and conversion to the sRGB device space.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::error::PilError;
use crate::raster::{Mode, RgbImage, RgbaImage};

pub(crate) fn convert_to_rgb(source: &RgbImage) -> Result<RgbImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    let mut output = Vec::with_capacity(dimensions.total_bytes());
    for pixel in srgb_pixels(source) {
        output.extend_from_slice(&pixel);
    }
    if output.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "LAB to RGB output shape mismatch".into(),
        ));
    }
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("LAB to RGB output shape mismatch".into()))
}

pub(crate) fn convert_to_four_byte(source: &RgbImage) -> Result<RgbaImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let hidden_lanes = source.hidden_pixel_lanes();
    if let Some(lanes) = hidden_lanes
        && (lanes.lane_count() != 1 || lanes.pixel_count() != dimensions.total_pixels())
    {
        return Err(PilError::InternalError(
            "LAB source auxiliary lane did not match its image extent".into(),
        ));
    }

    let mut output = Vec::with_capacity(dimensions.total_bytes());
    for (index, pixel) in srgb_pixels(source).enumerate() {
        output.extend_from_slice(&pixel);
        output.push(auxiliary_byte(hidden_lanes, index));
    }
    if output.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "LAB to four-byte RGB output shape mismatch".into(),
        ));
    }
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("LAB to four-byte RGB output shape mismatch".into()))
}

pub(crate) fn srgb_pixels(source: &RgbImage) -> impl ExactSizeIterator<Item = [u8; 3]> + '_ {
    crate::compute::pool_cpu::ops::color::helpers::lab_to_srgb::srgb_pixels(source)
}

fn auxiliary_byte(
    hidden_lanes: Option<&crate::raster::buffer::HiddenPixelLanes>,
    pixel_index: usize,
) -> u8 {
    hidden_lanes
        .and_then(|lanes| lanes.for_pixel(pixel_index))
        .and_then(|lanes| lanes.first())
        .copied()
        .or_else(|| default_hidden_pixel_lanes(Mode::ImageLab8, 0).map(|values| values[0]))
        .unwrap_or(u8::MAX)
}
