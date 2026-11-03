//! Pillow physical-byte behavior for compact-carrier Rotate modes.

use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GenericImageView, Mode};

use super::geometry::{discrete_fast_angle, identity_contract, rotate_geometry};

/// Composes omitted physical bytes after the selected exact-mode Rotate kernel.
///
/// Pillow's nearest filter copies complete pixels. Its filtered two-band
/// callback repeats the first band into physical bytes 1 and 2; LA converts
/// through premultiplied `La` and back, so the same rule applies to the returned
/// luma. Filtered three-band callbacks leave byte 3 at the output fill value.
pub(crate) fn finish(
    source: &DynamicImage,
    output: &mut DynamicImage,
    request: RotateRequest,
    architecture: Architecture,
) -> Result<(), PilError> {
    let mode = source.mode();
    if output.mode() != mode {
        return Err(PilError::InternalError(
            "Rotate output changed its source mode".into(),
        ));
    }
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    let source_pixels = pixel_count(source)?;
    validate_source_lanes(source, lane_count, source_pixels)?;
    if identity_contract(request) {
        return replace_lanes(output, source.hidden_pixel_lanes().cloned());
    }
    if let Some(degree) = discrete_fast_angle(request, source.width(), source.height()) {
        return finish_discrete(source, output, degree);
    }
    let output_pixels = pixel_count(output)?;
    if request.nearest {
        finish_nearest(source, output, request, architecture, lane_count, source_pixels, output_pixels)
    } else {
        finish_filtered(mode, output, request, lane_count, output_pixels)
    }
}

fn finish_discrete(
    source: &DynamicImage,
    output: &mut DynamicImage,
    degree: u32,
) -> Result<(), PilError> {
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let output_width = output.width() as usize;
    let expected_dimensions = if degree == 180 {
        (source.width(), source.height())
    } else {
        (source.height(), source.width())
    };
    if output.dimensions() != expected_dimensions {
        return Err(PilError::InternalError(
            "discrete Rotate output dimensions did not match its pixel map".into(),
        ));
    }
    if pixel_count(output)? == 0 {
        return replace_lanes(output, None);
    }
    if !output.remap_hidden_pixel_lanes_from(source, |output_index| {
        discrete_source_index(
            degree,
            output_index,
            source_width,
            source_height,
            output_width,
        )
    }) {
        return Err(invalid_lanes());
    }
    Ok(())
}

fn finish_nearest(
    source: &DynamicImage,
    output: &mut DynamicImage,
    request: RotateRequest,
    architecture: Architecture,
    lane_count: usize,
    source_pixels: usize,
    output_pixels: usize,
) -> Result<(), PilError> {
    let Some(source_lanes) = source.hidden_pixel_lanes() else {
        return replace_lanes(output, None);
    };
    let fill = hidden_fill(source.mode(), request);
    let fill_values = [fill.0, fill.3];
    if source_lanes.is_constant()
        && source_pixels > 0
        && source_lanes.for_pixel(0) == Some(&fill_values[..lane_count])
    {
        return replace_lanes(
            output,
            HiddenPixelLanes::constant(lane_count, output_pixels, fill_values),
        );
    }
    let lanes = match lane_count {
        1 => nearest_lanes::<1>(
            source,
            source_lanes,
            output,
            request,
            architecture,
            source_pixels,
            output_pixels,
        )?,
        2 => nearest_lanes::<2>(
            source,
            source_lanes,
            output,
            request,
            architecture,
            source_pixels,
            output_pixels,
        )?,
        _ => return Err(invalid_lanes()),
    };
    replace_lanes(output, Some(lanes))
}

