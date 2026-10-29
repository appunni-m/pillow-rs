//! Typed native-sample movement for Pillow's I;16 mode identities.

use crate::raster::{EncodableLayout, GenericImageView, ImageBuffer, Pixel};
use wide::u16x8;

/// Vertically flips an exact typed u16 raster without changing sample bits.
pub(crate) fn flip_image<P>(
    image: &ImageBuffer<P, Vec<u16>>,
) -> Option<(ImageBuffer<P, Vec<u16>>, u64, u64)>
where
    P: Pixel<Subpixel = u16>,
{
    let (width, height) = image.dimensions();
    let row_len = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    if row_len == 0 || height_usize == 0 {
        return None;
    }
    let total_samples = row_len.checked_mul(height_usize)?;
    let source = image.as_raw();
    if source.len() != total_samples {
        return None;
    }

    let mut output = vec![0u16; total_samples];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_row in 0..height_usize {
        let source_row = height_usize - 1 - output_row;
        let source_start = source_row.checked_mul(row_len)?;
        let output_start = output_row.checked_mul(row_len)?;
        let (blocks, tail) = copy_samples(
            source.get(source_start..source_start + row_len)?,
            output.get_mut(output_start..output_start + row_len)?,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let output = ImageBuffer::from_raw(width, height, output)?;
    Some((output, vector_blocks, scalar_tail))
}

/// Mirrors exact u16 samples while retaining each sample's native byte order.
pub(crate) fn mirror_image<P>(
    image: &ImageBuffer<P, Vec<u16>>,
) -> Option<(ImageBuffer<P, Vec<u16>>, u64, u64)>
where
    P: Pixel<Subpixel = u16>,
{
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let sample_count = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    if image.as_raw().len() != sample_count {
        return None;
    }
    let source = image.as_raw().as_slice().as_bytes();
    let (bytes, vector_blocks, scalar_tail) =
        super::native_mirror::mirror_bytes(source, width, height, 2)?;
    let samples = bytes
        .chunks_exact(2)
        .map(|sample| u16::from_ne_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    let output = ImageBuffer::from_raw(width, height, samples)?;
    Some((output, vector_blocks, scalar_tail))
}

fn copy_samples(source: &[u16], output: &mut [u16]) -> Option<(u64, u64)> {
    if source.len() != output.len() {
        return None;
    }
    let vector_samples = source.len() / 8 * 8;
    let mut vector_blocks = 0u64;
    for offset in (0..vector_samples).step_by(8) {
        let samples = <[u16; 8]>::try_from(source.get(offset..offset + 8)?).ok()?;
        output[offset..offset + 8].copy_from_slice(&u16x8::new(samples).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail_samples = source.len() - vector_samples;
    if tail_samples != 0 {
        let mut padded = [0u16; 8];
        padded[..tail_samples].copy_from_slice(source.get(vector_samples..)?);
        let block = u16x8::new(padded).to_array();
        output[vector_samples..].copy_from_slice(&block[..tail_samples]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, (tail_samples * 2) as u64))
}
