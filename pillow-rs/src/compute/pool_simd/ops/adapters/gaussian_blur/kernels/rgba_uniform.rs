//! Large uniform RGBA GaussianBlur identity proof for the SIMD adapter.

use wide::u8x16;

const MIN_PIXELS: usize = 256 * 256;
const PIXEL_BYTES: usize = 4;
const VECTOR_BYTES: usize = 16;
const ROUNDING_BIAS: u32 = 1 << 23;
const BLUR_SCALE_BITS: u32 = 24;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ScanEvidence {
    pub(super) identity: bool,
    pub(super) vector_blocks: u64,
    pub(super) scalar_tail_pixels: u64,
}

/// Prove that a large RGBA image is uniform and both fixed-point Gaussian
/// passes preserve every source channel byte. Probe points can reject quickly,
/// but only the full vector scan can admit the identity shortcut.
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

    let pattern = u8x16::new(std::array::from_fn(|index| {
        first_pixel[index % PIXEL_BYTES]
    }));
    let vector_end = raw.len() / VECTOR_BYTES * VECTOR_BYTES;
    let mut vector_blocks = 0u64;
    for block in raw[..vector_end].chunks_exact(VECTOR_BYTES) {
        let Ok(bytes) = <[u8; VECTOR_BYTES]>::try_from(block) else {
            return ScanEvidence::default();
        };
        vector_blocks += 1;
        if !u8x16::new(bytes).simd_eq(pattern).all() {
            return ScanEvidence {
                vector_blocks,
                ..ScanEvidence::default()
            };
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
fn constant_pass_preserves(
    value: u8,
    (radius, whole_weight, fractional_weight): (usize, u32, u32),
) -> bool {
    let Some(window_pixels) = radius
        .checked_mul(2)
        .and_then(|diameter| diameter.checked_add(1))
        .and_then(|window| u32::try_from(window).ok())
    else {
        return false;
    };

    let mut weighted = u32::from(value)
        .wrapping_mul(window_pixels)
        .wrapping_mul(whole_weight);
    if fractional_weight != 0 {
        weighted = weighted
            .wrapping_add((u32::from(value) + u32::from(value)).wrapping_mul(fractional_weight));
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
mod tests {
    use super::{ScanEvidence, prove_identity};

    fn gaussian_parameters(radius: f32) -> (usize, u32, u32) {
        let integer_radius = radius as i32 as usize;
        let window_pixels = (2 * integer_radius + 1) as u32;
        let whole_weight = ((1u32 << 24) as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = ((1u32 << 24).wrapping_sub(window_pixels * whole_weight)) / 2;
        (integer_radius, whole_weight, fractional_weight)
    }

    fn rgba(width: usize, height: usize, pixel: [u8; 4]) -> Vec<u8> {
        pixel.repeat(width * height)
    }

    #[test]
    fn vector_scan_proves_large_uniform_identity_and_counts_tail() {
        let (radius, whole, fractional) =
            gaussian_parameters(super::super::gaussian_blur_radius(7.5).unwrap());
        let parameters = (radius, whole, fractional);
        let raw = rgba(257, 257, [83, 149, 17, 191]);
        let evidence = prove_identity(&raw, 257, 257, parameters, parameters);

        assert!(evidence.identity);
        assert_eq!(evidence.vector_blocks, (raw.len() / 16) as u64);
        assert_eq!(evidence.scalar_tail_pixels, 1);
    }

    #[test]
    fn vector_scan_rejects_an_unsampled_pixel_outside_the_probe_grid() {
        let (radius, whole, fractional) =
            gaussian_parameters(super::super::gaussian_blur_radius(7.5).unwrap());
        let parameters = (radius, whole, fractional);
        let mut raw = rgba(257, 256, [83, 149, 17, 191]);
        raw[(257 + 1) * 4] ^= 1;
        let evidence = prove_identity(&raw, 257, 256, parameters, parameters);

        assert!(!evidence.identity);
        assert!(evidence.vector_blocks > 0);
        assert_eq!(evidence.scalar_tail_pixels, 0);
    }

    #[test]
    fn non_identity_constant_skips_the_full_scan() {
        let raw = rgba(257, 257, [255, 255, 255, 255]);
        let evidence = prove_identity(&raw, 257, 257, (7, 1, 1), (7, 1, 1));

        assert_eq!(evidence, ScanEvidence::default());
    }

    #[test]
    fn minimum_area_rejects_just_below_and_accepts_at_the_threshold() {
        let (radius, whole, fractional) =
            gaussian_parameters(super::super::gaussian_blur_radius(7.5).unwrap());
        let parameters = (radius, whole, fractional);
        let pixel = [83, 149, 17, 191];

        let below = rgba(255, 256, pixel);
        assert_eq!(
            prove_identity(&below, 255, 256, parameters, parameters),
            ScanEvidence::default()
        );

        let at_threshold = rgba(256, 256, pixel);
        let evidence = prove_identity(&at_threshold, 256, 256, parameters, parameters);
        assert!(evidence.identity);
        assert_eq!(
            evidence.vector_blocks,
            (at_threshold.len() / super::VECTOR_BYTES) as u64
        );
    }
}
