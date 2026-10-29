//! SIMD Convert family adapter helpers.
//!
//! Keep mode and architecture dispatch here; focused kernels perform native
//! byte-layout work and report the path used to the registry-facing adapter.

#[path = "convert/alpha.rs"]
mod alpha;
#[path = "convert/cmyk.rs"]
mod cmyk;
#[cfg(all(test, target_arch = "x86_64"))]
#[path = "convert/tests/cmyk_packing.rs"]
mod cmyk_packing_tests;
#[path = "convert/kernels/mod.rs"]
mod kernels;
use super::{
    SimdImageShape, SimdLayout, grayscale, native_byte_layout, shape_native_byte_channels,
    simd_unsupported,
};
use crate::error::PilError;
use crate::compute::{Backend, ExecutionContext};
use crate::pipeline::{ColorMode, PipelineOp};
use crate::raster::DynamicImage;
use wide::{u8x16, u16x8, u16x16, u32x8};

use self::cmyk::native_cmyk_to_rgb_bytes;
#[cfg(feature = "parallel")]
pub(super) use self::kernels::SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD;
use self::kernels::native_rgba_to_rgb_bytes;
pub(super) use self::kernels::{
    native_convert_bytes, native_convert_layout_block, native_rgb_to_rgba_bytes,
    native_rgba_color_degenerate,
};
#[cfg(test)]
pub(super) use self::kernels::{native_luma_to_rgb_bytes, native_luma_to_rgba_bytes};

#[derive(Clone, Copy)]
pub(super) struct NativeConvertLayout {
    pub(super) source_channels: usize,
    pub(super) target_channels: usize,
    pub(super) source_is_luma: bool,
    pub(super) source_is_rgbx: bool,
    pub(super) target_is_luma: bool,
    pub(super) target_is_cmyk: bool,
    pub(super) target_is_hsv: bool,
    pub(super) target_is_ycbcr: bool,
    pub(super) target_is_integer: bool,
    pub(super) target_is_float: bool,
}

pub(super) fn supports_lab_for_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .is_some_and(|pixels| img.as_bytes().len() == pixels.saturating_mul(3))
}

pub(super) fn supports_lab_for_shape(shape: SimdImageShape, mode: Option<&str>) -> bool {
    matches!(shape.layout, SimdLayout::Rgb8)
        && matches!(mode, None | Some("RGB"))
        && (shape.width as usize)
            .checked_mul(shape.height as usize)
            .is_some()
}

fn native_convert_target(mode: &ColorMode) -> Option<(usize, bool, bool, bool, bool)> {
    match mode {
        ColorMode::L => Some((1, true, false, false, false)),
        ColorMode::LA => Some((2, true, false, false, false)),
        ColorMode::RGB => Some((3, false, false, false, false)),
        ColorMode::RGBA => Some((4, false, false, false, false)),
        // Convert.c writes C/M/Y/K directly for byte RGB-family and luma
        // sources. The raster layer carries those four native samples in
        // Rgba8, but the fourth byte is K, never alpha.
        ColorMode::CMYK => Some((4, false, true, false, false)),
        // Pillow stores HSV and YCbCr in an RGB-sized byte buffer. These
        // destinations have distinct arithmetic contracts, so they are
        // admitted only through their dedicated native kernels below.
        ColorMode::HSV => Some((3, false, false, true, false)),
        ColorMode::YCbCr => Some((3, false, false, false, true)),
        // Pillow keeps I and F as four-byte scalar samples. The surrounding
        // image layer stores those bytes in Rgba8, but the logical target is
        // a luma sample, not an RGBA pixel; their arithmetic is handled by
        // the dedicated vector path below.
        ColorMode::I | ColorMode::F => Some((4, false, false, false, false)),
        _ => None,
    }
}

