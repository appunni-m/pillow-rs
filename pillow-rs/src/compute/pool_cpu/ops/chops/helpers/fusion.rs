//! Shared arithmetic for native Multiply-to-Screen fusion.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, Mode};
use wide::{u8x16, u16x16};

pub(crate) fn execute_default(
    image: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (dimensions, left, right) = native_operands(image, other, mode)?;
    if left.is_empty() {
        return Ok(image.clone());
    }
    let mut output = dimensions.alloc_buffer();
    for ((destination, &left), &right) in output.iter_mut().zip(left).zip(right) {
        *destination = multiply_then_screen(left, right);
    }
    super::output::from_raw_bytes(mode, image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

pub(crate) fn execute_vector(
    image: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (dimensions, left, right) = native_operands(image, other, mode)?;
    if left.is_empty() {
        return Ok(image.clone());
    }
    let mut output = dimensions.alloc_buffer();
    let (left_blocks, left_tail) = left.as_chunks::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
        *output = multiply_screen_block(*left, *right);
    }
    for ((&left, &right), output) in left_tail
        .iter()
        .zip(right_tail)
        .zip(output_tail)
    {
        *output = multiply_then_screen(left, right);
    }
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::try_from(left_blocks.len()).unwrap_or(u64::MAX),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        u64::try_from(left_tail.len()).unwrap_or(u64::MAX),
    );
    super::output::from_raw_bytes(mode, image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

fn native_operands<'a>(
    image: &'a DynamicImage,
    other: &'a DynamicImage,
    mode: Mode,
) -> Result<(CheckedDims, &'a [u8], &'a [u8]), PilError> {
    let channels = match mode {
        Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 => 2,
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 => 4,
        _ => {
            return Err(PilError::InternalError(
                "Chops fusion received a mode without a fused strategy".into(),
            ));
        }
    };
    if image.mode() != mode || other.mode() != mode || image.dimensions() != other.dimensions() {
        return Err(PilError::InternalError(
            "Chops fusion operands did not match their admitted native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    let left = image.as_bytes();
    let right = other.as_bytes();
    if left.len() != dimensions.total_bytes() || right.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "Chops fusion operands did not match their image dimensions".into(),
        ));
    }
    Ok((dimensions, left, right))
}

#[inline]
fn multiply_then_screen(left: u8, right: u8) -> u8 {
    let multiplied = (u32::from(left) * u32::from(right) / 255) as u8;
    (255u32 - ((255 - u32::from(multiplied)) * (255 - u32::from(right)) / 255)) as u8
}

#[inline(always)]
fn multiply_screen_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let multiplied = super::vector::divide_255(left * right);
    let result = multiplied + right - super::vector::divide_255(multiplied * right);
    crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(result).to_array()
}
