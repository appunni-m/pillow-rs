//! Native-byte Contrast blending shared by exact mode strategies.

use crate::raster::GenericImageView;

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Pixel};

pub(crate) fn execute(
    source: &[u8],
    base: &ContrastBase,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    let channels = base.channels;
    if !(1..=4).contains(&channels)
        || base.alpha.is_some_and(|alpha| alpha >= channels)
        || source.len() % channels != 0
    {
        return Err(PilError::InternalError(
            "Contrast source bytes did not match its exact mode".into(),
        ));
    }

    if let Some(lookup) = contrast_lookup(base, factor) {
        let mut output = Vec::with_capacity(source.len());
        for pixel in source.chunks_exact(channels) {
            for (channel, &sample) in pixel.iter().enumerate() {
                output.push(lookup[channel * 256 + usize::from(sample)]);
            }
        }
        return Ok(output);
    }

    let factor = factor as f32;
    let mut output = Vec::with_capacity(source.len());
    for pixel in source.chunks_exact(channels) {
        for (channel, &sample) in pixel.iter().enumerate() {
            if base.alpha == Some(channel) {
                output.push(sample);
            } else {
                output.push(contrast_byte(sample, base.values[channel], factor));
            }
        }
    }
    Ok(output)
}

fn contrast_lookup(base: &ContrastBase, factor: f64) -> Option<Vec<u8>> {
    let factor = factor as f32;
    if !factor.is_finite() {
        return None;
    }
    let mut lookup = Vec::with_capacity(base.channels * 256);
    for channel in 0..base.channels {
        for sample in 0..=u8::MAX {
            if base.alpha == Some(channel) {
                lookup.push(sample);
            } else {
                lookup.push(contrast_byte(sample, base.values[channel], factor));
            }
        }
    }
    Some(lookup)
}

#[inline]
pub(crate) fn contrast_byte(sample: u8, midpoint: u8, factor: f32) -> u8 {
    factor
        .mul_add(f32::from(sample) - f32::from(midpoint), f32::from(midpoint))
        .clamp(0.0, 255.0) as u8
}

/// Applies Contrast's native byte formula to compact omitted pixel lanes.
pub(crate) fn carry_hidden_lanes(
    source: &DynamicImage,
    output: &mut DynamicImage,
    base: &ContrastBase,
    factor: f64,
) -> Result<(), PilError> {
    if source.mode() != output.mode() || source.dimensions() != output.dimensions() {
        return Err(PilError::InternalError(
            "Contrast output did not preserve its source layout".into(),
        ));
    }
    let mode = source.mode();
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    if !matches!(
        mode,
        Mode::ImageLumaA8 | Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageHsv8
    ) {
        return Err(PilError::InternalError(
            "Contrast has no physical-lane strategy for this mode".into(),
        ));
    }
    let pixel_count = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("Contrast lane extent overflow".into()))?;
    let mut midpoints = [0; 2];
    for lane in 0..lane_count {
        let offset = mode.hidden_pixel_lane_offset(lane).ok_or_else(|| {
            PilError::InternalError("Contrast hidden-lane offset was unavailable".into())
        })?;
        midpoints[lane] = base.values[offset];
    }
    if lane_count == 2 && midpoints[0] != midpoints[1] {
        return Err(PilError::InternalError(
            "Contrast hidden lanes require distinct midpoint handling".into(),
        ));
    }

    let alpha = factor as f32;
    let lanes = match source.hidden_pixel_lanes() {
        Some(lanes) if lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count => {
            return Err(PilError::InternalError(
                "Contrast source lane payload did not match its mode".into(),
            ));
        }
        Some(lanes) if alpha == 1.0 => Some(lanes.clone()),
        Some(_) | None if alpha == 0.0 => {
            let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, midpoints)
                .ok_or_else(|| PilError::InternalError("invalid Contrast lane layout".into()))?;
            (pixel_count > 0).then_some(lanes)
        }
        Some(lanes) => Some(lanes.map_values(|sample| contrast_byte(sample, midpoints[0], alpha))),
        None => None,
    };
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Contrast output physical lanes did not match its mode".into(),
        ))
    }
}

pub(crate) fn execute_vector<P, const CHANNELS: usize>(
    source: &ImageBuffer<P, Vec<u8>>,
    base: &ContrastBase,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if !(1..=4).contains(&CHANNELS)
        || base.channels != CHANNELS
        || base.alpha.is_some_and(|alpha| alpha >= CHANNELS)
    {
        return Err(PilError::InternalError(
            "Contrast vector base did not match its exact mode".into(),
        ));
    }
    let lookup = contrast_lookup(base, factor).ok_or_else(|| {
        PilError::InternalError("Contrast vector strategy received a non-finite factor".into())
    })?;
    super::contrast_vector::execute::<P, CHANNELS>(source, &lookup)
}

pub(crate) fn execute_image<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    base: &ContrastBase,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let output = execute(source.as_raw(), base, factor)?;
    ImageBuffer::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("Contrast output dimensions were invalid".into()))
}
