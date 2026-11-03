//! Pillow physical-byte behavior for compact mode filter outputs.

use crate::raster::GenericImageView;

use crate::error::PilError;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_lanes_for_bytes;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::DynamicImage;

use super::super::ops::FilterRequest;

/// Applies mode-specific Pillow padding behavior after the selected filter.
pub(crate) fn finish(
    source: &DynamicImage,
    output: &mut DynamicImage,
    request: FilterRequest,
) -> Result<(), PilError> {
    validate_output(source, output)?;
    let mode = source.mode();
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    let Some(source_lanes) = source.hidden_pixel_lanes() else {
        return finish_missing_source_lanes(source, output, request, lane_count);
    };
    validate_source_lanes(source_lanes, lane_count, source)?;

    let lanes = match request {
        FilterRequest::Filter3x3 { .. } => filter_border_lanes(source_lanes, source, 1)?,
        FilterRequest::Filter5x5 { .. } => filter_border_lanes(source_lanes, source, 2)?,
        _ => execute_independent_lanes(source_lanes, source, request)?,
    };
    replace_lanes(output, Some(lanes))
}

fn finish_missing_source_lanes(
    source: &DynamicImage,
    output: &mut DynamicImage,
    request: FilterRequest,
    lane_count: usize,
) -> Result<(), PilError> {
    let radius = match request {
        FilterRequest::Filter3x3 { .. } => 1,
        FilterRequest::Filter5x5 { .. } => 2,
        _ => return replace_lanes(output, None),
    };
    let width = source.width() as usize;
    let height = source.height() as usize;
    if width < radius * 2 + 1 || height < radius * 2 + 1 {
        return replace_lanes(output, None);
    }
    let pixel_count = checked_pixel_count(source)?;
    let source_lanes = default_hidden_lanes_for_bytes(
        source.mode(),
        source.as_bytes(),
        pixel_count,
    )?
    .ok_or_else(invalid_lanes)?;
    validate_source_lanes(&source_lanes, lane_count, source)?;
    let lanes = filter_border_lanes(&source_lanes, source, radius)?;
    replace_lanes(output, Some(lanes))
}

/// Applies the same per-byte UnsharpMask blend to retained Pillow lanes.
pub(crate) fn finish_unsharp(
    source: &DynamicImage,
    output: &mut DynamicImage,
    percent: i32,
    threshold: i32,
) -> Result<(), PilError> {
    validate_output(source, output)?;
    let lane_count = source.mode().hidden_pixel_lane_count();
    let (Some(lane_count), Some(source_lanes), Some(blurred_lanes)) = (
        lane_count,
        source.hidden_pixel_lanes(),
        output.hidden_pixel_lanes(),
    ) else {
        return replace_lanes(output, None);
    };
    validate_source_lanes(source_lanes, lane_count, source)?;
    validate_source_lanes(blurred_lanes, lane_count, output)?;
    let pixels = checked_pixel_count(source)?;
    if pixels == 0 {
        return replace_lanes(output, None);
    }
    let lanes = if source_lanes.is_constant() && blurred_lanes.is_constant() {
        let mut values = [0; 2];
        let source_values = source_lanes.for_pixel(0).ok_or_else(invalid_lanes)?;
        let blur_values = blurred_lanes.for_pixel(0).ok_or_else(invalid_lanes)?;
        for lane in 0..lane_count {
            values[lane] = super::unsharp::blend_sample(
                source_values[lane],
                blur_values[lane],
                percent,
                threshold,
            );
        }
        HiddenPixelLanes::constant(lane_count, pixels, values).ok_or_else(invalid_lanes)?
    } else {
        let capacity = pixels.checked_mul(lane_count).ok_or_else(invalid_lanes)?;
        let mut values = Vec::with_capacity(capacity);
        for pixel in 0..pixels {
            let original = source_lanes.for_pixel(pixel).ok_or_else(invalid_lanes)?;
            let blurred = blurred_lanes.for_pixel(pixel).ok_or_else(invalid_lanes)?;
            for lane in 0..lane_count {
                values.push(super::unsharp::blend_sample(
                    original[lane],
                    blurred[lane],
                    percent,
                    threshold,
                ));
            }
        }
        compact_lanes(lane_count, pixels, values)?
    };
    replace_lanes(output, Some(lanes))
}

