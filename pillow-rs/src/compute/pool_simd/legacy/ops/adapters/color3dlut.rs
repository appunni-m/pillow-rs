//! Native 3D color lookup-table adapter and interpolation kernels.

use super::{SimdImageShape, SimdLayout, pixel_mode_name, simd_unsupported};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
use wide::i32x8;

fn color3dlut_target_channels(mode: PixelMode) -> Option<usize> {
    match mode {
        PixelMode::RGB => Some(3),
        PixelMode::RGBA | PixelMode::CMYK => Some(4),
        _ => None,
    }
}

fn color3dlut_table_len(size: (u32, u32, u32), channels: u32, table_len: usize) -> Option<usize> {
    if !matches!(size.0, 2..=65)
        || !matches!(size.1, 2..=65)
        || !matches!(size.2, 2..=65)
        || !matches!(channels, 3 | 4)
    {
        return None;
    }
    let expected = (size.0 as usize)
        .checked_mul(size.1 as usize)?
        .checked_mul(size.2 as usize)?
        .checked_mul(channels as usize)?;
    (table_len == expected).then_some(expected)
}

fn color3dlut_source_channels_for_image(
    img: &DynamicImage,
    source_mode: PixelMode,
) -> Option<usize> {
    match (source_mode, img) {
        (PixelMode::RGB, DynamicImage::ImageRgb8(_)) => Some(3),
        (PixelMode::RGBA | PixelMode::CMYK, DynamicImage::ImageRgba8(_)) => Some(4),
        _ => None,
    }
}

fn color3dlut_source_channels_for_shape(
    shape: SimdImageShape,
    source_mode: PixelMode,
) -> Option<usize> {
    match (source_mode, shape.layout) {
        (PixelMode::RGB, SimdLayout::Rgb8) => Some(3),
        (PixelMode::RGBA | PixelMode::CMYK, SimdLayout::Rgba8) => Some(4),
        _ => None,
    }
}

pub(super) fn color3dlut_supported_for_image(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table_len: usize,
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if mode.is_some_and(|value| pixel_mode_name(source_mode) != value) {
        return false;
    }
    let Some(source_channels) = color3dlut_source_channels_for_image(img, source_mode) else {
        return false;
    };
    let Some(target_channels) = color3dlut_target_channels(target_mode) else {
        return false;
    };
    color3dlut_table_len(size, channels, table_len).is_some()
        && target_channels >= channels as usize
        && source_channels == source_mode.channels()
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .and_then(|pixels| pixels.checked_mul(source_channels))
            .is_some_and(|expected| {
                expected == img.as_bytes().len() && expected / source_channels >= 8
            })
}

pub(super) fn color3dlut_supported_for_shape(
    shape: SimdImageShape,
    size: (u32, u32, u32),
    table_len: usize,
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if mode.is_some_and(|value| pixel_mode_name(source_mode) != value) {
        return false;
    }
    let Some(source_channels) = color3dlut_source_channels_for_shape(shape, source_mode) else {
        return false;
    };
    let Some(target_channels) = color3dlut_target_channels(target_mode) else {
        return false;
    };
    color3dlut_table_len(size, channels, table_len).is_some()
        && target_channels >= channels as usize
        && source_channels == source_mode.channels()
        && (shape.width as usize)
            .checked_mul(shape.height as usize)
            .is_some_and(|pixels| pixels >= 8)
}

#[inline]
fn color3dlut_prepare_fixed(value: f64) -> i16 {
    // Keep this conversion identical to `op_color3dlut`: Pillow first
    // narrows table values to float32 and then rounds signed 10.6 samples.
    const PRECISION_BITS: i32 = 6;
    let item = value as f32;
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
}

#[inline]
fn color3dlut_interpolate_scalar(a: i16, b: i16, shift: i32) -> i16 {
    const SHIFT_BITS: i32 = 15;
    let value = (i64::from(a) * i64::from((1 << SHIFT_BITS) - shift)
        + i64::from(b) * i64::from(shift))
        >> SHIFT_BITS;
    value as i16
}

