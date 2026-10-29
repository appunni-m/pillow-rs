//! Vector proof scan for constant native RGB images used by Filter5x5.

use wide::u8x16;

pub(super) const MAX_SMALL_PIXELS: usize = 256 * 256;
const CHANNELS: usize = 3;
const LANES: usize = 16;
const GROUP_BYTES: usize = CHANNELS * LANES;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ScanEvidence {
    pub(super) is_uniform: bool,
    pub(super) vector_blocks: u64,
    pub(super) scalar_tail_pixels: u64,
}

/// Prove a large RGB image is constant. Sparse probes reject ordinary noise
/// without a full extra read; a complete SIMD comparison admits the shortcut.
pub(super) fn scan_uniform_rgb(raw: &[u8], width: usize, height: usize) -> Option<ScanEvidence> {
    let pixel_count = width.checked_mul(height)?;
    let expected_bytes = pixel_count.checked_mul(CHANNELS)?;
    if width == 0 || height == 0 || raw.len() != expected_bytes {
        return None;
    }
    let first_pixel: [u8; CHANNELS] = raw.get(..CHANNELS)?.try_into().ok()?;
    if !sampled_pixels_match(raw, width, height, first_pixel) {
        return None;
    }

    let vector_pixels = pixel_count / LANES * LANES;
    let vector_bytes = vector_pixels.checked_mul(CHANNELS)?;
    let patterns: [u8x16; CHANNELS] = std::array::from_fn(|phase| {
        u8x16::new(std::array::from_fn(|lane| {
            first_pixel[(phase + lane) % CHANNELS]
        }))
    });
    let mut vector_blocks = 0u64;
    for group in raw[..vector_bytes].chunks_exact(GROUP_BYTES) {
        for (phase, start) in (0..CHANNELS).map(|phase| (phase, phase * LANES)) {
            let bytes: [u8; LANES] = group[start..start + LANES].try_into().ok()?;
            vector_blocks = vector_blocks.checked_add(1)?;
            if !u8x16::new(bytes).simd_eq(patterns[phase]).all() {
                return Some(ScanEvidence {
                    is_uniform: false,
                    vector_blocks,
                    scalar_tail_pixels: 0,
                });
            }
        }
    }

    let tail_is_uniform = raw[vector_bytes..]
        .chunks_exact(CHANNELS)
        .all(|pixel| pixel == first_pixel);
    Some(ScanEvidence {
        is_uniform: tail_is_uniform,
        vector_blocks,
        scalar_tail_pixels: u64::try_from(pixel_count - vector_pixels).ok()?,
    })
}

fn sampled_pixels_match(raw: &[u8], width: usize, height: usize, first: [u8; CHANNELS]) -> bool {
    let last_x = width - 1;
    let last_y = height - 1;
    for row in 0..=4 {
        let y = (last_y / 4) * row + (last_y % 4) * row / 4;
        for column in 0..=6 {
            let x = (last_x / 6) * column + (last_x % 6) * column / 6;
            let start = (y * width + x) * CHANNELS;
            if raw.get(start..start + CHANNELS) != Some(first.as_slice()) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{LANES, MAX_SMALL_PIXELS, scan_uniform_rgb};

    #[test]
    fn large_uniform_rgb_scan_counts_vector_groups_and_pixel_tail() {
        let (width, height) = (257usize, 257usize);
        let pixel_count = width * height;
        assert!(pixel_count > MAX_SMALL_PIXELS);
        let raw = [17u8, 83, 149].repeat(pixel_count);
        let evidence = scan_uniform_rgb(&raw, width, height).expect("valid RGB buffer");
        assert!(evidence.is_uniform);
        assert_eq!(evidence.vector_blocks, ((pixel_count / LANES) * 3) as u64);
        assert_eq!(evidence.scalar_tail_pixels, (pixel_count % LANES) as u64);
    }

    #[test]
    fn sampled_corner_mismatch_avoids_full_image_scan() {
        let (width, height) = (257usize, 257usize);
        let mut raw = [17u8, 83, 149].repeat(width * height);
        let final_byte = raw.len() - 1;
        raw[final_byte] ^= 1;
        assert!(scan_uniform_rgb(&raw, width, height).is_none());
    }

    #[test]
    fn vector_scan_rejects_an_unsampled_rgb_difference() {
        let (width, height) = (257usize, 257usize);
        let mut raw = [17u8, 83, 149].repeat(width * height);
        let pixel = width + 1;
        raw[pixel * 3] ^= 1;
        let evidence = scan_uniform_rgb(&raw, width, height).expect("complete vector scan");
        assert!(!evidence.is_uniform);
        assert!(evidence.vector_blocks > 0);
        assert!(evidence.vector_blocks < ((width * height / LANES) * 3) as u64);
    }
}
