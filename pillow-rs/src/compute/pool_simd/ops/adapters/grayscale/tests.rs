//! Focused contract tests for the SIMD Grayscale family.

use crate::compute::{Backend, ExecutionContext};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, RgbImage, RgbaImage};

#[test]
fn native_cmyk_grayscale_matches_exact_conversion_across_vector_tails() {
    let context = ExecutionContext::for_backend(Backend::Simd);
    for width in [1u32, 15, 16, 17, 31, 32, 33] {
        let height = 3u32;
        let source: Vec<u8> = (0..width * height * 4)
            .map(|index| ((index * 97 + index / 3 + 11) & 255) as u8)
            .collect();
        let expected: Vec<u8> = source
            .chunks_exact(4)
            .map(|pixel| {
                let nk = 255u32 - u32::from(pixel[3]);
                let red = (nk - crate::color::muldiv255(u32::from(pixel[0]), nk)) as u8;
                let green = (nk - crate::color::muldiv255(u32::from(pixel[1]), nk)) as u8;
                let blue = (nk - crate::color::muldiv255(u32::from(pixel[2]), nk)) as u8;
                crate::color::rgb_to_luma_u8(red, green, blue)
            })
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, source).expect("valid CMYK storage"),
        );
        let (actual, vector_blocks, scalar_tail) =
            super::native_cmyk_grayscale_bytes(&context, &image).expect("native CMYK grayscale");

        assert_eq!(actual, expected, "CMYK grayscale width {width}");
        assert_eq!(vector_blocks, (width as u64 * height as u64).div_ceil(16));
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_rgb_grayscale_matches_exact_conversion_across_vector_tails() {
    let context = ExecutionContext::for_backend(Backend::Simd);
    for (width, height) in [
        (1u32, 3u32),
        (15, 3),
        (16, 3),
        (17, 3),
        (31, 3),
        (32, 3),
        (33, 3),
        (1023, 512),
    ] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels * 3)
            .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("valid RGB storage"),
        );
        let expected = crate::color::pil_grayscale(&image).expect("reference grayscale");
        let (actual, vector_blocks, scalar_tail) =
            super::native_grayscale_bytes(&context, &image, 3).expect("native RGB grayscale");

        assert_eq!(
            actual.as_slice(),
            expected.as_raw().as_slice(),
            "RGB grayscale {width}x{height}"
        );
        assert_eq!(vector_blocks, pixels.div_ceil(16) as u64);
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_la_and_rgba_grayscale_match_exact_conversion_across_vector_tails() {
    let context = ExecutionContext::for_backend(Backend::Simd);
    use crate::raster::GrayAlphaImage;

    for width in [1u32, 15, 16, 17, 31, 32, 33] {
        let height = 3u32;
        let pixels = width as usize * height as usize;

        let la_source: Vec<u8> = (0..pixels * 2)
            .map(|index| ((index * 89 + index / 7 + 13) & 255) as u8)
            .collect();
        let la_image = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, la_source).expect("valid LA storage"),
        );
        let la_expected = crate::color::pil_grayscale(&la_image).expect("LA reference");
        let (la_actual, la_vector_blocks, la_scalar_tail) =
            super::native_grayscale_bytes(&context, &la_image, 2).expect("native LA grayscale");
        assert_eq!(
            la_actual.as_slice(),
            la_expected.as_raw().as_slice(),
            "LA grayscale width {width}"
        );
        assert_eq!(la_vector_blocks, pixels.div_ceil(16) as u64);
        assert_eq!(la_scalar_tail, 0);

        let rgba_source: Vec<u8> = (0..pixels * 4)
            .map(|index| ((index * 71 + index / 9 + 31) & 255) as u8)
            .collect();
        let rgba_image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, rgba_source).expect("valid RGBA storage"),
        );
        let rgba_expected = crate::color::pil_grayscale(&rgba_image).expect("RGBA reference");
        let (rgba_actual, rgba_vector_blocks, rgba_scalar_tail) =
            super::native_grayscale_bytes(&context, &rgba_image, 4).expect("native RGBA grayscale");
        assert_eq!(
            rgba_actual.as_slice(),
            rgba_expected.as_raw(),
            "RGBA grayscale width {width}"
        );
        assert_eq!(rgba_vector_blocks, pixels.div_ceil(16) as u64);
        assert_eq!(rgba_scalar_tail, 0);
    }
}

#[test]
fn native_ycbcr_grayscale_gathers_y_across_vector_tails() {
    let context = ExecutionContext::for_backend(Backend::Simd);
    for (width, height) in [(3u32, 1u32), (15, 3), (16, 3), (17, 3), (1024, 768)] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels * 3)
            .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
            .collect();
        let expected: Vec<u8> = source.chunks_exact(3).map(|pixel| pixel[0]).collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("valid YCbCr storage"),
        );
        let (actual, vector_blocks, scalar_tail) =
            super::native_ycbcr_grayscale_bytes(&context, &image).expect("native YCbCr grayscale");

        assert_eq!(actual, expected, "YCbCr grayscale {width}x{height}");
        assert_eq!(vector_blocks, (pixels / 16) as u64);
        assert_eq!(scalar_tail, (pixels % 16) as u64);

        let result = super::simd_grayscale(&image, &PipelineOp::Grayscale, Some("YCbCr"))
            .expect("SIMD YCbCr grayscale must be admitted");
        assert_eq!(result.as_bytes(), actual, "SIMD output {width}x{height}");
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("ssse3") {
        assert_eq!(
            super::native_ycbcr_grayscale_path(&context),
            "ssse3-deinterleave"
        );
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    assert_eq!(
        super::native_ycbcr_grayscale_path(&context),
        "neon-deinterleave"
    );
}
