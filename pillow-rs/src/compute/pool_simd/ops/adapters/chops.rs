//! SIMD ImageChops admission, native-byte kernels, and in-place execution.

mod kernels;

use self::kernels::{
    apply_native_blend_rows_in_place, native_chops_add_clamped, native_chops_add_modulo,
    native_chops_affine, native_chops_affine_in_place, native_chops_blend,
    native_chops_bytewise_in_place, native_chops_darker, native_chops_difference,
    native_chops_hard_light, native_chops_lighter, native_chops_logical_and,
    native_chops_logical_or, native_chops_logical_xor, native_chops_overlay,
    native_chops_pair_channels, native_chops_soft_light, native_chops_subtract_clamped,
    native_chops_subtract_modulo, native_logical_and_vector, native_logical_or_vector,
    native_logical_xor_vector, simd_fused_multiply_screen_row,
};
#[cfg(feature = "parallel")]
use super::simd_rows_mut_serial;
use super::{
    SimdImageShape, SimdLayout, has_empty_native_bytes, has_valid_byte_data,
    has_vectorized_byte_rows, materialize_chops_operand, shape_channels,
    shape_has_empty_native_bytes, shape_has_nonempty_byte_data, simd_unsupported,
};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;

/// Return the native channel stride for ImageChops operations.
///
/// Chops treats every stored byte as an active sample. In particular, the
/// fourth byte of a CMYK image is K, not alpha, so it must be admitted to the
/// same four-byte vector kernel as RGBA without going through packed-mode
/// interpretation. HSV/YCbCr and RGBa/RGBX likewise retain their raw byte
/// layout at this boundary; ImageChops does not reinterpret those samples as
/// colors or alpha.
fn native_chops_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn logical_byte_channels(mode: &str) -> Option<usize> {
    match mode {
        "1" | "L" | "P" => Some(1),
        "LA" | "PA" => Some(2),
        "RGB" | "HSV" | "YCbCr" => Some(3),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => Some(4),
        _ => None,
    }
}

/// Check the secondary image contract without decoding its pixels. Public
/// Chops constructors already validate these fields, but repeating the
/// cheap shape/layout check at SIMD preflight prevents a malformed internal
/// pipeline from reaching an adapter that would discover the mismatch after
/// execution began. `mode()` and `size()` use retained headers or pipeline
/// metadata whenever possible; they do not force a byte-buffer materialize for
/// a loaded secondary image.
fn simd_chops_operands_supported(img: &DynamicImage, other: &Image, mode: Option<&str>) -> bool {
    let Some(channels) = native_chops_layout(img, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    let Ok(other_size) = other.size() else {
        return false;
    };
    logical_byte_channels(&other_mode).is_some_and(|other_channels| channels == other_channels)
        && img.dimensions() == other_size
}

fn shape_native_chops_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX"))
        }
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

/// The affine Chops kernel consumes eight byte samples per `f64x8` block.
/// Keep its admission threshold separate from `u8x16` byte kernels so narrow
/// rows still execute a genuine vector data path instead of being needlessly
/// routed to CPU.
fn has_affine_vector_rows(img: &DynamicImage, channels: usize) -> bool {
    has_empty_native_bytes(img, channels)
        || (img.height() != 0
            && img
                .width()
                .checked_mul(channels as u32)
                .is_some_and(|row_bytes| row_bytes >= 8))
}

fn shape_has_affine_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape_has_empty_native_bytes(shape, channels)
        || (shape.height != 0
            && shape
                .width
                .checked_mul(channels as u32)
                .is_some_and(|row_bytes| row_bytes >= 8))
}

