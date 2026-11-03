//! Omitted physical-lane composition for Pad borders.

use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    default_hidden_lanes_for_bytes, default_hidden_pixel_lanes,
};
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, buffer::HiddenPixelLanes};

/// Carries known source lanes through a Pad whose content is not resampled.
pub(crate) fn preserve_same_size_content(
    source: &DynamicImage,
    output: &mut DynamicImage,
    frame: PadFrame,
) -> Result<(), PilError> {
    if output.mode() != source.mode() {
        return Err(PilError::InternalError(
            "Pad output changed the source mode while preserving physical lanes".into(),
        ));
    }
    if (source.width(), source.height()) != (frame.content_width, frame.content_height) {
        return Ok(());
    }

    let mode = source.mode();
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::DimensionError("Pad lane source width overflow".into()))?;
    let source_height = usize::try_from(source.height())
        .map_err(|_| PilError::DimensionError("Pad lane source height overflow".into()))?;
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| PilError::DimensionError("Pad lane source extent overflow".into()))?;
    if source.hidden_pixel_lanes().is_some_and(|source_lanes| {
        source_lanes.lane_count() != lane_count || source_lanes.pixel_count() != source_pixels
    }) {
        return Err(PilError::InternalError(
            "Pad source physical lanes did not match the source image".into(),
        ));
    }

    let output_width = usize::try_from(frame.width)
        .map_err(|_| PilError::DimensionError("Pad lane output width overflow".into()))?;
    let output_height = usize::try_from(frame.height)
        .map_err(|_| PilError::DimensionError("Pad lane output height overflow".into()))?;
    let output_pixels = output_width
        .checked_mul(output_height)
        .ok_or_else(|| PilError::DimensionError("Pad lane output extent overflow".into()))?;
    let fill = fill_lanes(mode, frame.color);
    if output_pixels == 0 {
        return match source.hidden_pixel_lanes() {
            Some(source_lanes) => {
                let lanes = source_lanes
                    .remap(0, |_| usize::MAX)
                    .ok_or_else(invalid_lane_output)?;
                replace_lanes(output, Some(lanes))
            }
            None => replace_lanes(output, None),
        };
    }
    if is_borderless(frame) {
        return replace_lanes(output, source.hidden_pixel_lanes().cloned());
    }
    if source.hidden_pixel_lanes().is_none()
        && default_hidden_pixel_lanes(mode, frame.color.map_or(0, |color| color.0))
            .is_some_and(|default| default[..lane_count] == fill[..lane_count])
    {
        return replace_lanes(output, None);
    }
    if source_pixels == 0 {
        let lanes = HiddenPixelLanes::constant(lane_count, output_pixels, fill)
            .ok_or_else(invalid_lane_output)?;
        return replace_lanes(output, Some(lanes));
    }
    let fallback_lanes = if source.hidden_pixel_lanes().is_none() {
        default_hidden_lanes_for_bytes(mode, source.as_bytes(), source_pixels)?
    } else {
        None
    };
    let source_lanes = source
        .hidden_pixel_lanes()
        .or(fallback_lanes.as_ref())
        .ok_or_else(invalid_lane_output)?;
    let lanes = if source_lanes.is_constant()
        && source_lanes.for_pixel(0) == Some(&fill[..lane_count])
    {
        HiddenPixelLanes::constant(lane_count, output_pixels, fill)
            .ok_or_else(invalid_lane_output)?
    } else {
        compose_lanes(
            source_lanes,
            lane_count,
            (source_width, source_height),
            (output_width, output_height),
            (frame.offset_x, frame.offset_y),
            fill,
        )?
    };
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(PilError::InternalError(
            "Pad output rejected its physical lane payload".into(),
        ));
    }
    Ok(())
}

fn fill_lanes(mode: Mode, color: Option<(u8, u8, u8, u8)>) -> [u8; 2] {
    match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            color.map_or([0, 0], |color| [color.1, color.2])
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            color.map_or([0, 0], |color| [color.3, 0])
        }
        _ => [0, 0],
    }
}

fn is_borderless(frame: PadFrame) -> bool {
    frame.offset_x == 0
        && frame.offset_y == 0
        && frame.content_width == frame.width
        && frame.content_height == frame.height
}

fn compose_lanes(
    source: &HiddenPixelLanes,
    lane_count: usize,
    source_dimensions: (usize, usize),
    output_dimensions: (usize, usize),
    offset: (i64, i64),
    fill: [u8; 2],
) -> Result<HiddenPixelLanes, PilError> {
    let output_pixels = output_dimensions
        .0
        .checked_mul(output_dimensions.1)
        .ok_or_else(|| PilError::DimensionError("Pad lane output extent overflow".into()))?;
    let value_count = output_pixels
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("Pad lane storage extent overflow".into()))?;
    let x_window = super::dimensions::clipped_axis(
        offset.0,
        source_dimensions.0,
        output_dimensions.0,
    );
    let y_window = super::dimensions::clipped_axis(
        offset.1,
        source_dimensions.1,
        output_dimensions.1,
    );
    let mut values = Vec::new();
    values
        .try_reserve_exact(value_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for output_y in 0..output_dimensions.1 {
        let source_y = (output_y >= y_window.destination_start
            && output_y - y_window.destination_start < y_window.length)
            .then(|| y_window.source_start + output_y - y_window.destination_start);
        for output_x in 0..output_dimensions.0 {
            let source_x = (output_x >= x_window.destination_start
                && output_x - x_window.destination_start < x_window.length)
                .then(|| x_window.source_start + output_x - x_window.destination_start);
            if let (Some(source_x), Some(source_y)) = (source_x, source_y)
                && source_x < source_dimensions.0
                && source_y < source_dimensions.1
            {
                let source_index = source_y
                    .checked_mul(source_dimensions.0)
                    .and_then(|row| row.checked_add(source_x))
                    .ok_or_else(|| {
                        PilError::DimensionError("Pad source lane index overflow".into())
                    })?;
                values.extend_from_slice(
                    source
                        .for_pixel(source_index)
                        .ok_or_else(invalid_lane_output)?,
                );
            } else {
                values.extend_from_slice(&fill[..lane_count]);
            }
        }
    }
    HiddenPixelLanes::per_pixel(lane_count, output_pixels, values)
        .ok_or_else(invalid_lane_output)
}

fn invalid_lane_output() -> PilError {
    PilError::InternalError("Pad physical lane output was invalid".into())
}
