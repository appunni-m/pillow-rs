//! Pillow-compatible byte-oriented I;16 Offset kernels.

use super::byte::{circular_byte_block, narrow_rotated_row};
use crate::raster::DynamicImage;
use wide::u8x16;

/// Copy one Pillow `I;16*` offset row through native byte vectors.
///
/// The scalar reference follows Pillow's historical `ImageChops` byte path:
/// it rotates the first `width` bytes of a `width * 2` row and leaves the
/// remaining bytes zero. Preserve that observable contract instead of
/// treating the samples as ordinary `u16` pixels. The byte-order conversion
/// is control-plane work; the copied portion uses the same circular `u8x16`
/// kernel as ordinary byte images.
#[inline]
fn offset_luma16_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    shift_bytes: usize,
) -> Option<(u64, u64)> {
    let row_bytes = width.checked_mul(2)?;
    if width < 16 || source.len() != row_bytes || output.len() != row_bytes || shift_bytes >= width
    {
        return None;
    }
    let visible_source = source.get(..width)?;
    let vector_len = width / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let source_start = (shift_bytes + output_offset) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = width - vector_len;
    if scalar_tail != 0 {
        let source_start = (shift_bytes + vector_len) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[vector_len..width].copy_from_slice(&block.to_array()[..scalar_tail]);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Build one masked vector for a narrow `I;16*` offset frame.
///
/// The public `ImageChops.offset` contract only copies the first `width`
/// bytes of each `width * 2` storage row. A vector block may therefore cross
/// both a row's copied/zero boundary and the next row's boundary. Scalar
/// arithmetic computes those boundaries; the selected source bytes and the
/// final masked store remain in `u8x16`.
#[inline]
fn offset_luma16_narrow_block(
    source: &[u8],
    output_offset: usize,
    active_len: usize,
    width: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<u8x16> {
    let storage_row_len = width.checked_mul(2)?;
    if active_len == 0
        || active_len > 16
        || output_offset.checked_add(active_len)? > source.len()
        || width == 0
        || width >= 16
        || height == 0
        || yshift >= height
        || shift_bytes >= width
    {
        return None;
    }

    let mut block = u8x16::splat(0);
    let mut lane = 0usize;
    while lane < active_len {
        let absolute = output_offset.checked_add(lane)?;
        let output_row = absolute / storage_row_len;
        let row_start = output_row.checked_mul(storage_row_len)?;
        let row_offset = absolute.checked_sub(row_start)?;
        let row_remaining = storage_row_len.checked_sub(row_offset)?;
        let segment_len = row_remaining.min(active_len - lane);

        if row_offset < width {
            let copied_len = (width - row_offset).min(segment_len);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(storage_row_len)?;
            let rotated = narrow_rotated_row(source, source_start, width, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + copied_len {
                indices[destination_lane] =
                    u8::try_from(row_offset + destination_lane - lane).ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.swizzle_relaxed(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
        }
        lane += segment_len;
    }
    Some(block)
}

/// Offset a narrow `I;16*` frame with masked vector blocks across rows.
///
/// Unlike ordinary byte images, the second half of every storage row is
/// intentionally zero in Pillow's historical `image8` path. Keeping that
/// zero fill inside the vector block lets 2×2 and other small typed images
/// take a real native SIMD path without inventing a scalar-only capability.
#[inline]
fn offset_luma16_narrow(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<(u64, u64)> {
    let expected_len = width.checked_mul(2)?.checked_mul(height)?;
    if source.len() != expected_len
        || output.len() != expected_len
        || width == 0
        || width >= 16
        || height == 0
        || yshift >= height
        || shift_bytes >= width
    {
        return None;
    }

    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let block = offset_luma16_narrow_block(
            source,
            output_offset,
            16,
            width,
            height,
            shift_bytes,
            yshift,
        )?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let block = offset_luma16_narrow_block(
            source,
            vector_len,
            scalar_tail,
            width,
            height,
            shift_bytes,
            yshift,
        )?;
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

pub(super) fn native_offset_luma16(
    img: &DynamicImage,
    xoffset: i32,
    yoffset: i32,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLuma16(image) = img else {
        return None;
    };
    if !matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        return None;
    }
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return image.as_raw().is_empty().then_some((img.clone(), 0, 0));
    }
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let row_bytes = width_usize.checked_mul(2)?;
    let total_bytes = row_bytes.checked_mul(height_usize)?;
    if image.as_raw().len().checked_mul(2)? != total_bytes {
        return None;
    }

    // Match `raster::dynamic::offset_luma16`: I;16B is the only explicit
    // big-endian variant in this byte-oriented ImageChops path.
    let big_endian = mode == Some("I;16B");
    let source = if big_endian {
        image
            .as_raw()
            .iter()
            .flat_map(|sample| sample.to_be_bytes())
            .collect::<Vec<_>>()
    } else {
        bytemuck::cast_slice(image.as_raw()).to_vec()
    };
    let mut output = vec![0u8; total_bytes];
    let xshift = (-i64::from(xoffset)).rem_euclid(i64::from(width)) as usize;
    let yshift = (-i64::from(yoffset)).rem_euclid(i64::from(height)) as usize;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if width_usize < 16 {
        (vector_blocks, scalar_tail) = offset_luma16_narrow(
            &source,
            &mut output,
            width_usize,
            height_usize,
            xshift,
            yshift,
        )?;
    } else {
        for output_y in 0..height_usize {
            let source_y = (output_y + yshift) % height_usize;
            let source_start = source_y.checked_mul(row_bytes)?;
            let output_start = output_y.checked_mul(row_bytes)?;
            let (blocks, tail) = offset_luma16_native_row(
                source.get(source_start..source_start + row_bytes)?,
                output.get_mut(output_start..output_start + row_bytes)?,
                width_usize,
                xshift,
            )?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    let samples = output
        .chunks_exact(2)
        .map(|bytes| {
            if big_endian {
                u16::from_be_bytes([bytes[0], bytes[1]])
            } else {
                u16::from_ne_bytes([bytes[0], bytes[1]])
            }
        })
        .collect();
    let result = crate::raster::ImageBuffer::from_raw(width, height, samples)?;
    Some((
        DynamicImage::ImageLuma16(result),
        vector_blocks,
        scalar_tail,
    ))
}
