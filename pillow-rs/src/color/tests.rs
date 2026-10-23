#[cfg(target_arch = "x86_64")]
use super::gather_interleaved_channel_ssse3_into;
use super::grayscale::{
    cmyk_muldiv255_byte, grayscale_rgb_bytes, grayscale_rgb_pixel, grayscale_rgb_pixel_delta,
    grayscale_rgb_pixel_lookup,
};
use super::{
    ColorValue, cmyk_to_grayscale, f_to_i, f_to_l, getcolor, i_to_f, i_to_l, muldiv255, rgb_to_hsv,
    rgb_to_luma_u8,
};
use crate::checked_dims::CheckedDims;
use crate::compute::Backend;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

#[cfg(target_arch = "x86_64")]
#[test]
fn ssse3_direct_channel_gather_covers_vector_boundaries() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }

    for channels in 2..=4 {
        for pixel_count in [0, 1, 15, 16, 17, 31, 32, 33] {
            let source: Vec<u8> = (0..pixel_count * channels)
                .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
                .collect();
            for channel in 0..channels {
                let mut output = vec![0xA5; pixel_count];
                let (vector_blocks, scalar_tail) =
                    gather_interleaved_channel_ssse3_into(&source, &mut output, channels, channel)
                        .expect("SSSE3 must be available on this test CPU");
                let expected: Vec<u8> = source
                    .chunks_exact(channels)
                    .map(|pixel| pixel[channel])
                    .collect();

                assert_eq!(
                    output, expected,
                    "channels={channels} channel={channel} pixels={pixel_count}"
                );
                assert_eq!(vector_blocks, (pixel_count / 16) as u64);
                assert_eq!(scalar_tail, (pixel_count % 16) as u64);
            }
        }
    }
}

#[test]
fn getcolor_accepts_pillow_mapping_and_lowercase_alpha_modes() {
    assert_eq!(
        getcolor(255, 0, 0, 255, "La").unwrap(),
        ColorValue::Gray(76)
    );
    assert_eq!(
        getcolor(255, 0, 0, 255, "I;32BS").unwrap(),
        ColorValue::Gray(76)
    );
}

#[test]
fn getcolor_rejects_unknown_modes_with_key_error() {
    assert!(matches!(
        getcolor(255, 0, 0, 255, "XYZ"),
        Err(PilError::KeyError(mode)) if mode == "XYZ"
    ));
}

#[test]
fn muldiv255_byte_product_never_exceeds_the_second_byte() {
    for channel in 0..=255 {
        for inverse_black in 0..=255 {
            let scaled = muldiv255(channel, inverse_black);
            assert!(scaled <= inverse_black);
            let pillow = (inverse_black as i32 - scaled as i32).clamp(0, 255) as u8;
            let native = (inverse_black - scaled) as u8;
            assert_eq!(
                native, pillow,
                "channel={channel} inverse_black={inverse_black}"
            );
        }
    }
}

#[test]
fn cmyk_byte_muldiv255_matches_shared_exact_formula() {
    for channel in 0..=u8::MAX {
        for ink in 0..=u8::MAX {
            assert_eq!(
                cmyk_muldiv255_byte(channel, ink),
                muldiv255(u32::from(channel), u32::from(ink)) as u8,
                "channel={channel} ink={ink}"
            );
        }
    }
}

#[test]
fn cmyk_grayscale_records_the_final_output_allocation() {
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }

    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![0, 0, 0, 0, 255, 255, 255, 255])
            .expect("valid CMYK samples"),
    );
    let _restore = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    Backend::reset_pipeline_allocation_telemetry();

    let actual = cmyk_to_grayscale(&source).expect("CMYK grayscale");
    let allocation = Backend::take_pipeline_allocation_telemetry();

    assert_eq!(actual.as_raw(), &[255, 0]);
    assert_eq!(allocation.allocation_count, 1);
    assert_eq!(allocation.allocated_bytes, 2);
}

#[test]
fn rgb_to_hsv_preserves_pillow_bytes_for_native_rgb() {
    let rgb_bytes = [
        0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 254, 1, 0, 13, 127, 249,
    ];
    let source = crate::raster::DynamicImage::ImageRgb8(
        crate::raster::RgbImage::from_raw(7, 1, rgb_bytes.to_vec())
            .expect("valid native RGB samples"),
    );

    let converted = rgb_to_hsv(&source);

    assert_eq!(
        converted.as_bytes(),
        [
            0, 0, 0, 0, 0, 255, 0, 255, 255, 85, 255, 255, 170, 255, 255, 0, 255, 254, 149, 241,
            249,
        ]
    );
}

