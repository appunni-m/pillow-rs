//! Portable 128-bit kernels used by the declared CPU architecture strategies.

use crate::raster::{RgbImage, RgbaImage};
use wide::u8x16;

use super::projection::{ProjectionAxes, cmyk_projection_with_blocks};

pub(crate) struct VectorProjection {
    pub(crate) axes: ProjectionAxes,
    pub(crate) vector_blocks: u64,
    pub(crate) scalar_tail: u64,
}

/// Preserve the archived grouped CMYK SIMD scan and its scalar edge handling.
pub(crate) fn cmyk_projection(image: &RgbaImage) -> VectorProjection {
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
    let (axes, scalar_tail) = cmyk_projection_with_blocks(image, block_bytes, |block| {
        if block.len() < 16 {
            scalar_blocks = scalar_blocks.saturating_add(1);
            let mut pixel = [0; 4];
            pixel.copy_from_slice(block);
            return u32::from_ne_bytes(pixel) != 0;
        }
        let zero = u8x16::splat(0);
        let mut combined = zero;
        for samples in block.chunks_exact(16) {
            let lanes: [u8; 16] = samples
                .try_into()
                .expect("CMYK vector block contains sixteen bytes");
            combined |= u8x16::new(lanes);
        }
        vector_blocks = vector_blocks.saturating_add((block.len() / 16) as u64);
        combined != zero
    });
    VectorProjection {
        axes,
        vector_blocks,
        scalar_tail: scalar_blocks.saturating_add(scalar_tail),
    }
}

fn rgb_block_has_nonzero(block: &[u8], zero: u8x16) -> (bool, u64) {
    let mut vectors = block.chunks_exact(16);
    let first_lanes: [u8; 16] = vectors
        .next()
        .expect("RGB vector block contains one vector")
        .try_into()
        .expect("RGB vector contains sixteen bytes");
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
            .expect("RGB vector contains sixteen bytes");
        combined |= u8x16::new(lanes);
    }
    (combined != zero, vectors_scanned)
}

/// Preserve the archived RGB 48-byte block scan and scalar remainder.
pub(crate) fn rgb_projection(image: &RgbImage) -> VectorProjection {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut horizontal = vec![0; width];
    let mut vertical = vec![0; height];
    if width == 0 || height == 0 {
        return VectorProjection {
            axes: (horizontal, vertical),
            vector_blocks: 0,
            scalar_tail: 0,
        };
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
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
                let (nonzero, scanned) = rgb_block_has_nonzero(block, zero);
                vector_blocks = vector_blocks.saturating_add(scanned);
                nonzero
            });
            if !row_nonzero {
                row_nonzero = blocks
                    .remainder()
                    .chunks_exact(3)
                    .any(|pixel| pixel.iter().any(|sample| *sample != 0));
            }
        } else {
            for (block_index, block) in blocks.by_ref().enumerate() {
                let (nonzero, scanned) = rgb_block_has_nonzero(block, zero);
                vector_blocks = vector_blocks.saturating_add(scanned);
                if !nonzero {
                    continue;
                }
                for (lane, pixel) in block.chunks_exact(3).enumerate() {
                    if pixel.iter().any(|sample| *sample != 0) {
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
                    if pixel.iter().any(|sample| *sample != 0) {
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
        let remainder = row.chunks_exact(48).remainder();
        scalar_tail = scalar_tail.saturating_add((remainder.len() / 3) as u64);
        vertical[y] = u32::from(row_nonzero);
    }

    VectorProjection {
        axes: (horizontal, vertical),
        vector_blocks,
        scalar_tail,
    }
}
