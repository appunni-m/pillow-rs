//! Serial native-sample Sharpness kernels shared by exact-mode strategies.

use crate::error::PilError;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Pixel, RgbaImage};

const BUFFER_ERROR: &str = "enhance_sharpness: buffer error";

fn checked_shape(width: u32, height: u32, channels: usize) -> Result<(usize, usize), PilError> {
    let width = width as usize;
    let height = height as usize;
    let length = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))?;
    Ok((length, width.checked_mul(channels).ok_or_else(|| {
        PilError::ValueError(BUFFER_ERROR.into())
    })?))
}

fn check_channels<const CHANNELS: usize, const ACTIVE: usize>() -> Result<(), PilError> {
    if CHANNELS == 0 || ACTIVE == 0 || ACTIVE > CHANNELS {
        return Err(PilError::InternalError(
            "Sharpness strategy declared an invalid native channel layout".into(),
        ));
    }
    Ok(())
}

/// Applies Pillow Filter's zeroed omitted lanes, then its Sharpness Blend.
pub(crate) fn carry_hidden_lanes<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    output: &mut DynamicImage,
    mode: Mode,
    factor: f64,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if output.mode() != mode
        || output.dimensions() != (source.width(), source.height())
    {
        return Err(PilError::InternalError(
            "Sharpness output did not preserve its source layout".into(),
        ));
    }
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("Sharpness lane extent overflow".into()))?;
    let alpha = factor as f32;
    let width = source.width() as usize;
    let height = source.height() as usize;
    let Some(source_lanes) = source.hidden_pixel_lanes() else {
        let lanes = default_sharpness_lanes(
            mode,
            lane_count,
            pixel_count,
            width,
            height,
            source.as_raw(),
            output.as_bytes(),
            alpha,
        )?;
        return replace_sharpness_lanes(output, lanes);
    };
    if source_lanes.lane_count() != lane_count || source_lanes.pixel_count() != pixel_count {
        return Err(PilError::InternalError(
            "Sharpness source lane payload did not match its mode".into(),
        ));
    }
    let lanes = if alpha == 1.0 || width < 3 || height < 3 {
        source_lanes.clone()
    } else {
        let lane_values = pixel_count
            .checked_mul(lane_count)
            .ok_or_else(|| PilError::DimensionError("Sharpness lane extent overflow".into()))?;
        let mut values = Vec::with_capacity(lane_values);
        for index in 0..pixel_count {
            let x = index % width;
            let y = index / width;
            let interior = width >= 3
                && height >= 3
                && x > 0
                && x + 1 < width
                && y > 0
                && y + 1 < height;
            for &sample in source_lanes.for_pixel(index).ok_or_else(|| {
                PilError::InternalError("Sharpness source lane pixel was unavailable".into())
            })? {
                let base = if interior { 0 } else { sample };
                values.push(super::native::blend_u8(base, sample, alpha));
            }
        }
        HiddenPixelLanes::per_pixel(lane_count, pixel_count, values)
            .ok_or_else(|| PilError::InternalError("invalid Sharpness lane layout".into()))?
    };
    replace_sharpness_lanes(output, Some(lanes))
}

fn default_sharpness_lanes(
    mode: Mode,
    lane_count: usize,
    pixel_count: usize,
    width: usize,
    height: usize,
    source_bytes: &[u8],
    output_bytes: &[u8],
    factor: f32,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    if pixel_count == 0 || factor == 1.0 || width < 3 || height < 3 {
        return Ok(None);
    }
    let carrier_width = mode.byte_carrier_pixel_width().ok_or_else(|| {
        PilError::InternalError("Sharpness mode omitted its byte-carrier width".into())
    })?;
    let byte_size = pixel_count
        .checked_mul(carrier_width)
        .ok_or_else(|| PilError::DimensionError("Sharpness byte extent overflow".into()))?;
    let source_bytes = source_bytes.get(..byte_size).ok_or_else(|| {
        PilError::InternalError("Sharpness source bytes did not match its mode extent".into())
    })?;
    let output_bytes = output_bytes.get(..byte_size).ok_or_else(|| {
        PilError::InternalError("Sharpness output bytes did not match its mode extent".into())
    })?;
    let lane_size = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("Sharpness lane extent overflow".into()))?;
    let mut values = Vec::with_capacity(lane_size);
    let mut differs_from_fallback = false;
    for index in 0..pixel_count {
        let x = index % width;
        let y = index / width;
        let interior = x > 0 && x + 1 < width && y > 0 && y + 1 < height;
        let byte_offset = index * carrier_width;
        let source_lanes = default_hidden_pixel_lanes(mode, source_bytes[byte_offset])
            .ok_or_else(|| {
                PilError::InternalError("Sharpness mode omitted its physical-lane fallback".into())
            })?;
        let output_fallback = default_hidden_pixel_lanes(mode, output_bytes[byte_offset])
            .ok_or_else(|| {
                PilError::InternalError("Sharpness mode omitted its physical-lane fallback".into())
            })?;
        for lane in 0..lane_count {
            let filtered = if interior { 0 } else { source_lanes[lane] };
            let value = super::native::blend_u8(filtered, source_lanes[lane], factor);
            differs_from_fallback |= value != output_fallback[lane];
            values.push(value);
        }
    }
    if !differs_from_fallback {
        return Ok(None);
    }
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("invalid Sharpness lane layout".into()))
}

