//! SIMD adapters for Pillow ImageEnhance operations.

use super::lut::{native_lut_apply, native_lut_chunk, native_lut_tables};
use super::module_blend::native_module_blend_arithmetic_supported;
use super::{
    SimdImageShape, SimdLayout, apply_native_rows, has_nonempty_byte_data,
    native_all_channel_transform_bytes, native_byte_transform_bytes,
    native_filter_3x3_channel_block_integer, native_filter_3x3_luma_block_integer,
    native_filter_3x3_pixel_block_integer, native_filter_3x3_rgb_byte_block_integer,
    native_filter_3x3_vector, native_filter_load_byte_block, native_rgba_color_degenerate,
    shape_channels, shape_has_nonempty_byte_data, simd_duplicate, simd_unsupported,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use wide::{f32x8, i16x8, u8x16};

mod point;

pub(crate) use self::point::{simd_posterize, simd_solarize};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            point::supports_for_image(img, op, mode)
        }
        PipelineOp::Brightness { factor } => {
            native_brightness_layout(img, mode)
                .is_some_and(|channels| has_nonempty_byte_data(img, channels))
                && factor.is_finite()
        }
        PipelineOp::Contrast { factor } | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite()
                && native_enhance_layout(img, mode)
                    .is_some_and(|(channels, _)| has_vectorized_float_bytes(img, channels))
        }
        PipelineOp::Sharpness { factor } => {
            factor.is_finite()
                && native_sharpness_layout(img, mode)
                    .is_some_and(|(channels, _)| has_vectorized_sharpness_bytes(img, channels))
        }
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            point::supports_for_shape(shape, op, mode)
        }
        PipelineOp::Brightness { factor } => {
            shape_native_brightness_channels(shape, mode)
                .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
                && factor.is_finite()
        }
        PipelineOp::Contrast { factor } | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite()
                && shape_native_enhance_channels(shape, mode)
                    .is_some_and(|(channels, _)| shape_has_vectorized_float_bytes(shape, channels))
        }
        PipelineOp::Sharpness { factor } => {
            factor.is_finite()
                && shape_native_sharpness_channels(shape, mode).is_some_and(|(channels, _)| {
                    shape_has_vectorized_sharpness_bytes(shape, channels)
                })
        }
        _ => false,
    }
}

pub(super) fn supports_in_place(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            point::supports_in_place(img, mode)
        }
        PipelineOp::Brightness { .. } => native_brightness_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        _ => false,
    }
}

pub(super) fn execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    match op {
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            point::execute_in_place(img, op, mode)
        }
        PipelineOp::Brightness { factor } => Ok(execute_brightness_in_place(img, *factor, mode)),
        _ => Ok(false),
    }
}

/// Return the native byte layout accepted by brightness blending.
///
/// HSV and YCbCr are physically stored as three-byte planes, while CMYK is
/// stored in `Rgba8` with its fourth byte representing K rather than alpha.
/// Brightness blends every sample with zero, so these layouts can use the
/// same byte-wise SIMD LUT without a color-space conversion or alpha rule.
fn native_brightness_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        // HSV and YCbCr are also three-byte native planes. Brightness is a
        // blend with the same-mode black image, so each stored sample follows
        // the same byte LUT without a color-space conversion.
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "CMYK")) => Some(4),
        _ => None,
    }
}

/// Return the native layout and active-channel count used by ImageEnhance.
///
/// Contrast and color saturation preserve alpha in `LA`/`RGBA`, while CMYK's
/// fourth byte is the K sample rather than an alpha byte.  Keeping that
/// distinction at the admission boundary prevents the vector kernel from
/// accidentally treating packed RGBA storage as a color contract.
fn native_enhance_layout(img: &DynamicImage, mode: Option<&str>) -> Option<(usize, usize)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, 1)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, 1)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

/// Return the native layout and active-channel count used by Sharpness.
///
/// Pillow applies Sharpness to RGB samples and preserves alpha for LA/RGBA;
/// CMYK and RGBX process all four stored bytes (including RGBX's X byte).
/// Keeping that distinction here lets the neighborhood kernel operate in the
/// native interleaved buffer without widening through packed RGBA storage.
fn native_sharpness_layout(img: &DynamicImage, mode: Option<&str>) -> Option<(usize, usize)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, 1)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, 1)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        DynamicImage::ImageRgba8(_) if mode == Some("RGBX") => Some((4, 4)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

/// The enhancement kernels consume eight independent byte samples per
/// `f64x8` block.  A partial final block is handled by the same padded vector
/// kernel, but at least one complete block is required for strict SIMD.
fn has_vectorized_float_bytes(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    expected >= 8 && img.as_bytes().len() == expected
}

/// Sharpness needs a non-empty 3x3 interior for its neighborhood pass. The
/// neighborhood kernel pads/repeats the final vector lanes, and the blend
/// phase permits a scalar tail after its eight-wide blocks, so width alone
/// must not reject valid 3x3-or-larger images.
fn has_vectorized_sharpness_bytes(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.width() >= 3 && img.height() >= 3 && expected != 0 && img.as_bytes().len() == expected
}

fn shape_native_brightness_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => matches!(mode, None | Some("RGBA" | "CMYK")),
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_enhance_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, 1)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, 1)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