/// Applies the core ModeFilter extension independently to compact padding bytes.
pub(crate) fn finish_mode_filter(
    source: &DynamicImage,
    output: &mut DynamicImage,
    size: u32,
) -> Result<(), PilError> {
    validate_output(source, output)?;
    let Some(lane_count) = source.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    let Some(source_lanes) = source.hidden_pixel_lanes() else {
        return replace_lanes(output, None);
    };
    validate_source_lanes(source_lanes, lane_count, source)?;
    let pixels = checked_pixel_count(source)?;
    let lanes = execute_independent_lanes_with(source_lanes, source, |input| {
        Ok(super::mode_filter::mode_filter_native_bytes::<1>(
            input,
            source.width() as usize,
            source.height() as usize,
            size,
        ))
    })?;
    if lanes.pixel_count() != pixels {
        return Err(invalid_lanes());
    }
    replace_lanes(output, Some(lanes))
}

/// Applies Pillow's Filter.c border copy and zeroed compact-mode padding.
fn filter_border_lanes(
    source_lanes: &HiddenPixelLanes,
    source: &DynamicImage,
    radius: usize,
) -> Result<HiddenPixelLanes, PilError> {
    let pixels = checked_pixel_count(source)?;
    let width = source.width() as usize;
    let height = source.height() as usize;
    if width < radius * 2 + 1 || height < radius * 2 + 1 {
        return Ok(source_lanes.clone());
    }
    if source_lanes.is_constant() {
        let first = source_lanes.for_pixel(0).ok_or_else(invalid_lanes)?;
        if first[..source_lanes.lane_count()].iter().all(|value| *value == 0) {
            return HiddenPixelLanes::constant(source_lanes.lane_count(), pixels, [0; 2])
                .ok_or_else(invalid_lanes);
        }
    }
    let lane_count = source_lanes.lane_count();
    let capacity = pixels.checked_mul(lane_count).ok_or_else(invalid_lanes)?;
    let mut values = Vec::with_capacity(capacity);
    for pixel in 0..pixels {
        let x = pixel % width;
        let y = pixel / width;
        let border = x < radius
            || x + radius >= width
            || y < radius
            || y + radius >= height;
        let lanes = source_lanes.for_pixel(pixel).ok_or_else(invalid_lanes)?;
        if border {
            values.extend_from_slice(&lanes[..lane_count]);
        } else {
            values.resize(values.len() + lane_count, 0);
        }
    }
    compact_lanes(lane_count, pixels, values)
}

/// Filters each omitted byte as an independent physical Pillow channel.
fn execute_independent_lanes(
    source_lanes: &HiddenPixelLanes,
    source: &DynamicImage,
    request: FilterRequest,
) -> Result<HiddenPixelLanes, PilError> {
    execute_independent_lanes_with(source_lanes, source, |input| {
        execute_lane(input, source.width(), source.height(), request)
    })
}

fn execute_independent_lanes_with<F>(
    source_lanes: &HiddenPixelLanes,
    source: &DynamicImage,
    mut execute: F,
) -> Result<HiddenPixelLanes, PilError>
where
    F: FnMut(&[u8]) -> Result<Vec<u8>, PilError>,
{
    let pixels = checked_pixel_count(source)?;
    let lane_count = source_lanes.lane_count();
    if source_lanes.is_constant() && pixels > 0 {
        return Ok(source_lanes.clone());
    }
    let capacity = pixels.checked_mul(lane_count).ok_or_else(invalid_lanes)?;
    let mut values = vec![0; capacity];
    for lane in 0..lane_count {
        let input = collect_lane(source_lanes, lane, pixels)?;
        let filtered = execute(&input)?;
        if filtered.len() != pixels {
            return Err(invalid_lanes());
        }
        for (pixel, value) in filtered.into_iter().enumerate() {
            values[pixel * lane_count + lane] = value;
        }
    }
    compact_lanes(lane_count, pixels, values)
}

