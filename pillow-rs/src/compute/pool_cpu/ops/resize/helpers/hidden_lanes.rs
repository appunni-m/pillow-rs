//! Omitted physical-byte behavior for Pillow Resize routes.
//!
//! Pillow's 2-/3-band `Resample.c` paths zero physical bytes without logical
//! bands. Non-nearest `LA` is wrapped through `La`, whose `Convert.c` return
//! path repeats the final luma into those bytes. Identity and exact crop paths
//! copy the original storage instead. The source audit is recorded in
//! `docs/CPU_REFACTOR_PLAN.md`.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode, buffer::HiddenPixelLanes};

/// Applies Pillow's copy, crop, or resample rule to compact-carrier lanes.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn finish(
    source: &DynamicImage,
    output: &mut DynamicImage,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<(), PilError> {
    let mode = source.mode();
    if output.mode() != mode {
        return Err(PilError::InternalError(
            "Resize output changed mode while composing physical lanes".into(),
        ));
    }
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };

    if let Some(bounds) = bounds
        && super::boxed_crop::uses_default_shortcut(
            (source.width(), source.height()),
            (output.width(), output.height()),
            filter,
            bounds,
            is_straight_alpha(mode),
        )
    {
        if (source.width(), source.height()) == (output.width(), output.height())
            && bounds.0 == 0.0
            && bounds.1 == 0.0
            && output.hidden_pixel_lanes() == source.hidden_pixel_lanes()
        {
            return Ok(());
        }
        return remap_lanes(
            source,
            output,
            bounds.0 as f32 as u32,
            bounds.1 as f32 as u32,
        );
    }
    if bounds.is_none() && (source.width(), source.height()) == (output.width(), output.height()) {
        if output.hidden_pixel_lanes() == source.hidden_pixel_lanes() {
            return Ok(());
        }
        return remap_lanes(source, output, 0, 0);
    }

    let lanes = resampled_lanes(output, mode, filter, lane_count)?;
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(invalid_lane_output());
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn remap_lanes(
    source: &DynamicImage,
    output: &mut DynamicImage,
    left: u32,
    top: u32,
) -> Result<(), PilError> {
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::DimensionError("Resize lane source width overflow".into()))?;
    let output_width = usize::try_from(output.width())
        .map_err(|_| PilError::DimensionError("Resize lane output width overflow".into()))?;
    let left = usize::try_from(left)
        .map_err(|_| PilError::DimensionError("Resize lane crop x overflow".into()))?;
    let top = usize::try_from(top)
        .map_err(|_| PilError::DimensionError("Resize lane crop y overflow".into()))?;
    if output_width == 0 {
        if !output.remap_hidden_pixel_lanes_from(source, |_| usize::MAX) {
            return Err(invalid_lane_output());
        }
        return Ok(());
    }
    if !output.remap_hidden_pixel_lanes_from(source, |index| {
        let output_x = index % output_width;
        let output_y = index / output_width;
        let Some(source_y) = top.checked_add(output_y) else {
            return usize::MAX;
        };
        let Some(source_x) = left.checked_add(output_x) else {
            return usize::MAX;
        };
        source_y
            .checked_mul(source_width)
            .and_then(|row| row.checked_add(source_x))
            .unwrap_or(usize::MAX)
    }) {
        return Err(invalid_lane_output());
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn resampled_lanes(
    output: &DynamicImage,
    mode: Mode,
    filter: ResampleFilter,
    lane_count: usize,
) -> Result<HiddenPixelLanes, PilError> {
    let pixel_count = usize::try_from(output.width())
        .ok()
        .and_then(|width| {
            usize::try_from(output.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("Resize lane output extent overflow".into()))?;
    if mode == Mode::ImageLumaA8 && !matches!(filter, ResampleFilter::Nearest) {
        return luma_alpha_resampled_lanes(output, pixel_count, lane_count);
    }
    HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2]).ok_or_else(invalid_lane_output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn luma_alpha_resampled_lanes(
    output: &DynamicImage,
    pixel_count: usize,
    lane_count: usize,
) -> Result<HiddenPixelLanes, PilError> {
    // Pillow's straight-LA non-nearest wrapper converts back from La after
    // filtering, writing final L into the omitted physical lanes 1 and 2.
    let DynamicImage::ImageLumaA8(image) = output else {
        return Err(invalid_lane_output());
    };
    let value_count = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("Resize lane storage extent overflow".into()))?;
    let source = image.as_raw();
    if source.len()
        != pixel_count
            .checked_mul(2)
            .ok_or_else(|| PilError::DimensionError("Resize luma-alpha extent overflow".into()))?
    {
        return Err(invalid_lane_output());
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(value_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for pixel in source.chunks_exact(2) {
        values.extend_from_slice(&[pixel[0]; 2]);
    }
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values).ok_or_else(invalid_lane_output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
const fn is_straight_alpha(mode: Mode) -> bool {
    matches!(mode, Mode::ImageLumaA8 | Mode::ImageRgba8)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn invalid_lane_output() -> PilError {
    PilError::InternalError("Resize physical lane output was invalid".into())
}
