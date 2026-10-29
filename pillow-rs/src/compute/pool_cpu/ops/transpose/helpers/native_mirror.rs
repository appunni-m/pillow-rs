//! Portable wide-lane horizontal reversal for native byte rows.

use wide::u8x16;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

/// Mirrors a typed byte image and preserves its pixel-buffer type.
pub(crate) fn mirror_image<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    channels: usize,
) -> Option<(ImageBuffer<P, Vec<u8>>, u64, u64)>
where
    P: Pixel<Subpixel = u8>,
{
    let (width, height) = image.dimensions();
    let (bytes, vector_blocks, scalar_tail) =
        mirror_bytes(image.as_raw(), width, height, channels)?;
    let output = ImageBuffer::from_raw(width, height, bytes)?;
    Some((output, vector_blocks, scalar_tail))
}

/// Reverse each row while keeping every pixel's native byte group intact.
///
/// Trailing source storage is ignored, matching the archived native adapter.
pub(crate) fn mirror_bytes(
    source: &[u8],
    width: u32,
    height: u32,
    channels: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    if width == 0 || height == 0 || !(1..=4).contains(&channels) {
        return None;
    }
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let row_len = width.checked_mul(channels)?;
    let total_len = row_len.checked_mul(height)?;
    let source = source.get(..total_len)?;
    let mut output = vec![0; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..height {
        let start = row.checked_mul(row_len)?;
        let (blocks, tail) = mirror_native_row(
            source.get(start..start + row_len)?,
            output.get_mut(start..start + row_len)?,
            width,
            channels,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((output, vector_blocks, scalar_tail))
}

pub(crate) fn mirror_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    channels: usize,
) -> Option<(u64, u64)> {
    let row_len = width.checked_mul(channels)?;
    if source.len() != row_len || output.len() != row_len {
        return None;
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if channels == 3 {
        let vector_pixels = width / 16 * 16;
        for output_pixel in (0..vector_pixels).step_by(16) {
            let source_pixel = width - output_pixel - 16;
            let source_start = source_pixel.checked_mul(3)?;
            let output_start = output_pixel.checked_mul(3)?;
            let block = reverse_rgb_block(source.get(source_start..source_start + 48)?)?;
            output[output_start..output_start + 48].copy_from_slice(&block);
            vector_blocks = vector_blocks.saturating_add(3);
        }
        let remainder_pixels = width - vector_pixels;
        if remainder_pixels != 0 {
            let remainder_len = remainder_pixels.checked_mul(3)?;
            let mut padded = [0u8; 48];
            padded[48usize.checked_sub(remainder_len)?..].copy_from_slice(source.get(..remainder_len)?);
            let block = reverse_rgb_block(&padded)?;
            let output_start = vector_pixels.checked_mul(3)?;
            output[output_start..output_start + remainder_len]
                .copy_from_slice(&block[..remainder_len]);
            vector_blocks = vector_blocks.saturating_add(3);
            scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
        }
        return Some((vector_blocks, scalar_tail));
    }
    let pixels_per_vector = 16usize.checked_div(channels)?;
    let vector_pixels = width / pixels_per_vector * pixels_per_vector;
    for output_pixel in (0..vector_pixels).step_by(pixels_per_vector) {
        let source_pixel = width - output_pixel - pixels_per_vector;
        let source_start = source_pixel.checked_mul(channels)?;
        let output_start = output_pixel.checked_mul(channels)?;
        let block = reverse_pixel_block(source.get(source_start..source_start + 16)?, channels)?;
        output[output_start..output_start + 16].copy_from_slice(&block);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder_pixels = width - vector_pixels;
    if remainder_pixels != 0 {
        let remainder_len = remainder_pixels.checked_mul(channels)?;
        let mut padded = [0u8; 16];
        padded[16usize.checked_sub(remainder_len)?..].copy_from_slice(source.get(..remainder_len)?);
        let block = reverse_pixel_block(&padded, channels)?;
        let output_start = vector_pixels.checked_mul(channels)?;
        output[output_start..output_start + remainder_len].copy_from_slice(&block[..remainder_len]);
        vector_blocks = vector_blocks.saturating_add(1);
        scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
    }
    Some((vector_blocks, scalar_tail))
}

fn reverse_pixel_block(input: &[u8], channels: usize) -> Option<[u8; 16]> {
    let indices = match channels {
        1 => [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        2 => [14, 15, 12, 13, 10, 11, 8, 9, 6, 7, 4, 5, 2, 3, 0, 1],
        4 => [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3],
        _ => return None,
    };
    let input = u8x16::new(<[u8; 16]>::try_from(input).ok()?);
    Some(input.swizzle_relaxed(u8x16::new(indices)).to_array())
}

fn reverse_rgb_block(input: &[u8]) -> Option<[u8; 48]> {
    let first = u8x16::new(<[u8; 16]>::try_from(input.get(..16)?).ok()?);
    let second = u8x16::new(<[u8; 16]>::try_from(input.get(16..32)?).ok()?);
    let third = u8x16::new(<[u8; 16]>::try_from(input.get(32..48)?).ok()?);
    let block0_from_third = third.swizzle_relaxed(u8x16::new([
        13, 14, 15, 10, 11, 12, 7, 8, 9, 4, 5, 6, 1, 2, 3, 0,
    ]));
    let block0_from_second = second.swizzle_relaxed(u8x16::splat(14));
    let block0 = u8x16::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX])
        .select(block0_from_second, block0_from_third);
    let block1_from_second = second.swizzle_relaxed(u8x16::new([
        15, 0, 11, 12, 13, 8, 9, 10, 5, 6, 7, 2, 3, 4, 0, 0,
    ]));
    let block1_from_third = third.swizzle_relaxed(u8x16::splat(0));
    let block1_from_first = first.swizzle_relaxed(u8x16::splat(15));
    let block1 = u8x16::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX, 0]).select(
        block1_from_first,
        u8x16::new([0, u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .select(block1_from_third, block1_from_second),
    );
    let block2_from_first = first.swizzle_relaxed(u8x16::new([
        0, 12, 13, 14, 9, 10, 11, 6, 7, 8, 3, 4, 5, 0, 1, 2,
    ]));
    let block2_from_second = second.swizzle_relaxed(u8x16::splat(1));
    let block2 = u8x16::new([u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .select(block2_from_second, block2_from_first);
    let mut output = [0u8; 48];
    output[..16].copy_from_slice(&block0.to_array());
    output[16..32].copy_from_slice(&block1.to_array());
    output[32..].copy_from_slice(&block2.to_array());
    Some(output)
}
