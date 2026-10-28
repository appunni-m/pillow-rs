#[cfg(target_arch = "x86_64")]
use super::{F64MulAdd, X86FmaToken, f_resize_samples_allow_finite_fma};
use super::{resize_f, resize_i_sum_eight};
#[cfg(target_arch = "x86_64")]
use super::{resize_f_with_fma, resize_i_with_fma};

use super::{resize_i_contiguous_eight_tap_weights, resize_i_round_up_to_i32};
use crate::ops::pil_resize::PortableFma;

use crate::ops::pil_resize::round_up;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, RgbaImage};

#[cfg(target_arch = "x86_64")]
#[test]
fn runtime_x86_fma_matches_portable_fused_operation() {
    let Some(x86_fma) = X86FmaToken::detect() else {
        return;
    };
    assert!(x86_fma.supports_finite_mul_add());
    let portable = PortableFma;
    for (weight, sample, accumulator) in [
        (0.1, 0.2, 0.3),
        (1.0 + f64::EPSILON, 1.0 - f64::EPSILON, -1.0),
        (f64::MAX, 2.0, -f64::MAX),
        (f64::MIN_POSITIVE, f64::EPSILON, -f64::MIN_POSITIVE),
        (-0.0, 1.0, -0.0),
        (1.0, -0.0, -0.0),
        (f64::INFINITY, 2.0, f64::NEG_INFINITY),
        (f64::NAN, 1.0, 0.0),
    ] {
        assert_eq!(
            x86_fma.mul_add(weight, sample, accumulator).to_bits(),
            portable.mul_add(weight, sample, accumulator).to_bits(),
            "FMA result differs for {weight:?} * {sample:?} + {accumulator:?}",
        );
        if weight.is_finite() && sample.is_finite() && accumulator.is_finite() {
            assert_eq!(
                x86_fma
                    .mul_add_finite(weight, sample, accumulator)
                    .to_bits(),
                portable.mul_add(weight, sample, accumulator).to_bits(),
                "finite FMA result differs for {weight:?} * {sample:?} + {accumulator:?}",
            );
        }
    }
}
#[cfg(target_endian = "little")]
#[cfg(target_arch = "x86_64")]
#[test]
fn finite_x86_f_resize_path_matches_checked_reference() {
    let Some(x86_fma) = X86FmaToken::detect() else {
        return;
    };
    let (source_width, source_height) = (32u32, 24u32);
    let mut sample_sets = vec![
        (0..source_width as usize * source_height as usize)
            .map(|index| (((index * 73 % 4093) as i32 - 2046) as f32) / 17.0)
            .collect::<Vec<_>>(),
        (0..source_width as usize * source_height as usize)
            .map(|index| if index % 2 == 0 { f32::MAX } else { -f32::MAX })
            .collect::<Vec<_>>(),
    ];
    let mut nonfinite = sample_sets[0].clone();
    nonfinite[7] = f32::from_bits(0x7fc1_2345);
    nonfinite[19] = f32::INFINITY;
    nonfinite[41] = f32::NEG_INFINITY;
    sample_sets.push(nonfinite);

    for (set_index, samples) in sample_sets.iter().enumerate() {
        assert_eq!(
            f_resize_samples_allow_finite_fma(&x86_fma, samples),
            set_index < 2,
            "finite-sample admission differs for sample set {set_index}",
        );
        let source_bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, source_bytes)
                .expect("F sample source shape must be valid"),
        );
        let actual = resize_f_with_fma(&source, 16, 12, &ResampleFilter::Bicubic, &x86_fma)
            .expect("x86 F resize must succeed");
        let portable = resize_f_with_fma(&source, 16, 12, &ResampleFilter::Bicubic, &PortableFma)
            .expect("portable F resize must succeed");
        assert_eq!(
            actual.as_bytes(),
            portable.as_bytes(),
            "F resize differs from checked reference for sample set {set_index}",
        );
    }
}
#[cfg(target_endian = "little")]
#[test]
fn f_bicubic_preserves_pillow_f64_rounding() {
    let source_words = [0xd9f6def9, 0x3210f3c9];
    let source_bytes = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
    );

    let output = resize_f(&source, 2, 6, &ResampleFilter::Bicubic)
        .expect("finite F-mode bicubic resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    let expected_words = [
        0xda086dc0, 0xda086dc0, 0xd9f6def9, 0xd9f6def9, 0xd9accf48, 0xd9accf48, 0xd9141f62,
        0xd9141f62, 0x3210f3c9, 0x3210f3c9, 0x584fe430, 0x584fe430,
    ];
    let expected_bytes: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_raw(), &expected_bytes);
}
#[cfg(target_endian = "little")]
#[test]
fn f_resize_tall_uses_pillow_vertical_first_order() {
    // Pillow's Image.resize switches a source taller than 100x its width
    // to vertical-first passes.  This 2x256 source is intentionally
    // heterogeneous so the materialized FLOAT32 intermediate is part of
    // the observable result rather than an algebraically interchangeable
    // detail.
    let source_bytes: Vec<u8> = (0..512)
        .map(|index| {
            let value = 0.25f32 + ((index * 29 % 120) as f32) * 0.01f32;
            value.to_bits()
        })
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 256, source_bytes).expect("tall F source shape must be valid"),
    );

    for (filter, expected_word) in [
        (ResampleFilter::Bilinear, 0x3f57_eb1eu32),
        (ResampleFilter::Bicubic, 0x3f57_ee45),
        (ResampleFilter::Lanczos, 0x3f57_e8a6),
        (ResampleFilter::Box, 0x3f57_b852),
        (ResampleFilter::Hamming, 0x3f58_0fba),
    ] {
        let output = resize_f(&source, 1, 1, &filter)
            .expect("tall F resize must preserve Pillow's pass order");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        assert_eq!(output.as_raw(), &expected_word.to_le_bytes());
    }
}
#[cfg(target_endian = "little")]
#[test]
fn f_nearest_uses_pillow_cumulative_row_mapping() {
    let source_words = [1.0f32.to_bits(), 2.0f32.to_bits()];
    let source_bytes = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
    );

    let output = resize_f(&source, 1, 7, &ResampleFilter::Nearest)
        .expect("finite F-mode nearest resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    let expected_words = [
        1.0f32.to_bits(),
        1.0f32.to_bits(),
        1.0f32.to_bits(),
        1.0f32.to_bits(),
        2.0f32.to_bits(),
        2.0f32.to_bits(),
        2.0f32.to_bits(),
    ];
    let expected_bytes: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_raw(), &expected_bytes);
}
#[cfg(target_endian = "little")]
#[test]
fn f_convolution_preserves_pillow_signed_zero() {
    let source_words = [0x8000_0000, 0x0000_0001];
    let source_bytes = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
    );

    let output = resize_f(&source, 1, 3, &ResampleFilter::Bicubic)
        .expect("mixed signed-zero F-mode bicubic resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    let expected_words = [0x8000_0000, 0x0000_0000, 0x0000_0001];
    let expected_bytes: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_raw(), &expected_bytes);
}
#[cfg(target_endian = "little")]
#[test]
fn f_hamming_matches_pillow_fma_window_on_cancellation() {
    let source_words = [
        1.0f32.to_bits(),
        (-1.0f32).to_bits(),
        3.0f32.to_bits(),
        (-3.0f32).to_bits(),
        1.0f32.to_bits(),
        (-1.0f32).to_bits(),
        3.0f32.to_bits(),
        (-3.0f32).to_bits(),
    ];
    let source_bytes: Vec<u8> = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(8, 1, source_bytes).expect("source shape must be valid"),
    );

    let output = resize_f(&source, 4, 1, &ResampleFilter::Hamming)
        .expect("finite F-mode Hamming resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    // Pillow's Resample.c Hamming window contracts its `0.46f * cos +
    // 0.54f` expression before the sinc product.  The exact cancellation
    // residuals here catch either a separated window expression or a
    // changed accumulation order.
    let expected_words = [0x3df4_077e, 0xa3c0_0000, 0xa300_0000, 0xbd22_afaa];
    let expected_bytes: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_raw(), &expected_bytes);
}
#[cfg(target_endian = "little")]
#[test]
fn f_hamming_preserves_near_zero_kernel_residual() {
    let source_values = [
        -2.0f32, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0,
        -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0,
    ];
    let source_bytes: Vec<u8> = source_values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(1, 25, source_bytes).expect("source shape must be valid"),
    );

    let output = resize_f(&source, 1, 11, &ResampleFilter::Hamming)
        .expect("finite F-mode Hamming resize must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode resize must retain packed float storage");
    };
    // For the centered output row, Resample.c evaluates a tiny non-zero
    // kernel argument rather than taking its exact-zero branch.  The
    // float Hamming constants therefore contribute a residual that is
    // visible in the serialized f32 word.
    let expected_words = [
        0xbfab_ebf1,
        0xbfb0_c781,
        0xbf86_fa08,
        0xbf6f_206e,
        0xbfa1_ad5c,
        0xbfb3_56af,
        0xbf8c_8ddf,
        0xbf6b_37f1,
        0xbf9b_c741,
        0xbfb4_e066,
        0xbf93_63f8,
    ];
    let expected_bytes: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_raw(), &expected_bytes);
}
#[test]
fn i_resize_eight_tap_accumulation_matches_scalar_fma_order() {
    let weights: [f64; 8] = [
        -0.03125, 0.09375, -0.15625, 0.59375, 0.59375, -0.15625, 0.09375, -0.03125,
    ];
    for source in [
        [i32::MIN, i32::MAX, -1, 0, 1, i32::MIN + 1, i32::MAX - 1, 17],
        [-1_000_003, 923_771, 0, 17, -31, 63, -127, 255],
    ] {
        let expected = weights
            .iter()
            .zip(source)
            .fold(0.0_f64, |acc, (&weight, sample)| {
                weight.mul_add(f64::from(sample), acc)
            });
        let actual = resize_i_sum_eight(&weights, 0, |index| source[index], &PortableFma);
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}
#[cfg(target_arch = "x86_64")]
#[test]
fn runtime_x86_fma_i_resize_matches_portable_path() {
    let Some(x86_fma) = X86FmaToken::detect() else {
        return;
    };
    let (source_width, source_height) = (32u32, 24u32);
    let samples: Vec<i32> = (0..source_width as usize * source_height as usize)
        .map(|index| match index % 8 {
            0 => i32::MIN,
            1 => i32::MAX,
            2 => -1,
            3 => 0,
            4 => i32::MIN + 1,
            5 => i32::MAX - 1,
            _ => (index as i32).wrapping_mul(73_856_093),
        })
        .collect();
    let source_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(source_width, source_height, source_bytes)
            .expect("I-mode source shape must be valid"),
    );

    for filter in [ResampleFilter::Bicubic, ResampleFilter::Lanczos] {
        let actual = resize_i_with_fma(&source, 16, 12, &filter, &x86_fma)
            .expect("x86 I-mode resize must succeed");
        let portable = resize_i_with_fma(&source, 16, 12, &filter, &PortableFma)
            .expect("portable I-mode resize must succeed");
        assert_eq!(
            actual.as_bytes(),
            portable.as_bytes(),
            "I-mode {filter:?} resize differs from the portable fused reference",
        );
    }
}

