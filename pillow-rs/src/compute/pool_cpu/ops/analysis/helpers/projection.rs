//! Native-sample projection loops shared by exact-mode strategies.

use crate::raster::{RgbImage, RgbaImage};

pub(crate) type ProjectionAxes = (Vec<u32>, Vec<u32>);

/// Project native samples without changing their mode or pixel representation.
pub(crate) fn project_native_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
) -> Result<ProjectionAxes, crate::error::PilError> {
    let width = width as usize;
    let height = height as usize;
    let sample_count = width.checked_mul(height).ok_or_else(|| {
        crate::error::PilError::DimensionError("getprojection sample count overflow".into())
    })?;
    let expected_bytes = sample_count.checked_mul(bytes_per_pixel).ok_or_else(|| {
        crate::error::PilError::DimensionError("getprojection byte count overflow".into())
    })?;
    if bytes_per_pixel == 0 || bytes.len() != expected_bytes {
        return Err(crate::error::PilError::InternalError(
            "GetProjection native byte storage did not match its mode dimensions".into(),
        ));
    }

    let mut horizontal = vec![0; width];
    let mut vertical = vec![0; height];
    if width == 0 {
        return Ok((horizontal, vertical));
    }
    for (index, pixel) in bytes.chunks_exact(bytes_per_pixel).enumerate() {
        if pixel.iter().any(|sample| *sample != 0) {
            horizontal[index % width] = 1;
            vertical[index / width] = 1;
        }
    }
    Ok((horizontal, vertical))
}

/// Preserve Pillow's I/F stored-word test, including F negative-zero bits.
pub(crate) fn project_stored_words(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<ProjectionAxes, crate::error::PilError> {
    let width = width as usize;
    let height = height as usize;
    let row_bytes = width.checked_mul(4).ok_or_else(|| {
        crate::error::PilError::DimensionError("getprojection row byte count overflow".into())
    })?;
    let expected_bytes = row_bytes.checked_mul(height).ok_or_else(|| {
        crate::error::PilError::DimensionError("getprojection byte count overflow".into())
    })?;
    if bytes.len() != expected_bytes {
        return Err(crate::error::PilError::InternalError(
            "GetProjection I/F storage did not match its dimensions".into(),
        ));
    }

    let mut horizontal = vec![0; width];
    let mut vertical = vec![0; height];
    if row_bytes == 0 {
        return Ok((horizontal, vertical));
    }
    let mut remaining_columns = width;
    for (y, row) in bytes.chunks_exact(row_bytes).enumerate() {
        let row_nonzero = if remaining_columns == 0 {
            row.chunks_exact(4).any(|sample| {
                u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) != 0
            })
        } else {
            let mut row_nonzero = false;
            for (x, sample) in row.chunks_exact(4).enumerate() {
                if u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) == 0 {
                    continue;
                }
                row_nonzero = true;
                if horizontal[x] == 0 {
                    horizontal[x] = 1;
                    remaining_columns -= 1;
                }
                if remaining_columns == 0 {
                    break;
                }
            }
            row_nonzero
        };
        vertical[y] = u32::from(row_nonzero);
    }
    Ok((horizontal, vertical))
}

/// LAB's encoded A/B neutral point is 128, not a zero byte.
pub(crate) fn project_lab(image: &RgbImage) -> ProjectionAxes {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut horizontal = vec![0; width];
    let mut vertical = vec![0; height];
    if width == 0 {
        return (horizontal, vertical);
    }
    for (index, pixel) in image.pixels().enumerate() {
        if pixel[0] != 0 || pixel[1] != 128 || pixel[2] != 128 {
            horizontal[index % width] = 1;
            vertical[index / width] = 1;
        }
    }
    (horizontal, vertical)
}

#[inline(always)]
fn cmyk_projection_pixel_is_nonzero(samples: &[u8]) -> bool {
    let mut packed = [0; 4];
    packed.copy_from_slice(samples);
    u32::from_ne_bytes(packed) != 0
}