fn execute_lane(
    input: &[u8],
    width: u32,
    height: u32,
    request: FilterRequest,
) -> Result<Vec<u8>, PilError> {
    let output = match request {
        FilterRequest::GaussianBlur { sigma } => {
            super::blur::execute_gaussian_blur_native_bytes(input, width, height, 1, sigma)?
        }
        FilterRequest::BoxBlur { radius } => {
            super::blur::execute_box_blur_native_bytes(input, width, height, 1, radius)?
        }
        FilterRequest::BoxBlurXY { radius_x, radius_y, passes } => {
            super::blur::execute_box_blur_xy_native_bytes(
                input, width, height, 1, radius_x, radius_y, passes,
            )?
        }
        FilterRequest::MedianFilter { size } => {
            super::rank::execute_native_bytes(input, width, height, 1, size, size * size / 2)?
        }
        FilterRequest::MaxFilter { size } => {
            super::rank::execute_native_bytes(input, width, height, 1, size, size * size - 1)?
        }
        FilterRequest::MinFilter { size } => {
            super::rank::execute_native_bytes(input, width, height, 1, size, 0)?
        }
        FilterRequest::RankFilter { size, rank } => {
            super::rank::execute_native_bytes(input, width, height, 1, size, rank)?
        }
        FilterRequest::Filter3x3 { .. } | FilterRequest::Filter5x5 { .. } => {
            return Err(PilError::InternalError(
                "convolution request reached independent lane filtering".into(),
            ));
        }
    };
    Ok(output.unwrap_or_else(|| input.to_vec()))
}

fn collect_lane(
    source: &HiddenPixelLanes,
    lane: usize,
    pixels: usize,
) -> Result<Vec<u8>, PilError> {
    let mut values = Vec::with_capacity(pixels);
    for pixel in 0..pixels {
        let value = source
            .for_pixel(pixel)
            .and_then(|lanes| lanes.get(lane))
            .copied()
            .ok_or_else(invalid_lanes)?;
        values.push(value);
    }
    Ok(values)
}

fn compact_lanes(
    lane_count: usize,
    pixels: usize,
    values: Vec<u8>,
) -> Result<HiddenPixelLanes, PilError> {
    if values.len() != pixels.checked_mul(lane_count).ok_or_else(invalid_lanes)? {
        return Err(invalid_lanes());
    }
    if let Some(first) = values.chunks_exact(lane_count).next() {
        let constant = first.to_vec();
        if values
            .chunks_exact(lane_count)
            .all(|pixel| pixel == constant.as_slice())
        {
            let mut lanes = [0; 2];
            lanes[..lane_count].copy_from_slice(&constant);
            return HiddenPixelLanes::constant(lane_count, pixels, lanes)
                .ok_or_else(invalid_lanes);
        }
    }
    HiddenPixelLanes::per_pixel(lane_count, pixels, values).ok_or_else(invalid_lanes)
}

fn validate_output(source: &DynamicImage, output: &DynamicImage) -> Result<(), PilError> {
    if output.mode() != source.mode() || output.dimensions() != source.dimensions() {
        return Err(PilError::InternalError(
            "mode-preserving filter changed its raster layout".into(),
        ));
    }
    Ok(())
}

fn validate_source_lanes(
    lanes: &HiddenPixelLanes,
    lane_count: usize,
    image: &DynamicImage,
) -> Result<(), PilError> {
    if lanes.lane_count() != lane_count || lanes.pixel_count() != checked_pixel_count(image)? {
        return Err(invalid_lanes());
    }
    Ok(())
}

fn checked_pixel_count(image: &DynamicImage) -> Result<usize, PilError> {
    usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("Filter lane extent overflow".into()))
}

fn replace_lanes(output: &mut DynamicImage, lanes: Option<HiddenPixelLanes>) -> Result<(), PilError> {
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(invalid_lanes())
    }
}

fn invalid_lanes() -> PilError {
    PilError::InternalError("filter physical lane output was invalid".into())
}
