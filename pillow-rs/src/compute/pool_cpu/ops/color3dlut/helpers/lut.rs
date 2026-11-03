//! Scalar Color3DLut implementation shared by its supported source modes.

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GenericImageView, Mode, RgbImage, RgbaImage};

const PRECISION_BITS: i32 = 6;
pub(super) const SCALE_BITS: u32 = 18;
pub(super) const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
pub(super) const SHIFT_BITS: u32 = 15;

#[inline(always)]
pub(super) fn table_index_3d(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

#[inline(always)]
fn interpolate(a: i16, b: i16, shift: i32) -> i16 {
    let value = (i64::from(a) * i64::from((1 << SHIFT_BITS) - shift)
        + i64::from(b) * i64::from(shift))
        >> SHIFT_BITS;
    value as i16
}

pub(super) fn prepare_table(table: &[f64]) -> Vec<i16> {
    table
        .iter()
        .map(|value| {
            let item = *value as f32;
            let scaled = item * ((255 << PRECISION_BITS) as f32);
            if scaled >= i16::MAX as f32 - 0.5 {
                i16::MAX
            } else if scaled <= i16::MIN as f32 + 0.5 {
                i16::MIN
            } else if item < 0.0 {
                (scaled - 0.5) as i16
            } else {
                (scaled + 0.5) as i16
            }
        })
        .collect()
}

pub(super) fn color_scales(size: (usize, usize, usize)) -> [u32; 3] {
    [
        ((size.0 - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((size.1 - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((size.2 - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
    ]
}

#[inline(always)]
fn interpolate_channel(
    table: &[i16],
    base: usize,
    sx: usize,
    sxy: usize,
    channels: usize,
    channel: usize,
    shifts: [i32; 3],
) -> i16 {
    let left_left = interpolate(table[base + channel], table[base + channels + channel], shifts[0]);
    let left_right = interpolate(
        table[base + sx * channels + channel],
        table[base + sx * channels + channels + channel],
        shifts[0],
    );
    let left = interpolate(left_left, left_right, shifts[1]);
    let right_left = interpolate(
        table[base + sxy * channels + channel],
        table[base + sxy * channels + channels + channel],
        shifts[0],
    );
    let right_right = interpolate(
        table[base + sxy * channels + sx * channels + channel],
        table[base + sxy * channels + sx * channels + channels + channel],
        shifts[0],
    );
    let right = interpolate(right_left, right_right, shifts[1]);
    interpolate(left, right, shifts[2])
}

#[inline(always)]
pub(super) fn write_pixel(
    pixel: [u8; 4],
    preserve_source_fourth: bool,
    prepared: &[i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    target_channels: usize,
    output: &mut [u8],
) {
    let (sx, sy, _) = size;
    let sxy = sx * sy;
    let indices = [
        u32::from(pixel[0]) * scales[0],
        u32::from(pixel[1]) * scales[1],
        u32::from(pixel[2]) * scales[2],
    ];
    let shifts = indices
        .map(|index| ((SCALE_MASK & index) >> (SCALE_BITS - SHIFT_BITS)) as i32);
    let base = table_index_3d(
        (indices[0] >> SCALE_BITS) as usize,
        (indices[1] >> SCALE_BITS) as usize,
        (indices[2] >> SCALE_BITS) as usize,
        sx,
        sxy,
    ) * channels;

    for channel in 0..channels {
        let value = interpolate_channel(
            prepared, base, sx, sxy, channels, channel, shifts,
        );
        output[channel] = ((i32::from(value) + (1 << (PRECISION_BITS - 1)))
            >> PRECISION_BITS)
            .clamp(0, 255) as u8;
    }
    if channels == 3 && target_channels == 4 {
        output[3] = if preserve_source_fourth { pixel[3] } else { 255 };
    }
}

/// Applies the same LUT arithmetic directly to a native three-byte raster.
///
/// This path is used by core-only color-space strategies that intentionally
/// interpret their stored channels as table coordinates without converting
/// the source mode to RGB.
pub(crate) fn apply_native_rgb_samples(
    image: &RgbImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (width, height) = image.dimensions();
    let source = image.as_raw();
    let source_len = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| PilError::DimensionError("Color3DLut source size overflows".into()))?;
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "native Color3DLut source shape mismatch".into(),
        ));
    }
    let target_channels = match target_mode {
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    };
    let output_dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
        width,
        height,
        target_channels as u8,
    )?;
    let size = (size.0 as usize, size.1 as usize, size.2 as usize);
    let channels = channels as usize;
    let scales = color_scales(size);
    let prepared = prepare_table(table);
    let mut output = output_dimensions.alloc_buffer();
    for (source_pixel, output_pixel) in source
        .chunks_exact(3)
        .zip(output.chunks_exact_mut(target_channels))
    {
        write_pixel(
            [source_pixel[0], source_pixel[1], source_pixel[2], 255],
            false,
            &prepared,
            size,
            scales,
            channels,
            target_channels,
            output_pixel,
        );
    }
    let mut output = image_from_native_bytes(width, height, output, target_mode)?;
    if target_mode == Mode::ImageRgb8 && channels == 3
        && !output.replace_hidden_pixel_lanes(image.hidden_pixel_lanes().cloned())
    {
        return Err(PilError::InternalError(
            "Color3DLut RGB output padding did not match its source extent".into(),
        ));
    }
    Ok(output)
}

/// Applies the scalar LUT directly to a native four-byte source carrier.
///
/// The mode owner supplies the exact typed buffer and the operation preserves
/// the source's fourth sample when a three-channel table expands to four
/// output channels. This is alpha for RGBA and K for CMYK; the byte kernel
/// does not reinterpret or convert either source mode.
pub(crate) fn apply_native_rgba_samples(
    image: &RgbaImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (width, height) = image.dimensions();
    let source_len = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::DimensionError("Color3DLut source size overflows".into()))?;
    let source = image.as_raw();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "native Color3DLut source shape mismatch".into(),
        ));
    }
    let target_channels = match target_mode {
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    };
    let output_dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
        width,
        height,
        target_channels as u8,
    )?;
    let size = (size.0 as usize, size.1 as usize, size.2 as usize);
    let channels = channels as usize;
    let scales = color_scales(size);
    let prepared = prepare_table(table);
    let mut output = output_dimensions.alloc_buffer();
    for (source_pixel, output_pixel) in source
        .chunks_exact(4)
        .zip(output.chunks_exact_mut(target_channels))
    {
        write_pixel(
            [source_pixel[0], source_pixel[1], source_pixel[2], source_pixel[3]],
            true,
            &prepared,
            size,
            scales,
            channels,
            target_channels,
            output_pixel,
        );
    }
    let mut output = image_from_native_bytes(width, height, output, target_mode)?;
    if target_mode == Mode::ImageRgb8 && channels == 3 {
        let pixel_count = source.len() / 4;
        let padding = source.chunks_exact(4).map(|pixel| pixel[3]).collect();
        let lanes = HiddenPixelLanes::per_pixel(1, pixel_count, padding).ok_or_else(|| {
            PilError::InternalError("Color3DLut RGB output padding shape mismatch".into())
        })?;
        if !output.replace_hidden_pixel_lanes(Some(lanes)) {
            return Err(PilError::InternalError(
                "Color3DLut RGB output padding did not match its source extent".into(),
            ));
        }
    }
    Ok(output)
}

