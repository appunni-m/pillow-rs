//! Focused contract tests for SIMD resize modes and coefficient bounds.

use super::simd_resize_f;
use crate::ops::pil_resize::{FilterCoeffs, precompute_coeffs};
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbaImage};

#[test]
fn f_boxed_resize_sample_view_matches_cpu_reference() {
    use crate::ops::pil_resize::pil_resize_boxed;
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};

    let cases = [
        (
            "bicubic scale two",
            512,
            384,
            256,
            192,
            0.0,
            0.0,
            512.0,
            384.0,
            ResampleFilter::Bicubic,
        ),
        (
            "odd bicubic tail",
            511,
            383,
            255,
            191,
            0.0,
            0.0,
            511.0,
            383.0,
            ResampleFilter::Bicubic,
        ),
        (
            "fractional bicubic box",
            512,
            384,
            256,
            192,
            0.25,
            0.5,
            511.5,
            383.5,
            ResampleFilter::Bicubic,
        ),
        (
            "lanczos fallback",
            512,
            384,
            256,
            192,
            0.0,
            0.0,
            512.0,
            384.0,
            ResampleFilter::Lanczos,
        ),
    ];

    for (
        case,
        source_width,
        source_height,
        output_width,
        output_height,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
    ) in cases
    {
        let samples = (0..source_width as usize * source_height as usize)
            .map(|index| (((index * 41 % 1021) as i32 - 510) as f32) / 37.0)
            .collect::<Vec<_>>();
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, bytes).expect("packed F samples"),
        );
        let expected = pil_resize_boxed(
            &image,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            filter,
            Some("F"),
        );
        let actual = super::simd_resize_f_boxed(
            &image,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            filter,
        )
        .expect("SIMD boxed F resize");

        let actual_bytes = actual.as_bytes();
        let expected_bytes = expected.as_bytes();
        let first_mismatch = actual_bytes
            .iter()
            .zip(expected_bytes)
            .position(|(actual, expected)| actual != expected);
        assert!(
            actual_bytes.len() == expected_bytes.len() && first_mismatch.is_none(),
            "{case}: first mismatching byte {first_mismatch:?}; actual={:?}, expected={:?}",
            first_mismatch.and_then(|index| actual_bytes.get(index..index + 4)),
            first_mismatch.and_then(|index| expected_bytes.get(index..index + 4)),
        );
    }
}

#[test]
fn cover_luma_i32_coefficient_guard_fails_closed() {
    let horizontal = precompute_coeffs(1365, 1024, ResampleFilter::Bicubic);
    let vertical = precompute_coeffs(1024, 768, ResampleFilter::Bicubic);
    assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
    assert!(super::resize_u8_coefficients_fit_i32(&vertical));

    let at_bound = FilterCoeffs {
        xmin: vec![0],
        count: vec![1],
        offsets: vec![0],
        weights: vec![8_413_280],
    };
    assert!(super::resize_u8_coefficients_fit_i32(&at_bound));
    let above_bound = FilterCoeffs {
        xmin: vec![0],
        count: vec![1],
        offsets: vec![0],
        weights: vec![8_413_281],
    };
    assert!(!super::resize_u8_coefficients_fit_i32(&above_bound));
    let outside_i32 = FilterCoeffs {
        xmin: vec![0],
        count: vec![1],
        offsets: vec![0],
        weights: vec![i64::MAX],
    };
    assert!(!super::resize_u8_coefficients_fit_i32(&outside_i32));
}

