use super::{kernels, op_paste};
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use std::sync::Arc;

fn native_byte_image(mode: &str, width: u32, height: u32, bytes: Vec<u8>) -> DynamicImage {
    match mode {
        "L" | "1" => DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, bytes).expect("L image shape"),
        ),
        "LA" | "PA" => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("LA image shape"),
        ),
        "RGB" | "HSV" => DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, bytes).expect("RGB image shape"),
        ),
        "RGBA" | "RGBa" | "CMYK" => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, bytes).expect("four-byte image shape"),
        ),
        _ => panic!("unsupported test mode {mode}"),
    }
}

#[test]
fn native_rgb_paste_copies_clipped_rows_without_changing_other_pixels() {
    let destination = DynamicImage::ImageRgb8(
        RgbImage::from_raw(
            3,
            3,
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27,
            ],
        )
        .expect("destination RGB buffer"),
    );
    let source = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(
                4,
                2,
                vec![
                    101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
                    117, 118, 119, 120, 121, 122, 123, 124,
                ],
            )
            .expect("source RGB buffer"),
        ),
        Some("RGB".to_owned()),
    ));

    let result = op_paste(&destination, &source, -1, 1, &None, false, Some("RGB"))
        .expect("native RGB paste");
    let DynamicImage::ImageRgb8(result) = result else {
        panic!("native RGB paste must retain RGB storage");
    };
    assert_eq!(
        result.as_raw(),
        &[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 104, 105, 106, 107, 108, 109, 110, 111, 112, 116, 117, 118,
            119, 120, 121, 122, 123, 124,
        ]
    );

    let extreme_offset = op_paste(
        &destination,
        &source,
        i64::MIN,
        0,
        &None,
        false,
        Some("RGB"),
    )
    .expect("extreme clipped paste");
    assert_eq!(extreme_offset, destination);
}

#[test]
fn native_unmasked_paste_copies_each_supported_layout_verbatim() {
    let cases = [
        ("1", 1),
        ("L", 1),
        ("P", 1),
        ("LA", 2),
        ("PA", 2),
        ("RGB", 3),
        ("HSV", 3),
        ("YCbCr", 3),
        ("LAB", 3),
        ("RGBA", 4),
        ("RGBa", 4),
        ("RGBX", 4),
        ("CMYK", 4),
        ("I", 4),
        ("F", 4),
    ];

    for (mode, channels) in cases {
        let destination_bytes = (0..channels * 2)
            .map(|index| 10 + index as u8)
            .collect::<Vec<_>>();
        let source_bytes = (0..channels)
            .map(|index| 100 + index as u8)
            .collect::<Vec<_>>();
        let destination = match channels {
            1 => DynamicImage::ImageLuma8(
                GrayImage::from_raw(2, 1, destination_bytes.clone()).expect("destination L buffer"),
            ),
            2 => DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(2, 1, destination_bytes.clone())
                    .expect("destination LA buffer"),
            ),
            3 => DynamicImage::ImageRgb8(
                RgbImage::from_raw(2, 1, destination_bytes.clone())
                    .expect("destination RGB buffer"),
            ),
            4 => DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, destination_bytes.clone())
                    .expect("destination four-byte buffer"),
            ),
            _ => unreachable!("test cases use one through four channels"),
        };
        let source_image = match channels {
            1 => DynamicImage::ImageLuma8(
                GrayImage::from_raw(1, 1, source_bytes.clone()).expect("source L buffer"),
            ),
            2 => DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(1, 1, source_bytes.clone()).expect("source LA buffer"),
            ),
            3 => DynamicImage::ImageRgb8(
                RgbImage::from_raw(1, 1, source_bytes.clone()).expect("source RGB buffer"),
            ),
            4 => DynamicImage::ImageRgba8(
                RgbaImage::from_raw(1, 1, source_bytes.clone()).expect("source four-byte buffer"),
            ),
            _ => unreachable!("test cases use one through four channels"),
        };
        let source = Arc::new(crate::Image::from_dynamic(
            source_image,
            Some(mode.to_owned()),
        ));

        let result = op_paste(&destination, &source, 1, 0, &None, false, Some(mode))
            .unwrap_or_else(|error| panic!("native {mode} paste: {error}"));
        let expected = destination_bytes[..channels]
            .iter()
            .chain(source_bytes.iter())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(result.as_bytes(), expected, "native {mode} bytes");
    }
}