/// Retain the fourth physical byte copied by Pillow's three-channel LUT path
/// when the destination is the compact RGB carrier.
pub(crate) fn preserve_target_padding(
    source: &DynamicImage,
    output: &mut DynamicImage,
    table_channels: u32,
) -> Result<(), PilError> {
    if output.mode() != Mode::ImageRgb8 || table_channels != 3 {
        return Ok(());
    }
    let pixel_count = usize::try_from(source.width())
        .ok()
        .and_then(|width| {
            usize::try_from(source.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("Color3DLut padding extent overflows".into()))?;
    let lanes = match source {
        DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageYCbCr8(_)
        | DynamicImage::ImageLab8(_)
        | DynamicImage::ImageHsv8(_) => match source.hidden_pixel_lanes() {
            Some(lanes) if lanes.lane_count() == 1 && lanes.pixel_count() == pixel_count => {
                Some(lanes.clone())
            }
            Some(_) => {
                return Err(PilError::InternalError(
                    "Color3DLut source padding did not match its image".into(),
                ));
            }
            None => None,
        },
        DynamicImage::ImageRgba8(_) | DynamicImage::ImageCmyk8(_) => {
            let source_bytes = source.as_bytes();
            let expected = pixel_count.checked_mul(4).ok_or_else(|| {
                PilError::DimensionError("Color3DLut source padding size overflows".into())
            })?;
            if source_bytes.len() != expected {
                return Err(PilError::InternalError(
                    "Color3DLut four-band source shape mismatch".into(),
                ));
            }
            let padding = source_bytes
                .chunks_exact(4)
                .map(|pixel| pixel[3])
                .collect();
            Some(HiddenPixelLanes::per_pixel(1, pixel_count, padding).ok_or_else(|| {
                PilError::InternalError("Color3DLut RGB output padding shape mismatch".into())
            })?)
        }
        _ => {
            return Err(PilError::InternalError(
                "Color3DLut RGB output received an unsupported source mode".into(),
            ));
        }
    };
    if output.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Color3DLut RGB output padding did not match its image".into(),
        ))
    }
}

pub(super) fn image_from_native_bytes(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    match target_mode {
        Mode::ImageRgb8 => RgbImage::from_raw(width, height, bytes)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into())),
        Mode::ImageRgba8 => RgbaImage::from_raw(width, height, bytes)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into())),
        Mode::ImageCmyk8 => RgbaImage::from_raw(width, height, bytes)
            .map(DynamicImage::ImageCmyk8)
            .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into())),
        _ => Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}