#[inline]
fn color3dlut_interpolate_vector(a: i32x8, b: i32x8, shift: i32x8) -> i32x8 {
    const SHIFT_BITS: u32 = 15;
    let one = i32x8::splat(1i32 << SHIFT_BITS);
    (a * (one - shift) + b * shift) >> SHIFT_BITS
}

#[inline]
fn color3dlut_table_index(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

fn color3dlut_write_scalar_pixel(
    source: &[u8],
    pixel: usize,
    source_channels: usize,
    prepared: &[i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    target_channels: usize,
    output: &mut [u8],
) {
    const PRECISION_BITS: i32 = 6;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let (sx, sy, _) = size;
    let sxy = sx * sy;
    let source_offset = pixel * source_channels;
    let indices = [
        u32::from(source[source_offset]) * scales[0],
        u32::from(source[source_offset + 1]) * scales[1],
        u32::from(source[source_offset + 2]) * scales[2],
    ];
    let shifts = indices.map(|index| ((SCALE_MASK & index) >> (SCALE_BITS - SHIFT_BITS)) as i32);
    let base = color3dlut_table_index(
        (indices[0] >> SCALE_BITS) as usize,
        (indices[1] >> SCALE_BITS) as usize,
        (indices[2] >> SCALE_BITS) as usize,
        sx,
        sxy,
    ) * channels;
    let output_offset = pixel * target_channels;
    for c in 0..channels {
        let left_left = color3dlut_interpolate_scalar(
            prepared[base + c],
            prepared[base + channels + c],
            shifts[0],
        );
        let left_right = color3dlut_interpolate_scalar(
            prepared[base + sx * channels + c],
            prepared[base + sx * channels + channels + c],
            shifts[0],
        );
        let left = color3dlut_interpolate_scalar(left_left, left_right, shifts[1]);
        let right_left = color3dlut_interpolate_scalar(
            prepared[base + sxy * channels + c],
            prepared[base + sxy * channels + channels + c],
            shifts[0],
        );
        let right_right = color3dlut_interpolate_scalar(
            prepared[base + sxy * channels + sx * channels + c],
            prepared[base + sxy * channels + sx * channels + channels + c],
            shifts[0],
        );
        let right = color3dlut_interpolate_scalar(right_left, right_right, shifts[1]);
        let result = color3dlut_interpolate_scalar(left, right, shifts[2]);
        output[output_offset + c] = ((i32::from(result) + (1 << (PRECISION_BITS - 1)))
            >> PRECISION_BITS)
            .clamp(0, 255) as u8;
    }
    if channels == 3 && target_channels == 4 {
        output[output_offset + 3] = if source_channels == 4 {
            source[source_offset + 3]
        } else {
            255
        };
    }
}

fn color3dlut_write_vector_batch(
    source: &[u8],
    first_pixel: usize,
    source_channels: usize,
    prepared: &[i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    target_channels: usize,
    output: &mut [u8],
) {
    const PRECISION_BITS: i32 = 6;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let (sx, sy, _) = size;
    let sxy = sx * sy;
    let mut bases = [0usize; 8];
    let mut shift_x = [0i32; 8];
    let mut shift_y = [0i32; 8];
    let mut shift_z = [0i32; 8];
    for lane in 0..8 {
        let source_offset = (first_pixel + lane) * source_channels;
        let indices = [
            u32::from(source[source_offset]) * scales[0],
            u32::from(source[source_offset + 1]) * scales[1],
            u32::from(source[source_offset + 2]) * scales[2],
        ];
        bases[lane] = color3dlut_table_index(
            (indices[0] >> SCALE_BITS) as usize,
            (indices[1] >> SCALE_BITS) as usize,
            (indices[2] >> SCALE_BITS) as usize,
            sx,
            sxy,
        ) * channels;
        shift_x[lane] = ((SCALE_MASK & indices[0]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
        shift_y[lane] = ((SCALE_MASK & indices[1]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
        shift_z[lane] = ((SCALE_MASK & indices[2]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
    }
    let shift_x = i32x8::new(shift_x);
    let shift_y = i32x8::new(shift_y);
    let shift_z = i32x8::new(shift_z);

    for c in 0..channels {
        let left_left = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| prepared[bases[lane] + c] as i32)),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + channels + c] as i32
            })),
            shift_x,
        );
        let left_right = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sx * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sx * channels + channels + c] as i32
            })),
            shift_x,
        );
        let left = color3dlut_interpolate_vector(left_left, left_right, shift_y);
        let right_left = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + channels + c] as i32
            })),
            shift_x,
        );
        let right_right = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + sx * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + sx * channels + channels + c] as i32
            })),
            shift_x,
        );
        let right = color3dlut_interpolate_vector(right_left, right_right, shift_y);
        let result = color3dlut_interpolate_vector(left, right, shift_z).to_array();
        for (lane, value) in result.into_iter().enumerate() {
            let output_offset = (first_pixel + lane) * target_channels;
            output[output_offset + c] =
                ((value + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS).clamp(0, 255) as u8;
        }
    }
    if channels == 3 && target_channels == 4 {
        for lane in 0..8 {
            let source_offset = (first_pixel + lane) * source_channels;
            let output_offset = (first_pixel + lane) * target_channels;
            output[output_offset + 3] = if source_channels == 4 {
                source[source_offset + 3]
            } else {
                255
            };
        }
    }
}