#[test]
fn native_masked_paste_blends_only_clipped_native_channels() {
    for (mode, channels) in [
        ("L", 1),
        ("LA", 2),
        ("PA", 2),
        ("RGB", 3),
        ("HSV", 3),
        ("RGBA", 4),
        ("CMYK", 4),
    ] {
        let destination_bytes = (0..2 * channels)
            .map(|index| 11 + index as u8 * 7)
            .collect::<Vec<_>>();
        let source_bytes = (0..3 * channels)
            .map(|index| 229 - index as u8 * 9)
            .collect::<Vec<_>>();
        let destination = native_byte_image(mode, 2, 1, destination_bytes.clone());
        let source = Arc::new(crate::Image::from_dynamic(
            native_byte_image(mode, 3, 1, source_bytes.clone()),
            Some(mode.to_owned()),
        ));
        let mask = Arc::new(crate::Image::from_dynamic(
            native_byte_image("L", 3, 1, vec![0, 128, 255]),
            Some("L".to_owned()),
        ));

        // Clipping at x=-1 shifts the mask with the source: source/mask
        // pixels 1 and 2 land on destination pixels 0 and 1.
        let result = op_paste(&destination, &source, -1, 0, &Some(mask), false, Some(mode))
            .unwrap_or_else(|error| panic!("native masked {mode} paste: {error}"));
        let mut expected = destination_bytes;
        for (destination_pixel, source_pixel, weight) in [(0, 1, 128u32), (1, 2, 255u32)] {
            for channel in 0..channels {
                let destination_index = destination_pixel * channels + channel;
                let source_value = u32::from(source_bytes[source_pixel * channels + channel]);
                let destination_value = u32::from(expected[destination_index]);
                expected[destination_index] =
                    ((source_value * weight + destination_value * (255 - weight) + 127) / 255)
                        as u8;
            }
        }
        assert_eq!(result.as_bytes(), expected, "native masked {mode} bytes");
    }
}

#[test]
fn native_two_channel_l_masked_paste_row_matches_every_mask_weight() {
    let mask: Vec<u8> = (0..=u8::MAX).collect();
    let source: Vec<u8> = (0..mask.len() * 2)
        .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
        .collect();
    let mut actual: Vec<u8> = (0..mask.len() * 2)
        .map(|index| (index.wrapping_mul(29).wrapping_add(211)) as u8)
        .collect();
    let mut expected = actual.clone();
    for pixel in 0..mask.len() {
        let weight = u32::from(mask[pixel]);
        for channel in 0..2 {
            let index = pixel * 2 + channel;
            let source_value = u32::from(source[index]);
            let destination_value = u32::from(expected[index]);
            expected[index] =
                ((source_value * weight + destination_value * (255 - weight) + 127) / 255) as u8;
        }
    }

    kernels::paste_native_masked_two_channel_l_row(&source, &mut actual, &mask);

    assert_eq!(actual, expected);
}

#[test]
fn native_l_masked_paste_row_matches_div255_endpoints_and_rounding() {
    let masks = [0u8, 1, 127, 128, 254, 255];
    let source = [0u8, 255, 11, 201, 31, 255];
    let mut actual = [255u8, 0, 245, 53, 98, 0];
    let mut expected = actual;

    for (index, mask) in masks.iter().copied().enumerate() {
        let source_value = u32::from(source[index]);
        let destination_value = u32::from(expected[index]);
        let mask_value = u32::from(mask);
        expected[index] =
            ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                as u8;
    }

    kernels::paste_native_masked_l_row(&source, &mut actual, &masks);

    assert_eq!(actual, expected);
}

#[test]
fn native_rgb_l_masked_paste_row_matches_div255_endpoints_and_rounding() {
    let masks = [0u8, 1, 127, 128, 254, 255];
    let source = [
        0, 255, 123, 255, 0, 128, 11, 245, 65, 201, 54, 150, 31, 99, 254, 255, 1, 0,
    ];
    let destination = [
        255, 0, 122, 0, 255, 127, 245, 11, 66, 53, 200, 149, 98, 32, 1, 0, 254, 255,
    ];
    let mut actual = destination;
    let mut expected = destination;

    for (pixel, mask) in masks.iter().copied().enumerate() {
        for channel in 0..3 {
            let byte = pixel * 3 + channel;
            let source_value = u32::from(source[byte]);
            let destination_value = u32::from(expected[byte]);
            let mask_value = u32::from(mask);
            expected[byte] =
                ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                    as u8;
        }
    }

    kernels::paste_native_masked_rgb_l_row(&source, &mut actual, &masks);

    assert_eq!(actual, expected);
}

