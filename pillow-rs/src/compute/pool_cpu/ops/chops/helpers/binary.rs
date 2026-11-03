//! Serial native-byte ImageChops kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::{HiddenPixelLanes, ImageBuffer};
use crate::raster::traits::Pixel;
use crate::raster::DynamicImage;

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

impl BinaryOperation {
    /// Applies the same byte arithmetic as the native Chops strategy to an
    /// omitted physical pixel lane.
    pub(crate) fn apply_to_byte(self, left: u8, right: u8) -> u8 {
        match self {
            Self::Add { scale, offset, .. } if scale == 1.0 && offset == 0.0 => {
                left.saturating_add(right)
            }
            Self::Add { scale, offset, .. } => {
                ((f32::from(left) + f32::from(right)) / scale + offset).clamp(0.0, 255.0)
                    as u8
            }
            Self::Subtract { scale, offset, .. } if scale == 1.0 && offset == 0.0 => {
                left.saturating_sub(right)
            }
            Self::Subtract { scale, offset, .. } => {
                ((f32::from(left) - f32::from(right)) / scale + offset).clamp(0.0, 255.0)
                    as u8
            }
            Self::Multiply => ((u32::from(left) * u32::from(right)) / 255) as u8,
            Self::Screen => {
                (255 - ((255 - u32::from(left)) * (255 - u32::from(right)) / 255)) as u8
            }
            Self::Darker => left.min(right),
            Self::Lighter => left.max(right),
            Self::Difference => left.abs_diff(right),
            Self::Overlay => OVERLAY_LUT[usize::from(left) * 256 + usize::from(right)],
            Self::HardLight => HARD_LIGHT_LUT[usize::from(left) * 256 + usize::from(right)],
            Self::SoftLight => SOFT_LIGHT_LUT[usize::from(left) * 256 + usize::from(right)],
            Self::AddModulo => left.wrapping_add(right),
            Self::SubtractModulo => left.wrapping_sub(right),
            Self::LogicalAnd => u8::from(left != 0 && right != 0) * 255,
            Self::LogicalOr => u8::from((left | right) != 0) * 255,
            Self::LogicalXor => u8::from((left != 0) ^ (right != 0)) * 255,
        }
    }
}

/// Produces Chops results for retained physical lanes when both inputs carry
/// their omitted bytes. A missing sidecar remains unknown and is not guessed.
pub(crate) fn output_hidden_lanes(
    image: &DynamicImage,
    other: &DynamicImage,
    operation: BinaryOperation,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    output_hidden_lanes_with(image, other, |left, right| {
        operation.apply_to_byte(left, right)
    })
}

/// Produces retained physical lanes using an operation-specific byte mapping.
pub(crate) fn output_hidden_lanes_with<F>(
    image: &DynamicImage,
    other: &DynamicImage,
    mut map: F,
) -> Result<Option<HiddenPixelLanes>, PilError>
where
    F: FnMut(u8, u8) -> u8,
{
    let Some(lane_count) = image.mode().hidden_pixel_lane_count() else {
        return Ok(None);
    };
    if other.mode().hidden_pixel_lane_count() != Some(lane_count) {
        return Err(PilError::InternalError(
            "Chops operands did not have matching physical pixel layouts".into(),
        ));
    }
    let (Some(left), Some(right)) = (image.hidden_pixel_lanes(), other.hidden_pixel_lanes()) else {
        return Ok(None);
    };
    if left.lane_count() != lane_count || right.lane_count() != lane_count {
        return Err(PilError::InternalError(
            "Chops physical lane payload did not match its mode".into(),
        ));
    }
    let output_dimensions = (
        image.width().min(other.width()) as usize,
        image.height().min(other.height()) as usize,
    );
    left.zip_map_clipped(
        right,
        (image.width() as usize, image.height() as usize),
        (other.width() as usize, other.height() as usize),
        output_dimensions,
        &mut map,
    )
    .map(Some)
    .ok_or_else(|| PilError::InternalError("Chops physical lane extents did not match".into()))
}

/// Attaches Chops arithmetic for omitted bytes to an already computed image.
pub(crate) fn carry_hidden_lanes(
    image: &DynamicImage,
    other: &DynamicImage,
    output: &mut DynamicImage,
    operation: BinaryOperation,
) -> Result<(), PilError> {
    if !output.replace_hidden_pixel_lanes(output_hidden_lanes(image, other, operation)?) {
        return Err(PilError::InternalError(
            "Chops output physical lanes did not match its mode".into(),
        ));
    }
    Ok(())
}

/// Clips two admitted native-byte inputs and applies the operation to the
/// primary mode's typed buffer. The mode owner wraps the result in its exact
/// `DynamicImage` variant.
pub(crate) fn execute<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    other: &ImageBuffer<P, Vec<u8>>,
    operation: BinaryOperation,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let channels = P::CHANNEL_COUNT;
    let width = image.width().min(other.width());
    let height = image.height().min(other.height());
    let dimensions = if width == 0 || height == 0 {
        CheckedDims::new_allow_empty(width, height, channels)?
    } else {
        CheckedDims::new(width, height, channels)?
    };
    let mut output = dimensions.alloc_buffer();
    apply(
        image.as_raw().as_slice(),
        other.as_raw().as_slice(),
        &mut output,
        width as usize,
        height as usize,
        usize::from(channels),
        image.width() as usize * usize::from(channels),
        other.width() as usize * usize::from(channels),
        operation,
    );
    ImageBuffer::<P, Vec<u8>>::from_raw(width, height, output)
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