#[test]
fn alpha_source_row_precompute_matches_per_tap_simd_premultiplication() {
    for channels in [2usize, 4] {
        let source_width = 37usize;
        let output_width = 53usize;
        let source = (0..source_width * channels)
            .map(|index| {
                let pixel = index / channels;
                let channel = index % channels;
                if channel == channels - 1 {
                    match pixel % 4 {
                        0 => 0,
                        1 => 255,
                        _ => (pixel * 47 + 13) as u8,
                    }
                } else {
                    (pixel * 83 + channel * 61 + 19) as u8
                }
            })
            .collect::<Vec<_>>();
        let coeffs = precompute_coeffs(
            output_width as u32,
            source_width as u32,
            ResampleFilter::Bicubic,
        );
        let plan = super::build_resize_horizontal_plan(&coeffs, output_width, channels, true)
            .expect("horizontal plan");
        let mut expected = vec![0; output_width * channels];
        super::resize_horizontal_vector_row(
            &source,
            channels,
            &coeffs,
            &plan,
            output_width,
            &mut expected,
            true,
        )
        .expect("reference alpha row");

        let mut scratch = vec![0; source.len()];
        let mut actual = vec![0; output_width * channels];
        super::resize_horizontal_convolution_row(
            &source,
            channels,
            &coeffs,
            &plan,
            output_width,
            &mut actual,
            true,
            false,
            &mut scratch,
        )
        .expect("precomputed alpha row");
        assert_eq!(actual, expected, "channels={channels}");
    }
}

#[test]
fn cover_luma_i32_two_pass_matches_widened_for_extreme_samples() {
    let width = 37u32;
    let height = 17u32;
    let output_width = 53u32;
    let output_height = 23u32;
    for pattern in 0..4u32 {
        let samples = (0..width * height)
            .map(|index| match pattern {
                0 => 0,
                1 => 255,
                2 => {
                    let x = index % width;
                    let y = index / width;
                    if (x + y) % 2 == 0 { 0 } else { 255 }
                }
                _ => index.wrapping_mul(97).wrapping_add(index / width * 53) as u8,
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, samples).expect("test L image shape"),
        );
        let output_len = (output_width * output_height) as usize;
        let mut widened = vec![0u8; output_len];
        let mut narrow = vec![0u8; output_len];
        super::simd_resize_convolution_into(
            &image,
            output_width,
            output_height,
            ResampleFilter::Bicubic,
            1,
            false,
            false,
            0,
            false,
            &mut widened,
        )
        .expect("widened Cover resize must succeed");
        super::simd_resize_convolution_into(
            &image,
            output_width,
            output_height,
            ResampleFilter::Bicubic,
            1,
            false,
            true,
            0,
            false,
            &mut narrow,
        )
        .expect("bounded i32 Cover resize must succeed");
        assert_eq!(narrow, widened, "sample pattern {pattern}");
    }
}

#[test]
fn cover_la_i32_two_pass_matches_widened_for_alpha_patterns() {
    let width = 37u32;
    let height = 17u32;
    let output_width = 53u32;
    let output_height = 23u32;
    let horizontal = precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
    let vertical = precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
    assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
    assert!(super::resize_u8_coefficients_fit_i32(&vertical));

    for pattern in 0..4u32 {
        let samples = (0..width * height)
            .flat_map(|index| {
                let luma = index.wrapping_mul(97).wrapping_add(index / width * 53) as u8;
                let alpha = match pattern {
                    0 => 0,
                    1 => 255,
                    2 if (index + index / width) % 2 == 0 => 0,
                    2 => 255,
                    _ => index.wrapping_mul(71).wrapping_add(index / width * 29) as u8,
                };
                [luma, alpha]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, samples).expect("test LA image shape"),
        );
        let output_len = (output_width * output_height * 2) as usize;
        let mut widened = vec![0u8; output_len];
        let mut narrow = vec![0u8; output_len];
        super::simd_resize_convolution_into(
            &image,
            output_width,
            output_height,
            ResampleFilter::Bicubic,
            2,
            true,
            false,
            0,
            false,
            &mut widened,
        )
        .expect("widened LA Cover resize must succeed");
        super::simd_resize_convolution_into(
            &image,
            output_width,
            output_height,
            ResampleFilter::Bicubic,
            2,
            true,
            true,
            0,
            false,
            &mut narrow,
        )
        .expect("bounded i32 LA Cover resize must succeed");
        assert_eq!(narrow, widened, "alpha pattern {pattern}");
    }
}

