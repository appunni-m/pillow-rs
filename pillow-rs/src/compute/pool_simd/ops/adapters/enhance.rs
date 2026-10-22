//! SIMD adapters for Pillow ImageEnhance operations.

use super::lut::{native_lut_apply, native_lut_chunk, native_lut_tables};
use super::{
    SimdImageShape, SimdLayout, apply_native_rows, has_nonempty_byte_data,
    native_all_channel_transform_bytes, native_byte_transform_bytes,
    native_filter_3x3_channel_block_integer, native_filter_3x3_luma_block_integer,
    native_filter_3x3_pixel_block_integer, native_filter_3x3_rgb_byte_block_integer,
    native_filter_3x3_vector, native_filter_load_byte_block,
    native_module_blend_arithmetic_supported, native_rgba_color_degenerate, shape_channels,
    shape_has_nonempty_byte_data, simd_duplicate, simd_unsupported,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use wide::{f32x8, i16x8, u8x16};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
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

pub(super) fn supports_in_place(img: &DynamicImage, mode: Option<&str>) -> bool {
    native_brightness_layout(img, mode)
        .is_some_and(|channels| has_nonempty_byte_data(img, channels))
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
    let shift_layout = match (img, mode) {
        (DynamicImage::ImageLuma8(_), None | Some("L"))
        | (DynamicImage::ImageLumaA8(_), None | Some("LA"))
        | (DynamicImage::ImageRgb8(_), None | Some("RGB")) => true,
        _ => false,
    };
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
    if let Some(tables) = native_lut_tables(&lut) {
        if let Some(result) =
            native_brightness_transform(img, mode, |input| native_lut_chunk(input, &tables))
        {
            return Ok(result);
        }
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

/// Run Sharpness's 3x3 byte filter and Pillow's f32 blend in one output pass.
///
/// Interior filtered bytes are narrowed exactly as Pillow's SMOOTH result is
/// before they enter the blend. Edge pixels use the original byte as their
/// filtered value, preserving the reference's exact-copy border behavior.
/// LA/RGBA alpha stays copied from the source.
fn native_sharpness_filter_blend(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    mode: Option<&str>,
    kernel: &[f32; 9],
    rounding_bias: f32,
    factor: f64,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(channels)?;
    if source.len() != output.len()
        || source.len() != row_stride.checked_mul(height)?
        || !(1..=4).contains(&channels)
        || active_channels == 0
        || active_channels > channels
        || width < 3
        || height < 3
    {
        return None;
    }
    let interior_width = width - 2;
    let interior_height = height - 2;
    let alpha = factor as f32;
    // Preserve the scalar-gather path for finite f64 factors that overflow
    // when Pillow's blend factor is narrowed to f32. That path also defines
    // the unusual NaN behavior at unchanged border samples.
    let grouped_rgb_bytes = channels == 3 && active_channels == 3 && alpha.is_finite();
    let grouped_pixels =
        mode == Some("RGBX") && active_channels == channels && channels == 4 && alpha.is_finite();
    let pixels_per_block = (8 / channels).max(1);
    let vector_blocks = if grouped_rgb_bytes {
        (interior_width * channels).div_ceil(8) * interior_height
    } else if grouped_pixels {
        interior_width
            .div_ceil(pixels_per_block)
            .saturating_mul(interior_height)
    } else {
        interior_width
            .div_ceil(8)
            .saturating_mul(interior_height)
            .saturating_mul(active_channels)
    };
    if vector_blocks == 0 {
        return None;
    }
    let scalar_tail = (width * 2 + height * 2 - 4).saturating_mul(active_channels);
    let apply_row = |y: usize, row: &mut [u8]| {
        let source_row = y * row_stride;
        if (1..height - 1).contains(&y) {
            if grouped_rgb_bytes {
                // RGB's channel bytes are contiguous. Vectorize that native
                // stream directly instead of gathering one channel across
                // eight pixels for each of the three channels.
                let interior_end = (width - 1) * channels;
                let mut byte_offset = channels;
                while byte_offset < interior_end {
                    let active_bytes = (interior_end - byte_offset).min(8);
                    let smooth = native_filter_3x3_rgb_byte_block_integer(
                        source,
                        width,
                        y,
                        byte_offset,
                        active_bytes,
                    );
                    let original = native_filter_load_byte_block(
                        source,
                        source_row + byte_offset,
                        active_bytes,
                    );
                    let values = native_sharpness_blend_lanes(original, smooth, alpha).to_array();
                    for (lane, value) in values.into_iter().enumerate().take(active_bytes) {
                        row[byte_offset + lane] = native_sharpness_blend_result(value);
                    }
                    byte_offset += active_bytes;
                }
            } else if grouped_pixels {
                // RGBX sharpness processes the padding byte too. One x-vector
                // per channel performs four separate strided gathers; group
                // two adjacent pixels so the eight lanes are one compact span.
                let mut x = 1usize;
                while x < width - 1 {
                    let active_pixels = (width - 1 - x).min(pixels_per_block);
                    let smooth_bytes = native_filter_3x3_pixel_block_integer(
                        source,
                        width,
                        channels,
                        y,
                        x,
                        active_pixels * channels,
                    );
                    let active_bytes = active_pixels * channels;
                    let source_start = source_row + x * channels;
                    let original_bytes = std::array::from_fn(|lane| {
                        if lane < active_bytes {
                            source[source_start + lane]
                        } else {
                            0
                        }
                    });
                    let values =
                        native_sharpness_blend_integer_block(original_bytes, smooth_bytes, alpha);
                    for pixel in 0..active_pixels {
                        for channel in 0..channels {
                            let lane = pixel * channels + channel;
                            row[(x + pixel) * channels + channel] = values[lane];
                        }
                    }
                    x += active_pixels;
                }
            } else {
                for channel in 0..active_channels {
                    let first = source_row + channel;
                    row[channel] =
                        native_sharpness_blend_value(source[first], source[first], alpha);
                    let mut x = 1usize;
                    while x < width - 1 {
                        let active = (width - 1 - x).min(8);
                        let smooth = if alpha.is_finite() {
                            if channels == 1 && active == 8 {
                                native_filter_3x3_luma_block_integer(source, width, y, x)
                                    .unwrap_or_else(|| {
                                        native_filter_3x3_channel_block_integer(
                                            source, width, channels, channel, y, x, active,
                                        )
                                    })
                            } else {
                                native_filter_3x3_channel_block_integer(
                                    source, width, channels, channel, y, x, active,
                                )
                            }
                        } else {
                            // Keep the established floating path for finite
                            // f64 factors that overflow when narrowed to f32.
                            let smooth = native_filter_3x3_vector(
                                source,
                                width,
                                channels,
                                channel,
                                y,
                                x,
                                kernel,
                                rounding_bias,
                            );
                            f32x8::from(smooth.map(f32::from))
                        };
                        let original: [f32; 8] = std::array::from_fn(|lane| {
                            let pixel_x = (x + lane).min(width - 2);
                            f32::from(source[source_row + pixel_x * channels + channel])
                        });
                        let values =
                            native_sharpness_blend_lanes(f32x8::from(original), smooth, alpha)
                                .to_array();
                        for (lane, value) in values.into_iter().enumerate().take(active) {
                            row[(x + lane) * channels + channel] =
                                native_sharpness_blend_result(value);
                        }
                        x += active;
                    }
                    let last = source_row + (width - 1) * channels + channel;
                    row[(width - 1) * channels + channel] =
                        native_sharpness_blend_value(source[last], source[last], alpha);
                }
            }
        } else {
            for x in 0..width {
                for channel in 0..active_channels {
                    let index = source_row + x * channels + channel;
                    row[x * channels + channel] =
                        native_sharpness_blend_value(source[index], source[index], alpha);
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    super::simd_rows_mut_serial!(
        output,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            apply_row(y as usize, row);
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        apply_row(y, &mut output[row_start..row_start + row_stride]);
    }
    Some((vector_blocks as u64, scalar_tail as u64))
}

#[inline]
fn native_sharpness_blend_value(original: u8, smooth: u8, alpha: f32) -> u8 {
    native_sharpness_blend_result(
        alpha.mul_add(f32::from(original) - f32::from(smooth), f32::from(smooth)),
    )
}

#[inline]
fn native_sharpness_blend_lanes(original: f32x8, smooth: f32x8, alpha: f32) -> f32x8 {
    let delta = original - smooth;
    let alpha = f32x8::splat(alpha);
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        alpha.mul_add(delta, smooth)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let alpha = alpha.to_array();
        let delta = delta.to_array();
        let smooth = smooth.to_array();
        f32x8::new(std::array::from_fn(|lane| {
            alpha[lane].mul_add(delta[lane], smooth[lane])
        }))
    }
}

/// Blend grouped byte lanes with exact integer forms for common binary factors.
/// For 0.5, 1.5, and 2.0 the reference FMA is exact on byte-valued inputs; the
/// integer forms preserve its clamp and truncation while avoiding float lanes.
#[inline]
fn native_sharpness_blend_integer_block(original: [u8; 8], smooth: [u8; 8], alpha: f32) -> [u8; 8] {
    if alpha == 0.0 {
        return smooth;
    }
    let original = i16x8::new(original.map(i16::from));
    let smooth_lanes = i16x8::new(smooth.map(i16::from));
    let blended = if alpha == 0.5 {
        (original + smooth_lanes) >> 1u32
    } else if alpha == 1.5 {
        ((original * i16x8::splat(3) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(510)))
            >> 1u32
    } else if alpha == 2.0 {
        (original * i16x8::splat(2) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(255))
    } else {
        return native_sharpness_blend_lanes(
            f32x8::from(original.to_array().map(f32::from)),
            f32x8::from(smooth_lanes.to_array().map(f32::from)),
            alpha,
        )
        .to_array()
        .map(native_sharpness_blend_result);
    };
    blended
        .to_array()
        .map(|value| u8::try_from(value).unwrap_or_default())
}

#[inline]
fn native_sharpness_blend_result(value: f32) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

/// Apply ImageEnhance.Sharpness using the native interleaved layout.
///
/// Pillow first applies the 3x3 SMOOTH kernel, then blends that result with
/// the original image. The existing exact 3x3 SIMD kernel supplies the
/// neighborhood pass; this adapter keeps alpha out of that pass and uses
/// f32x8 fused multiply-add lanes to match Image.blend. No CPU pixel adapter
/// or packed RGBA conversion is used.
pub fn simd_sharpness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Sharpness { factor } = op else {
        return Err(PilError::ValueError("expected Sharpness op".into()));
    };
    let Some((channels, active_channels)) = native_sharpness_layout(img, mode) else {
        return Err(simd_unsupported("Sharpness"));
    };
    if !factor.is_finite() || !has_vectorized_sharpness_bytes(img, channels) {
        return Err(simd_unsupported("Sharpness"));
    }
    if *factor == 1.0 {
        // Reuse the native copy kernel: the exact identity needs neither the
        // 3x3 temporary nor a blend output pass.
        return simd_duplicate(img, &PipelineOp::Duplicate, mode);
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let source = img.as_bytes();
    let kernel = [
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        5.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
    ];
    let mut output = source.to_vec();
    let (vector_blocks, scalar_tail) = native_sharpness_filter_blend(
        source,
        &mut output,
        width,
        height,
        channels,
        active_channels,
        mode,
        &kernel,
        0.5,
        *factor,
    )
    .ok_or_else(|| simd_unsupported("Sharpness"))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

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