fn shape_native_sharpness_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, 1)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, 1)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        SimdLayout::Rgba8 if mode == Some("RGBX") => Some((4, 4)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

fn shape_has_vectorized_float_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= 8)
}

fn shape_has_vectorized_sharpness_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape.width >= 3
        && shape.height >= 3
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

/// Byte-wise brightness transform with CMYK explicitly admitted. CMYK's K
/// sample is data, not alpha, so this wrapper uses an all-channel mask rather
/// than the RGBA-preserving mask used by [`native_byte_transform`].
fn native_brightness_transform<F>(
    img: &DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> Option<DynamicImage>
where
    F: Fn(u8x16) -> u8x16,
{
    let channels = native_brightness_layout(img, mode)?;
    if !has_nonempty_byte_data(img, channels) {
        return None;
    }
    let result = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 1, &transform);
            DynamicImage::ImageLuma8(result)
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 2, &transform);
            DynamicImage::ImageLumaA8(result)
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 3, &transform);
            DynamicImage::ImageRgb8(result)
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 4, &transform);
            DynamicImage::ImageRgba8(result)
        }
        DynamicImage::ImageRgba8(image) if mode == Some("CMYK") => {
            let mut result = image.clone();
            // CMYK has no alpha channel; all four packed bytes are active.
            native_all_channel_transform_bytes(result.as_mut(), &transform);
            DynamicImage::ImageRgba8(result)
        }
        _ => return None,
    };
    crate::compute::record_pipeline_operation_vector_blocks(
        img.as_bytes().len().div_ceil(16) as u64
    );
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

fn vectorize_color_bytes(
    source: &[u8],
    output: &mut [u8],
    channels: usize,
    _active_channels: usize,
    mode: Option<&str>,
    factor: f64,
) -> (u64, u64) {
    let alpha = f32x8::splat(factor as f32);
    let vector_bytes = output.len() / 8 * 8;
    for (block, chunk) in output[..vector_bytes].chunks_exact_mut(8).enumerate() {
        let offset = block * 8;
        let input = f32x8::new(std::array::from_fn(|lane| f32::from(source[offset + lane])));
        let base = f32x8::new(std::array::from_fn(|lane| {
            let index = offset + lane;
            let start = index / channels * channels;
            f32::from(
                crate::ops::enhance::color_pixel_base(&source[start..start + channels], mode)
                    [index % channels],
            )
        }));
        let values = alpha.mul_add(input - base, base);
        for (out, value) in chunk.iter_mut().zip(values.to_array()) {
            *out = value.clamp(0.0, 255.0) as u8;
        }
    }
    for (lane, out) in output[vector_bytes..].iter_mut().enumerate() {
        let index = vector_bytes + lane;
        let start = index / channels * channels;
        let base = f32::from(
            crate::ops::enhance::color_pixel_base(&source[start..start + channels], mode)
                [index % channels],
        );
        *out = (factor as f32)
            .mul_add(f32::from(source[index]) - base, base)
            .clamp(0.0, 255.0) as u8;
    }
    (
        (vector_bytes / 8) as u64,
        (output.len() - vector_bytes) as u64,
    )
}

fn native_enhance_output(
    img: &DynamicImage,
    mode: Option<&str>,
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> Option<(DynamicImage, u64, u64)> {
    let source = img.as_bytes();
    let mut result = img.clone();
    let output = result.as_bytes_mut()?;
    if output.len() != source.len() {
        return None;
    }
    let (vector_blocks, scalar_tail) =
        vectorize_color_bytes(source, output, channels, active_channels, mode, factor);
    Some((result, vector_blocks, scalar_tail))
}

/// In-place brightness transform for native byte images, including CMYK.
/// Unlike RGBA, CMYK keeps all four lanes active because K is a color sample.
fn native_brightness_transform_in_place<F>(
    img: &mut DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> bool
where
    F: Fn(u8x16) -> u8x16 + Send + Sync,
{
    let Some(channels) = native_brightness_layout(img, mode) else {
        return false;
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    if width == 0 || height == 0 {
        return false;
    }
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let row_bytes = width.saturating_mul(channels);
    let vector_blocks = row_bytes.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    if mode == Some("CMYK") {
        apply_native_rows(bytes, width, height, channels, |row| {
            native_all_channel_transform_bytes(row, &transform);
        });
    } else {
        apply_native_rows(bytes, width, height, channels, |row| {
            native_byte_transform_bytes(row, channels, &transform);
        });
    }
    crate::compute::record_pipeline_operation_path("vector");
    true
}

pub fn simd_brightness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Brightness { factor } = op else {
        return Err(PilError::ValueError("expected Brightness op".into()));
    };
    // Pillow's Brightness enhancement blends the source with black; factor
    // 1.0 is therefore an exact source copy. Preserve the native layout and
    // avoid rebuilding/scanning an identity LUT.
    if *factor == 1.0
        && native_brightness_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels))
    {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(img.clone());
    }
    // The packed scalar adapter intentionally quantizes the public factor
    // to the same fixed-point domain as the SIMD operation. Build that exact
    // 256-entry map once and apply it in native L/LA/RGB/RGBA/CMYK storage;
    // CMYK is admitted explicitly because all four bytes are active samples.
    let factor_fp = (*factor * 1000.0) as u32;
    // For L, LA, and RGB, these power-of-two factors are exact right shifts
    // in the adapter's fixed-point domain. LA's active-byte mask keeps alpha
    // unchanged while shifting luma; RGB applies the shift to each native
    // three-byte sample.
    let shift_layout = matches!(
        (img, mode),
        (DynamicImage::ImageLuma8(_), None | Some("L"))
            | (DynamicImage::ImageLumaA8(_), None | Some("LA"))
            | (DynamicImage::ImageRgb8(_), None | Some("RGB"))
    );
    if shift_layout
        && let Some(shift) = match factor_fp {
            500 => Some(1),
            250 => Some(2),
            125 => Some(3),
            _ => None,
        }
        && let Some(result) = native_brightness_transform(img, mode, |input| input >> shift)
    {
        return Ok(result);
    }
    let lut: Vec<u8> = (0u32..=255)
        .map(|value| ((value as u64 * factor_fp as u64) / 1000).min(255) as u8)
        .collect();
    if let Some(tables) = native_lut_tables(&lut)
        && let Some(result) =
            native_brightness_transform(img, mode, |input| native_lut_chunk(input, &tables))
    {
        return Ok(result);
    }
    Err(simd_unsupported("Brightness"))
}