fn shape_preserves_chops_operands(
    shape: SimdImageShape,
    other: &Arc<Image>,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = shape_native_chops_channels(shape, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    let Ok(other_size) = other.size() else {
        return false;
    };
    logical_byte_channels(&other_mode).is_some_and(|other_channels| {
        channels == other_channels && (shape.width, shape.height) == other_size
    })
}

#[inline]
pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let native_chops =
        native_chops_layout(img, mode).is_some_and(|channels| has_valid_byte_data(img, channels));
    let lut_chops =
        native_chops_layout(img, mode).is_some_and(|channels| has_valid_byte_data(img, channels));
    let blend_chops =
        native_chops_layout(img, mode).is_some_and(|channels| has_valid_byte_data(img, channels));
    let affine_chops = native_chops_layout(img, mode)
        .is_some_and(|channels| has_affine_vector_rows(img, channels));
    match op {
        PipelineOp::Multiply { other } | PipelineOp::Screen { other } => {
            blend_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other } => {
            native_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Overlay { other } | PipelineOp::HardLight { other } => {
            lut_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::SoftLight { other } => {
            lut_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        }
        | PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => {
            scale.is_finite()
                && offset.is_finite()
                && simd_chops_operands_supported(img, other, mode)
                && if *scale == 1.0 && *offset == 0.0 {
                    native_chops
                } else {
                    affine_chops
                }
        }
        _ => false,
    }
}

#[inline]
pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let native_chops = shape_native_chops_channels(shape, mode).is_some_and(|channels| {
        shape_has_empty_native_bytes(shape, channels)
            || shape_has_nonempty_byte_data(shape, channels)
    });
    let lut_chops = shape_native_chops_channels(shape, mode).is_some_and(|channels| {
        shape_has_empty_native_bytes(shape, channels)
            || shape_has_nonempty_byte_data(shape, channels)
    });
    let blend_chops = shape_native_chops_channels(shape, mode).is_some_and(|channels| {
        shape_has_empty_native_bytes(shape, channels)
            || shape_has_nonempty_byte_data(shape, channels)
    });
    let affine_chops = shape_native_chops_channels(shape, mode)
        .is_some_and(|channels| shape_has_affine_vector_rows(shape, channels));
    match op {
        PipelineOp::Multiply { other } | PipelineOp::Screen { other } => {
            blend_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other } => {
            native_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Overlay { other } | PipelineOp::HardLight { other } => {
            lut_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::SoftLight { other } => {
            lut_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        }
        | PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => {
            scale.is_finite()
                && offset.is_finite()
                && shape_preserves_chops_operands(shape, other, mode)
                && if *scale == 1.0 && *offset == 0.0 {
                    native_chops
                } else {
                    affine_chops
                }
        }
        _ => false,
    }
}

pub(super) fn supports_in_place(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            scale.is_finite()
                && offset.is_finite()
                && native_chops_layout(img, mode)
                    .is_some_and(|channels| has_affine_vector_rows(img, channels))
        }
        _ => false,
    }
}

macro_rules! execute_chops_bytewise_in_place {
    ($img:expr, $other:expr, $mode:expr, $vector_op:expr, $scalar_op:expr) => {{
        let other = materialize_chops_operand($other, $mode)?;
        Ok(native_chops_bytewise_in_place(
            $img, &other, $mode, $vector_op, $scalar_op,
        ))
    }};
}

#[inline]
pub(super) fn execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    match op {
        PipelineOp::Multiply { other } => execute_blend_in_place(img, other, mode, false),
        PipelineOp::Screen { other } => execute_blend_in_place(img, other, mode, true),
        PipelineOp::Darker { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            |left, right| left.min(right),
            |left, right| left.min(right)
        ),
        PipelineOp::Lighter { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            |left, right| left.max(right),
            |left, right| left.max(right)
        ),
        PipelineOp::Difference { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            |left, right| left.max(right) - left.min(right),
            |left, right| left.abs_diff(right)
        ),
        PipelineOp::AddModulo { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            |left, right| left + right,
            |left, right| left.wrapping_add(right)
        ),
        PipelineOp::SubtractModulo { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            |left, right| left - right,
            |left, right| left.wrapping_sub(right)
        ),
        PipelineOp::LogicalAnd { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            native_logical_and_vector,
            |left, right| u8::from(left != 0 && right != 0) * 255
        ),
        PipelineOp::LogicalOr { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            native_logical_or_vector,
            |left, right| u8::from((left | right) != 0) * 255
        ),
        PipelineOp::LogicalXor { other } => execute_chops_bytewise_in_place!(
            img,
            other,
            mode,
            native_logical_xor_vector,
            |left, right| u8::from((left != 0) ^ (right != 0)) * 255
        ),
        PipelineOp::Add {
            other,
            scale,
            offset,
        } => execute_add_subtract_in_place(img, other, *scale, *offset, false, mode),
        PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => execute_add_subtract_in_place(img, other, *scale, *offset, true, mode),
        _ => Ok(false),
    }
}