fn replace_sharpness_lanes(
    output: &mut DynamicImage,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Sharpness output physical lanes did not match its mode".into(),
        ))
    }
}

/// Apply the archived byte filter and blend without converting the raster mode.
pub(crate) fn execute_u8<const CHANNELS: usize, const ACTIVE: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    check_channels::<CHANNELS, ACTIVE>()?;
    if factor == 1.0 {
        return Ok(source.to_vec());
    }
    let (expected, row_stride) = checked_shape(width, height, CHANNELS)?;
    if source.len() != expected {
        return Err(PilError::ValueError(BUFFER_ERROR.into()));
    }
    let (width, height, alpha) = (width as usize, height as usize, factor as f32);
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = y * row_stride + x * CHANNELS;
            for channel in 0..ACTIVE {
                let sample = index + channel;
                let smooth = if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                    let above = sample - row_stride;
                    let below = sample + row_stride;
                    let left = CHANNELS;
                    let right = CHANNELS;
                    let weighted = u16::from(source[below - left])
                        + u16::from(source[below])
                        + u16::from(source[below + right])
                        + u16::from(source[sample - left])
                        + u16::from(source[sample]) * 5
                        + u16::from(source[sample + right])
                        + u16::from(source[above - left])
                        + u16::from(source[above])
                        + u16::from(source[above + right]);
                    ((weighted + 6) / 13) as u8
                } else {
                    source[sample]
                };
                output[sample] = alpha
                    .mul_add(f32::from(source[sample]) - f32::from(smooth), f32::from(smooth))
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(output)
}

/// Sharpen one typed byte buffer while retaining its concrete pixel type.
pub(crate) fn execute_u8_image<P, const CHANNELS: usize, const ACTIVE: usize>(
    source: &ImageBuffer<P, Vec<u8>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let bytes = execute_u8::<CHANNELS, ACTIVE>(
        source.as_raw(),
        source.width(),
        source.height(),
        factor,
    )?;
    ImageBuffer::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))
}

/// Apply the same neighborhood and blend to native unsigned 16-bit samples.
pub(crate) fn execute_u16<const CHANNELS: usize, const ACTIVE: usize>(
    source: &[u16],
    width: u32,
    height: u32,
    factor: f64,
) -> Result<Vec<u16>, PilError> {
    check_channels::<CHANNELS, ACTIVE>()?;
    if factor == 1.0 {
        return Ok(source.to_vec());
    }
    let (expected, row_stride) = checked_shape(width, height, CHANNELS)?;
    if source.len() != expected {
        return Err(PilError::ValueError(BUFFER_ERROR.into()));
    }
    let (width, height, alpha) = (width as usize, height as usize, factor as f32);
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = y * row_stride + x * CHANNELS;
            for channel in 0..ACTIVE {
                let sample = index + channel;
                let smooth = if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                    let above = sample - row_stride;
                    let below = sample + row_stride;
                    let left = CHANNELS;
                    let right = CHANNELS;
                    let weighted = u32::from(source[below - left])
                        + u32::from(source[below])
                        + u32::from(source[below + right])
                        + u32::from(source[sample - left])
                        + u32::from(source[sample]) * 5
                        + u32::from(source[sample + right])
                        + u32::from(source[above - left])
                        + u32::from(source[above])
                        + u32::from(source[above + right]);
                    ((weighted + 6) / 13) as u16
                } else {
                    source[sample]
                };
                output[sample] = alpha
                    .mul_add(f32::from(source[sample]) - f32::from(smooth), f32::from(smooth))
                    .clamp(0.0, f32::from(u16::MAX)) as u16;
            }
        }
    }
    Ok(output)
}

/// Sharpen a native unsigned 16-bit buffer while retaining its pixel type.
pub(crate) fn execute_u16_image<P, const CHANNELS: usize, const ACTIVE: usize>(
    source: &ImageBuffer<P, Vec<u16>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let samples = execute_u16::<CHANNELS, ACTIVE>(
        source.as_raw(),
        source.width(),
        source.height(),
        factor,
    )?;
    ImageBuffer::from_raw(source.width(), source.height(), samples)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))
}

/// Apply the neighborhood blend to native f32 samples without clipping values.
pub(crate) fn execute_f32<const CHANNELS: usize, const ACTIVE: usize>(
    source: &[f32],
    width: u32,
    height: u32,
    factor: f64,
) -> Result<Vec<f32>, PilError> {
    check_channels::<CHANNELS, ACTIVE>()?;
    if factor == 1.0 {
        return Ok(source.to_vec());
    }
    let (expected, row_stride) = checked_shape(width, height, CHANNELS)?;
    if source.len() != expected {
        return Err(PilError::ValueError(BUFFER_ERROR.into()));
    }
    let (width, height, alpha) = (width as usize, height as usize, factor as f32);
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = y * row_stride + x * CHANNELS;
            for channel in 0..ACTIVE {
                let sample = index + channel;
                let smooth = if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                    let above = sample - row_stride;
                    let below = sample + row_stride;
                    let left = CHANNELS;
                    let right = CHANNELS;
                    (source[below - left]
                        + source[below]
                        + source[below + right]
                        + source[sample - left]
                        + source[sample] * 5.0
                        + source[sample + right]
                        + source[above - left]
                        + source[above]
                        + source[above + right])
                        / 13.0
                } else {
                    source[sample]
                };
                output[sample] = alpha.mul_add(source[sample] - smooth, smooth);
            }
        }
    }
    Ok(output)
}

