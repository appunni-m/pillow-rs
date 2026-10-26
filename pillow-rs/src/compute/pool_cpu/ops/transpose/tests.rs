use crate::raster::{DynamicImage, GenericImageView, GrayImage, RgbImage, RgbaImage};

fn assert_tiled_transpose_case(width: u32, height: u32, channels: usize) {
    use crate::pipeline::TransposeMethod;
    use crate::raster::GrayAlphaImage;
    let length = width as usize * height as usize * channels;
    // ImageBuffer permits extra samples after its logical raster.
    // Both paths must copy only pixels within the declared shape.
    let source: Vec<u8> = (0..length + channels * 3 + 1)
        .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
        .collect();
    let image = match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, source.clone()).unwrap()),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, source.clone()).unwrap(),
        ),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, source.clone()).unwrap()),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, source.clone()).unwrap()),
        _ => unreachable!(),
    };
    for method in [
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate270,
    ] {
        let mut expected = vec![0u8; length];
        for source_y in 0..height as usize {
            for source_x in 0..width as usize {
                let (target_x, target_y) = match method {
                    TransposeMethod::Transpose => (source_y, source_x),
                    TransposeMethod::Transverse => (
                        height as usize - 1 - source_y,
                        width as usize - 1 - source_x,
                    ),
                    TransposeMethod::Rotate90 => (source_y, width as usize - 1 - source_x),
                    TransposeMethod::Rotate270 => (height as usize - 1 - source_y, source_x),
                    _ => unreachable!(),
                };
                let input = (source_y * width as usize + source_x) * channels;
                let output = (target_y * height as usize + target_x) * channels;
                expected[output..output + channels]
                    .copy_from_slice(&source[input..input + channels]);
            }
        }
        // Directly exercise partial tiles even below admission,
        // and prove every output byte is written independently of
        // initialized storage contents.
        for sentinel in [0u8, 0xA5] {
            let mut output = vec![sentinel; length];
            macro_rules! transpose {
                ($channels:literal) => {
                    super::transpose_bytes_tiled::<$channels>(
                        &source,
                        &mut output,
                        width,
                        height,
                        &method,
                        super::TRANSPOSE_TILE_SIZE,
                    )
                };
            }
            match channels {
                1 => transpose!(1),
                2 => transpose!(2),
                3 => transpose!(3),
                4 => transpose!(4),
                _ => unreachable!(),
            }
            assert_eq!(
                output, expected,
                "tiled {width}x{height} C{channels} {method:?}"
            );
        }
        let actual = super::execute_transpose(&image, &method).expect("native transpose");
        assert_eq!(actual.dimensions(), (height, width));
        assert_eq!(actual.color(), image.color());
        assert_eq!(
            actual.as_bytes(),
            expected,
            "dispatch {width}x{height} C{channels} {method:?}"
        );
        assert_eq!(image.as_bytes(), source, "source remains unchanged");
    }
}

#[test]
fn transpose_tiled_rows_preserve_native_pixels_edges_and_trailing_storage() {
    for (width, height) in [
        (1, 1),
        (1, 257),
        (257, 1),
        (3, 7),
        (9, 31),
        (31, 9),
        (31, 33),
        (32, 32),
        (33, 31),
        (63, 65),
        (127, 129),
        (255, 257),
        (511, 513),
        (512, 512),
        (513, 515),
        (515, 513),
    ] {
        for channels in 1..=4 {
            assert_tiled_transpose_case(width, height, channels);
        }
    }
}
#[test]
fn transpose_row_copies_preserve_native_pixels_and_edges() {
    use crate::pipeline::TransposeMethod;
    for (width, height) in [
        (0, 3),
        (3, 0),
        (1, 9),
        (9, 1),
        (9, 31),
        (511, 513),
        (512, 512),
        (513, 515),
        (1, 262145),
        (262145, 1),
    ] {
        for channels in 1..=4 {
            let length = width as usize * height as usize * channels;
            let source: Vec<u8> = (0..length)
                .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
                .collect();
            let image = crate::image_utils::raw_bytes_to_image_allow_empty(
                width,
                height,
                source.clone(),
                channels,
            )
            .expect("native source shape");
            for method in [
                TransposeMethod::FlipLeftRight,
                TransposeMethod::FlipTopBottom,
                TransposeMethod::Rotate180,
            ] {
                let mut expected = vec![0u8; length];
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        let (destination_x, destination_y) = match method {
                            TransposeMethod::FlipLeftRight => (width as usize - 1 - x, y),
                            TransposeMethod::FlipTopBottom => (x, height as usize - 1 - y),
                            TransposeMethod::Rotate180 => {
                                (width as usize - 1 - x, height as usize - 1 - y)
                            }
                            _ => unreachable!(),
                        };
                        let input = (y * width as usize + x) * channels;
                        let output = (destination_y * width as usize + destination_x) * channels;
                        expected[output..output + channels]
                            .copy_from_slice(&source[input..input + channels]);
                    }
                }
                let actual = super::execute_transpose(&image, &method).expect("native flip");
                assert_eq!(actual.dimensions(), (width, height));
                assert_eq!(actual.color(), image.color());
                assert_eq!(
                    actual.as_bytes(),
                    expected,
                    "{width}x{height} C{channels} {method:?}"
                );
                assert_eq!(image.as_bytes(), source, "source remains unchanged");
            }
        }
    }
}
#[test]
fn transpose_row_copies_discard_trailing_storage() {
    use crate::pipeline::TransposeMethod;
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(
            2,
            2,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 99, 98, 97],
        )
        .expect("trailing samples are permitted by ImageBuffer"),
    );
    for (method, expected) in [
        (
            TransposeMethod::FlipLeftRight,
            [4, 5, 6, 1, 2, 3, 10, 11, 12, 7, 8, 9],
        ),
        (
            TransposeMethod::FlipTopBottom,
            [7, 8, 9, 10, 11, 12, 1, 2, 3, 4, 5, 6],
        ),
        (
            TransposeMethod::Rotate180,
            [10, 11, 12, 7, 8, 9, 4, 5, 6, 1, 2, 3],
        ),
    ] {
        let output = super::execute_transpose(&image, &method).expect("native flip");
        assert_eq!(output.as_bytes(), expected);
    }
}
