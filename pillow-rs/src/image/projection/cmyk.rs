use crate::raster::{RgbImage, RgbaImage};

use super::super::Image;

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

impl Image {
    pub(super) fn cmyk_projection(image: &RgbaImage) -> (Vec<u32>, Vec<u32>) {
        Self::cmyk_projection_with_blocks(image, 16, |samples| {
            let mut packed = [0; 16];
            packed.copy_from_slice(samples);
            u128::from_ne_bytes(packed) != 0
        })
        .0
    }

    pub(super) fn rgb_projection(image: &RgbImage) -> (Vec<u32>, Vec<u32>) {
        let (width, height) = (image.width() as usize, image.height() as usize);
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

    /// Scan CMYK storage in fixed-size groups while leaving block detection to
    /// the selected execution profile. Every group is a whole number of
    /// four-byte pixels; the final partial group is handled pixel by pixel.
    pub(crate) fn cmyk_projection_with_blocks(
        image: &RgbaImage,
        block_bytes: usize,
        mut block_has_nonzero: impl FnMut(&[u8]) -> bool,
    ) -> ((Vec<u32>, Vec<u32>), u64) {
        let (w, h) = (image.width() as usize, image.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        let mut covered_columns = 0usize;
        let mut all_columns_covered = w == 0;
        let row_bytes = w * 4;
        let bytes = image.as_raw();
        debug_assert!(block_bytes >= 4 && block_bytes.is_multiple_of(4));
        let pixels_per_block = block_bytes / 4;
        let mut scalar_tail_pixels = 0u64;
        for y in 0..h {
            let row = &bytes[y * row_bytes..(y + 1) * row_bytes];
            let (row_nonzero, tail_pixels) = cmyk_projection_scan_row(
                row,
                w,
                block_bytes,
                pixels_per_block,
                &mut all_columns_covered,
                &mut covered_columns,
                &mut h_proj,
                &mut block_has_nonzero,
            );
            scalar_tail_pixels = scalar_tail_pixels.saturating_add(tail_pixels);
            if row_nonzero {
                v_proj[y] = 1;
            }
        }
        ((h_proj, v_proj), scalar_tail_pixels)
    }
}