#[inline(always)]
fn cmyk_projection_scan_row(
    row: &[u8],
    width: usize,
    block_bytes: usize,
    pixels_per_block: usize,
    all_columns_covered: &mut bool,
    covered_columns: &mut usize,
    horizontal: &mut [u32],
    block_has_nonzero: &mut impl FnMut(&[u8]) -> bool,
) -> (bool, u64) {
    let blocks = row.chunks_exact(block_bytes);
    let scalar_tail_pixels = (blocks.remainder().len() / 4) as u64;
    let row_nonzero = if *all_columns_covered {
        blocks.clone().any(&mut *block_has_nonzero)
            || blocks
                .remainder()
                .chunks_exact(4)
                .any(cmyk_projection_pixel_is_nonzero)
    } else {
        let mut any = false;
        for (block_index, block) in blocks.clone().enumerate() {
            if !block_has_nonzero(block) {
                continue;
            }
            for (lane, pixel) in block.chunks_exact(4).enumerate() {
                if !cmyk_projection_pixel_is_nonzero(pixel) {
                    continue;
                }
                let x = block_index * pixels_per_block + lane;
                if horizontal[x] == 0 {
                    horizontal[x] = 1;
                    *covered_columns += 1;
                    *all_columns_covered = *covered_columns == width;
                }
                any = true;
            }
        }
        let tail_start = blocks.len() * block_bytes;
        for (tail_index, pixel) in row[tail_start..].chunks_exact(4).enumerate() {
            if !cmyk_projection_pixel_is_nonzero(pixel) {
                continue;
            }
            let x = tail_start / 4 + tail_index;
            if horizontal[x] == 0 {
                horizontal[x] = 1;
                *covered_columns += 1;
                *all_columns_covered = *covered_columns == width;
            }
            any = true;
        }
        any
    };
    (row_nonzero, scalar_tail_pixels)
}

/// Scan native CMYK bytes with the existing 16-byte serial grouping.
pub(crate) fn cmyk_projection(image: &RgbaImage) -> (Vec<u32>, Vec<u32>) {
    cmyk_projection_with_blocks(image, 16, |samples| {
        let mut packed = [0; 16];
        packed.copy_from_slice(samples);
        u128::from_ne_bytes(packed) != 0
    })
    .0
}

/// Preserve the existing scalar RGB row traversal for narrow images.
pub(crate) fn rgb_projection(image: &RgbImage) -> (Vec<u32>, Vec<u32>) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width >= 64 {
        return rgb_projection_wide(image, width, height);
    }
    let mut horizontal = vec![0u32; width];
    let mut vertical = vec![0u32; height];
    for (y, row) in image.rows().enumerate() {
        let mut row_nonzero = false;
        for (x, pixel) in row.enumerate() {
            if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                horizontal[x] = 1;
                row_nonzero = true;
            }
        }
        vertical[y] = u32::from(row_nonzero);
    }
    (horizontal, vertical)
}

fn rgb_projection_wide(image: &RgbImage, width: usize, height: usize) -> ProjectionAxes {
    let mut horizontal = vec![0; width];
    let mut vertical = vec![0; height];
    let row_bytes = width * 3;
    let mut covered_columns = 0usize;
    let mut all_columns_covered = false;
    for (y, row) in image.as_raw().chunks_exact(row_bytes).enumerate() {
        if all_columns_covered {
            vertical[y] = u32::from(row.iter().any(|sample| *sample != 0));
            continue;
        }
        let mut row_nonzero = false;
        for (x, pixel) in row.chunks_exact(3).enumerate() {
            if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
                continue;
            }
            if horizontal[x] == 0 {
                horizontal[x] = 1;
                covered_columns += 1;
            }
            row_nonzero = true;
            if covered_columns == width {
                all_columns_covered = true;
                break;
            }
        }
        vertical[y] = u32::from(row_nonzero);
    }
    (horizontal, vertical)
}

/// Keep CMYK block detection separate from row and axis bookkeeping.
pub(crate) fn cmyk_projection_with_blocks(
    image: &RgbaImage,
    block_bytes: usize,
    mut block_has_nonzero: impl FnMut(&[u8]) -> bool,
) -> ((Vec<u32>, Vec<u32>), u64) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut horizontal = vec![0u32; width];
    let mut vertical = vec![0u32; height];
    let mut covered_columns = 0usize;
    let mut all_columns_covered = width == 0;
    let row_bytes = width * 4;
    let bytes = image.as_raw();
    debug_assert!(block_bytes >= 4 && block_bytes.is_multiple_of(4));
    let pixels_per_block = block_bytes / 4;
    let mut scalar_tail_pixels = 0u64;
    for y in 0..height {
        let row = &bytes[y * row_bytes..(y + 1) * row_bytes];
        let (row_nonzero, tail_pixels) = cmyk_projection_scan_row(
            row,
            width,
            block_bytes,
            pixels_per_block,
            &mut all_columns_covered,
            &mut covered_columns,
            &mut horizontal,
            &mut block_has_nonzero,
        );
        scalar_tail_pixels = scalar_tail_pixels.saturating_add(tail_pixels);
        if row_nonzero {
            vertical[y] = 1;
        }
    }
    ((horizontal, vertical), scalar_tail_pixels)
}
