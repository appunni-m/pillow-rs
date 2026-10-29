//! Focused contract tests for the SIMD Convert family.

use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayImage, RgbImage, RgbaImage};

#[test]
fn native_luma_to_rgb_matches_exact_conversion_across_vector_tails() {
    for (width, height) in [
        (1u32, 1u32),
        (5, 3),
        (15, 3),
        (16, 3),
        (17, 3),
        (31, 3),
        (32, 3),
        (33, 3),
        (1023, 512),
    ] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels)
            .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, source).expect("valid L storage"),
        );
        let expected = image.to_rgb8();
        let (actual, vector_blocks, scalar_tail) =
            super::native_luma_to_rgb_bytes(&image).expect("native L to RGB");

        assert_eq!(
            actual.as_slice(),
            expected.as_raw().as_slice(),
            "{width}x{height}"
        );
        let expected_vector_blocks = pixels.div_ceil(16) as u64;
        assert_eq!(vector_blocks, expected_vector_blocks);
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_luma_to_typed_samples_does_not_use_rgba_expansion() {
    use crate::ColorMode;

    for width in [1u32, 7, 8, 9, 17, 33] {
        for mode in ["L", "LA"] {
            let luma: Vec<u8> = (0..width as usize * 3)
                .map(|index| [0, 1, 127, 128, 254, 255][index % 6])
                .collect();
            let image = if mode == "L" {
                DynamicImage::ImageLuma8(
                    GrayImage::from_raw(width, 3, luma.clone()).expect("L samples"),
                )
            } else {
                let bytes = luma
                    .iter()
                    .enumerate()
                    .flat_map(|(index, &sample)| [sample, index as u8])
                    .collect();
                DynamicImage::ImageLumaA8(
                    crate::raster::GrayAlphaImage::from_raw(width, 3, bytes).expect("LA samples"),
                )
            };
            for target in [ColorMode::I, ColorMode::F] {
                let expected: Vec<u8> = luma
                    .iter()
                    .flat_map(|&sample| match target {
                        ColorMode::I => i32::from(sample).to_le_bytes(),
                        ColorMode::F => f32::from(sample).to_le_bytes(),
                        _ => unreachable!(),
                    })
                    .collect();
                let op = PipelineOp::Convert {
                    mode: target.clone(),
                    matrix: None,
                    dither: None,
                };
                let actual = super::simd_convert(&image, &op, Some(mode))
                    .expect("typed native SIMD conversion");
                assert_eq!(
                    actual.as_bytes(),
                    expected,
                    "{mode} to {target:?}, width {width}"
                );
            }
        }
    }
}

#[test]
fn native_luma_to_rgba_matches_exact_conversion_across_vector_tails() {
    for (width, height) in [
        (1u32, 1u32),
        (3, 5),
        (15, 3),
        (16, 3),
        (17, 3),
        (31, 3),
        (32, 3),
        (33, 3),
        (1023, 512),
    ] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels)
            .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, source).expect("valid L storage"),
        );
        let expected = image.to_rgba8();
        let (actual, vector_blocks, scalar_tail) =
            super::native_luma_to_rgba_bytes(&image).expect("native L to RGBA");

        assert_eq!(
            actual.as_slice(),
            expected.as_raw().as_slice(),
            "{width}x{height}"
        );
        assert_eq!(vector_blocks, pixels.div_ceil(16) as u64);
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_luma_alpha_to_rgba_matches_exact_conversion_across_vector_tails() {
    for (width, height) in [
        (1u32, 1u32),
        (3, 5),
        (15, 3),
        (16, 3),
        (17, 3),
        (31, 3),
        (32, 3),
        (33, 3),
        (1023, 512),
    ] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels * 2)
            .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(width, height, source)
                .expect("valid LA storage"),
        );
        let expected = image.to_rgba8();
        let layout = super::NativeConvertLayout {
            source_channels: 2,
            target_channels: 4,
            source_is_luma: true,
            source_is_rgbx: false,
            target_is_luma: false,
            target_is_cmyk: false,
            target_is_hsv: false,
            target_is_ycbcr: false,
            target_is_integer: false,
            target_is_float: false,
        };
        let (actual, vector_blocks, scalar_tail) =
            super::native_convert_bytes(&image, layout).expect("native LA to RGBA");

        assert_eq!(
            actual.as_slice(),
            expected.as_raw().as_slice(),
            "{width}x{height}"
        );
        assert_eq!(vector_blocks, pixels.div_ceil(4) as u64);
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_rgba_to_la_matches_exact_conversion_across_vector_tails() {
    for (width, height) in [
        (1u32, 1u32),
        (15, 3),
        (16, 3),
        (17, 3),
        (31, 3),
        (32, 3),
        (33, 3),
        (1023, 512),
    ] {
        let pixels = width as usize * height as usize;
        let source: Vec<u8> = (0..pixels * 4)
            .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, source).expect("valid RGBA storage"),
        );
        let gray = crate::color::pil_grayscale(&image).expect("reference luma");
        let expected: Vec<u8> = gray
            .as_raw()
            .iter()
            .copied()
            .zip(image.as_bytes().chunks_exact(4).map(|pixel| pixel[3]))
            .flat_map(|(luma, alpha)| [luma, alpha])
            .collect();
        let layout = super::NativeConvertLayout {
            source_channels: 4,
            target_channels: 2,
            source_is_luma: false,
            source_is_rgbx: false,
            target_is_luma: true,
            target_is_cmyk: false,
            target_is_hsv: false,
            target_is_ycbcr: false,
            target_is_integer: false,
            target_is_float: false,
        };
        let (actual, vector_blocks, scalar_tail) =
            super::native_convert_bytes(&image, layout).expect("native RGBA to LA");

        assert_eq!(actual, expected, "RGBA to LA {width}x{height}");
        assert_eq!(vector_blocks, pixels.div_ceil(8) as u64);
        assert_eq!(scalar_tail, 0);
    }
}

#[test]
fn native_rgb_to_rgba_matches_exact_conversion_across_vector_tails() {
    for (width, height) in [
        (0u32, 3u32),
        (3, 0),
        (1, 1),
        (3, 1),
        (4, 1),
        (5, 1),
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
            .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("valid RGB storage"),
        );
        let expected = image.to_rgba8();
        let (actual, vector_blocks, scalar_tail) =
            super::native_rgb_to_rgba_bytes(&image).expect("native RGB to RGBA");

        assert_eq!(
            actual.as_slice(),
            expected.as_raw().as_slice(),
            "{width}x{height}"
        );
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        let pixels_per_vector = 16;
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        let pixels_per_vector = 4;
        assert_eq!(
            vector_blocks,
            (pixels / pixels_per_vector) as u64,
            "vector blocks for {width}x{height}"
        );
        assert_eq!(
            scalar_tail,
            (pixels % pixels_per_vector) as u64,
            "scalar tail for {width}x{height}"
        );
    }
}

#[cfg(target_arch = "x86_64")]
#[test]
fn rgba_to_rgb_dispatch_reports_the_ssse3_kernel() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(32, 1, (0..128).map(|byte| byte as u8).collect())
            .expect("RGBA source has four bytes per pixel"),
    );
    let context = crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Simd);
    let (_, vector_blocks, scalar_tail, path) =
        super::native_rgba_to_rgb_bytes(&context, &image).expect("SSSE3 conversion is available");

    assert_eq!(path, "ssse3-rgba-to-rgb");
    assert_eq!(vector_blocks, 2);
    assert_eq!(scalar_tail, 0);
}
