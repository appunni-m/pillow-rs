#![allow(missing_docs, unused_crate_dependencies)]

use pillow_rs::font::{Font, imagingft};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    font: FontOracle,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct FontOracle {
    kind: String,
    expected_name: [String; 2],
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
    freetype_version: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    text: String,
    start: Option<[f64; 2]>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    mode: String,
    size: [u32; 2],
    offset: [i32; 2],
    pixels_hex: String,
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
                .expect("valid hex")
        })
        .collect()
}

#[test]
fn getmask2_matches_independent_pillow_oracle() {
    let manifest: Manifest = serde_json::from_str(include_str!("fixtures/imagefont/getmask2.json"))
        .expect("valid generated getmask2 oracle");
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    assert_eq!(manifest.oracle.freetype_version, "2.14.3");
    assert_eq!(manifest.font.kind, "load_default");

    let font = Font::load_default(10.0).expect("pinned Pillow default font");
    let actual_name = imagingft::getname(&font);
    assert_eq!(
        [actual_name.0, actual_name.1],
        [
            manifest.font.expected_name[0].as_str(),
            manifest.font.expected_name[1].as_str(),
        ],
        "exact default-font family and style"
    );
    for case in manifest.cases {
        let start = case.start.map_or((0.0, 0.0), |value| (value[0], value[1]));
        let (width, height, pixels, offset) =
            imagingft::getmask2_with_start(&font, &case.text, start);
        assert_eq!(case.expected.mode, "L", "{} mode", case.id);
        assert_eq!([width, height], case.expected.size, "{} size", case.id);
        assert_eq!(
            [offset.0, offset.1],
            case.expected.offset,
            "{} offset",
            case.id
        );
        assert_eq!(
            pixels,
            decode_hex(&case.expected.pixels_hex),
            "{} exact mask bytes",
            case.id
        );
    }
}
