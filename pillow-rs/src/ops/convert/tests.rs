use crate::image::Image;
use crate::ops::paste::PasteSource;

#[test]
fn rgba_palette_split_preserves_rgb_and_alpha_bytes() {
    let source = crate::raster::DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80])
            .expect("valid RGBA source"),
    );

    let (rgb, alpha) =
        super::palette::split_rgba_for_palette(&source).expect("split RGBA palette source");
    assert_eq!(rgb.into_raw(), [10, 20, 30, 50, 60, 70]);
    assert_eq!(alpha, [40, 80]);
}

#[test]
fn rgba_palette_split_ignores_trailing_storage() {
    let source = crate::raster::DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(1, 1, vec![10, 20, 30, 40, 50, 60, 70, 80])
            .expect("valid RGBA source with trailing storage"),
    );

    let (rgb, alpha) =
        super::palette::split_rgba_for_palette(&source).expect("split logical RGBA pixels");
    assert_eq!(rgb.into_raw(), [10, 20, 30]);
    assert_eq!(alpha, [40]);
}

#[test]
fn integer_to_rgb_clamps_native_i_samples_and_broadcasts_them() {
    let samples = [-1_i32, 0, 1, 127, 255, 256, i32::MIN, i32::MAX];
    let source_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::from_dynamic(
        crate::raster::DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(samples.len() as u32, 1, source_bytes)
                .expect("valid I-mode carrier"),
        ),
        Some(crate::raster::Mode::ImageI32),
    );

    let converted = source
        .convert("RGB", None, None, None, None)
        .expect("I to RGB conversion");

    assert_eq!(converted.mode().expect("converted mode"), "RGB");
    assert_eq!(
        converted.tobytes().expect("converted bytes"),
        [
            0, 0, 0, 0, 0, 0, 1, 1, 1, 127, 127, 127, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
            255, 255,
        ]
    );
}

#[test]
fn float_to_rgb_clamps_and_truncates_native_f_samples() {
    let samples = [-1.5_f32, 0.0, 0.9, 1.9, 127.9, 255.9, 256.5];
    let source_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::from_dynamic(
        crate::raster::DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(samples.len() as u32, 1, source_bytes)
                .expect("valid F-mode carrier"),
        ),
        Some(crate::raster::Mode::ImageF32),
    );

    let converted = source
        .convert("RGB", None, None, None, None)
        .expect("F to RGB conversion");

    assert_eq!(converted.mode().expect("converted mode"), "RGB");
    assert_eq!(
        converted.tobytes().expect("converted bytes"),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 127, 127, 127, 255, 255, 255, 255, 255, 255,
        ]
    );
}

#[test]
fn luma8_to_luma16_conversion_and_paste_preserve_numeric_sample() {
    for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
        let source = Image::new(1, 1, "L", (17, 17, 17, 255)).expect("source image");
        let converted = source
            .convert(mode, None, None, None, None)
            .expect("I;16 conversion");
        assert_eq!(converted.mode().expect("converted mode"), mode);
        assert_eq!(
            converted.tobytes().expect("converted bytes"),
            match mode {
                "I;16B" => vec![0, 17],
                _ => vec![17, 0],
            }
        );

        let mut destination = Image::new(1, 1, mode, (0, 0, 0, 255)).expect("destination image");
        destination
            .paste(
                PasteSource::Image(Box::new(source)),
                Some((0, 0, 1, 1)),
                None,
            )
            .expect("I;16 paste");
        assert_eq!(
            destination.tobytes().expect("pasted bytes"),
            match mode {
                "I;16B" => vec![0, 17],
                _ => vec![17, 0],
            }
        );
    }
}
