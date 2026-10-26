//! SIMD adapters for the terminal GetProjection operation.
//!
//! These kernels preserve native RGB and CMYK storage, returning Pillow's two
//! projection axes plus backend execution evidence.

use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{RgbImage, RgbaImage};
use wide::{u8x16, u32x4};

/// Compute CMYK projections while keeping the native four-byte carrier.
/// Complete 64-byte groups combine four portable SIMD vectors before one zero
/// comparison, avoiding per-vector branches on sparse rows. Pixels in nonzero
/// groups are inspected only to set their individual horizontal columns. The
/// final partial group remains scalar.
pub(crate) fn simd_cmyk_projection(image: &RgbaImage) -> (Vec<u32>, Vec<u32>, u64, u64) {
    let row_bytes = image.width() as usize * 4;
    let block_bytes = if row_bytes >= 64 {
        64
    } else if row_bytes >= 16 {
        16
    } else {
        4
    };
    let mut vector_blocks = 0u64;
    let mut scalar_blocks = 0u64;
    let ((horizontal, vertical), scalar_tail) =
        Image::cmyk_projection_with_blocks(image, block_bytes, |block| {
            if block.len() < 16 {
                scalar_blocks = scalar_blocks.saturating_add(1);
                let mut packed = [0; 4];
                packed.copy_from_slice(block);
                return u32::from_ne_bytes(packed) != 0;
            }
            let zero = u8x16::splat(0);
            let mut combined = zero;
            for samples in block.chunks_exact(16) {
                let lanes: [u8; 16] = samples
                    .try_into()
                    .expect("CMYK SIMD projection block has sixteen bytes");
                combined |= u8x16::new(lanes);
            }
            // Count 16-byte vector equivalents per scan block instead of
            // updating this counter once for every vector in the hot loop.
            vector_blocks = vector_blocks.saturating_add((block.len() / 16) as u64);
            combined != zero
        });
    (
        horizontal,
        vertical,
        vector_blocks,
        scalar_blocks.saturating_add(scalar_tail),
    )
}

fn simd_cmyk_projection_fill_axis(output: &mut [u32], marks: &[u32]) -> (u64, u64) {
    let zero = u32x4::splat(0);
    let one = u32x4::splat(1);
    let vector_len = output.len() / 4 * 4;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(4) {
        let lanes = u32x4::new([
            start as u32,
            start as u32 + 1,
            start as u32 + 2,
            start as u32 + 3,
        ]);
        let mut projected = zero;
        for &mark in marks {
            projected |= lanes.simd_eq(u32x4::splat(mark)).bitselect(one, zero);
        }
        output[start..start + 4].copy_from_slice(&projected.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }

    for (index, value) in output.iter_mut().enumerate().skip(vector_len) {
        *value = u32::from(marks.contains(&(index as u32)));
    }
    (
        vector_blocks,
        output.len().saturating_sub(vector_len) as u64,
    )
}

/// Build CMYK projection axes from a proven zero source and a small set of
/// positive deferred writes. The SIMD comparisons materialize each four-axis
/// output block directly, avoiding a full-frame source scan.
pub(crate) fn simd_cmyk_projection_from_positive_writes(
    width: u32,
    height: u32,
    ops: &[PipelineOp],
) -> Option<(Vec<u32>, Vec<u32>, u64, u64)> {
    const MAX_WRITES: usize = 8;
    if ops.is_empty() || ops.len() > MAX_WRITES {
        return None;
    }

    let (mut x_marks, mut y_marks) = ([0u32; MAX_WRITES], [0u32; MAX_WRITES]);
    for (index, op) in ops.iter().enumerate() {
        let PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index,
            ..
        } = op
        else {
            return None;
        };
        if *palette_index
            || *x >= width
            || *y >= height
            || (color.0 == 0 && color.1 == 0 && color.2 == 0 && color.3 == 0)
        {
            // A zero write can clear an earlier value, so it needs the
            // general overwrite-aware projection path.
            return None;
        }
        x_marks[index] = *x;
        y_marks[index] = *y;
    }

    let mut horizontal = vec![0u32; width as usize];
    let mut vertical = vec![0u32; height as usize];
    let (horizontal_vectors, horizontal_tail) =
        simd_cmyk_projection_fill_axis(&mut horizontal, &x_marks[..ops.len()]);
    let (vertical_vectors, vertical_tail) =
        simd_cmyk_projection_fill_axis(&mut vertical, &y_marks[..ops.len()]);
    Some((
        horizontal,
        vertical,
        horizontal_vectors.saturating_add(vertical_vectors),
        horizontal_tail.saturating_add(vertical_tail),
    ))
}