#[test]
fn cover_hsv_i32_two_pass_matches_widened_at_edges_and_tails() {
    for (width, height, output_width, output_height) in
        [(5, 3, 7, 5), (37, 17, 53, 23), (67, 29, 31, 41)]
    {
        let horizontal = precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
        let vertical = precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
        assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
        assert!(super::resize_u8_coefficients_fit_i32(&vertical));
        let plan =
            super::build_resize_horizontal_plan(&horizontal, output_width as usize, 3, false)
                .expect("HSV horizontal plan");

        for pattern in 0..4u32 {
            let source = (0..width * height * 3)
                .map(|index| match pattern {
                    0 => 0,
                    1 => 255,
                    2 if (index + index / (width * 3)) % 2 == 0 => 0,
                    2 => 255,
                    _ => index
                        .wrapping_mul(97)
                        .wrapping_add(index / (width * 3) * 53) as u8,
                })
                .collect::<Vec<_>>();
            let mut widened_intermediate = vec![0u8; height as usize * output_width as usize * 3];
            let mut narrow_intermediate = widened_intermediate.clone();
            for y in 0..height as usize {
                let source_row = &source[y * width as usize * 3..(y + 1) * width as usize * 3];
                let row_start = y * output_width as usize * 3;
                let row_end = row_start + output_width as usize * 3;
                super::resize_horizontal_vector_row(
                    source_row,
                    3,
                    &horizontal,
                    &plan,
                    output_width as usize,
                    &mut widened_intermediate[row_start..row_end],
                    false,
                )
                .expect("wide HSV horizontal pass");
                super::resize_horizontal_hsv_i32_vector_row(
                    source_row,
                    &horizontal,
                    &plan,
                    output_width as usize,
                    &mut narrow_intermediate[row_start..row_end],
                )
                .expect("bounded HSV horizontal pass");
            }
            assert_eq!(
                narrow_intermediate, widened_intermediate,
                "horizontal {pattern}"
            );

            let mut widened = vec![0u8; output_width as usize * output_height as usize * 3];
            let mut narrow = widened.clone();
            for y in 0..output_height as usize {
                let row_start = y * output_width as usize * 3;
                let row_end = row_start + output_width as usize * 3;
                super::resize_vertical_vector_row(
                    &widened_intermediate,
                    output_width as usize,
                    height as usize,
                    3,
                    &vertical,
                    y,
                    &mut widened[row_start..row_end],
                    false,
                    false,
                )
                .expect("wide HSV vertical pass");
                super::resize_vertical_hsv_i32_vector_row(
                    &narrow_intermediate,
                    output_width as usize,
                    height as usize,
                    &vertical,
                    y,
                    &mut narrow[row_start..row_end],
                )
                .expect("bounded HSV vertical pass");
            }
            assert_eq!(narrow, widened, "vertical {pattern}");
        }
    }
}

#[test]
fn cover_cmyk_i32_two_pass_matches_widened_at_edges_and_tails() {
    for (width, height, output_width, output_height) in
        [(5u32, 3u32, 7u32, 5u32), (37, 17, 53, 23), (67, 29, 31, 41)]
    {
        let horizontal = precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
        let vertical = precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
        assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
        assert!(super::resize_u8_coefficients_fit_i32(&vertical));

        for pattern in 0..4u32 {
            let source = (0..width as usize * height as usize * 4)
                .map(|index| match pattern {
                    0 => 0,
                    1 => 255,
                    2 => {
                        let pixel = index / 4;
                        if ((pixel % width as usize) ^ (pixel / width as usize)) & 1 == 0 {
                            0
                        } else {
                            255
                        }
                    }
                    _ => index
                        .wrapping_mul(97)
                        .wrapping_add((index / (width as usize * 4)).wrapping_mul(53))
                        .to_le_bytes()[0],
                })
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, source)
                    .expect("test CMYK byte-storage image shape"),
            );
            let output_len = (output_width * output_height * 4) as usize;
            let mut widened = vec![0u8; output_len];
            let mut narrow = vec![0u8; output_len];
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                4,
                false,
                false,
                0,
                false,
                &mut widened,
            )
            .expect("widened CMYK Cover resize must succeed");
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                4,
                false,
                true,
                0,
                false,
                &mut narrow,
            )
            .expect("bounded i32 CMYK Cover resize must succeed");
            assert_eq!(
                narrow, widened,
                "sample pattern {pattern}, {width}x{height} -> {output_width}x{output_height}"
            );
        }
    }
}

