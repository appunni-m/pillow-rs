//! Physical storage lanes omitted by compact ImageOps result carriers.

use crate::error::PilError;
use crate::pipeline::ExpandMargins;
use crate::raster::buffer::{HiddenPixelLanes, ImageBuffer};
use crate::raster::{DynamicImage, Mode, Pixel};

/// Records omitted physical lanes left zero by Pillow's new point result.
///
/// Pillow 12.2 allocates an initialized output in `ImagingPoint`, then its
/// two- and three-band handlers write only their mode-defined lanes. Keep the
/// omitted lanes at zero for later raw-byte operations such as Paste.
pub(crate) fn initialize_point_output_lanes(
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    let Some(lane_count) = output.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = (output.width() as usize)
        .checked_mul(output.height() as usize)
        .ok_or_else(|| PilError::DimensionError("ImageOps pixel count overflow".into()))?;
    if pixel_count == 0 {
        return Ok(());
    }
    let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2])
        .ok_or_else(|| PilError::InternalError("invalid ImageOps point lane layout".into()))?;
    if output.replace_hidden_pixel_lanes(Some(lanes)) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "ImageOps point output lane shape mismatch".into(),
        ))
    }
}

/// Composes compact physical lanes for ImageOps.expand's copied and fill areas.
pub(crate) fn compose_expand_lanes<P: Pixel<Subpixel = u8>>(
    mode: Mode,
    source: &ImageBuffer<P, Vec<u8>>,
    output: &mut DynamicImage,
    border: ExpandMargins,
    fill: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    if output.mode() != mode {
        return Err(PilError::InternalError(
            "ImageOps Expand changed its exact mode".into(),
        ));
    }
    let output_pixels = pixel_count(output.width(), output.height())?;
    if output_pixels == 0 {
        return replace_lanes(output, None);
    }
    let fill_lanes = expand_fill_lanes(mode, fill)?;
    if border.uniform_nonnegative() == Some(0) {
        return replace_lanes(output, source.hidden_pixel_lanes().cloned());
    }

    let source_pixels = pixel_count(source.width(), source.height())?;
    if !source_overlaps_output(source.width(), source.height(), output.width(), output.height(), border) {
        return replace_lanes(output, constant_lanes(lane_count, output_pixels, fill_lanes)?);
    }
    if uniform_source_lanes(source, lane_count, source_pixels)
        .is_some_and(|source_lanes| source_lanes[..lane_count] == fill_lanes[..lane_count])
    {
        return replace_lanes(output, constant_lanes(lane_count, output_pixels, fill_lanes)?);
    }

    let channels = usize::from(P::CHANNEL_COUNT);
    let source_len = source_pixels
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("Expand source lane size overflow".into()))?;
    if channels == 0 || source.as_raw().len() < source_len {
        return Err(PilError::InternalError(
            "Expand source lane carrier is shorter than its dimensions".into(),
        ));
    }
    let capacity = output_pixels
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("Expand output lane size overflow".into()))?;
    let mut values = Vec::with_capacity(capacity);
    for output_y in 0..output.height() {
        let source_y = i128::from(output_y) - i128::from(border.top);
        for output_x in 0..output.width() {
            let source_x = i128::from(output_x) - i128::from(border.left);
            if (0..i128::from(source.width())).contains(&source_x)
                && (0..i128::from(source.height())).contains(&source_y)
            {
                let index = source_y as usize * source.width() as usize + source_x as usize;
                append_source_lanes(source, index, channels, lane_count, &mut values);
            } else {
                values.extend_from_slice(&fill_lanes[..lane_count]);
            }
        }
    }
    let lanes = HiddenPixelLanes::per_pixel(lane_count, output_pixels, values)
        .ok_or_else(|| PilError::InternalError("invalid Expand lane output".into()))?;
    replace_lanes(output, Some(lanes))
}

fn pixel_count(width: u32, height: u32) -> Result<usize, PilError> {
    (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::DimensionError("ImageOps pixel count overflow".into()))
}

fn expand_fill_lanes(mode: Mode, fill: (u8, u8, u8, u8)) -> Result<[u8; 2], PilError> {
    match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            Ok([fill.1, fill.2])
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            Ok([fill.3, 0])
        }
        _ => Err(PilError::InternalError(
            "Expand mode declares an unknown hidden-lane layout".into(),
        )),
    }
}

fn constant_lanes(
    lane_count: usize,
    pixel_count: usize,
    values: [u8; 2],
) -> Result<Option<HiddenPixelLanes>, PilError> {
    HiddenPixelLanes::constant(lane_count, pixel_count, values)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("invalid constant Expand lanes".into()))
}

fn source_overlaps_output(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    border: ExpandMargins,
) -> bool {
    let left = i128::from(border.left).max(0);
    let top = i128::from(border.top).max(0);
    let right = (i128::from(border.left) + i128::from(source_width))
        .min(i128::from(output_width));
    let bottom = (i128::from(border.top) + i128::from(source_height))
        .min(i128::from(output_height));
    left < right && top < bottom
}

fn uniform_source_lanes<P: Pixel<Subpixel = u8>>(
    source: &ImageBuffer<P, Vec<u8>>,
    lane_count: usize,
    source_pixels: usize,
) -> Option<[u8; 2]> {
    if source_pixels == 0 {
        return None;
    }
    if let Some(lanes) = source.hidden_pixel_lanes() {
        return lanes
            .is_constant()
            .then(|| lanes.for_pixel(0))
            .flatten()
            .and_then(|values| values.get(..lane_count))
            .map(|values| {
                let mut result = [0; 2];
                result[..lane_count].copy_from_slice(values);
                result
            });
    }
    (lane_count == 1).then_some([u8::MAX, 0])
}

fn append_source_lanes<P: Pixel<Subpixel = u8>>(
    source: &ImageBuffer<P, Vec<u8>>,
    index: usize,
    channels: usize,
    lane_count: usize,
    output: &mut Vec<u8>,
) {
    let logical = &source.as_raw()[index * channels..(index + 1) * channels];
    let fallback = if lane_count == 2 {
        [logical[0], logical[0]]
    } else {
        [u8::MAX, 0]
    };
    let retained = source.hidden_pixel_lanes().and_then(|lanes| lanes.for_pixel(index));
    for lane in 0..lane_count {
        output.push(
            retained
                .and_then(|values| values.get(lane))
                .copied()
                .unwrap_or(fallback[lane]),
        );
    }
}

fn replace_lanes(
    output: &mut DynamicImage,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "ImageOps Expand lane shape mismatch".into(),
        ))
    }
}