/// Compute RGB projections while screening 48-byte blocks with native SIMD.
/// A nonzero block is decoded into RGB pixels only to mark its columns.
#[inline]
fn simd_rgb_block_has_nonzero(block: &[u8], zero: u8x16) -> (bool, u64) {
    let mut vectors = block.chunks_exact(16);
    let first_lanes: [u8; 16] = vectors
        .next()
        .expect("RGB SIMD projection block has one vector")
        .try_into()
        .expect("RGB SIMD projection vector has sixteen bytes");
    let first = u8x16::new(first_lanes);
    if first != zero {
        return (true, 1);
    }

    let mut combined = first;
    let remaining_vectors = vectors.len();
    let vectors_scanned = (remaining_vectors + 1) as u64;
    for samples in vectors {
        let lanes: [u8; 16] = samples
            .try_into()
            .expect("RGB SIMD projection vector has sixteen bytes");
        combined |= u8x16::new(lanes);
    }
    (combined != zero, vectors_scanned)
}

pub(crate) fn simd_rgb_projection(image: &RgbImage) -> (Vec<u32>, Vec<u32>, u64, u64) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut horizontal = vec![0u32; width];
    let mut vertical = vec![0u32; height];
    if width == 0 || height == 0 {
        return (horizontal, vertical, 0, 0);
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail_pixels = 0u64;
    let mut covered_columns = 0usize;
    let mut all_columns_covered = false;
    let zero = u8x16::splat(0);

    for (y, row) in image
        .as_raw()
        .chunks_exact(width.saturating_mul(3))
        .enumerate()
    {
        let mut blocks = row.chunks_exact(48);
        let mut row_nonzero = false;
        if all_columns_covered {
            row_nonzero = blocks.clone().any(|block| {
                let (block_nonzero, scanned) = simd_rgb_block_has_nonzero(block, zero);
                vector_blocks = vector_blocks.saturating_add(scanned);
                block_nonzero
            });
            if !row_nonzero {
                row_nonzero = blocks
                    .remainder()
                    .chunks_exact(3)
                    .any(|pixel| pixel.iter().any(|value| *value != 0));
            }
        } else {
            for (block_index, block) in blocks.by_ref().enumerate() {
                let (block_nonzero, scanned) = simd_rgb_block_has_nonzero(block, zero);
                vector_blocks = vector_blocks.saturating_add(scanned);
                if !block_nonzero {
                    continue;
                }
                for (lane, pixel) in block.chunks_exact(3).enumerate() {
                    if pixel.iter().any(|value| *value != 0) {
                        let column = &mut horizontal[block_index * 16 + lane];
                        if *column == 0 {
                            *column = 1;
                            covered_columns += 1;
                            all_columns_covered = covered_columns == width;
                        }
                        row_nonzero = true;
                    }
                }
                if all_columns_covered {
                    row_nonzero = true;
                    break;
                }
            }
            if !all_columns_covered {
                let tail = blocks.remainder();
                let tail_start = row.len() - tail.len();
                for (tail_index, pixel) in tail.chunks_exact(3).enumerate() {
                    if pixel.iter().any(|value| *value != 0) {
                        let column = &mut horizontal[tail_start / 3 + tail_index];
                        if *column == 0 {
                            *column = 1;
                            covered_columns += 1;
                            all_columns_covered = covered_columns == width;
                        }
                        row_nonzero = true;
                        if all_columns_covered {
                            break;
                        }
                    }
                }
            }
        }
        let tail = row.chunks_exact(48).remainder();
        scalar_tail_pixels = scalar_tail_pixels.saturating_add((tail.len() / 3) as u64);
        if row_nonzero {
            vertical[y] = 1;
        }
    }

    (horizontal, vertical, vector_blocks, scalar_tail_pixels)
}
