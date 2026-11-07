//! Pillow's native fixed-point AlphaComposite pixel arithmetic.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{GrayAlphaImage, Pixel};

pub(crate) fn apply<const CHANNELS: usize>(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
    mode_name: &'static str,
) -> Result<Vec<u8>, PilError> {
    let channels = u8::try_from(CHANNELS)
        .map_err(|_| PilError::InternalError("AlphaComposite channel count is invalid".into()))?;
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(format!(
            "{mode_name} AlphaComposite source storage did not match its dimensions"
        )));
    }

    let mut output = destination.to_vec();
    for (source_pixel, destination_pixel) in source
        .chunks_exact(CHANNELS)
        .zip(output.chunks_exact_mut(CHANNELS))
    {
        alpha_composite_pixel::<CHANNELS>(source_pixel, destination_pixel);
    }
    Ok(output)
}

/// Carries Pillow's LA physical color lanes through AlphaComposite.
///
/// Pillow views LA rows through the same four-byte pixel layout used for RGBA.
/// Its typed Rust carrier stores only luma and alpha, so the two intermediate
/// color lanes must follow the same compositing arithmetic as the visible
/// luma channel.
pub(crate) fn preserve_luma_alpha_lanes(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
    output: &mut GrayAlphaImage,
) -> Result<(), PilError> {
    let pixel_count = validate_luma_alpha_lane_shapes(source, destination, output)?;
    let source_lanes = source.hidden_pixel_lanes();
    let destination_lanes = destination.hidden_pixel_lanes();
    if input_lanes_match_luma(
        source,
        destination,
        source_lanes,
        destination_lanes,
    )? {
        return Ok(());
    }

    let capacity = pixel_count
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("AlphaComposite lane extent overflow".into()))?;
    let mut lanes = Vec::with_capacity(capacity);
    let mut output_lanes_match_luma = true;
    for (index, ((source_pixel, destination_pixel), output_pixel)) in source
        .pixels()
        .zip(destination.pixels())
        .zip(output.pixels())
        .enumerate()
    {
        let source_channels = source_pixel.channels();
        let destination_channels = destination_pixel.channels();
        let output_channels = output_pixel.channels();
        let source_hidden = luma_alpha_lanes_for_pixel(
            source_lanes,
            index,
            source_channels[0],
        )?;
        let destination_hidden = luma_alpha_lanes_for_pixel(
            destination_lanes,
            index,
            destination_channels[0],
        )?;
        let source_pixel = [
            source_hidden[0],
            source_hidden[1],
            source_channels[1],
        ];
        let mut destination_pixel = [
            destination_hidden[0],
            destination_hidden[1],
            destination_channels[1],
        ];
        alpha_composite_pixel::<3>(&source_pixel, &mut destination_pixel);
        output_lanes_match_luma &= destination_pixel[0] == output_channels[0]
            && destination_pixel[1] == output_channels[0];
        lanes.extend_from_slice(&destination_pixel[..2]);
    }

    let hidden_lanes = if output_lanes_match_luma {
        None
    } else {
        Some(
            HiddenPixelLanes::per_pixel(2, pixel_count, lanes).ok_or_else(|| {
                PilError::InternalError("AlphaComposite lane storage was invalid".into())
            })?,
        )
    };
    if !output.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "AlphaComposite output lanes did not match its dimensions".into(),
        ));
    }
    Ok(())
}

fn validate_luma_alpha_lane_shapes(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
    output: &GrayAlphaImage,
) -> Result<usize, PilError> {
    if source.dimensions() != destination.dimensions()
        || source.dimensions() != output.dimensions()
    {
        return Err(PilError::InternalError(
            "AlphaComposite lane images did not match".into(),
        ));
    }
    let (width, height) = source.dimensions();
    let pixel_count = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("AlphaComposite lane extent overflow".into()))?;
    for lanes in [source.hidden_pixel_lanes(), destination.hidden_pixel_lanes()]
        .into_iter()
        .flatten()
    {
        if lanes.lane_count() != 2 || lanes.pixel_count() != pixel_count {
            return Err(PilError::InternalError(
                "AlphaComposite input lanes did not match its dimensions".into(),
            ));
        }
    }
    Ok(pixel_count)
}

fn input_lanes_match_luma(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
    source_lanes: Option<&HiddenPixelLanes>,
    destination_lanes: Option<&HiddenPixelLanes>,
) -> Result<bool, PilError> {
    if source_lanes.is_none() && destination_lanes.is_none() {
        return Ok(true);
    }
    for (index, (source_pixel, destination_pixel)) in source
        .pixels()
        .zip(destination.pixels())
        .enumerate()
    {
        if !pixel_lanes_match_luma(source_lanes, index, source_pixel.channels()[0])?
            || !pixel_lanes_match_luma(
                destination_lanes,
                index,
                destination_pixel.channels()[0],
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn pixel_lanes_match_luma(
    lanes: Option<&HiddenPixelLanes>,
    index: usize,
    luma: u8,
) -> Result<bool, PilError> {
    let Some(lanes) = lanes else {
        return Ok(true);
    };
    let values = lanes.for_pixel(index).ok_or_else(|| {
        PilError::InternalError("AlphaComposite input lane index was invalid".into())
    })?;
    Ok(values[0] == luma && values[1] == luma)
}

fn luma_alpha_lanes_for_pixel(
    lanes: Option<&HiddenPixelLanes>,
    index: usize,
    luma: u8,
) -> Result<[u8; 2], PilError> {
    let Some(lanes) = lanes else {
        return Ok([luma, luma]);
    };
    let values = lanes.for_pixel(index).ok_or_else(|| {
        PilError::InternalError("AlphaComposite input lane index was invalid".into())
    })?;
    Ok([values[0], values[1]])
}

#[inline]
pub(crate) fn alpha_composite_pixel<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
) {
    let source_alpha = u32::from(source[CHANNELS - 1]);
    if source_alpha == 0 {
        return;
    }

    let destination_alpha = u32::from(destination[CHANNELS - 1]);
    let blend = destination_alpha * (255 - source_alpha);
    let output_alpha_255 = source_alpha * 255 + blend;
    let source_coefficient = source_alpha * 255 * 255 * (1 << 7) / output_alpha_255;
    let destination_coefficient = (255 << 7) - source_coefficient;
    for channel in 0..CHANNELS - 1 {
        let blended = u32::from(source[channel]) * source_coefficient
            + u32::from(destination[channel]) * destination_coefficient;
        destination[channel] = (shift_for_div255(blended + (0x80 << 7)) >> 7) as u8;
    }
    destination[CHANNELS - 1] = shift_for_div255(output_alpha_255 + 0x80) as u8;
}

#[inline]
fn shift_for_div255(value: u32) -> u32 {
    ((value >> 8) + value) >> 8
}