/// Compute the exact native-mode base, then apply its float32 blend as byte LUTs.
pub fn simd_contrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Contrast { factor } = op else {
        return Err(PilError::ValueError("expected Contrast op".into()));
    };
    let base = crate::ops::enhance::contrast_base(img, mode)
        .ok_or_else(|| simd_unsupported("Contrast"))?;
    if !(*factor as f32).is_finite() || !has_vectorized_float_bytes(img, base.channels) {
        return Err(simd_unsupported("Contrast"));
    }
    let lut = crate::ops::enhance::contrast_lut(&base, *factor);
    let mut result = img.clone();
    let output = result
        .as_bytes_mut()
        .ok_or_else(|| simd_unsupported("Contrast"))?;
    let (blocks, tail) = native_lut_apply(
        output,
        img.width() as usize,
        img.height() as usize,
        base.channels,
        &lut,
    )
    .ok_or_else(|| simd_unsupported("Contrast"))?;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Apply ImageEnhance.Color's per-pixel grayscale blend.  Grayscale and CMYK
/// bases are derived in scalar control code; the channel blend itself runs in
/// eight-lane vectors and never widens the whole image to packed RGBA.
pub fn simd_color_saturation(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::ColorSaturation { factor } = op else {
        return Err(PilError::ValueError("expected ColorSaturation op".into()));
    };
    let Some((channels, active_channels)) = native_enhance_layout(img, mode) else {
        return Err(simd_unsupported("ColorSaturation"));
    };
    if !native_module_blend_arithmetic_supported(*factor)
        || !has_vectorized_float_bytes(img, channels)
    {
        return Err(simd_unsupported("ColorSaturation"));
    }
    if *factor == 0.0
        && channels == 4
        && matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(mode, None | Some("RGBA"))
    {
        let Some((result, vector_blocks, scalar_tail)) = native_rgba_color_degenerate(img) else {
            return Err(simd_unsupported("ColorSaturation"));
        };
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("vector");
        return Ok(result);
    }
    let Some((result, vector_blocks, scalar_tail)) =
        native_enhance_output(img, mode, channels, active_channels, *factor)
    else {
        return Err(simd_unsupported("ColorSaturation"));
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

mod sharpness;
pub(crate) use sharpness::simd_sharpness;

pub(super) fn execute_brightness_in_place(
    img: &mut DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> bool {
    // Pillow's Brightness enhancement blends the source with black; factor
    // 1.0 is therefore an exact source copy. Preserve the native layout and
    // avoid rebuilding/scanning an identity LUT.
    if factor == 1.0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    let factor_fp = (factor * 1000.0) as u32;
    let shift_layout = matches!(
        (&*img, mode),
        (DynamicImage::ImageLumaA8(_), None | Some("LA"))
    );
    if shift_layout
        && let Some(shift) = match factor_fp {
            500 => Some(1),
            250 => Some(2),
            125 => Some(3),
            _ => None,
        }
    {
        return native_brightness_transform_in_place(img, mode, |input| input >> shift);
    }
    let lut: Vec<u8> = (0u32..=255)
        .map(|value| ((value as u64 * factor_fp as u64) / 1000).min(255) as u8)
        .collect();
    let Some(tables) = native_lut_tables(&lut) else {
        return false;
    };
    native_brightness_transform_in_place(img, mode, |input| native_lut_chunk(input, &tables))
}

#[cfg(test)]
mod tests;
