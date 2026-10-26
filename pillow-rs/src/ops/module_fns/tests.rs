use super::merge;
use crate::image::{FormattedImageData, FormattedPixelValue, Image};

fn l_band(bytes: &[u8]) -> Image {
    Image::frombytes("L", (2, 1), bytes).expect("valid L band")
}

#[test]
fn merge_preserves_alias_modes_and_lab_encoding() {
    let l0 = l_band(&[0x10, 0x11]);
    let l1 = l_band(&[0x20, 0x21]);
    let l2 = l_band(&[0x30, 0x31]);
    let l3 = l_band(&[0x40, 0x41]);

    let cases = [
        (
            "La",
            vec![l0.clone(), l1.clone()],
            vec![0x10, 0x20, 0x11, 0x21],
            vec!["L", "a"],
        ),
        (
            "PA",
            vec![l0.clone(), l1.clone()],
            vec![0x10, 0x20, 0x11, 0x21],
            vec!["P", "A"],
        ),
        (
            "RGBX",
            vec![l0.clone(), l1.clone(), l2.clone(), l3.clone()],
            vec![0x10, 0x20, 0x30, 0x40, 0x11, 0x21, 0x31, 0x41],
            vec!["R", "G", "B", "X"],
        ),
        (
            "RGBa",
            vec![l0.clone(), l1.clone(), l2.clone(), l3.clone()],
            vec![0x10, 0x20, 0x30, 0x40, 0x11, 0x21, 0x31, 0x41],
            vec!["R", "G", "B", "a"],
        ),
        (
            "YCbCr",
            vec![l0.clone(), l1.clone(), l2.clone()],
            vec![0x10, 0x20, 0x30, 0x11, 0x21, 0x31],
            vec!["Y", "Cb", "Cr"],
        ),
        (
            "HSV",
            vec![l0.clone(), l1.clone(), l2.clone()],
            vec![0x10, 0x20, 0x30, 0x11, 0x21, 0x31],
            vec!["H", "S", "V"],
        ),
    ];
    for (mode, bands, bytes, names) in cases {
        let result = merge(mode, &bands).expect("alias merge succeeds");
        assert_eq!(result.mode().expect("mode"), mode);
        assert_eq!(result.tobytes().expect("bytes"), bytes);
        assert_eq!(result.getbands().expect("bands"), names);
    }
    assert_lab_merge_encoding(l0, l1, l2);
}

fn assert_lab_merge_encoding(l0: Image, l1: Image, l2: Image) {
    let lab = merge("LAB", &[l0, l1, l2]).expect("LAB merge succeeds");
    assert_eq!(lab.mode().expect("mode"), "LAB");
    assert_eq!(
        lab.tobytes().expect("bytes"),
        [0x10, 0xa0, 0xb0, 0x11, 0xa1, 0xb1]
    );
    assert_eq!(
        lab.getpixel_formatted(0, 0).expect("pixel"),
        FormattedPixelValue::Components(vec![0x10, 0x20, 0x30])
    );
    assert_eq!(
        lab.getdata_formatted(None).expect("data"),
        FormattedImageData::Components(vec![vec![0x10, 0x20, 0x30], vec![0x11, 0x21, 0x31]])
    );
}

#[test]
fn merge_preserves_first_palette_band_and_rejects_later_palette() {
    let palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("valid P band");
    let l0 = l_band(&[0x20, 0x21]);
    let l1 = l_band(&[0x30, 0x31]);

    let result = merge("RGB", &[palette.clone(), l0, l1.clone()]).expect("first P is accepted");
    assert_eq!(
        result.tobytes().expect("bytes"),
        [1, 0x20, 0x30, 2, 0x21, 0x31]
    );
    assert_eq!(result.mode().expect("mode"), "RGB");

    let later = merge(
        "RGB",
        &[Image::frombytes("L", (2, 1), &[1, 2]).unwrap(), palette, l1],
    );
    assert_eq!(
        later.expect_err("later P must fail").to_string(),
        "mode mismatch"
    );
}

#[test]
fn merge_keeps_typed_scalar_storage() {
    let mode1 = merge(
        "1",
        &[Image::frombytes("1", (2, 1), &[0xa0]).expect("valid 1 band")],
    )
    .expect("1 merge");
    assert_eq!(mode1.tobytes().expect("1 bytes"), [0x80]);

    let int = merge(
        "I",
        &[
            Image::frombytes("I", (2, 1), &[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff])
                .expect("valid I band"),
        ],
    )
    .expect("I merge");
    assert_eq!(
        int.tobytes().expect("I bytes"),
        [1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]
    );

    let float = merge(
        "F",
        &[
            Image::frombytes("F", (2, 1), &[0, 0, 0xc0, 0x3f, 0, 0, 0x10, 0xc0])
                .expect("valid F band"),
        ],
    )
    .expect("F merge");
    assert_eq!(
        float.tobytes().expect("F bytes"),
        [0, 0, 0xc0, 0x3f, 0, 0, 0x10, 0xc0]
    );

    assert_eq!(
        merge("I", &[l_band(&[1, 2])])
            .expect_err("L cannot merge into I")
            .to_string(),
        "images do not match"
    );
}

#[test]
fn merge_keeps_i16_byte_order_and_scalar_values() {
    for (mode, bytes) in [
        ("I;16", vec![0x01, 0x02, 0x03, 0x04]),
        ("I;16L", vec![0x01, 0x02, 0x03, 0x04]),
        ("I;16B", vec![0x01, 0x02, 0x03, 0x04]),
        ("I;16N", vec![0x01, 0x02, 0x03, 0x04]),
    ] {
        let band = Image::frombytes(mode, (2, 1), &bytes).expect("valid I;16 band");
        let merged = merge(mode, &[band]).expect("I;16 merge succeeds");
        assert_eq!(merged.mode().expect("mode"), mode);
        assert_eq!(merged.tobytes().expect("raw bytes"), bytes);
        assert!(matches!(
            merged.getdata_formatted(None).expect("data"),
            FormattedImageData::IntegerScalars(values) if values.len() == 2
        ));
    }
}

#[test]
fn merge_rejects_i16_band_mode_and_arity_mismatches() {
    let band = Image::frombytes("I;16", (1, 1), &[0x2a, 0x00]).expect("valid I;16 band");
    let other = Image::frombytes("I;16L", (1, 1), &[0x2a, 0x00]).expect("valid I;16L band");
    assert_eq!(
        merge("I;16", &[]).expect_err("missing band").to_string(),
        "wrong number of bands"
    );
    assert_eq!(
        merge("I;16", &[band.clone(), band.clone()])
            .expect_err("extra band")
            .to_string(),
        "wrong number of bands"
    );
    assert_eq!(
        merge("I;16", &[other])
            .expect_err("wrong I;16 spelling")
            .to_string(),
        "images do not match"
    );
}
