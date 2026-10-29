//! SIMD adapter for Pillow's module-level image blend.

use super::chops::materialize_chops_operand;
use super::{SimdImageShape, SimdLayout, simd_unsupported};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;
use wide::{f32x4, u32x4};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::BlendModule { other, alpha } => {
            simd_module_blend_supported(img, other, mode, *alpha)
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
        PipelineOp::BlendModule { other, alpha } => {
            shape_module_blend_supported(shape, other, mode, *alpha)
        }
        _ => false,
    }
}

/// Return the native byte count for one of Pillow's `Image.blend` mode
/// families. Unlike ImageChops, module-level blend accepts the three-byte
/// HSV/YCbCr family and the four-byte RGBa/RGBX family as raw native samples.
fn native_module_blend_mode_channels(mode: &str) -> Option<usize> {
    match mode {
        "L" => Some(1),
        "LA" | "La" => Some(2),
        // LAB requires per-operand bias removal; keep it on the exact CPU path.
        "RGB" | "HSV" | "YCbCr" => Some(3),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => Some(4),
        _ => None,
    }
}

/// Validate a concrete image against the logical mode supplied at an
/// operation boundary and return its native interleaved byte count.
fn native_module_blend_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    let channels = match mode {
        Some(mode) => native_module_blend_mode_channels(mode)?,
        None => match img {
            DynamicImage::ImageLuma8(_) => 1,
            DynamicImage::ImageLumaA8(_) => 2,
            DynamicImage::ImageRgb8(_) => 3,
            DynamicImage::ImageRgba8(_) => 4,
            _ => return None,
        },
    };
    match (img, channels) {
        (DynamicImage::ImageLuma8(_), 1)
        | (DynamicImage::ImageLumaA8(_), 2)
        | (DynamicImage::ImageRgb8(_), 3)
        | (DynamicImage::ImageRgba8(_), 4) => Some(channels),
        _ => None,
    }
}

fn native_module_blend_pair_channels(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    other_mode: Option<&str>,
) -> Option<usize> {
    let channels = native_module_blend_layout(img, mode)?;
    (native_module_blend_layout(other, other_mode) == Some(channels)
        && img.dimensions() == other.dimensions())
    .then_some(channels)
}

fn native_module_blend_data_supported(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    let actual_len = img.as_bytes().len();
    actual_len == expected_len && (actual_len == 0 || actual_len >= 8)
}

/// Check the module-level blend contract without materializing the secondary
/// image. Pillow validates the mode family and dimensions before touching
/// pixels; only a non-empty byte buffer needs the SIMD block-size gate.
fn simd_module_blend_supported(
    img: &DynamicImage,
    other: &Image,
    mode: Option<&str>,
    alpha: f64,
) -> bool {
    if !native_module_blend_arithmetic_supported(alpha) {
        return false;
    }
    let Some(channels) = native_module_blend_layout(img, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    if native_module_blend_mode_channels(&other_mode) != Some(channels)
        || other.size().ok() != Some(img.dimensions())
    {
        return false;
    }
    native_module_blend_data_supported(img, channels)
}

fn shape_native_module_blend_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, None | Some("L")) => Some(1),
        (SimdLayout::LumaA8, None | Some("LA")) => Some(2),
        (SimdLayout::Rgb8, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        (SimdLayout::Rgba8, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) => Some(4),
        _ => None,
    }
}