#[test]
fn native_rgba_l_masked_paste_row_matches_div255_for_all_mask_values() {
    let masks = (0..=u8::MAX).collect::<Vec<_>>();
    let source = (0..masks.len() * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped value fits in a byte")
                .wrapping_mul(37)
                .wrapping_add(13)
        })
        .collect::<Vec<u8>>();
    let destination = (0..masks.len() * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped value fits in a byte")
                .wrapping_mul(71)
                .wrapping_add(29)
        })
        .collect::<Vec<u8>>();
    let mut actual = destination.clone();
    let mut expected = destination;

    for (pixel, mask) in masks.iter().copied().enumerate() {
        for channel in 0..4 {
            let byte = pixel * 4 + channel;
            let source_value = u32::from(source[byte]);
            let destination_value = u32::from(expected[byte]);
            let mask_value = u32::from(mask);
            expected[byte] =
                ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                    as u8;
        }
    }

    kernels::paste_native_masked_rgba_l_row(&source, &mut actual, &masks);

    assert_eq!(actual, expected, "RGBA alpha uses the same native L blend");
}

#[test]
fn native_cmyk_masked_paste_blends_all_four_stored_samples() {
    let masks = [0u8, 1, 127, 128, 254, 255];
    let source = (0..masks.len() * 4)
        .map(|index| (index * 37 + 11) as u8)
        .collect::<Vec<_>>();
    let destination = (0..masks.len() * 4)
        .map(|index| (index * 53 + 7) as u8)
        .collect::<Vec<_>>();
    let mut actual = destination.clone();
    let mut expected = destination;

    for (pixel, mask) in masks.iter().copied().enumerate() {
        for channel in 0..4 {
            let byte = pixel * 4 + channel;
            let source_value = u32::from(source[byte]);
            let destination_value = u32::from(expected[byte]);
            let mask_value = u32::from(mask);
            expected[byte] =
                ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                    as u8;
        }
    }

    kernels::paste_native_masked_cmyk_row(&source, &mut actual, &masks);

    assert_eq!(actual, expected, "CMYK K is blended as its fourth sample");
}

#[cfg(feature = "parallel")]
#[test]
fn native_masked_paste_parallel_clipping_touches_only_the_intersection() {
    let width = 1024u32;
    let destination_height = 1024u32;
    let source_height = 612u32;
    let source_stride = width as usize * 4;
    let destination_stride = width as usize * 4;
    let mut destination_bytes = vec![0u8; destination_stride * destination_height as usize];
    for pixel in destination_bytes.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[9, 19, 29, 39]);
    }
    let mut source_bytes = vec![0u8; source_stride * source_height as usize];
    for (y, row) in source_bytes.chunks_exact_mut(source_stride).enumerate() {
        for pixel in row.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[y as u8, 70, 130, 210]);
        }
    }
    let mask_bytes = vec![255u8; width as usize * source_height as usize];
    let destination =
        native_byte_image("RGBA", width, destination_height, destination_bytes.clone());
    let source = Arc::new(crate::Image::from_dynamic(
        native_byte_image("RGBA", width, source_height, source_bytes.clone()),
        Some("RGBA".to_owned()),
    ));
    let mask = Arc::new(crate::Image::from_dynamic(
        native_byte_image("L", width, source_height, mask_bytes),
        Some("L".to_owned()),
    ));

    // The clipped intersection is 924×512 pixels, above the Rayon
    // threshold. It begins at source (100,100), destination (0,0), and
    // leaves destination rows below the intersection untouched.
    let result = op_paste(
        &destination,
        &source,
        -100,
        -100,
        &Some(mask),
        false,
        Some("RGBA"),
    )
    .expect("parallel clipped native masked paste");
    for destination_y in 0..512usize {
        let source_y = destination_y + 100;
        let destination_row = &mut destination_bytes
            [destination_y * destination_stride..(destination_y + 1) * destination_stride];
        let source_row = &source_bytes[source_y * source_stride..(source_y + 1) * source_stride];
        for destination_x in 0..924usize {
            let source_x = destination_x + 100;
            let destination_pixel = &mut destination_row[destination_x * 4..destination_x * 4 + 4];
            let source_pixel = &source_row[source_x * 4..source_x * 4 + 4];
            destination_pixel.copy_from_slice(source_pixel);
        }
    }
    assert_eq!(result.as_bytes(), destination_bytes);
}

