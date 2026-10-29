//! Focused contract tests for SIMD Filter and UnsharpMask blend paths.

#[cfg(not(feature = "parallel"))]
use super::rgb_blur_ring_5x5;
use super::uniform_rgb_5x5_identity;
use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
use crate::raster::{DynamicImage, RgbImage};

#[cfg(not(feature = "parallel"))]
const RGB_BLUR_RING: [f32; 25] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];

#[test]
fn uniform_rgb_box_fast_path_matches_cpu_reference_for_byte_edges() {
    const WIDTH: usize = 257;
    const HEIGHT: usize = 256;
    let kernel = [1.0f32; 25];

    for value in [0u8, 1, 2, 17, 127, 128, 253, 254, 255] {
        let pixel = [value, 255 - value, value.wrapping_mul(31)];
        let raw = pixel.repeat(WIDTH * HEIGHT);
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid uniform RGB image"),
        );
        let expected = execute_filter5x5(&image, &kernel, 25.0, 0.0, Some("RGB"))
            .expect("CPU Filter5x5 reference");
        let actual = uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("RGB"))
            .expect("uniform RGB fast path admission");
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "pixel={pixel:?}");
    }
}

#[test]
fn uniform_rgb_box_fast_path_rejects_other_modes_and_parameters() {
    const WIDTH: usize = 257;
    const HEIGHT: usize = 256;
    let raw = [17u8, 83, 149].repeat(WIDTH * HEIGHT);
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid uniform RGB image"),
    );
    let kernel = [1.0f32; 25];
    assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("L")).is_none());
    assert!(uniform_rgb_5x5_identity(&image, &kernel, 24.0, 0.0, Some("RGB")).is_none());
    assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 1.0, Some("RGB")).is_none());
}

#[test]
#[cfg(not(feature = "parallel"))]
fn rgb_blur_ring_simd_path_matches_cpu_for_unaligned_interleaved_tails() {
    const WIDTH: usize = 257;
    const HEIGHT: usize = 257;
    let raw = (0..WIDTH * HEIGHT * 3)
        .map(|index| ((index * 73 + index / 7 * 29 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid RGB image"),
    );
    let expected = execute_filter5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
        .expect("CPU RGB BLUR ring reference");
    let actual = rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
        .expect("large RGB BLUR ring SIMD admission");
    assert_eq!(actual.as_bytes(), expected.as_bytes());
}

#[test]
#[cfg(not(feature = "parallel"))]
fn rgb_blur_ring_simd_path_declines_small_or_nonmatching_inputs() {
    let raw = [17u8, 83, 149].repeat(256 * 256);
    let image =
        DynamicImage::ImageRgb8(RgbImage::from_raw(256, 256, raw).expect("valid RGB image"));
    assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB")).is_none());
    assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 15.0, 0.0, Some("RGB")).is_none());
    assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("L")).is_none());
}

#[test]
fn unsharp_mask_150_blend_matches_scalar_for_every_byte_pair() {
    use super::simd_unsharp_blend_rgb_150;

    let mut original = Vec::with_capacity(256 * 256);
    let mut blurred = Vec::with_capacity(256 * 256);
    for source in 0..=u8::MAX {
        for smooth in 0..=u8::MAX {
            original.push(source);
            blurred.push(smooth);
        }
    }

    for threshold in [-1, 0, 3, i32::from(u8::MAX), 256] {
        let expected: Vec<u8> = original
            .iter()
            .zip(&blurred)
            .map(|(&source, &smooth)| {
                let difference = i32::from(source) - i32::from(smooth);
                if difference.abs() > threshold {
                    (i32::from(source) + difference * 150 / 100).clamp(0, 255) as u8
                } else {
                    source
                }
            })
            .collect();

        let mut actual = blurred.clone();
        assert_eq!(
            simd_unsharp_blend_rgb_150(&original, &mut actual, threshold),
            true,
            "matching byte buffers use the exact SIMD blend"
        );
        assert_eq!(actual, expected, "threshold={threshold}");
    }
}

#[test]
fn unsharp_mask_150_blend_handles_scalar_tail_and_mismatched_buffers() {
    use super::simd_unsharp_blend_rgb_150;

    let original = [
        0, 1, 2, 3, 128, 254, 255, 100, 101, 200, 10, 11, 12, 250, 7, 9, 16,
    ];
    let blurred = [
        255, 255, 255, 0, 127, 255, 0, 105, 96, 190, 20, 8, 9, 0, 10, 11, 20,
    ];
    let expected: Vec<u8> = original
        .iter()
        .zip(blurred)
        .map(|(&source, smooth)| {
            let difference = i32::from(source) - i32::from(smooth);
            if difference.abs() > 3 {
                (i32::from(source) + difference * 150 / 100).clamp(0, 255) as u8
            } else {
                source
            }
        })
        .collect();
    let mut actual = blurred;
    assert!(simd_unsharp_blend_rgb_150(&original, &mut actual, 3));
    assert_eq!(actual.as_slice(), expected.as_slice());
    let mut mismatched = [0; 16];
    assert!(!simd_unsharp_blend_rgb_150(&original, &mut mismatched, 3));
}
