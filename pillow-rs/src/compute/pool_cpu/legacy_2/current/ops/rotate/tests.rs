#[cfg(not(feature = "parallel"))]
use super::affine::fixed_coordinate_in_bounds_span;
#[cfg(not(feature = "parallel"))]
use crate::ops::rotate::nearest_contiguous_rgb_run_len;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView, GrayImage, RgbImage, RgbaImage};

#[cfg(not(feature = "parallel"))]
#[test]
fn rotate_contiguous_rgb_run_length_matches_fixed_point_scan() {
    let x_steps = [1, 32_768, 65_526, 65_536, 65_537, 98_304, 131_072, 655_360];
    let y_steps = [0, 1, 1_144, 65_535, 65_536, 100_000];
    let x_fractions = [0, 1, 5, 9, 10, 32_767, 65_525, 65_535];
    let y_fractions = [0, 1, 1_143, 32_767, 65_535];
    let remaining_lengths = [1, 2, 17, 257];

    for x_step in x_steps {
        for y_step in y_steps {
            for x_fraction in x_fractions {
                for y_fraction in y_fractions {
                    for remaining in remaining_lengths {
                        let source_x = (3_i64 << 16) + x_fraction;
                        let source_y = (2_i64 << 16) + y_fraction;
                        let base_x = source_x >> 16;
                        let base_y = source_y >> 16;
                        let expected = (1..=remaining)
                            .take_while(|&length| {
                                let offset = (length - 1) as i64;
                                ((source_x + offset * x_step) >> 16) == base_x + offset
                                    && ((source_y + offset * y_step) >> 16) == base_y
                            })
                            .last()
                            .unwrap_or(0);

                        assert_eq!(
                            nearest_contiguous_rgb_run_len(
                                source_x, source_y, x_step, y_step, remaining
                            ),
                            Some(expected),
                            "x_step={x_step}, y_step={y_step}, x_fraction={x_fraction}, \
                                 y_fraction={y_fraction}, remaining={remaining}"
                        );
                    }
                }
            }
        }
    }
}
#[cfg(not(feature = "parallel"))]
#[test]
fn rotate_positive_step_span_matches_signed_fixed_point_bounds() {
    let origins = [
        i64::MIN,
        -1_000_001,
        -65_537,
        -65_536,
        -65_535,
        -1,
        0,
        1,
        32_768,
        65_535,
        65_536,
        65_537,
        1_000_001,
        i64::MAX,
    ];
    let steps = [1, 32_767, 32_768, 65_535, 65_536, 65_537, 131_072, i64::MAX];

    for source_length in 0..=11 {
        for destination_length in 0..=17 {
            for origin in origins {
                for step in steps {
                    let span = fixed_coordinate_in_bounds_span(
                        origin,
                        step,
                        source_length,
                        destination_length,
                    )
                    .expect("positive step has a representable clipped span");
                    let fixed_source_limit = i128::from(source_length) * (1 << 16);
                    for x in 0..destination_length as usize {
                        let coordinate = i128::from(origin)
                            + i128::try_from(x).expect("small test coordinate") * i128::from(step);
                        let expected =
                            coordinate >> 16 >= 0 && coordinate >> 16 < i128::from(source_length);
                        assert_eq!(
                            (span.0..span.1).contains(&x),
                            expected,
                            "source={source_length}, destination={destination_length}, origin={origin}, step={step}, x={x}, limit={fixed_source_limit}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn rotate_near_right_angle_uses_affine_sampling() {
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60])
            .expect("RGB source shape must be valid"),
    );
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80])
            .expect("RGBA source shape must be valid"),
    );

    // Pillow only selects its transpose fast path for an exact 90°
    // multiple.  Rounding 89.9° or 90.1° into that path moves the source
    // pixel selected at the edge and diverges from Geometry.c's affine
    // nearest sampler.
    let rgb_output = super::execute_rotate(
        &rgb,
        89.9,
        false,
        None,
        None,
        None,
        ResampleFilter::Nearest,
        true,
        Some("RGB"),
    )
    .expect("near-right RGB rotation");
    assert_eq!(rgb_output.dimensions(), (2, 1));
    assert_eq!(rgb_output.as_bytes(), &[10, 20, 30, 0, 0, 0]);

    let rgba_output = super::execute_rotate(
        &rgba,
        90.1,
        false,
        None,
        None,
        None,
        ResampleFilter::Nearest,
        true,
        Some("RGBA"),
    )
    .expect("near-right RGBA rotation");
    assert_eq!(rgba_output.dimensions(), (2, 1));
    assert_eq!(rgba_output.as_bytes(), &[50, 60, 70, 80, 0, 0, 0, 0]);
}
#[cfg(target_endian = "little")]
#[test]
fn f_rotate_bicubic_uses_scalar_filter() {
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [0.0f32, 1.0, 2.0, 3.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .expect("F source shape must be valid"),
    );
    let output = super::execute_rotate(
        &source,
        45.0,
        true,
        None,
        None,
        None,
        ResampleFilter::Bicubic,
        false,
        Some("F"),
    )
    .expect("F bicubic rotation must succeed");
    assert_eq!(output.dimensions(), (4, 4));
    let expected_words = [
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x3e75_57b3,
        0x4008_5542,
        0x0000_0000,
        0x0000_0000,
        0x3f5e_aaf6,
        0x4030_aa85,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
    ];
    let expected: Vec<u8> = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(output.as_bytes(), expected);
}
#[test]
fn byte_rotate_bicubic_uses_geometry_c_horner() {
    let source = DynamicImage::ImageLuma8(
        GrayImage::from_raw(2, 2, vec![0, 1, 2, 3]).expect("L source shape must be valid"),
    );
    let output = super::execute_rotate(
        &source,
        45.0,
        true,
        None,
        None,
        None,
        ResampleFilter::Bicubic,
        false,
        Some("L"),
    )
    .expect("L bicubic rotation must succeed");
    assert_eq!(output.dimensions(), (4, 4));
    assert_eq!(
        output.as_bytes(),
        &[0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0]
    );
}
