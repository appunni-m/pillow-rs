//! Physical lane transfer for compact RGB-family Convert results.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode, RgbImage};

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

pub(crate) fn preserve_fourth_byte(
    source: &[u8],
    width: u32,
    height: u32,
    target: Mode,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    if !uses_rgb_family_storage(target) {
        return Ok(());
    }

    let dimensions = CheckedDims::new_allow_empty(width, height, 4)?;
    let input = source.get(..dimensions.total_bytes()).ok_or_else(|| {
        PilError::InternalError("Convert source did not contain all four-byte pixels".into())
    })?;
    let expected = default_hidden_pixel_lanes(target, 0)
        .ok_or_else(|| PilError::InternalError("Convert target has no omitted byte".into()))?[0];

    let mut first = None;
    let mut differs_from_default = false;
    let mut varying = None;
    for (index, pixel) in input.chunks_exact(4).enumerate() {
        let value = pixel[3];
        differs_from_default |= value != expected;
        match first {
            None => first = Some(value),
            Some(initial) if varying.is_none() && value != initial => {
                let mut values = Vec::with_capacity(dimensions.total_pixels());
                values.resize(index, initial);
                values.push(value);
                varying = Some(values);
            }
            Some(_) => {
                if let Some(values) = &mut varying {
                    values.push(value);
                }
            }
        }
    }

    let lanes = if !differs_from_default {
        None
    } else if let Some(values) = varying {
        Some(
            HiddenPixelLanes::per_pixel(1, dimensions.total_pixels(), values).ok_or_else(|| {
                PilError::InternalError("Convert physical-lane extent was invalid".into())
            })?,
        )
    } else {
        let value = first.ok_or_else(|| {
            PilError::InternalError("Convert physical-lane source was unexpectedly empty".into())
        })?;
        Some(
            HiddenPixelLanes::constant(1, dimensions.total_pixels(), [value, 0]).ok_or_else(
                || PilError::InternalError("Convert physical-lane extent was invalid".into()),
            )?,
        )
    };
    replace_target_lanes(target, output, lanes)
}

fn uses_rgb_family_storage(target: Mode) -> bool {
    matches!(target, Mode::ImageYCbCr8 | Mode::ImageHsv8)
}

fn replace_target_lanes(
    target: Mode,
    output: &mut DynamicImage,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    let has_expected_mode = matches!(
        (target, output),
        (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(_))
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