fn nearest_lanes<const CHANNELS: usize>(
    source: &DynamicImage,
    source_lanes: &HiddenPixelLanes,
    output: &DynamicImage,
    request: RotateRequest,
    architecture: Architecture,
    source_pixels: usize,
    output_pixels: usize,
) -> Result<HiddenPixelLanes, PilError> {
    let geometry = rotate_geometry(source.width(), source.height(), request);
    if output.dimensions() != (geometry.width, geometry.height) {
        return Err(PilError::InternalError(
            "Rotate nearest output dimensions did not match its geometry".into(),
        ));
    }
    let source_bytes = source_lane_bytes(source_lanes, CHANNELS, source_pixels)?;
    let value_count = output_pixels.checked_mul(CHANNELS).ok_or_else(invalid_lanes)?;
    let mut output_bytes = Vec::new();
    output_bytes
        .try_reserve_exact(value_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    output_bytes.resize(value_count, 0);
    if output_pixels == 0 {
        return HiddenPixelLanes::per_pixel(CHANNELS, output_pixels, output_bytes)
            .ok_or_else(invalid_lanes);
    }
    let fill = hidden_fill(source.mode(), request);
    if architecture == Architecture::Default {
        super::byte::affine_nearest::<CHANNELS>(
            &source_bytes,
            source.dimensions(),
            geometry,
            fill,
            &mut output_bytes,
        );
    } else {
        super::vector_nearest::copy_nearest_rows::<CHANNELS>(
            &source_bytes,
            source.dimensions(),
            geometry,
            fill,
            &mut output_bytes,
        );
    }
    HiddenPixelLanes::per_pixel(CHANNELS, output_pixels, output_bytes).ok_or_else(invalid_lanes)
}

fn finish_filtered(
    mode: Mode,
    output: &mut DynamicImage,
    request: RotateRequest,
    lane_count: usize,
    output_pixels: usize,
) -> Result<(), PilError> {
    let lanes = match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            repeat_output_luma(output, lane_count, output_pixels)?
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            let fill = request.fill.unwrap_or((0, 0, 0, 0));
            HiddenPixelLanes::constant(lane_count, output_pixels, [fill.3, 0])
                .ok_or_else(invalid_lanes)?
        }
        _ => return Ok(()),
    };
    replace_lanes(output, Some(lanes))
}

fn repeat_output_luma(
    output: &DynamicImage,
    lane_count: usize,
    pixel_count: usize,
) -> Result<HiddenPixelLanes, PilError> {
    let carrier_width = output
        .mode()
        .byte_carrier_pixel_width()
        .ok_or_else(invalid_lanes)?;
    let logical = output.as_bytes();
    if logical.len() < pixel_count.checked_mul(carrier_width).ok_or_else(invalid_lanes)? {
        return Err(invalid_lanes());
    }
    let value_count = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(invalid_lanes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(value_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for pixel in logical.chunks_exact(carrier_width).take(pixel_count) {
        values.resize(values.len() + lane_count, pixel[0]);
    }
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values).ok_or_else(invalid_lanes)
}

fn source_lane_bytes(
    lanes: &HiddenPixelLanes,
    lane_count: usize,
    pixel_count: usize,
) -> Result<Vec<u8>, PilError> {
    let byte_count = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(invalid_lanes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(byte_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for index in 0..pixel_count {
        values.extend_from_slice(lanes.for_pixel(index).ok_or_else(invalid_lanes)?);
    }
    Ok(values)
}

fn hidden_fill(mode: Mode, request: RotateRequest) -> (u8, u8, u8, u8) {
    let fill = request.fill.unwrap_or((0, 0, 0, 0));
    match mode.hidden_pixel_lane_count() {
        // LA, La, and PA keep physical lanes 1 and 2. Pillow's two-band
        // callbacks repeat the first component into both omitted lanes.
        Some(2) => (fill.0, 0, 0, fill.0),
        // Three-band RGB-family pixels keep their physical padding byte at 3.
        Some(1) => (fill.3, 0, 0, 0),
        _ => fill,
    }
}

fn validate_source_lanes(
    source: &DynamicImage,
    lane_count: usize,
    pixel_count: usize,
) -> Result<(), PilError> {
    if source.hidden_pixel_lanes().is_some_and(|lanes| {
        lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count
    }) {
        return Err(invalid_lanes());
    }
    Ok(())
}

fn pixel_count(image: &DynamicImage) -> Result<usize, PilError> {
    usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(invalid_lanes)
}

fn discrete_source_index(
    degree: u32,
    output_index: usize,
    source_width: usize,
    source_height: usize,
    output_width: usize,
) -> usize {
    let output_x = output_index % output_width;
    let output_y = output_index / output_width;
    let (source_x, source_y) = match degree {
        90 => (source_width - 1 - output_y, output_x),
        180 => (source_width - 1 - output_x, source_height - 1 - output_y),
        270 => (output_y, source_height - 1 - output_x),
        _ => unreachable!("discrete Rotate angle was validated before lane mapping"),
    };
    source_y * source_width + source_x
}

fn replace_lanes(
    output: &mut DynamicImage,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(invalid_lanes())
    }
}

fn invalid_lanes() -> PilError {
    PilError::InternalError("Rotate physical lane output was invalid".into())
}