/// Apply a validated Color3DLUT with scalar table-index gathers and a
/// fixed-point eight-lane interpolation kernel.  Table addressing and the
/// scalar tail stay in the control plane because the LUT is an indirect
/// memory access; all trilinear multiply/shift stages for complete batches
/// execute through `i32x8`.
pub fn simd_color3dlut(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Color3DLut {
        size,
        table,
        channels,
        source_mode,
        target_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected Color3DLut op".into()));
    };
    if !color3dlut_supported_for_image(
        img,
        *size,
        table.len(),
        *channels,
        *source_mode,
        *target_mode,
        mode,
    ) {
        return Err(simd_unsupported("Color3DLut"));
    }
    let source_channels = color3dlut_source_channels_for_image(img, *source_mode)
        .ok_or_else(|| simd_unsupported("Color3DLut"))?;
    let target_channels =
        color3dlut_target_channels(*target_mode).ok_or_else(|| simd_unsupported("Color3DLut"))?;
    let (width, height) = img.dimensions();
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut pixel count overflow".into()))?;
    let output_len = pixels
        .checked_mul(target_channels)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut output size overflow".into()))?;
    let prepared: Vec<i16> = table
        .iter()
        .copied()
        .map(color3dlut_prepare_fixed)
        .collect();
    let size_usize = (size.0 as usize, size.1 as usize, size.2 as usize);
    const SCALE_BITS: u32 = 18;
    let scales = [
        ((size_usize.0 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
        ((size_usize.1 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
        ((size_usize.2 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
    ];
    let source = img.as_bytes();
    let mut output = vec![0u8; output_len];
    let vector_end = pixels - pixels % 8;
    for pixel in (0..vector_end).step_by(8) {
        color3dlut_write_vector_batch(
            source,
            pixel,
            source_channels,
            &prepared,
            size_usize,
            scales,
            *channels as usize,
            target_channels,
            &mut output,
        );
    }
    for pixel in vector_end..pixels {
        color3dlut_write_scalar_pixel(
            source,
            pixel,
            source_channels,
            &prepared,
            size_usize,
            scales,
            *channels as usize,
            target_channels,
            &mut output,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks((vector_end / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_end) as u64);
    match target_mode {
        PixelMode::RGB => RgbImage::from_raw(width, height, output)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| PilError::InternalError("SIMD Color3DLut output shape mismatch".into())),
        PixelMode::RGBA | PixelMode::CMYK => RgbaImage::from_raw(width, height, output)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| PilError::InternalError("SIMD Color3DLut output shape mismatch".into())),
        _ => Err(simd_unsupported("Color3DLut")),
    }
}