#[test]
fn i_resize_rounding_fast_path_matches_pillow_conversion() {
    let mut values = vec![
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        f64::MAX,
        f64::MIN,
        -0.0,
        0.0,
        f64::from(i32::MIN) - 0.5,
        f64::from(i32::MIN),
        f64::from(i32::MAX),
        f64::from(i32::MAX) + 0.5,
        f64::from_bits(0.5_f64.to_bits() - 1),
        0.5,
        f64::from_bits(0.5_f64.to_bits() + 1),
        f64::from_bits((-0.5_f64).to_bits() - 1),
        -0.5,
        f64::from_bits((-0.5_f64).to_bits() + 1),
    ];

    for integer in -1024_i32..=1024 {
        let positive_half = f64::from(integer) + 0.5;
        let negative_half = -positive_half;
        for boundary in [positive_half, negative_half] {
            values.push(f64::from_bits(boundary.to_bits() - 1));
            values.push(boundary);
            values.push(f64::from_bits(boundary.to_bits() + 1));
        }
    }

    let mut state = 0x8a5c_13d7_6b24_e901_u64;
    for _ in 0..10_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let bytes = state.to_le_bytes();
        let integer = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let fraction_bits = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let fraction = f64::from(fraction_bits) / f64::from(u32::MAX);
        values.push(f64::from(integer) + fraction - 0.5);
    }

    for value in values {
        assert_eq!(
            resize_i_round_up_to_i32(value),
            round_up(value) as i32,
            "rounding mismatch for {value:?}"
        );
    }
}

#[test]
fn i_resize_packs_only_a_contiguous_eight_tap_interior() {
    let coefficients = vec![
        vec![0.0; 4],
        (0..8).map(f64::from).collect(),
        (8..16).map(f64::from).collect(),
        vec![0.0; 3],
    ];
    let Some((first, packed)) = resize_i_contiguous_eight_tap_weights(&coefficients) else {
        panic!("the adjacent eight-tap spans must be packed");
    };
    assert_eq!(first, 1);
    assert_eq!(packed[0], [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    assert_eq!(packed[1], [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0]);

    let disjoint = vec![vec![1.0; 8], vec![1.0; 3], vec![1.0; 8]];
    assert!(resize_i_contiguous_eight_tap_weights(&disjoint).is_none());
}
