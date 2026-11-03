//! Native u16 Convert outputs shared by the four exact I;16 source strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::compute::{Architecture, registry::support::{ImplementationSupport, OperationId, StrategyGuard}};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Mode,
    Rgb, RgbImage, Rgba, RgbaImage,
};
use wide::{u16x8, u32x8};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

macro_rules! architecture_strategy {
    ($mode:expr, $target:ident, $architecture:ident, $available:expr, $requirements:expr) => {
        ImplementationSupport::cpu(
            OperationId::Convert,
            $mode,
            Architecture::$architecture,
            $available,
            $requirements,
            StrategyGuard::ConvertTargetMode(Mode::$target),
        )
    };
}

pub(crate) const fn architecture_support(mode: Mode) -> [ImplementationSupport; 21] {
    [
        architecture_strategy!(mode, ImageLuma8, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageLuma8, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageLuma8, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageLumaA8, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageLumaA8, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageLumaA8, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageRgb8, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageRgb8, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageRgb8, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageRgba8, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageRgba8, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageRgba8, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageCmyk8, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageCmyk8, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageCmyk8, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageI32, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageI32, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageI32, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
        architecture_strategy!(mode, ImageF32, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]),
        architecture_strategy!(mode, ImageF32, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")), &[Architecture::Neon]),
        architecture_strategy!(mode, ImageF32, WasmSimd128, cfg!(all(target_arch = "wasm32", target_feature = "simd128")), &[Architecture::WasmSimd128]),
    ]
}

/// Converts a native u16 raster; I;16N changes decode byte order, not sample range.
pub(crate) fn execute_mode(
    source: &Samples,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    if is_luma16_target(target) {
        return execute_i16_target(source, target);
    }
    execute(source, target)
}

pub(crate) fn from_luma8_samples<I>(
    width: u32,
    height: u32,
    target: Mode,
    samples: I,
) -> Result<DynamicImage, PilError>
where
    I: Iterator<Item = u8>,
{
    from_u16_samples(width, height, target, samples.map(u16::from))
}

pub(crate) fn from_u16_samples<I>(
    width: u32,
    height: u32,
    target: Mode,
    samples: I,
) -> Result<DynamicImage, PilError>
where
    I: Iterator<Item = u16>,
{
    let values = samples.collect::<Vec<_>>();
    let output = Samples::from_raw(width, height, values)
        .ok_or_else(|| PilError::InternalError("I;16 Convert output shape mismatch".into()))?;
    match target {
        Mode::ImageI16 => Ok(DynamicImage::ImageI16(output)),
        Mode::ImageI16L => Ok(DynamicImage::ImageI16L(output)),
        Mode::ImageI16B => Ok(DynamicImage::ImageI16B(output)),
        Mode::ImageI16N => Ok(DynamicImage::ImageI16N(output)),
        _ => Err(PilError::InternalError(
            "I;16 Convert selected an undeclared target mode".into(),
        )),
    }
}

// NO_PILLOW_SUPPORT: Pillow's palette converter rejects all four I;16 sources.
pub(crate) fn execute_palette_target(
    source: &Samples,
    target: Mode,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    // Match DynamicImage::to_luma8's saturating u16-to-u8 mapping without
    // materializing an intermediate L raster before building palette indices.
    // TODO(pillow-rs#3ptb): cover every exact I;16 source, P/PA target, and
    // requested Architecture in CPU parity.
    super::luma8::execute_palette_target(
        source.width(),
        source.height(),
        target,
        source
            .pixels()
            .map(|pixel| pixel[0].min(u16::from(u8::MAX)) as u8),
    )
}

pub(crate) fn execute_native_vector(
    source: &Samples,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    execute_vector(source, target)
}

fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(to_luma(source))),
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(to_luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(to_rgb(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(to_rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(to_cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(to_ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(to_hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn execute_i16_target(
    source: &Samples,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    // TODO(pillow-rs#3ptb): verify CoreOnly source/target alias pairs with
    // samples above 255 and mode-specific byte export in the CPU parity lane.
    // I;16 variants share native u16 storage; only their external byte-order
    // contract differs. Preserve every sample when selecting another I;16
    // identity so reads and writes apply endianness at the image boundary.
    let samples = source.clone();
    match target {
        Mode::ImageI16 => Ok(DynamicImage::ImageI16(samples)),
        Mode::ImageI16L => Ok(DynamicImage::ImageI16L(samples)),
        Mode::ImageI16B => Ok(DynamicImage::ImageI16B(samples)),
        Mode::ImageI16N => Ok(DynamicImage::ImageI16N(samples)),
        _ => Err(PilError::InternalError(
            "I;16 Convert selected an undeclared target mode".into(),
        )),
    }
}

pub(crate) const fn is_luma16_target(target: Mode) -> bool {
    matches!(
        target,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

fn execute_vector(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    let target_channels = vector_target_channels(target)?;
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 1)?;
    if source.as_raw().len() != source_dims.total_pixels() {
        return Err(PilError::InternalError(
            "I;16 Convert source storage did not match its dimensions".into(),
        ));
    }
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), target_channels)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let lanes = std::array::from_fn(|lane| source.as_raw().get(start + lane).copied().unwrap_or(0));
        let samples = u16x8::new(lanes);
        append_target_samples(target, samples, active, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish_vector(source, target, output, output_dims.total_bytes(), vector_blocks)
}

fn vector_target_channels(target: Mode) -> Result<u8, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(1),
        Mode::ImageLumaA8 => Ok(2),
        Mode::ImageRgb8 => Ok(3),
        Mode::ImageRgba8 | Mode::ImageCmyk8 | Mode::ImageI32 | Mode::ImageF32 => Ok(4),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn append_target_samples(
    target: Mode,
    samples: u16x8,
    active: usize,
    output: &mut Vec<u8>,
) {
    if target == Mode::ImageI32 {
        for value in samples.to_array().into_iter().take(active) {
            output.extend_from_slice(&i32::from(value).to_le_bytes());
        }
    } else if target == Mode::ImageF32 {
        append_float_samples(samples, active, output);
    } else {
        let clipped = samples
            .simd_gt(u16x8::splat(u16::from(u8::MAX)))
            .select(u16x8::splat(u16::from(u8::MAX)), samples)
            .to_array()
            .map(|value| value as u8);
        append_clipped_samples(target, clipped, active, output);
    }
}

fn append_float_samples(samples: u16x8, active: usize, output: &mut Vec<u8>) {
    let samples = u32x8::from(samples);
    let mut bits = u32x8::splat(0);
    for exponent in 0..16u32 {
        let base = 1u32 << exponent;
        let range = samples.simd_ge(u32x8::splat(base)) & samples.simd_lt(u32x8::splat(base << 1));
        let mantissa = (samples - u32x8::splat(base)) << (23 - exponent);
        let encoded = u32x8::splat((exponent + 127) << 23) | mantissa;
        bits = range.select(encoded, bits);
    }
    append_f32_bytes(
        bits.to_array().map(f32::from_bits),
        active,
        output,
    );
}

fn append_f32_bytes(values: [f32; 8], active: usize, output: &mut Vec<u8>) {
    for value in values.into_iter().take(active) {
        output.extend_from_slice(&value.to_le_bytes());
    }
}

fn append_clipped_samples(target: Mode, samples: [u8; 8], active: usize, output: &mut Vec<u8>) {
    for sample in samples.into_iter().take(active) {
        match target {
            Mode::ImageLuma8 => output.push(sample),
            Mode::ImageLumaA8 => output.extend_from_slice(&[sample, u8::MAX]),
            Mode::ImageRgb8 => output.extend_from_slice(&[sample, sample, sample]),
            Mode::ImageRgba8 => output.extend_from_slice(&[sample, sample, sample, u8::MAX]),
            Mode::ImageCmyk8 => output.extend_from_slice(&[0, 0, 0, u8::MAX.wrapping_sub(sample)]),
            _ => unreachable!("target was classified before writing native u16 samples"),
        }
    }
}

fn finish_vector(
    source: &Samples,
    target: Mode,
    output: Vec<u8>,
    expected_bytes: usize,
    vector_blocks: u64,
) -> Result<DynamicImage, PilError> {
    if output.len() != expected_bytes {
        return Err(PilError::InternalError("I;16 Convert output shape mismatch".into()));
    }
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        crate::compute::record_pipeline_operation_path("vector");
    }
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    let image = match target {
        Mode::ImageLuma8 => GrayImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageLuma8),
        Mode::ImageLumaA8 => GrayAlphaImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageLumaA8),
        Mode::ImageRgb8 => RgbImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageRgb8),
        Mode::ImageRgba8 => RgbaImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageRgba8),
        Mode::ImageCmyk8 => RgbaImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageCmyk8),
        Mode::ImageI32 => RgbaImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageI32),
        Mode::ImageF32 => RgbaImage::from_raw(source.width(), source.height(), output)
            .map(DynamicImage::ImageF32),
        _ => None,
    };
    image.ok_or_else(|| PilError::InternalError("I;16 Convert output shape mismatch".into()))
}

