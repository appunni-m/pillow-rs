//! Migrated exact-mode CPU and vector Expand regressions.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::raster::{DynamicImage, GrayImage, Mode, RgbImage, RgbaImage};

const FILL: (u8, u8, u8, u8) = (7, 11, 13, 17);

fn expand(
    image: &DynamicImage,
    border: u32,
    architecture: Architecture,
) -> DynamicImage {
    let context = ExecutionContext::new(Backend::Cpu, architecture);
    super::super::expand::prepare(image, border, FILL, &context)
        .expect("native Expand preparation must succeed")
        .execute()
        .expect("native Expand execution must succeed")
}

#[test]
fn expand_hsv_keeps_three_native_bytes_and_its_exact_mode() {
    let image = DynamicImage::ImageHsv8(
        RgbImage::from_raw(1, 1, vec![23, 47, 89]).expect("HSV source"),
    );
    let expanded = expand(&image, 1, Architecture::Default);
    let DynamicImage::ImageHsv8(expanded) = expanded else {
        panic!("HSV expansion must retain its exact mode identity");
    };
    assert_eq!(
        expanded.as_raw(),
        &[
            7, 11, 13, 7, 11, 13, 7, 11, 13, 7, 11, 13, 23, 47, 89, 7, 11, 13, 7, 11, 13, 7,
            11, 13, 7, 11, 13,
        ]
    );
}

#[test]
fn expand_cmyk_keeps_k_as_the_fourth_native_sample() {
    let image = DynamicImage::ImageCmyk8(
        RgbaImage::from_raw(1, 1, vec![23, 47, 89, 131]).expect("CMYK source"),
    );
    let expanded = expand(&image, 1, Architecture::Default);
    let DynamicImage::ImageCmyk8(expanded) = expanded else {
        panic!("CMYK expansion must retain its exact mode identity");
    };
    assert_eq!(
        expanded.as_raw(),
        &[
            7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 23, 47, 89, 131, 7, 11,
            13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17,
        ]
    );
}

#[test]
fn expand_rgbx_keeps_x_bytes_and_uses_fill_x_for_the_border() {
    let image = DynamicImage::ImageRgbx8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
            .expect("RGBX source"),
    );
    let expanded = expand(&image, 1, Architecture::Default);
    let DynamicImage::ImageRgbx8(expanded) = expanded else {
        panic!("RGBX expansion must retain its exact mode identity");
    };
    let fill = [7, 11, 13, 17];
    let mut expected = Vec::with_capacity(4 * 3 * 4);
    expected.extend_from_slice(&fill.repeat(4));
    expected.extend_from_slice(&fill);
    expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
    expected.extend_from_slice(&fill);
    expected.extend_from_slice(&fill.repeat(4));
    assert_eq!(expanded.as_raw(), &expected);
}

#[test]
fn expand_rgbx_architecture_request_matches_default_bytes() {
    let image = DynamicImage::ImageRgbx8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
            .expect("RGBX source"),
    );
    let actual = expand(&image, 1, Architecture::Sse2);
    let expected = expand(&image, 1, Architecture::Default);
    assert_eq!(actual.mode(), Mode::ImageRgbx8);
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    assert_eq!(actual.as_bytes().len(), 4 * 3 * 4);
}

#[test]
fn expand_single_byte_modes_preserve_identity_and_match_default_rows() {
    for (width, height, border) in [(1u32, 1u32, 1u32), (17, 3, 2), (33, 32, 1), (16, 16, 0)] {
        let pixels = (0..width * height)
            .map(|index| (index.wrapping_mul(67).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let samples = GrayImage::from_raw(width, height, pixels).expect("one-byte source");
        for mode in [Mode::One, Mode::ImageLuma8, Mode::ImagePalette8] {
            let source = match mode {
                Mode::One => DynamicImage::One(samples.clone()),
                Mode::ImageLuma8 => DynamicImage::ImageLuma8(samples.clone()),
                Mode::ImagePalette8 => DynamicImage::ImagePalette8(samples.clone()),
                _ => unreachable!("test mode list contains one-byte identities"),
            };
            let actual = expand(&source, border, Architecture::Sse2);
            let expected = expand(&source, border, Architecture::Default);
            assert_eq!(actual.mode(), mode);
            assert_eq!(
                (actual.width(), actual.height()),
                (expected.width(), expected.height())
            );
            assert_eq!(actual.as_bytes(), expected.as_bytes());
        }
    }
}

#[test]
fn expand_border_zero_rgbx_vector_copy_preserves_exact_identity() {
    let image = DynamicImage::ImageRgbx8(
        RgbaImage::from_raw(1, 1, vec![23, 47, 89, 131]).expect("RGBX source"),
    );
    let actual = expand(&image, 0, Architecture::Sse2);
    assert_eq!(actual.mode(), Mode::ImageRgbx8);
    assert_eq!(actual.as_bytes(), image.as_bytes());
}
