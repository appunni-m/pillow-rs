//! Physical-lane compatibility for compact RGB-family Convert results.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode, RgbImage};

pub(crate) fn preserve_compact_rgb_lane(
    source: &RgbImage,
    target: Mode,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    if !uses_rgb_family_storage(target) {
        return Ok(());
    }
    replace_target_lanes(target, output, source.hidden_pixel_lanes().cloned())
}

pub(crate) fn preserve_luma_alpha_lane(
    source: &GrayAlphaImage,
    target: Mode,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    if !carries_luma_alpha_lane(target) {
        return Ok(());
    }
    // LA-to-RGB and LA-to-HSV expose alpha as the compact output's fourth byte.
    replace_target_lane_values(
        target,
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel[1]),
        output,
    )?;
    Ok(())
}

/// Carries the input X/A byte through Pillow's LAB, YCbCr, and HSV conversions.
pub(crate) fn preserve_fourth_byte(
    source: &[u8],
    width: u32,
    height: u32,
    target: Mode,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    if !carries_fourth_byte_to_color_space_target(target) {
        return Ok(());
    }

    let dimensions = CheckedDims::new_allow_empty(width, height, 4)?;
    let input = source.get(..dimensions.total_bytes()).ok_or_else(|| {
        PilError::InternalError("Convert source did not contain all four-byte pixels".into())
    })?;
    replace_target_lane_values(
        target,
        width,
        height,
        input.chunks_exact(4).map(|pixel| pixel[3]),
        output,
    )?;
    Ok(())
}

fn replace_target_lane_values(
    target: Mode,
    width: u32,
    height: u32,
    lane_values: impl IntoIterator<Item = u8>,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    if !uses_rgb_family_storage(target) {
        return Ok(());
    }
    let dimensions = CheckedDims::new_allow_empty(width, height, 1)?;
    let pixel_count = dimensions.total_pixels();
    let expected = default_hidden_pixel_lanes(target, 0)
        .ok_or_else(|| PilError::InternalError("Convert target has no omitted byte".into()))?[0];
    let mut values = lane_values.into_iter();
    let mut first = None;
    let mut differs_from_default = false;
    let mut varying = None;
    for index in 0..pixel_count {
        let value = values.next().ok_or_else(|| {
            PilError::InternalError("Convert physical-lane source was truncated".into())
        })?;
        differs_from_default |= value != expected;
        match first {
            None => first = Some(value),
            Some(initial) if varying.is_none() && value != initial => {
                let mut per_pixel = Vec::with_capacity(pixel_count);
                per_pixel.resize(index, initial);
                per_pixel.push(value);
                varying = Some(per_pixel);
            }
            Some(_) => {
                if let Some(per_pixel) = &mut varying {
                    per_pixel.push(value);
                }
            }
        }
    }
    if values.next().is_some() {
        return Err(PilError::InternalError(
            "Convert physical-lane source exceeded its image extent".into(),
        ));
    }
    let lanes = if !differs_from_default {
        None
    } else if let Some(values) = varying {
        Some(
            HiddenPixelLanes::per_pixel(1, pixel_count, values).ok_or_else(|| {
                PilError::InternalError("Convert physical-lane extent was invalid".into())
            })?,
        )
    } else {
        let value = first.ok_or_else(|| {
            PilError::InternalError("Convert physical-lane source was unexpectedly empty".into())
        })?;
        Some(
            HiddenPixelLanes::constant(1, pixel_count, [value, 0]).ok_or_else(|| {
                PilError::InternalError("Convert physical-lane extent was invalid".into())
            })?,
        )
    };
    replace_target_lanes(target, output, lanes)
}

fn uses_rgb_family_storage(target: Mode) -> bool {
    matches!(
        target,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8
    )
}

fn carries_fourth_byte_to_color_space_target(target: Mode) -> bool {
    matches!(target, Mode::ImageLab8 | Mode::ImageYCbCr8 | Mode::ImageHsv8)
}

fn carries_luma_alpha_lane(target: Mode) -> bool {
    matches!(target, Mode::ImageRgb8 | Mode::ImageHsv8)
}

fn replace_target_lanes(
    target: Mode,
    output: &mut DynamicImage,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    let has_expected_mode = matches!(
        (target, output),
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(_))
            | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(_))
            | (Mode::ImageLab8, DynamicImage::ImageLab8(_))
            | (Mode::ImageHsv8, DynamicImage::ImageHsv8(_))
    );
    if !has_expected_mode {
        return Err(PilError::InternalError(
            "Convert physical-lane output did not match its target mode".into(),
        ));
    }
    if !output.replace_hidden_pixel_lanes(lanes) {
        return Err(PilError::InternalError(
            "Convert physical-lane output did not match its target extent".into(),
        ));
    }
    Ok(())
}