fn native_convert_luma16_supported(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> bool {
    matches!(
        target,
        ColorMode::L
            | ColorMode::LA
            | ColorMode::RGB
            | ColorMode::RGBA
            | ColorMode::CMYK
            | ColorMode::I
            | ColorMode::F
    ) && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(img, DynamicImage::ImageLuma16(_))
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .is_some()
}

/// Convert native unsigned-16-bit luma to the byte L/LA destinations.
///
/// Pillow's I;16 converter clamps each unsigned sample to the byte range; it
/// does not scale 65535 down to 255. The sample load and clamp use eight
/// `u16` lanes, then the result is packed into the native byte destination.
/// This stays separate from the ordinary byte converter so raw endianness is
/// resolved once by `Image::frombytes` and is never reinterpreted as pixels.
fn native_convert_luma16_bytes(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<(Vec<u8>, u64, u64)> {
    if !native_convert_luma16_supported(img, target, mode) {
        return None;
    }
    let DynamicImage::ImageLuma16(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels {
        return None;
    }
    let source = source.as_raw();
    let output_channels = match target {
        ColorMode::L => 1,
        ColorMode::LA => 2,
        ColorMode::RGB => 3,
        ColorMode::RGBA | ColorMode::CMYK | ColorMode::I | ColorMode::F => 4,
        _ => return None,
    };
    let mut output = Vec::with_capacity(pixels * output_channels);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let samples = std::array::from_fn(|lane| {
            if lane < active {
                source[start + lane]
            } else {
                0
            }
        });
        let sample_lanes = u16x8::new(samples);
        if matches!(target, ColorMode::I) {
            for &value in u32x8::from(sample_lanes).to_array().iter().take(active) {
                output.extend_from_slice(&(value as i32).to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        if matches!(target, ColorMode::F) {
            // A u16 is exactly representable as f32. Form its IEEE exponent
            // and mantissa with vector integer operations to avoid a scalar
            // lane conversion masquerading as SIMD execution.
            let values = u32x8::from(sample_lanes);
            let mut bits = u32x8::splat(0);
            for exponent in 0..16u32 {
                let base = 1u32 << exponent;
                let range =
                    values.simd_ge(u32x8::splat(base)) & values.simd_lt(u32x8::splat(base << 1));
                let mantissa = (values - u32x8::splat(base)) << (23 - exponent);
                let encoded = u32x8::splat((exponent + 127) << 23) | mantissa;
                bits = range.select(encoded, bits);
            }
            for &value in bits.to_array().iter().take(active) {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        let samples = sample_lanes
            .simd_gt(u16x8::splat(u16::from(u8::MAX)))
            .select(u16x8::splat(u16::from(u8::MAX)), sample_lanes)
            .to_array()
            .map(|value| value as u8);
        for &value in samples.iter().take(active) {
            match target {
                ColorMode::L => output.push(value),
                ColorMode::LA => output.extend_from_slice(&[value, u8::MAX]),
                ColorMode::RGB => output.extend_from_slice(&[value, value, value]),
                ColorMode::RGBA => output.extend_from_slice(&[value, value, value, u8::MAX]),
                ColorMode::CMYK => {
                    output.extend_from_slice(&[0, 0, 0, u8::MAX.saturating_sub(value)])
                }
                _ => return None,
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

fn native_convert_layout(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<NativeConvertLayout> {
    // RGBa needs unpremultiplication before luma. The byte converter below
    // has no such arithmetic; retain the exact CPU path for this contract.
    if mode == Some("RGBa") && matches!(target, ColorMode::L) {
        return None;
    }
    let (target_channels, target_is_luma, target_is_cmyk, target_is_hsv, target_is_ycbcr) =
        native_convert_target(target)?;
    let source_channels = if target_is_hsv || target_is_ycbcr {
        match img {
            DynamicImage::ImageRgba8(_) if mode == Some("RGBX") => 4,
            _ => native_byte_layout(img, mode)?,
        }
    } else {
        native_byte_layout(img, mode)?
    };
    Some(NativeConvertLayout {
        source_channels,
        target_channels,
        source_is_luma: source_channels <= 2,
        source_is_rgbx: mode == Some("RGBX"),
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
        target_is_integer: matches!(target, ColorMode::I),
        target_is_float: matches!(target, ColorMode::F),
    })
}

pub(super) fn native_convert_pixels_per_vector(layout: NativeConvertLayout) -> usize {
    if layout.target_is_cmyk {
        // Four output samples are emitted per input pixel. The inverse and
        // luma-to-K subtraction both run in byte vector lanes.
        4
    } else if layout.target_is_hsv || layout.target_is_ycbcr {
        // Both color-space kernels process eight RGB-family pixels per wide
        // block. Their output is three bytes per pixel, so the block is
        // gathered/scattered by lane while the arithmetic stays vectorized.
        8
    } else if layout.target_is_integer || layout.target_is_float {
        // I/F emit one four-byte scalar per pixel. Gather eight source
        // pixels and vectorize the luma arithmetic before packing the native
        // little-endian samples.
        8
    } else if !layout.source_is_luma && layout.target_is_luma {
        // The luma formula uses eight u32 lanes. Input bytes are gathered
        // into those lanes before the vector arithmetic begins.
        8
    } else {
        // The layout-only path loads one padded u8x16 block and stores at
        // most one u8x16 block. This also handles three-byte RGB pixels
        // without converting them through a packed RGBA representation.
        16 / layout.source_channels.max(layout.target_channels)
    }
}

pub(super) fn native_convert_supported_for_image(
    img: &DynamicImage,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> bool {
    // Pillow ignores the dither enum for these standard byte destinations;
    // only matrix conversion changes the arithmetic contract.
    if matrix.is_some() {
        return false;
    }
    if alpha::supports_image(img, target, mode, matrix) {
        return true;
    }
    if matches!(target, ColorMode::I16) {
        return matches!(mode, Some("1" | "L"))
            && matches!(img, DynamicImage::ImageLuma8(_))
            && (img.width() as usize)
                .checked_mul(img.height() as usize)
                .is_some_and(|pixels| img.as_bytes().len() == pixels);
    }
    if matches!(target, ColorMode::RGB) && mode == Some("CMYK") {
        let Some(pixel_count) = (img.width() as usize).checked_mul(img.height() as usize) else {
            return false;
        };
        return matches!(img, DynamicImage::ImageRgba8(_))
            && pixel_count.checked_mul(4) == Some(img.as_bytes().len());
    }
    if native_convert_luma16_supported(img, target, mode) {
        return true;
    }
    let Some(layout) = native_convert_layout(img, target, mode) else {
        return false;
    };
    let Some(pixel_count) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    let Some(expected_bytes) = pixel_count.checked_mul(layout.source_channels) else {
        return false;
    };
    // Every native converter pads its final vector block. This includes
    // one-pixel images: the inactive lanes are zero-filled and never copied
    // to the output, so a short public input still exercises the real SIMD
    // data path instead of being rejected solely for its width.
    // Empty conversions are valid scalar control in this executor; they
    // perform no vector work and retain their destination shape/mode.
    img.as_bytes().len() == expected_bytes
}

fn native_convert_shape_layout(
    shape: SimdImageShape,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<NativeConvertLayout> {
    if mode == Some("RGBa") && matches!(target, ColorMode::L) {
        return None;
    }
    let (target_channels, target_is_luma, target_is_cmyk, target_is_hsv, target_is_ycbcr) =
        native_convert_target(target)?;
    let source_channels = if target_is_hsv || target_is_ycbcr {
        match (shape.layout, mode) {
            (SimdLayout::Rgba8, Some("RGBX")) => 4,
            _ => shape_native_byte_channels(shape, mode)?,
        }
    } else {
        shape_native_byte_channels(shape, mode)?
    };
    Some(NativeConvertLayout {
        source_channels,
        target_channels,
        source_is_luma: source_channels <= 2,
        source_is_rgbx: mode == Some("RGBX"),
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
        target_is_integer: matches!(target, ColorMode::I),
        target_is_float: matches!(target, ColorMode::F),
    })
}

pub(super) fn native_convert_supported_for_shape(
    shape: SimdImageShape,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> bool {
    if matrix.is_some() {
        return false;
    }
    if alpha::supports_shape(
        shape.layout,
        shape.width,
        shape.height,
        target,
        mode,
        matrix,
    ) {
        return true;
    }
    if matches!(target, ColorMode::I16) {
        return matches!(mode, Some("1" | "L"))
            && shape.layout == SimdLayout::Luma8
            && (shape.width as usize)
                .checked_mul(shape.height as usize)
                .is_some();
    }
    if matches!(target, ColorMode::RGB) && mode == Some("CMYK") {
        return shape.layout == SimdLayout::Rgba8
            && (shape.width as usize)
                .checked_mul(shape.height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .is_some();
    }
    if matches!(
        target,
        ColorMode::L
            | ColorMode::LA
            | ColorMode::RGB
            | ColorMode::RGBA
            | ColorMode::CMYK
            | ColorMode::I
            | ColorMode::F
    ) && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(shape.layout, SimdLayout::Luma16)
    {
        return (shape.width as usize)
            .checked_mul(shape.height as usize)
            .is_some();
    }
    let Some(_layout) = native_convert_shape_layout(shape, target, mode) else {
        return false;
    };
    // Match image preflight, including empty control-only conversions.
    (shape.width as usize)
        .checked_mul(shape.height as usize)
        .is_some()
}

#[inline(always)]
fn finish_vector_conversion(
    img: &DynamicImage,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
    path: &'static str,
    record_zero: bool,
) -> Result<DynamicImage, PilError> {
    if record_zero || vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if record_zero || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(path);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
}

fn try_source_special_conversion(
    context: &ExecutionContext,
    img: &DynamicImage,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if let Some(converted) = alpha::try_convert(img, target, mode, matrix)? {
        return Ok(Some(converted));
    }
    if (img.width() == 0 || img.height() == 0)
        && native_convert_supported_for_image(img, target, matrix, mode)
    {
        let Some((channels, ..)) = native_convert_target(target) else {
            return Err(simd_unsupported("Convert"));
        };
        crate::compute::record_pipeline_operation_path("scalar-control");
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            Vec::new(),
            channels,
        )
        .map(Some);
    }
    if matches!(target, ColorMode::RGB) && mode == Some("CMYK") {
        if !native_convert_supported_for_image(img, target, matrix, mode) {
            return Err(simd_unsupported("Convert"));
        }
        let (output, vector_blocks, scalar_tail) =
            native_cmyk_to_rgb_bytes(context, img).ok_or_else(|| simd_unsupported("Convert"))?;
        return finish_vector_conversion(
            img,
            output,
            3,
            vector_blocks,
            scalar_tail,
            "vector",
            false,
        )
        .map(Some);
    }
    if let Some((output, vector_blocks, scalar_tail)) =
        native_convert_luma16_bytes(img, target, mode)
    {
        let channels = match target {
            ColorMode::L => 1,
            ColorMode::LA => 2,
            ColorMode::RGB => 3,
            ColorMode::RGBA | ColorMode::CMYK | ColorMode::I | ColorMode::F => 4,
            _ => return Err(simd_unsupported("Convert")),
        };
        return finish_vector_conversion(
            img,
            output,
            channels,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    Ok(None)
}

fn try_layout_special_conversion(
    context: &ExecutionContext,
    img: &DynamicImage,
    target: &ColorMode,
    layout: NativeConvertLayout,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(target, ColorMode::RGB)
        && matches!(mode, None | Some("RGBA"))
        && matches!(img, DynamicImage::ImageRgba8(_))
        && let Some((output, vector_blocks, scalar_tail, path)) =
            native_rgba_to_rgb_bytes(context, img)
    {
        return finish_vector_conversion(img, output, 3, vector_blocks, scalar_tail, path, false)
            .map(Some);
    }
    if matches!(target, ColorMode::L)
        && layout.source_channels == 3
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
    {
        let Some((output, vector_blocks, scalar_tail)) =
            grayscale::native_grayscale_bytes(context, img, 3)
        else {
            return Err(simd_unsupported("Convert"));
        };
        return finish_vector_conversion(
            img,
            output,
            1,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    if matches!(target, ColorMode::RGBA)
        && layout.source_channels == 3
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
    {
        let Some((output, vector_blocks, scalar_tail)) = native_rgb_to_rgba_bytes(img) else {
            return Err(simd_unsupported("Convert"));
        };
        return finish_vector_conversion(
            img,
            output,
            4,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    Ok(None)
}

pub(crate) fn simd_convert(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    simd_convert_in_context(
        &ExecutionContext::for_backend(Backend::Simd),
        img,
        op,
        mode,
    )
}

pub(super) fn simd_convert_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Simd || !context.architecture().supports_simd() {
        return Err(simd_unsupported("Convert"));
    }
    let PipelineOp::Convert {
        mode: target,
        matrix,
        dither: _,
    } = op
    else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    if matches!(target, ColorMode::I16) {
        if matrix.is_some() || !matches!(mode, Some("1" | "L")) {
            return Err(simd_unsupported("Convert"));
        }
        let DynamicImage::ImageLuma8(source) = img else {
            return Err(simd_unsupported("Convert"));
        };
        let pixels = (img.width() as usize)
            .checked_mul(img.height() as usize)
            .ok_or_else(|| simd_unsupported("Convert"))?;
        if source.as_raw().len() != pixels {
            return Err(simd_unsupported("Convert"));
        }
        let mut output = Vec::with_capacity(pixels);
        for start in (0..pixels).step_by(16) {
            let active = (pixels - start).min(16);
            let lanes =
                std::array::from_fn(|lane| source.as_raw().get(start + lane).copied().unwrap_or(0));
            let bytes = u8x16::new(lanes);
            let samples = if mode == Some("1") {
                bytes
                    .simd_eq(u8x16::ZERO)
                    .select(u8x16::ZERO, u8x16::splat(255))
            } else {
                bytes
            };
            output.extend(u16x16::from(samples).to_array().into_iter().take(active));
        }
        crate::compute::record_pipeline_operation_vector_blocks(pixels.div_ceil(16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        crate::compute::record_pipeline_operation_path("vector");
        let image = crate::raster::ImageBuffer::from_raw(img.width(), img.height(), output)
            .ok_or_else(|| PilError::InternalError("SIMD I;16 output shape mismatch".into()))?;
        return Ok(DynamicImage::ImageLuma16(image));
    }
    if let Some(result) = try_source_special_conversion(
        context,
        img,
        target,
        matrix.as_deref(),
        mode,
    )? {
        return Ok(result);
    }
    let Some(layout) = native_convert_layout(img, target, mode) else {
        return Err(simd_unsupported("Convert"));
    };
    if !native_convert_supported_for_image(img, target, matrix.as_deref(), mode) {
        return Err(simd_unsupported("Convert"));
    }
    if let Some(result) = try_layout_special_conversion(context, img, target, layout, mode)? {
        return Ok(result);
    }
    let Some((output, vector_blocks, scalar_tail)) = native_convert_bytes(img, layout) else {
        return Err(simd_unsupported("Convert"));
    };
    if vector_blocks == 0 {
        return Err(simd_unsupported("Convert"));
    }
    finish_vector_conversion(
        img,
        output,
        layout.target_channels,
        vector_blocks,
        scalar_tail,
        "vector",
        true,
    )
}

pub(crate) fn simd_convert_lab(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::ConvertLab { .. }) {
        return Err(PilError::ValueError("expected ConvertLab op".into()));
    }
    if !matches!(img, DynamicImage::ImageRgb8(_)) || !matches!(mode, None | Some("RGB")) {
        return Err(simd_unsupported("ConvertLab"));
    }
    crate::compute::record_pipeline_operation_path("scalar-control");
    crate::lab::convert_rgb_image(img)
}

#[cfg(test)]
#[path = "convert/tests.rs"]
mod tests;
