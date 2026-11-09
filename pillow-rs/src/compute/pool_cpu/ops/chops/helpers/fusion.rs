//! Shared arithmetic for native Multiply-to-Screen fusion.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;
use wide::{u8x16, u16x16};

pub(crate) struct FusedChopsOutput<I = DynamicImage> {
    pub(crate) image: I,
    pub(crate) vector_blocks: u64,
    pub(crate) scalar_tail: u64,
    pub(crate) path: &'static str,
}

impl<I> FusedChopsOutput<I> {
    pub(crate) fn map_image<T>(self, map: impl FnOnce(I) -> T) -> FusedChopsOutput<T> {
        FusedChopsOutput {
            image: map(self.image),
            vector_blocks: self.vector_blocks,
            scalar_tail: self.scalar_tail,
            path: self.path,
        }
    }
}

pub(crate) fn execute_default<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    other: &ImageBuffer<P, Vec<u8>>,
) -> Result<FusedChopsOutput<ImageBuffer<P, Vec<u8>>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (dimensions, left, right) = native_operands(image, other)?;
    if left.is_empty() {
        return Ok(FusedChopsOutput {
            image: image.clone(),
            vector_blocks: 0,
            scalar_tail: 0,
            path: "cpu",
        });
    }
    let mut output = dimensions.alloc_buffer();
    for ((destination, &left), &right) in output.iter_mut().zip(left).zip(right) {
        *destination = multiply_then_screen(left, right);
    }
    let image = ImageBuffer::<P, Vec<u8>>::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?;
    Ok(FusedChopsOutput {
        image,
        vector_blocks: 0,
        scalar_tail: 0,
        path: "cpu",
    })
}

pub(crate) fn execute_vector<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    other: &ImageBuffer<P, Vec<u8>>,
) -> Result<FusedChopsOutput<ImageBuffer<P, Vec<u8>>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (dimensions, left, right) = native_operands(image, other)?;
    if left.is_empty() {
        return Ok(FusedChopsOutput {
            image: image.clone(),
            vector_blocks: 0,
            scalar_tail: 0,
            path: "native-copy",
        });
    }
    let mut output = dimensions.alloc_buffer();
    let (left_blocks, left_tail) = left.as_chunks::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
        *output = multiply_screen_block(*left, *right);
    }
    for ((&left, &right), output) in left_tail.iter().zip(right_tail).zip(output_tail) {
        *output = multiply_then_screen(left, right);
    }
    let image = ImageBuffer::<P, Vec<u8>>::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?;
    Ok(FusedChopsOutput {
        image,
        vector_blocks: u64::try_from(left_blocks.len()).unwrap_or(u64::MAX),
        scalar_tail: u64::try_from(left_tail.len()).unwrap_or(u64::MAX),
        path: "vector",
    })
}

fn native_operands<'a, P>(
    image: &'a ImageBuffer<P, Vec<u8>>,
    other: &'a ImageBuffer<P, Vec<u8>>,
) -> Result<(CheckedDims, &'a [u8], &'a [u8]), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if image.dimensions() != other.dimensions() {
        return Err(PilError::InternalError(
            "Chops fusion operands did not match their admitted native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), P::CHANNEL_COUNT)?;
    let left = image.as_raw().as_slice();
    let right = other.as_raw().as_slice();
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
