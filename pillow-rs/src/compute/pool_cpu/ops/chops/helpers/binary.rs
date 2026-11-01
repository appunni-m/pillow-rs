//! Serial native-byte ImageChops kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

static OVERLAY_LUT: &[u8; 65_536] = include_bytes!("../../../../../ops/lut_overlay.bin");
static HARD_LIGHT_LUT: &[u8; 65_536] = include_bytes!("../../../../../ops/lut_hardlight.bin");
static SOFT_LIGHT_LUT: &[u8; 65_536] = include_bytes!("../../../../../ops/lut_softlight.bin");

/// Native-byte binary operation after wrapper arguments have been normalized.
#[derive(Clone, Copy)]
pub(crate) enum BinaryOperation {
    /// Pillow-scaled addition.
    Add {
        /// Scale narrowed to the original float32 arithmetic type.
        scale: f32,
        /// Offset narrowed to the original float32 arithmetic type.
        offset: f32,
        /// Whether the original float64 request was exactly `(1, 0)`.
        exact_default_parameters: bool,
    },
    /// Pillow-scaled subtraction.
    Subtract {
        /// Scale narrowed to the original float32 arithmetic type.
        scale: f32,
        /// Offset narrowed to the original float32 arithmetic type.
        offset: f32,
        /// Whether the original float64 request was exactly `(1, 0)`.
        exact_default_parameters: bool,
    },
    /// Multiply with Pillow's 8-bit normalization.
    Multiply,
    /// Screen blend.
    Screen,
    /// Per-byte minimum.
    Darker,
    /// Per-byte maximum.
    Lighter,
    /// Absolute per-byte difference.
    Difference,
    /// Overlay lookup.
    Overlay,
    /// Hard-light lookup.
    HardLight,
    /// Soft-light lookup.
    SoftLight,
    /// Wrapping addition.
    AddModulo,
    /// Wrapping subtraction.
    SubtractModulo,
    /// Mode-1 truth conjunction.
    LogicalAnd,
    /// Mode-1 truth disjunction.
    LogicalOr,
    /// Mode-1 truth exclusive-or.
    LogicalXor,
}

/// Clips the inputs to their overlap and applies one native-byte operation.
pub(crate) fn execute(
    image: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    operation: BinaryOperation,
) -> Result<DynamicImage, PilError> {
    let channels = byte_channels(mode).ok_or_else(|| {
        PilError::InternalError("Chops binary strategy received a non-byte mode".into())
    })?;
    let width = image.width().min(other.width());
    let height = image.height().min(other.height());
    let dimensions = if width == 0 || height == 0 {
        CheckedDims::new_allow_empty(width, height, channels)?
    } else {
        CheckedDims::new(width, height, channels)?
    };
    let mut output = dimensions.alloc_buffer();
    apply(
        image.as_bytes(),
        other.as_bytes(),
        &mut output,
        width as usize,
        height as usize,
        usize::from(channels),
        image.width() as usize * usize::from(channels),
        other.width() as usize * usize::from(channels),
        operation,
    );
    output::from_raw_bytes(mode, width, height, output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

fn apply(
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    left_stride: usize,
    right_stride: usize,
    operation: BinaryOperation,
) {
    match operation {
        BinaryOperation::Add { scale, offset, .. } if scale == 1.0 && offset == 0.0 => {
            apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::saturating_add)
        }
        BinaryOperation::Add { scale, offset, .. } => apply_rows(
            left, right, output, width, height, channels, left_stride, right_stride,
            |a, b| ((f32::from(a) + f32::from(b)) / scale + offset).clamp(0.0, 255.0) as u8,
        ),
        BinaryOperation::Subtract { scale, offset, .. } if scale == 1.0 && offset == 0.0 => {
            apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::saturating_sub)
        }
        BinaryOperation::Subtract { scale, offset, .. } => apply_rows(
            left, right, output, width, height, channels, left_stride, right_stride,
            |a, b| ((f32::from(a) - f32::from(b)) / scale + offset).clamp(0.0, 255.0) as u8,
        ),
        BinaryOperation::Multiply => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, |a, b| ((u32::from(a) * u32::from(b)) / 255) as u8),
        BinaryOperation::Screen => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, |a, b| (255 - ((255 - u32::from(a)) * (255 - u32::from(b)) / 255)) as u8),
        BinaryOperation::Darker => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::min),
        BinaryOperation::Lighter => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::max),
        BinaryOperation::Difference => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::abs_diff),
        BinaryOperation::Overlay => apply_lut(left, right, output, width, height, channels, left_stride, right_stride, OVERLAY_LUT),
        BinaryOperation::HardLight => apply_lut(left, right, output, width, height, channels, left_stride, right_stride, HARD_LIGHT_LUT),
        BinaryOperation::SoftLight => apply_lut(left, right, output, width, height, channels, left_stride, right_stride, SOFT_LIGHT_LUT),
        BinaryOperation::AddModulo => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::wrapping_add),
        BinaryOperation::SubtractModulo => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, u8::wrapping_sub),
        BinaryOperation::LogicalAnd => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, |a, b| u8::from(a != 0 && b != 0) * 255),
        BinaryOperation::LogicalOr => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, |a, b| u8::from((a | b) != 0) * 255),
        BinaryOperation::LogicalXor => apply_rows(left, right, output, width, height, channels, left_stride, right_stride, |a, b| u8::from((a != 0) ^ (b != 0)) * 255),
    }
}

fn apply_lut(
    left: &[u8], right: &[u8], output: &mut [u8], width: usize, height: usize,
    channels: usize, left_stride: usize, right_stride: usize, lut: &[u8; 65_536],
) {
    apply_rows(left, right, output, width, height, channels, left_stride, right_stride,
        |a, b| lut[usize::from(a) * 256 + usize::from(b)]);
}

fn apply_rows<F: Fn(u8, u8) -> u8>(
    left: &[u8], right: &[u8], output: &mut [u8], width: usize, height: usize,
    channels: usize, left_stride: usize, right_stride: usize, operation: F,
) {
    let output_stride = width * channels;
    if output_stride == 0 || height == 0 {
        return;
    }
    for (y, row) in output.chunks_exact_mut(output_stride).take(height).enumerate() {
        let left_start = y * left_stride;
        let right_start = y * right_stride;
        let left_row = &left[left_start..left_start + output_stride];
        let right_row = &right[right_start..right_start + output_stride];
        for ((destination, &a), &b) in row.iter_mut().zip(left_row).zip(right_row) {
            *destination = operation(a, b);
        }
    }
}

pub(crate) const fn byte_channels(mode: Mode) -> Option<u8> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => Some(1),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => Some(2),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageHsv8 => Some(3),
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 | Mode::ImageCmyk8 => Some(4),
        _ => None,
    }
}