fn to_luma(source: &Samples) -> GrayImage {
    GrayImage::from_fn(source.width(), source.height(), |x, y| {
        Luma([clip(source.get_pixel(x, y)[0])])
    })
}

fn to_luma_alpha(source: &Samples) -> GrayAlphaImage {
    GrayAlphaImage::from_fn(source.width(), source.height(), |x, y| {
        LumaA([clip(source.get_pixel(x, y)[0]), u8::MAX])
    })
}

fn to_rgb(source: &Samples) -> RgbImage {
    RgbImage::from_fn(source.width(), source.height(), |x, y| {
        let sample = clip(source.get_pixel(x, y)[0]);
        Rgb([sample, sample, sample])
    })
}

fn to_rgba(source: &Samples) -> RgbaImage {
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        let sample = clip(source.get_pixel(x, y)[0]);
        Rgba([sample, sample, sample, u8::MAX])
    })
}

fn to_cmyk(source: &Samples) -> RgbaImage {
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        Rgba([0, 0, 0, u8::MAX.wrapping_sub(clip(source.get_pixel(x, y)[0]))])
    })
}

fn to_ycbcr(source: &Samples) -> RgbImage {
    RgbImage::from_fn(source.width(), source.height(), |x, y| {
        Rgb([clip(source.get_pixel(x, y)[0]), 128, 128])
    })
}

fn to_hsv(source: &Samples) -> RgbImage {
    RgbImage::from_fn(source.width(), source.height(), |x, y| {
        Rgb([0, 0, clip(source.get_pixel(x, y)[0])])
    })
}

fn to_i32(source: &Samples) -> RgbaImage {
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        Rgba(i32::from(source.get_pixel(x, y)[0]).to_le_bytes())
    })
}

fn to_f32(source: &Samples) -> RgbaImage {
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        Rgba(f32::from(source.get_pixel(x, y)[0]).to_le_bytes())
    })
}

#[inline]
fn clip(sample: u16) -> u8 {
    sample.min(u16::from(u8::MAX)) as u8
}