/// Sharpen a native f32 buffer while retaining its pixel type.
pub(crate) fn execute_f32_image<P, const CHANNELS: usize, const ACTIVE: usize>(
    source: &ImageBuffer<P, Vec<f32>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<f32>>, PilError>
where
    P: Pixel<Subpixel = f32>,
{
    let samples = execute_f32::<CHANNELS, ACTIVE>(
        source.as_raw(),
        source.width(),
        source.height(),
        factor,
    )?;
    ImageBuffer::from_raw(source.width(), source.height(), samples)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))
}

/// Sharpen little-endian signed I samples stored in the existing four-byte carrier.
pub(crate) fn execute_i32_carrier_image(
    source: &RgbaImage,
    factor: f64,
) -> Result<RgbaImage, PilError> {
    let width = source.width();
    let height = source.height();
    let bytes = execute_i32_carrier(source.as_raw(), width, height, factor)?;
    ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))
}

fn execute_i32_carrier(
    source: &[u8],
    width: u32,
    height: u32,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    let (samples, row_stride) = checked_shape(width, height, 1)?;
    let expected_bytes = samples
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))?;
    if factor == 1.0 {
        return Ok(source.to_vec());
    }
    if source.len() != expected_bytes {
        return Err(PilError::ValueError(BUFFER_ERROR.into()));
    }
    let (width, height) = (width as usize, height as usize);
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = y * row_stride + x;
            let original = read_i32(source, index);
            let smooth = if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                let above = index - row_stride;
                let below = index + row_stride;
                let weighted = i64::from(read_i32(source, below - 1))
                    + i64::from(read_i32(source, below))
                    + i64::from(read_i32(source, below + 1))
                    + i64::from(read_i32(source, index - 1))
                    + i64::from(original) * 5
                    + i64::from(read_i32(source, index + 1))
                    + i64::from(read_i32(source, above - 1))
                    + i64::from(read_i32(source, above))
                    + i64::from(read_i32(source, above + 1));
                weighted as f64 / 13.0
            } else {
                f64::from(original)
            };
            let result = factor.mul_add(f64::from(original) - smooth, smooth) as i32;
            write_i32(&mut output, index, result);
        }
    }
    Ok(output)
}

/// Sharpen little-endian F samples without routing through a byte-channel kernel.
pub(crate) fn execute_f32_carrier_image(
    source: &RgbaImage,
    factor: f64,
) -> Result<RgbaImage, PilError> {
    let width = source.width();
    let height = source.height();
    let bytes = execute_f32_carrier(source.as_raw(), width, height, factor)?;
    ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))
}

fn execute_f32_carrier(
    source: &[u8],
    width: u32,
    height: u32,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    let (samples, row_stride) = checked_shape(width, height, 1)?;
    let expected_bytes = samples
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError(BUFFER_ERROR.into()))?;
    if factor == 1.0 {
        return Ok(source.to_vec());
    }
    if source.len() != expected_bytes {
        return Err(PilError::ValueError(BUFFER_ERROR.into()));
    }
    let (width, height, alpha) = (width as usize, height as usize, factor as f32);
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = y * row_stride + x;
            let original = read_f32(source, index);
            let smooth = if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                let above = index - row_stride;
                let below = index + row_stride;
                (read_f32(source, below - 1)
                    + read_f32(source, below)
                    + read_f32(source, below + 1)
                    + read_f32(source, index - 1)
                    + original * 5.0
                    + read_f32(source, index + 1)
                    + read_f32(source, above - 1)
                    + read_f32(source, above)
                    + read_f32(source, above + 1))
                    / 13.0
            } else {
                original
            };
            write_f32(&mut output, index, alpha.mul_add(original - smooth, smooth));
        }
    }
    Ok(output)
}

fn read_i32(source: &[u8], sample: usize) -> i32 {
    let start = sample * 4;
    i32::from_le_bytes([
        source[start],
        source[start + 1],
        source[start + 2],
        source[start + 3],
    ])
}

fn write_i32(output: &mut [u8], sample: usize, value: i32) {
    let start = sample * 4;
    output[start..start + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_f32(source: &[u8], sample: usize) -> f32 {
    let start = sample * 4;
    f32::from_le_bytes([
        source[start],
        source[start + 1],
        source[start + 2],
        source[start + 3],
    ])
}

fn write_f32(output: &mut [u8], sample: usize, value: f32) {
    let start = sample * 4;
    output[start..start + 4].copy_from_slice(&value.to_le_bytes());
}
