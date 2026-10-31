//! Large uniform RGBA GaussianBlur identity proof for the vector strategy.

use wide::u8x16;

const MIN_PIXELS: usize = 256 * 256;
const PIXEL_BYTES: usize = 4;
const VECTOR_BYTES: usize = 16;
const ROUNDING_BIAS: u32 = 1 << 23;
const BLUR_SCALE_BITS: u32 = 24;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::compute::pool_cpu::ops::filter::rgba8) struct ScanEvidence {
    pub(in crate::compute::pool_cpu::ops::filter::rgba8) identity: bool,
    pub(in crate::compute::pool_cpu::ops::filter::rgba8) vector_blocks: u64,
    pub(in crate::compute::pool_cpu::ops::filter::rgba8) scalar_tail_pixels: u64,
}

/// Prove that a large RGBA image is uniform and both fixed-point Gaussian
/// passes preserve every source channel byte.
pub(super) fn prove_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    horizontal: (usize, u32, u32),
    vertical: (usize, u32, u32),
) -> ScanEvidence {
    let Some(pixel_count) = width.checked_mul(height) else {
        return ScanEvidence::default();
    };
    let Some(expected_bytes) = pixel_count.checked_mul(PIXEL_BYTES) else {
        return ScanEvidence::default();
    };
    if pixel_count < MIN_PIXELS || width == 0 || height == 0 || raw.len() != expected_bytes {
        return ScanEvidence::default();
    }

    let Some(first_pixel) = raw.get(..PIXEL_BYTES) else {
        return ScanEvidence::default();
    };
    if !first_pixel.iter().all(|&value| {
        constant_pass_preserves(value, horizontal) && constant_pass_preserves(value, vertical)
    }) {
        return ScanEvidence::default();
    }
    if !sampled_pixels_match(raw, width, height, first_pixel) {
        return ScanEvidence::default();
    }

    scan_uniform_pixels(raw, first_pixel)
}

fn scan_uniform_pixels(raw: &[u8], first_pixel: &[u8]) -> ScanEvidence {
    let pattern = u8x16::new(std::array::from_fn(|index| first_pixel[index % PIXEL_BYTES]));
    let vector_end = raw.len() / VECTOR_BYTES * VECTOR_BYTES;
    let mut vector_blocks = 0u64;
    for block in raw[..vector_end].chunks_exact(VECTOR_BYTES) {
        let Ok(bytes) = <[u8; VECTOR_BYTES]>::try_from(block) else {
            return ScanEvidence::default();
        };
        vector_blocks += 1;
        if !u8x16::new(bytes).simd_eq(pattern).all() {
            return ScanEvidence { vector_blocks, ..ScanEvidence::default() };
        }
    }

    let tail = &raw[vector_end..];
    let uniform_tail = tail
        .iter()
        .enumerate()
        .all(|(index, &value)| value == first_pixel[index % PIXEL_BYTES]);
    ScanEvidence {
        identity: uniform_tail,
        vector_blocks,
        scalar_tail_pixels: (tail.len() / PIXEL_BYTES) as u64,
    }
}

#[inline]
fn constant_pass_preserves(value: u8, (radius, whole, fractional): (usize, u32, u32)) -> bool {
    let Some(window_pixels) = radius
        .checked_mul(2)
        .and_then(|diameter| diameter.checked_add(1))
        .and_then(|window| u32::try_from(window).ok())
    else {
        return false;
    };
    let mut weighted = u32::from(value)
        .wrapping_mul(window_pixels)
        .wrapping_mul(whole);
    if fractional != 0 {
        weighted = weighted.wrapping_add(
            (u32::from(value) + u32::from(value)).wrapping_mul(fractional),
        );
    }
    weighted.wrapping_add(ROUNDING_BIAS) >> BLUR_SCALE_BITS == u32::from(value)
}

fn sampled_pixels_match(raw: &[u8], width: usize, height: usize, first_pixel: &[u8]) -> bool {
    let last_x = width - 1;
    let last_y = height - 1;
    for row in 0..=4 {
        let y = (last_y / 4) * row + (last_y % 4) * row / 4;
        for column in 0..=6 {
            let x = (last_x / 6) * column + (last_x % 6) * column / 6;
            let start = (y * width + x) * PIXEL_BYTES;
            if &raw[start..start + PIXEL_BYTES] != first_pixel {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
#[path = "gaussian_blur_uniform_tests.rs"]
mod tests;