#[test]
fn native_masked_paste_selects_alpha_from_la_and_rgba_masks() {
    let destination = native_byte_image("RGBA", 1, 1, vec![10, 20, 30, 40]);
    let source = Arc::new(crate::Image::from_dynamic(
        native_byte_image("RGBA", 1, 1, vec![110, 120, 130, 140]),
        Some("RGBA".to_owned()),
    ));
    for (mask_mode, mask_bytes) in [("LA", vec![3, 128]), ("RGBA", vec![1, 2, 3, 128])] {
        let mask = Arc::new(crate::Image::from_dynamic(
            native_byte_image(mask_mode, 1, 1, mask_bytes),
            Some(mask_mode.to_owned()),
        ));
        let result = op_paste(&destination, &source, 0, 0, &Some(mask), true, Some("RGBA"))
            .unwrap_or_else(|error| panic!("{mask_mode} alpha mask paste: {error}"));
        let expected = [(10u32, 110u32), (20, 120), (30, 130), (40, 140)]
            .map(|(dst, src)| ((src * 128 + dst * 127 + 127) / 255) as u8);
        assert_eq!(result.as_bytes(), expected, "{mask_mode} alpha band");
    }
}

#[test]
fn native_masked_paste_uses_rgb_a_premultiplied_mask_rule() {
    let destination = native_byte_image(
        "RGBA",
        3,
        1,
        vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
    );
    let source = Arc::new(crate::Image::from_dynamic(
        native_byte_image(
            "RGBA",
            3,
            1,
            vec![110, 120, 130, 140, 150, 160, 170, 180, 210, 220, 230, 240],
        ),
        Some("RGBA".to_owned()),
    ));
    let mask = Arc::new(crate::Image::from_dynamic(
        native_byte_image(
            "RGBa",
            3,
            1,
            vec![255, 0, 7, 0, 255, 0, 7, 128, 0, 255, 7, 255],
        ),
        Some("RGBa".to_owned()),
    ));

    let result = op_paste(&destination, &source, 0, 0, &Some(mask), true, Some("RGBA"))
        .expect("RGBa premultiplied mask paste");
    let expected = [
        (110u32, 10u32, 0u32),
        (120, 20, 0),
        (130, 30, 0),
        (140, 40, 0),
        (150, 50, 128),
        (160, 60, 128),
        (170, 70, 128),
        (180, 80, 128),
        (210, 90, 255),
        (220, 100, 255),
        (230, 110, 255),
        (240, 120, 255),
    ]
    .map(|(source, destination, mask)| {
        let scaled = destination * (255 - mask) + 128;
        ((((scaled >> 8) + scaled) >> 8) + source) as u8
    });
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn native_masked_i16_paste_blends_only_the_first_storage_byte() {
    for (mode, shift) in [("I;16L", 0u32), ("I;16B", 8u32)] {
        for (mask_mode, mask_alpha, premultiplied) in [
            ("L", false, false),
            ("RGBA", true, false),
            ("RGBa", true, true),
        ] {
            for alpha in [0u8, 128, 255] {
                let destination_value = 0x1234u16;
                let source_value = 0xabcdu16;
                let destination =
                    DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                        1,
                        1,
                        crate::raster::Luma([destination_value]),
                    ));
                let source = Arc::new(crate::Image::from_dynamic(
                    DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                        1,
                        1,
                        crate::raster::Luma([source_value]),
                    )),
                    Some(mode.to_owned()),
                ));
                let mask_bytes = if mask_mode == "L" {
                    vec![alpha]
                } else {
                    vec![255, 0, 17, alpha]
                };
                let mask = Arc::new(crate::Image::from_dynamic(
                    native_byte_image(mask_mode, 1, 1, mask_bytes),
                    Some(mask_mode.to_owned()),
                ));
                let result = op_paste(
                    &destination,
                    &source,
                    0,
                    0,
                    &Some(mask),
                    mask_alpha,
                    Some(mode),
                )
                .unwrap_or_else(|error| panic!("masked {mode} paste with {mask_mode}: {error}"));

                let source_byte = u32::from(((source_value >> shift) & 0xff) as u8);
                let destination_byte = u32::from(((destination_value >> shift) & 0xff) as u8);
                let mask_value = u32::from(alpha);
                let blended = if premultiplied {
                    let scaled = destination_byte * (255 - mask_value) + 128;
                    ((((scaled >> 8) + scaled) >> 8) + source_byte) as u8
                } else {
                    ((source_byte * mask_value + destination_byte * (255 - mask_value) + 127) / 255)
                        as u8
                };
                let expected =
                    (destination_value & !(0xffu16 << shift)) | (u16::from(blended) << shift);
                let DynamicImage::ImageLuma16(result) = result else {
                    panic!("masked {mode} paste changed the physical image type");
                };
                assert_eq!(result.as_raw()[0], expected, "{mode}/{mask_mode}/{alpha}");
            }
        }
    }
}