#[test]
fn grayscale_rgb_preserves_exact_luma_tails_and_short_buffers() {
    for channels in [3usize, 4] {
        for pixel_count in 0..=12 {
            let mut source = Vec::with_capacity(pixel_count * channels);
            for index in 0..pixel_count {
                source.extend_from_slice(&[
                    (index * 73 + 19) as u8,
                    (index * 31 + 127) as u8,
                    (index * 151 + 251) as u8,
                ]);
                if channels == 4 {
                    source.push((index * 47 + 1) as u8);
                }
            }

            for bytes in [source.as_slice(), &source[..source.len().saturating_sub(1)]] {
                for pixel in bytes.chunks_exact(channels) {
                    assert_eq!(
                        grayscale_rgb_pixel_delta(pixel, 32768),
                        rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]),
                        "rounded delta grayscale for channels={channels}"
                    );
                    let truncated = ((19595 * u32::from(pixel[0])
                        + 38470 * u32::from(pixel[1])
                        + 7471 * u32::from(pixel[2]))
                        >> 16) as u8;
                    assert_eq!(
                        grayscale_rgb_pixel_delta(pixel, 0),
                        truncated,
                        "truncated delta grayscale for channels={channels}"
                    );
                }
                let dims = CheckedDims::new_allow_empty(pixel_count as u32, 1, 1)
                    .expect("valid grayscale output dimensions");
                let actual = match channels {
                    3 => grayscale_rgb_bytes::<3>(bytes, dims, 32768),
                    4 => grayscale_rgb_bytes::<4>(bytes, dims, 32768),
                    _ => unreachable!(),
                };
                let mut expected: Vec<u8> = bytes
                    .chunks_exact(channels)
                    .take(pixel_count)
                    .map(|pixel| rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]))
                    .collect();
                expected.resize(pixel_count, 0);
                assert_eq!(
                    actual, expected,
                    "channels={channels}, pixels={pixel_count}"
                );
            }
        }
    }
}

#[test]
fn grayscale_rgb_pixel_paths_match_reference_for_all_rgb_values() {
    for red in 0..=u8::MAX {
        for green in 0..=u8::MAX {
            for blue in 0..=u8::MAX {
                let pixel = [red, green, blue];
                for rounding in [0, 32768] {
                    let expected = ((19595 * u32::from(red)
                        + 38470 * u32::from(green)
                        + 7471 * u32::from(blue)
                        + rounding)
                        >> 16) as u8;
                    assert_eq!(grayscale_rgb_pixel_delta(&pixel, rounding), expected);
                    assert_eq!(grayscale_rgb_pixel_lookup(&pixel, rounding), expected);
                    assert_eq!(grayscale_rgb_pixel(&pixel, rounding), expected);
                }
            }
        }
    }
}

#[test]
fn scalar_color_conversions_preserve_clamping_truncation_and_le_bytes() {
    let integers = [i32::MIN, -1, 0, 1, 254, 255, 256, i32::MAX];
    let integer_bytes: Vec<u8> = integers
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let integer_image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(integers.len() as u32, 1, integer_bytes).expect("packed I samples"),
    );

    let DynamicImage::ImageLuma8(integer_luma) = i_to_l(&integer_image) else {
        panic!("I to L must return L storage");
    };
    assert_eq!(integer_luma.as_raw(), &[0, 0, 0, 1, 254, 255, 255, 255]);

    let DynamicImage::ImageRgba8(integer_float) = i_to_f(&integer_image) else {
        panic!("I to F must return the scalar RGBA carrier");
    };
    let actual: Vec<f32> = integer_float
        .as_raw()
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    assert_eq!(actual, integers.map(|value| value as f32));

    let floats = [
        f32::NEG_INFINITY,
        -1.5,
        0.0,
        1.9,
        254.9,
        255.0,
        256.0,
        f32::INFINITY,
        f32::NAN,
    ];
    let float_bytes: Vec<u8> = floats
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let float_image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(floats.len() as u32, 1, float_bytes).expect("packed F samples"),
    );

    let DynamicImage::ImageLuma8(float_luma) = f_to_l(&float_image) else {
        panic!("F to L must return L storage");
    };
    assert_eq!(float_luma.as_raw(), &[0, 0, 0, 1, 254, 255, 255, 255, 0]);

    let DynamicImage::ImageRgba8(float_integer) = f_to_i(&float_image) else {
        panic!("F to I must return the scalar RGBA carrier");
    };
    let actual: Vec<i32> = float_integer
        .as_raw()
        .chunks_exact(4)
        .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    assert_eq!(actual, [i32::MIN, -1, 0, 1, 254, 255, 256, i32::MAX, 0,]);
}