#[inline]
fn execute_blend_in_place(
    img: &mut DynamicImage,
    other: &Arc<Image>,
    mode: Option<&str>,
    screen: bool,
) -> Result<bool, PilError> {
    let other = materialize_chops_operand(other, mode)?;
    let Some(channels) = native_chops_pair_channels(&*img, &other, mode) else {
        return Ok(false);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let right = other.as_bytes();
    let Some(left) = img.as_bytes_mut() else {
        return Ok(false);
    };
    Ok(apply_native_blend_rows_in_place(
        left, right, width, height, channels, screen,
    ))
}

fn execute_add_subtract_in_place(
    img: &mut DynamicImage,
    other: &Arc<Image>,
    scale: f64,
    offset: f64,
    subtract: bool,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    let other = materialize_chops_operand(other, mode)?;
    if scale == 1.0
        && offset == 0.0
        && native_chops_layout(img, mode)
            .is_some_and(|channels| has_vectorized_byte_rows(img, channels))
    {
        if subtract {
            return Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.saturating_sub(right),
                |left, right| left.saturating_sub(right),
            ));
        }
        return Ok(native_chops_bytewise_in_place(
            img,
            &other,
            mode,
            |left, right| left.saturating_add(right),
            |left, right| left.saturating_add(right),
        ));
    }
    Ok(native_chops_affine_in_place(
        img, &other, mode, scale, offset, subtract,
    ))
}
fn simd_native_chops(
    img: &DynamicImage,
    other: &Arc<Image>,
    mode: Option<&str>,
    native: fn(&DynamicImage, &DynamicImage, Option<&str>) -> Option<DynamicImage>,
) -> Result<DynamicImage, PilError> {
    let other_img = materialize_chops_operand(other, mode)?;
    if let Some(result) = native(img, &other_img, mode) {
        return Ok(result);
    }
    Err(PilError::NotImplementedError(
        "SIMD Chops requires matching native byte layouts".into(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Section E: Dual-image per-pixel ops (Add, Subtract, Multiply, ...)
// ═══════════════════════════════════════════════════════════════════════

macro_rules! native_dual_op_adapter {
    ($name:ident, $variant:ident, $native:path) => {
        pub fn $name(
            img: &DynamicImage,
            op: &PipelineOp,
            mode: Option<&str>,
        ) -> Result<DynamicImage, PilError> {
            let PipelineOp::$variant { other } = op else {
                return Err(PilError::ValueError(
                    concat!("expected ", stringify!($variant), " op").into(),
                ));
            };
            simd_native_chops(img, other, mode, $native)
        }
    };
}

pub(crate) fn simd_multiply(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Multiply { other } = op else {
        return Err(PilError::ValueError("expected Multiply op".into()));
    };
    simd_native_chops(img, other, mode, |image, operand, current_mode| {
        native_chops_blend(image, operand, current_mode, false)
    })
}

pub(crate) fn simd_screen(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Screen { other } = op else {
        return Err(PilError::ValueError("expected Screen op".into()));
    };
    simd_native_chops(img, other, mode, |image, operand, current_mode| {
        native_chops_blend(image, operand, current_mode, true)
    })
}

pub(crate) fn simd_overlay(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Overlay { other } = op else {
        return Err(PilError::ValueError("expected Overlay op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_overlay)
}

pub(crate) fn simd_hard_light(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::HardLight { other } = op else {
        return Err(PilError::ValueError("expected HardLight op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_hard_light)
}

pub(crate) fn simd_soft_light(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::SoftLight { other } = op else {
        return Err(PilError::ValueError("expected SoftLight op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_soft_light)
}

native_dual_op_adapter!(simd_darker, Darker, native_chops_darker);
native_dual_op_adapter!(simd_lighter, Lighter, native_chops_lighter);
native_dual_op_adapter!(simd_difference, Difference, native_chops_difference);
native_dual_op_adapter!(simd_add_modulo, AddModulo, native_chops_add_modulo);
native_dual_op_adapter!(
    simd_subtract_modulo,
    SubtractModulo,
    native_chops_subtract_modulo
);
native_dual_op_adapter!(simd_logical_and, LogicalAnd, native_chops_logical_and);
native_dual_op_adapter!(simd_logical_or, LogicalOr, native_chops_logical_or);
native_dual_op_adapter!(simd_logical_xor, LogicalXor, native_chops_logical_xor);

pub(crate) fn simd_add(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Add {
        other,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Add op".into()));
    };
    let other_img = materialize_chops_operand(other, mode)?;
    // Keep ordinary byte images in their native interleaved layout for both
    // the default and scaled/offset contracts. The arithmetic helper follows
    // Pillow's `(left + right) / scale + offset` order before clamping.
    if *scale == 1.0
        && *offset == 0.0
        && native_chops_layout(img, mode).is_some_and(|channels| has_valid_byte_data(img, channels))
    {
        if let Some(result) = native_chops_add_clamped(img, &other_img, mode) {
            return Ok(result);
        }
    }
    if *scale == 1.0 && *offset == 0.0 {
        if let Some(result) = native_chops_affine(img, &other_img, mode, 1.0, 0.0, false) {
            return Ok(result);
        }
    } else if let Some(result) = native_chops_affine(img, &other_img, mode, *scale, *offset, false)
    {
        return Ok(result);
    }
    Err(simd_unsupported("Add"))
}

pub(crate) fn simd_subtract(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Subtract {
        other,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Subtract op".into()));
    };
    let other_img = materialize_chops_operand(other, mode)?;
    // For both default and scaled parameters use native bytes only when the
    // representation and public mode are an exact match. The arithmetic
    // helper retains Pillow's subtraction/division/addition ordering.
    if *scale == 1.0
        && *offset == 0.0
        && native_chops_layout(img, mode).is_some_and(|channels| has_valid_byte_data(img, channels))
    {
        if let Some(result) = native_chops_subtract_clamped(img, &other_img, mode) {
            return Ok(result);
        }
    }
    if *scale == 1.0 && *offset == 0.0 {
        if let Some(result) = native_chops_affine(img, &other_img, mode, 1.0, 0.0, true) {
            return Ok(result);
        }
    } else if let Some(result) = native_chops_affine(img, &other_img, mode, *scale, *offset, true) {
        return Ok(result);
    }
    Err(simd_unsupported("Subtract"))
}

pub(crate) fn simd_fused_multiply_screen(
    img: &DynamicImage,
    first_other: &Arc<Image>,
    second_other: &Arc<Image>,
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    if mode.is_some() || !first_other.shares_execution_source(second_other) {
        return Ok(None);
    }
    let other = materialize_chops_operand(first_other, mode)?;
    let channels = match (img, other.as_ref()) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_)) => 1usize,
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_)) => 3,
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_)) => 4,
        _ => return Ok(None),
    };
    if img.dimensions() != other.dimensions() {
        return Ok(None);
    }

    let left_bytes = img.as_bytes();
    let right_bytes = other.as_bytes();
    let mut output = vec![0u8; left_bytes.len()];
    let (width, height) = img.dimensions();
    let row_stride = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD fused Chops row stride overflow".into()))?;
    if row_stride == 0 || row_stride.checked_mul(height as usize) != Some(output.len()) {
        return Ok(None);
    }
    // Both operands are tightly packed and each byte is independent. Stream
    // across rows; only the final tile needs a tail. Cheap fused arithmetic
    // must amortize scheduling just like the standalone native blend path.
    let vector_blocks = output.len() / 16;
    let scalar_tail = output.len() % 16;
    #[cfg(feature = "parallel")]
    if output.len() >= 4 * 1024 * 1024 {
        const TILE_BYTES: usize = 64 * 1024;
        let tiles = output.len().div_ceil(TILE_BYTES);
        simd_rows_mut_serial!(
            &mut output,
            TILE_BYTES,
            tiles,
            |row_start, _row_end, _y, row| {
                let row_end = row_start + row.len();
                let _ = simd_fused_multiply_screen_row(
                    &left_bytes[row_start..row_end],
                    &right_bytes[row_start..row_end],
                    row,
                );
            }
        );
    } else if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }

    #[cfg(not(feature = "parallel"))]
    if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels)?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks as u64,
        scalar_tail as u64,
    )))
}
