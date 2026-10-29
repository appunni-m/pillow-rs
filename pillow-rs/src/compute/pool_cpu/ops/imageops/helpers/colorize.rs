//! Shared Colorize LUT construction and pixel kernels.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};
use wide::u8x16;

/// Builds Pillow's three 256-entry colorize tables with floor division.
pub(crate) fn lookup_tables(
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> [[u8; 256]; 3] {
    let mut lut = [[0u8; 256]; 3];
    for channel in 0..3 {
        let black_c = [black.0, black.1, black.2][channel] as i32;
        let white_c = [white.0, white.1, white.2][channel] as i32;
        let mid_c = mid.map(|color| [color.0, color.1, color.2][channel] as i32);
        let blackpoint = i32::from(blackpoint);
        let midpoint = i32::from(midpoint);
        let whitepoint = i32::from(whitepoint);
        for (index, slot) in lut[channel].iter_mut().enumerate() {
            let index = index as i32;
            let value = if index < blackpoint {
                black_c
            } else if let Some(mid_c) = mid_c {
                if index < midpoint {
                    interpolate(index, blackpoint, midpoint, black_c, mid_c)
                } else if index < whitepoint {
                    interpolate(index, midpoint, whitepoint, mid_c, white_c)
                } else {
                    white_c
                }
            } else if index < whitepoint {
                interpolate(index, blackpoint, whitepoint, black_c, white_c)
            } else {
                white_c
            };
            *slot = value.clamp(0, 255) as u8;
        }
    }
    lut
}

fn interpolate(index: i32, start: i32, end: i32, from: i32, to: i32) -> i32 {
    let span = end - start;
    if span == 0 {
        from
    } else {
        from + ((index - start) * (to - from)).div_euclid(span)
    }
}

/// Applies the prepared tables to the source's Pillow-compatible luma bytes.
pub(crate) fn execute_default(
    gray: &[u8],
    width: u32,
    height: u32,
    lut: &[[u8; 256]; 3],
) -> Result<DynamicImage, PilError> {
    let output_len = gray
        .len()
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("colorize output is too large".into()))?;
    let mut output = Vec::with_capacity(output_len);
    for &sample in gray {
        let index = usize::from(sample);
        output.extend_from_slice(&[lut[0][index], lut[1][index], lut[2][index]]);
    }
    RgbImage::from_raw(width, height, output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("colorize buffer mismatch".into()))
}

/// Applies the existing vectorized native-L LUT path selected by an ISA owner.
pub(crate) fn execute_native_luma(
    gray: &[u8],
    width: u32,
    height: u32,
    lut: &[[u8; 256]; 3],
) -> Result<DynamicImage, PilError> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("colorize output is too large".into()))?;
    if width == 0 || height == 0 || gray.len() != pixels {
        return Err(PilError::InternalError("SIMD colorize buffer mismatch".into()));
    }
    let output_bytes = pixels
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("colorize output is too large".into()))?;
    let tables = [native_lut_tables(&lut[0]), native_lut_tables(&lut[1]), native_lut_tables(&lut[2])];
    let mut output = Vec::<[u8; 3]>::with_capacity(output_bytes / 3);
    let mut vector_blocks = 0u64;

    if pixels >= 65_536 {
        vectorize_large_pixels(gray, &tables, &mut output, &mut vector_blocks);
    } else {
        vectorize_small_pixels(gray, &tables, &mut output, &mut vector_blocks);
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    RgbImage::from_raw(width, height, output.into_flattened())
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("SIMD colorize buffer mismatch".into()))
}

fn vectorize_large_pixels(
    gray: &[u8],
    tables: &[[u8x16; 16]; 3],
    output: &mut Vec<[u8; 3]>,
    vector_blocks: &mut u64,
) {
    for chunk in gray.chunks(16) {
        let mut padded = [0u8; 16];
        padded[..chunk.len()].copy_from_slice(chunk);
        let input = u8x16::new(padded);
        let red = colorize_lut_chunk(input, &tables[0]).to_array();
        let green = colorize_lut_chunk(input, &tables[1]).to_array();
        let blue = colorize_lut_chunk(input, &tables[2]).to_array();
        let mut packed = [[0u8; 3]; 16];
        for lane in 0..chunk.len() {
            packed[lane] = [red[lane], green[lane], blue[lane]];
        }
        output.extend_from_slice(&packed[..chunk.len()]);
        *vector_blocks = (*vector_blocks).saturating_add(4);
    }
}

fn vectorize_small_pixels(
    gray: &[u8],
    tables: &[[u8x16; 16]; 3],
    output: &mut Vec<[u8; 3]>,
    vector_blocks: &mut u64,
) {
    let interleave = u8x16::new([0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15]);
    for chunk in gray.chunks(16) {
        let mut padded = [0u8; 16];
        padded[..chunk.len()].copy_from_slice(chunk);
        let input = u8x16::new(padded);
        let red = colorize_lut_chunk(input, &tables[0]).to_array();
        let green = colorize_lut_chunk(input, &tables[1]).to_array();
        let blue = colorize_lut_chunk(input, &tables[2]).to_array();

        for group in 0..4 {
            let lane = group * 4;
            let packed = u8x16::new([
                red[lane],
                red[lane + 1],
                red[lane + 2],
                red[lane + 3],
                green[lane],
                green[lane + 1],
                green[lane + 2],
                green[lane + 3],
                blue[lane],
                blue[lane + 1],
                blue[lane + 2],
                blue[lane + 3],
                0,
                0,
                0,
                0,
            ])
            .swizzle_relaxed(interleave)
            .to_array();
            let group_pixels = chunk.len().saturating_sub(lane).min(4);
            if group_pixels != 0 {
                let pixels = [
                    [packed[0], packed[1], packed[2]],
                    [packed[3], packed[4], packed[5]],
                    [packed[6], packed[7], packed[8]],
                    [packed[9], packed[10], packed[11]],
                ];
                output.extend_from_slice(&pixels[..group_pixels]);
            }
            *vector_blocks = (*vector_blocks).saturating_add(1);
        }
    }
}

fn native_lut_tables(lut: &[u8; 256]) -> [u8x16; 16] {
    std::array::from_fn(|table| {
        let start = table * 16;
        u8x16::new(std::array::from_fn(|lane| lut[start + lane]))
    })
}

#[inline]
fn colorize_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let lookups: [u8x16; 16] = std::array::from_fn(|index| tables[index].swizzle_relaxed(low));
    let bit0 = (high & u8x16::splat(1)).simd_eq(u8x16::splat(0));
    let bit1 = (high & u8x16::splat(2)).simd_eq(u8x16::splat(0));
    let bit2 = (high & u8x16::splat(4)).simd_eq(u8x16::splat(0));
    let bit3 = (high & u8x16::splat(8)).simd_eq(u8x16::splat(0));
    let level1: [u8x16; 8] =
        std::array::from_fn(|index| bit0.select(lookups[index * 2], lookups[index * 2 + 1]));
    let level2: [u8x16; 4] =
        std::array::from_fn(|index| bit1.select(level1[index * 2], level1[index * 2 + 1]));
    let level3: [u8x16; 2] =
        std::array::from_fn(|index| bit2.select(level2[index * 2], level2[index * 2 + 1]));
    bit3.select(level3[0], level3[1])
}