#[test]
fn rgba_i32_resize_matches_widened_for_filters_and_alpha_patterns() {
    for (width, height, output_width, output_height) in
        [(5u32, 3u32, 7u32, 5u32), (37, 17, 53, 23), (73, 41, 13, 9)]
    {
        for filter in [
            ResampleFilter::Bilinear,
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Hamming,
            ResampleFilter::Box,
        ] {
            let horizontal = precompute_coeffs(output_width, width, filter);
            let vertical = precompute_coeffs(output_height, height, filter);
            assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
            assert!(super::resize_u8_coefficients_fit_i32(&vertical));
            let operation = PipelineOp::Resize {
                w: output_width,
                h: output_height,
                filter,
            };

            for pattern in 0..4u32 {
                let source = (0..width * height)
                    .flat_map(|index| {
                        let alpha = match pattern {
                            0 => 0,
                            1 => 255,
                            2 if (index + index / width) % 2 == 0 => 0,
                            2 => 255,
                            _ => index.wrapping_mul(47).wrapping_add(index / width * 61) as u8,
                        };
                        [
                            index.wrapping_mul(73).wrapping_add(31) as u8,
                            index.wrapping_mul(29).wrapping_add(19) as u8,
                            index.wrapping_mul(97).wrapping_add(11) as u8,
                            alpha,
                        ]
                    })
                    .collect::<Vec<_>>();
                let image = DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(width, height, source).expect("test RGBA image shape"),
                );
                let widened = super::simd_resize_convolution(
                    &image,
                    output_width,
                    output_height,
                    filter,
                    4,
                    true,
                    false,
                )
                .expect("wide SIMD RGBA resize");
                let narrow = super::simd_resize(&image, &operation, Some("RGBA"))
                    .expect("bounded i32 SIMD RGBA resize");
                assert!(matches!(narrow, DynamicImage::ImageRgba8(_)));
                assert_eq!(
                    narrow.as_bytes(),
                    widened.as_bytes(),
                    "{width}x{height}->{output_width}x{output_height} {filter:?} pattern {pattern}"
                );
            }
        }
    }
}

#[cfg(target_endian = "little")]
#[test]
fn f_resize_preserves_simd_negative_zero() {
    // Pillow's 32bpc resampler keeps the sign of a zero produced by
    // cancellation. The SIMD adapter used to erase that sign while
    // converting each f64 lane back to f32.
    let source_words = [
        0x0000_0001,
        0x8000_0001,
        0x0000_0002,
        0x8000_0002,
        0x0000_0008,
        0x0000_0001,
    ];
    let source_bytes: Vec<u8> = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, source_bytes).expect("source shape must be valid"),
    );

    let output = simd_resize_f(&source, 7, 5, &ResampleFilter::Bilinear)
        .expect("finite F-mode SIMD resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    let word_at = |index: usize| {
        let offset = index * 4;
        u32::from_le_bytes(output.as_raw()[offset..offset + 4].try_into().unwrap())
    };
    assert_eq!(word_at(10), 0x8000_0000);
    assert_eq!(word_at(14), 0x8000_0000);
}
