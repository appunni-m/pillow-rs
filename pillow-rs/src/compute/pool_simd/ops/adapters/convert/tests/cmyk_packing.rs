//! Focused guards for the native x86 CMYK RGB packing path.

use super::cmyk::native_cmyk_to_rgb_bytes;
use crate::compute::{Backend, ExecutionContext};
use crate::raster::{DynamicImage, RgbImage, RgbaImage};

#[test]
#[allow(unsafe_code)]
fn rgb16_packing_preserves_channel_order_across_output_blocks() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }
    for seed in 0..=u8::MAX {
        let red = std::array::from_fn(|i| seed.wrapping_add((i * 11) as u8));
        let green = std::array::from_fn(|i| seed.wrapping_sub((i * 17) as u8));
        let blue = std::array::from_fn(|i| seed ^ (i * 13) as u8);
        // SAFETY: this test checks SSSE3 support above, and all inputs have
        // sixteen initialized bytes.
        let packed = unsafe {
            super::super::super::native_merge::rgb16_interleave_ssse3(&red, &green, &blue)
        };
        for pixel in 0..16 {
            assert_eq!(
                &packed[pixel * 3..pixel * 3 + 3],
                &[red[pixel], green[pixel], blue[pixel]],
            );
        }
    }
}

#[test]
fn cmyk_conversion_preserves_rounding_length_and_vector_tails() {
    let context = ExecutionContext::for_backend(Backend::Simd);
    const SAMPLES: [[u8; 4]; 8] = [
        [0, 0, 0, 0],
        [255, 255, 255, 0],
        [0, 255, 0, 255],
        [255, 0, 255, 255],
        [17, 83, 149, 211],
        [128, 127, 129, 1],
        [1, 254, 255, 128],
        [250, 3, 123, 254],
    ];
    for pixels in [0usize, 1, 15, 16, 17, 31, 32, 33] {
        let source: Vec<u8> = (0..pixels)
            .flat_map(|i| SAMPLES[i % SAMPLES.len()])
            .collect();
        let expected: Vec<u8> = source
            .chunks_exact(4)
            .flat_map(|pixel| {
                let ink = 255u32 - u32::from(pixel[3]);
                [pixel[0], pixel[1], pixel[2]]
                    .map(|sample| (ink - crate::color::muldiv255(u32::from(sample), ink)) as u8)
            })
            .collect();
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_raw(pixels as u32, 1, source).unwrap());
        let (output, vector_blocks, scalar_tail) =
            native_cmyk_to_rgb_bytes(&context, &image).unwrap();
        assert_eq!(output, expected, "pixel count {pixels}");
        assert_eq!(output.len(), pixels * 3);
        assert_eq!(vector_blocks, pixels.div_ceil(16) as u64);
        assert_eq!(scalar_tail, 0);
    }
    let wrong_layout = DynamicImage::ImageRgb8(RgbImage::new(16, 1));
    assert!(native_cmyk_to_rgb_bytes(&context, &wrong_layout).is_none());
    let extra_storage = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![0; 8]).unwrap());
    assert!(native_cmyk_to_rgb_bytes(&context, &extra_storage).is_none());
}
