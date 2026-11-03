//! Native arithmetic and physical-lane preservation shared by Enhance modes.

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::DynamicImage;

/// Applies Pillow's byte blend expression with a black or mode-specific base.
/// The factor has already crossed Pillow's C `float` boundary.
/// This preserves the boundary case `10 * 0.09999999999999999 -> 1`.
#[inline]
pub(crate) fn blend_u8(base: u8, sample: u8, alpha: f32) -> u8 {
    let value = f32::from(base) + alpha * (f32::from(sample) - f32::from(base));
    value.clamp(0.0, 255.0) as u8
}

/// Applies Pillow Brightness's zero-image Blend rule to omitted physical lanes.
pub(crate) fn carry_brightness_hidden_lanes(
    source: &DynamicImage,
    output: &mut DynamicImage,
    factor: f64,
) -> Result<(), PilError> {
    if source.mode() != output.mode() || source.dimensions() != output.dimensions() {
        return Err(PilError::InternalError(
            "Brightness output did not preserve its source layout".into(),
        ));
    }
    let Some(lane_count) = source.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("Brightness lane extent overflow".into()))?;
    let alpha = factor as f32;
    let lanes = match source.hidden_pixel_lanes() {
        Some(lanes) if lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count => {
            return Err(PilError::InternalError(
                "Brightness source lane payload did not match its mode".into(),
            ));
        }
        Some(lanes) if alpha == 1.0 => Some(lanes.clone()),
        Some(_) | None if alpha == 0.0 => {
            let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2])
                .ok_or_else(|| PilError::InternalError("invalid Brightness lane layout".into()))?;
            (pixel_count > 0).then_some(lanes)
        }
        Some(lanes) => Some(lanes.map_values(|sample| blend_u8(0, sample, alpha))),
        None => None,
    };
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Brightness output physical lanes did not match its mode".into(),
        ))
    }
}

/// Applies the native Color Saturation blend to retained omitted physical lanes.
pub(crate) fn carry_color_saturation_hidden_lanes(
    source: &DynamicImage,
    output: &mut DynamicImage,
    factor: f64,
) -> Result<(), PilError> {
    if source.mode() != output.mode() || source.dimensions() != output.dimensions() {
        return Err(PilError::InternalError(
            "ColorSaturation output did not preserve its source layout".into(),
        ));
    }
    let Some(lane_count) = source.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("ColorSaturation lane extent overflow".into()))?;
    let alpha = factor as f32;
    let lanes = match source {
        DynamicImage::ImageLumaA8(_) => clone_color_saturation_lanes(
            source.hidden_pixel_lanes(),
            lane_count,
            pixel_count,
        )?,
        DynamicImage::ImageRgb8(_) => color_saturation_rgb_lanes(
            source.hidden_pixel_lanes(),
            lane_count,
            pixel_count,
            alpha,
        )?,
        _ => {
            return Err(PilError::InternalError(
                "ColorSaturation has no physical-lane strategy for this mode".into(),
            ));
        }
    };
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "ColorSaturation output physical lanes did not match its mode".into(),
        ))
    }
}

fn clone_color_saturation_lanes(
    lanes: Option<&HiddenPixelLanes>,
    lane_count: usize,
    pixel_count: usize,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    let Some(lanes) = lanes else {
        return Ok(None);
    };
    validate_color_saturation_lanes(lanes, lane_count, pixel_count)?;
    Ok(Some(lanes.clone()))
}

fn color_saturation_rgb_lanes(
    lanes: Option<&HiddenPixelLanes>,
    lane_count: usize,
    pixel_count: usize,
    factor: f32,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    if lane_count != 1 {
        return Err(PilError::InternalError(
            "ColorSaturation RGB lane count did not match its mode".into(),
        ));
    }
    if let Some(lanes) = lanes {
        validate_color_saturation_lanes(lanes, lane_count, pixel_count)?;
        if factor == 1.0 {
            return Ok(Some(lanes.clone()));
        }
        if factor == 0.0 {
            return constant_color_saturation_rgb_lane(lane_count, pixel_count);
        }
        return Ok(Some(lanes.map_values(|sample| blend_byte(255, sample, factor))));
    }
    if factor == 0.0 {
        return constant_color_saturation_rgb_lane(lane_count, pixel_count);
    }
    Ok(None)
}

fn constant_color_saturation_rgb_lane(
    lane_count: usize,
    pixel_count: usize,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    if pixel_count == 0 {
        return Ok(None);
    }
    HiddenPixelLanes::constant(lane_count, pixel_count, [255, 0])
        .map(Some)
        .ok_or_else(|| PilError::InternalError("invalid ColorSaturation RGB lanes".into()))
}

fn validate_color_saturation_lanes(
    lanes: &HiddenPixelLanes,
    lane_count: usize,
    pixel_count: usize,
) -> Result<(), PilError> {
    if lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count {
        return Err(PilError::InternalError(
            "ColorSaturation source lane payload did not match its mode".into(),
        ));
    }
    Ok(())
}

#[inline]
fn blend_byte(base: u8, sample: u8, factor: f32) -> u8 {
    factor
        .mul_add(f32::from(sample) - f32::from(base), f32::from(base))
        .clamp(0.0, 255.0) as u8
}

pub(crate) fn scale_u8(
    source: &[u8],
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> Vec<u8> {
    let alpha = factor as f32;
    if alpha == 1.0 {
        return source.to_vec();
    }
    let mut output = source.to_vec();
    for pixel in output.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample = blend_u8(0, *sample, alpha);
        }
    }
    output
}

pub(crate) fn scale_u16(
    samples: &mut [u16],
    channels: usize,
    active_channels: usize,
    factor: f64,
) {
    for pixel in samples.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample = (f64::from(*sample) * factor).clamp(0.0, f64::from(u16::MAX)) as u16;
        }
    }
}

pub(crate) fn scale_f32(
    samples: &mut [f32],
    channels: usize,
    active_channels: usize,
    factor: f64,
) {
    let factor = factor as f32;
    for pixel in samples.chunks_exact_mut(channels) {
        for sample in &mut pixel[..active_channels] {
            *sample *= factor;
        }
    }
}

pub(crate) fn scale_i32_carrier(source: &[u8], factor: f64) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len());
    for sample in source.chunks_exact(4) {
        let value = i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let scaled = (f64::from(value) * factor)
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        output.extend_from_slice(&scaled.to_le_bytes());
    }
    output
}

pub(crate) fn scale_f32_carrier(source: &[u8], factor: f64) -> Vec<u8> {
    let factor = factor as f32;
    let mut output = Vec::with_capacity(source.len());
    for sample in source.chunks_exact(4) {
        let value = f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        output.extend_from_slice(&(value * factor).to_le_bytes());
    }
    output
}
