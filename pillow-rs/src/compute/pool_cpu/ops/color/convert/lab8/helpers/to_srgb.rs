//! LABV2 byte decoding and conversion to the sRGB device space.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::error::PilError;
use crate::raster::{GenericImageView, Mode, RgbImage, RgbaImage};

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
    for (index, pixel) in source.pixels().enumerate() {
        output.extend_from_slice(&lab_to_srgb_pixel(pixel.0));
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

pub(crate) fn srgb_pixels(source: &RgbImage) -> impl Iterator<Item = [u8; 3]> + '_ {
    source.pixels().map(|pixel| lab_to_srgb_pixel(pixel.0))
}

fn auxiliary_byte(
    hidden_lanes: Option<&crate::raster::buffer::HiddenPixelLanes>,
    pixel_index: usize,
) -> u8 {
    hidden_lanes
        .and_then(|lanes| lanes.for_pixel(pixel_index))
        .and_then(|lanes| lanes.first())
        .copied()
        .or_else(|| default_hidden_pixel_lanes(Mode::ImageLab8, pixel_index).map(|values| values[0]))
        .unwrap_or(u8::MAX)
}

#[inline]
pub(crate) fn lab_to_srgb_pixel(lab: [u8; 3]) -> [u8; 3] {
    // LAB's logical A/B bytes store signed values; the hidden fourth byte is
    // carried separately because Pillow's CMS bridge treats it as auxiliary.
    let lightness = f64::from(lab[0]) * (100.0 / 255.0);
    let a = f64::from(i8::from_ne_bytes([lab[1]]));
    let b = f64::from(i8::from_ne_bytes([lab[2]]));
    let fy = (lightness + 16.0) / 116.0;
    let xyz = [
        lab_inverse_curve(fy + a / 500.0) * 0.9642,
        lab_inverse_curve(fy),
        lab_inverse_curve(fy - b / 200.0) * 0.8249,
    ];
    [
        encode_srgb(3.133_856_1 * xyz[0] - 1.616_866_7 * xyz[1] - 0.490_614_6 * xyz[2]),
        encode_srgb(-0.978_768_4 * xyz[0] + 1.916_141_5 * xyz[1] + 0.033_454_0 * xyz[2]),
        encode_srgb(0.071_945_3 * xyz[0] - 0.228_991_4 * xyz[1] + 1.405_242_7 * xyz[2]),
    ]
}

#[inline]
fn lab_inverse_curve(value: f64) -> f64 {
    if value > 6.0 / 29.0 {
        value * value * value
    } else {
        (108.0 / 841.0) * (value - 4.0 / 29.0)
    }
}

#[inline]
fn encode_srgb(linear: f64) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let encoded = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5).floor() as u8
}