fn shape_module_blend_supported(
    shape: SimdImageShape,
    other: &Arc<Image>,
    mode: Option<&str>,
    alpha: f64,
) -> bool {
    if !native_module_blend_arithmetic_supported(alpha) {
        return false;
    }
    let Some(channels) = shape_native_module_blend_channels(shape, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    if native_module_blend_mode_channels(&other_mode) != Some(channels)
        || other.size().ok() != Some((shape.width, shape.height))
    {
        return false;
    }
    let Some(total_bytes) = (shape.width as usize)
        .checked_mul(shape.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    total_bytes == 0 || total_bytes >= 8
}

#[inline]
fn native_module_blend_byte(left: u8, right: u8, alpha: f64) -> u8 {
    let value = (alpha as f32).mul_add(f32::from(right) - f32::from(left), f32::from(left));
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

/// Whether wide's mul_add guarantees fused float32 arithmetic on this target.
/// Its non-FMA fallback has two roundings and differs on extrapolation.
pub(super) fn native_module_blend_arithmetic_supported(alpha: f64) -> bool {
    cfg!(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        )
    )) && alpha.is_finite()
        && (alpha as f32).is_finite()
}

/// Blend independent bytes in four packed words. Extracting a byte position
/// from each word avoids scalar gathers and widening/repacking temporary arrays.
#[inline]
fn native_module_blend_block(left: [u8; 16], right: [u8; 16], alpha: f32x4) -> [u8; 16] {
    const BYTE: u32x4 = u32x4::new([255; 4]);
    const MAX: f32x4 = f32x4::new([255.0; 4]);
    let left: [u32; 4] = bytemuck::cast(left);
    let right: [u32; 4] = bytemuck::cast(right);
    let left = u32x4::new(left.map(u32::from_le));
    let right = u32x4::new(right.map(u32::from_le));
    let channel = |shift: u32| {
        let a: wide::i32x4 = bytemuck::cast((left >> shift) & BYTE);
        let b: wide::i32x4 = bytemuck::cast((right >> shift) & BYTE);
        let a = a.round_float();
        let value = alpha
            .mul_add(b.round_float() - a, a)
            .max(f32x4::ZERO)
            .min(MAX);
        let result: u32x4 = bytemuck::cast(value.trunc_int());
        result << shift
    };
    let result = channel(0) | channel(8) | channel(16) | channel(24);
    bytemuck::cast(result.to_array().map(u32::to_le))
}

/// Blend two matching native byte images with fused f32 vector arithmetic.
///
/// Pillow's `Image.blend` interpolates every stored sample independently,
/// including alpha and CMYK K. Since the formula has no row-dependent state,
/// vectors may cross row boundaries and only the final partial block is scalar.
fn native_module_blend(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    other_mode: Option<&str>,
    alpha: f64,
) -> Option<DynamicImage> {
    if !native_module_blend_arithmetic_supported(alpha) {
        return None;
    }
    let channels = native_module_blend_pair_channels(img, other, mode, other_mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    let expected_len = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))?;
    if left.len() != right.len()
        || left.len() != expected_len
        || !left.len().is_multiple_of(channels)
    {
        return None;
    }
    if left.is_empty() {
        // Pillow returns a correctly typed zero-pixel image without entering
        // its arithmetic loop. Keep this a native-copy receipt; non-empty
        // inputs remain subject to the real eight-byte vector path below.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if left.len() < 8 {
        return None;
    }
    let mut output = vec![0u8; left.len()];
    let alpha_vector = f32x4::splat(alpha as f32);
    let mut left_chunks = left.chunks_exact(16);
    let mut right_chunks = right.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((left, right), output) in left_chunks
        .by_ref()
        .zip(right_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        output.copy_from_slice(&native_module_blend_block(
            left.try_into().expect("complete vector"),
            right.try_into().expect("complete vector"),
            alpha_vector,
        ));
    }
    let remainder = output_chunks.into_remainder();
    let mut vector_blocks = left.len() / 16;
    let scalar_tail = if remainder.len() >= 8 {
        // Retain the existing eight-byte admission boundary without reading
        // beyond the final slice. Padding contributes no observable samples.
        let mut left = [0; 16];
        let mut right = [0; 16];
        left[..remainder.len()].copy_from_slice(left_chunks.remainder());
        right[..remainder.len()].copy_from_slice(right_chunks.remainder());
        remainder.copy_from_slice(
            &native_module_blend_block(left, right, alpha_vector)[..remainder.len()],
        );
        vector_blocks += 1;
        0
    } else {
        for ((out, &left), &right) in remainder
            .iter_mut()
            .zip(left_chunks.remainder())
            .zip(right_chunks.remainder())
        {
            *out = native_module_blend_byte(left, right, alpha);
        }
        remainder.len()
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

pub fn simd_blend_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::BlendModule { other, alpha } = op else {
        return Err(PilError::ValueError("expected BlendModule op".into()));
    };
    if !alpha.is_finite() {
        return Err(simd_unsupported("BlendModule"));
    }
    let other_mode = other.mode()?;
    let other_img = materialize_chops_operand(other, mode)?;
    native_module_blend(img, &other_img, mode, Some(&other_mode), *alpha)
        .ok_or_else(|| simd_unsupported("BlendModule"))
}
