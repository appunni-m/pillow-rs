//! Binary Chops secondary storage mapping for three-band exact modes.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

use super::binary::BinaryOperation;

/// Applies raw channel arithmetic to the primary mode's typed pixel buffer.
pub(crate) fn execute(
    image: &RgbImage,
    other: Option<&DynamicImage>,
    operation: BinaryOperation,
) -> Result<RgbImage, PilError> {
    let Some(other) = other else {
        return Err(PilError::InternalError(
            "Chops binary plan lost its secondary image".into(),
        ));
    };
    let right = three_band_image(other)?;
    if other.mode() == Mode::ImageLab8 {
        return execute_encoded::<false, true, false>(image, right, operation);
    }
    super::binary::execute(image, right, operation)
}

/// Applies Pillow's native LAB chroma representation around a three-band
/// binary operation. LAB's public byte carrier biases signed a/b samples by
/// 128, while Pillow's internal Chops loop operates on their signed-byte
/// storage. Convert only at the two LAB operand/output boundaries.
pub(crate) fn execute_lab_primary(
    image: &RgbImage,
    other: Option<&DynamicImage>,
    operation: BinaryOperation,
) -> Result<RgbImage, PilError> {
    match other {
        Some(DynamicImage::ImageLab8(right)) => {
            execute_encoded::<true, true, true>(image, right, operation)
        }
        Some(
            DynamicImage::ImageRgb8(right)
            | DynamicImage::ImageYCbCr8(right)
            | DynamicImage::ImageHsv8(right),
        ) => execute_encoded::<true, false, true>(image, right, operation),
        Some(_) => Err(PilError::InternalError(
            "three-band Chops received a different raster variant".into(),
        )),
        None => Err(PilError::InternalError(
            "Chops binary plan lost its secondary image".into(),
        )),
    }
}

fn execute_encoded<const LEFT_LAB: bool, const RIGHT_LAB: bool, const OUTPUT_LAB: bool>(
    image: &RgbImage,
    other: &RgbImage,
    operation: BinaryOperation,
) -> Result<RgbImage, PilError> {
    let width = image.width().min(other.width());
    let height = image.height().min(other.height());
    let dimensions = if width == 0 || height == 0 {
        CheckedDims::new_allow_empty(width, height, 3)?
    } else {
        CheckedDims::new(width, height, 3)?
    };
    let mut output = dimensions.alloc_buffer();
    let left = image.as_raw();
    let right = other.as_raw();
    let width = width as usize;
    let height = height as usize;
    let left_stride = image.width() as usize * 3;
    let right_stride = other.width() as usize * 3;

    for y in 0..height {
        let left_row = y * left_stride;
        let right_row = y * right_stride;
        let output_row = y * width * 3;
        for x in 0..width {
            let left_index = left_row + x * 3;
            let right_index = right_row + x * 3;
            let output_index = output_row + x * 3;
            output[output_index] = operation.apply_to_byte(left[left_index], right[right_index]);
            output[output_index + 1] = map_chroma::<LEFT_LAB, RIGHT_LAB, OUTPUT_LAB>(
                left[left_index + 1],
                right[right_index + 1],
                operation,
            );
            output[output_index + 2] = map_chroma::<LEFT_LAB, RIGHT_LAB, OUTPUT_LAB>(
                left[left_index + 2],
                right[right_index + 2],
                operation,
            );
        }
    }

    RgbImage::from_raw(width as u32, height as u32, output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

#[inline(always)]
fn map_chroma<const LEFT_LAB: bool, const RIGHT_LAB: bool, const OUTPUT_LAB: bool>(
    left: u8,
    right: u8,
    operation: BinaryOperation,
) -> u8 {
    let left = if LEFT_LAB {
        left.wrapping_sub(128)
    } else {
        left
    };
    let right = if RIGHT_LAB {
        right.wrapping_sub(128)
    } else {
        right
    };
    let output = operation.apply_to_byte(left, right);
    if OUTPUT_LAB {
        output.wrapping_add(128)
    } else {
        output
    }
}

fn three_band_image(image: &DynamicImage) -> Result<&RgbImage, PilError> {
    match image {
        DynamicImage::ImageRgb8(buffer)
        | DynamicImage::ImageYCbCr8(buffer)
        | DynamicImage::ImageLab8(buffer)
        | DynamicImage::ImageHsv8(buffer) => Ok(buffer),
        _ => Err(PilError::InternalError(
            "three-band Chops received a different raster variant".into(),
        )),
    }
}
