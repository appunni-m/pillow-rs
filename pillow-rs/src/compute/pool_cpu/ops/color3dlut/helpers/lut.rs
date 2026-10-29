//! Scalar Color3DLut implementation shared by its supported source modes.

use crate::error::PilError;
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
    source_mode: Mode,
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
        output[3] = if matches!(source_mode, Mode::ImageRgba8 | Mode::ImageCmyk8) {
            pixel[3]
        } else {
            255
        };
    }
}

pub(crate) fn apply(
    image: &DynamicImage,
    source_mode: Mode,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (sx, sy, sz) = (size.0 as usize, size.1 as usize, size.2 as usize);
    let channels = channels as usize;
    let target_channels = match target_mode {
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    };
    let scales = color_scales((sx, sy, sz));
    let prepared = prepare_table(table);
    let (width, height) = image.dimensions();
    let mut output = vec![0u8; (width * height) as usize * 4];

    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) as usize) * 4;
            write_pixel(
                image.get_pixel(x, y).0,
                source_mode,
                &prepared,
                (sx, sy, sz),
                scales,
                channels,
                target_channels,
                &mut output[offset..offset + 4],
            );
        }
    }
    image_from_rgba_bytes(width, height, output, target_mode)
}

/// Applies the same LUT arithmetic directly to a native three-byte raster.
///
/// This path is used by core-only color-space strategies that intentionally
/// interpret their stored channels as table coordinates without converting
/// the source mode to RGB.
pub(crate) fn apply_native_rgb_samples(
    image: &RgbImage,
    source_mode: Mode,
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
            source_mode,
            &prepared,
            size,
            scales,
            channels,
            target_channels,
            output_pixel,
        );
    }
    image_from_native_bytes(width, height, output, target_mode)
}

fn image_from_rgba_bytes(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let rgba = RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into()))?;
    match target_mode {
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(RgbImage::from_fn(
            width,
            height,
            |x, y| {
                let pixel = rgba.get_pixel(x, y);
                crate::raster::Rgb([pixel[0], pixel[1], pixel[2]])
            },
        ))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba)),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(rgba)),
        _ => Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
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
