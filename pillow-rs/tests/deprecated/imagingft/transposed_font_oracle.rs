#![allow(missing_docs, unused_crate_dependencies)]

use pillow_rs::Font;
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    font: FontSpec,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
    freetype_version: String,
}

#[derive(Deserialize)]
struct FontSpec {
    kind: String,
    size: f32,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    orientation: Option<String>,
    text: String,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    bbox: [i32; 4],
    length: Option<f32>,
    length_error: Option<ExpectedError>,
    mask: ExpectedMask,
}

#[derive(Deserialize)]
struct ExpectedError {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

#[derive(Deserialize)]
struct ExpectedMask {
    mode: String,
    size: [u32; 2],
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
fn transposed_font_matches_independent_pillow_oracle() {
    let manifest: Manifest =
        serde_json::from_str(include_str!("fixtures/imagefont/transposed_font.json"))
            .expect("valid generated TransposedFont oracle");
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    assert_eq!(manifest.oracle.freetype_version, "2.14.3");
    assert_eq!(manifest.font.kind, "load_default");

    let font = Font::load_default(manifest.font.size).expect("pinned Pillow default font");
    for case in manifest.cases {
        let orientation = case.orientation.as_deref();
        let bbox = pillow_rs::transposed_bbox(pillow_rs::font_getbbox(&font, &case.text), orientation);
        assert_eq!(
            [bbox.0, bbox.1, bbox.2, bbox.3],
            case.expected.bbox,
            "{} bbox",
            case.id
        );

        match (
            pillow_rs::validate_transposed_length(orientation),
            case.expected.length,
            case.expected.length_error,
        ) {
            (Ok(()), Some(expected), None) => {
                assert_eq!(
                    pillow_rs::font_getlength(&font, &case.text),
                    expected,
                    "{} length",
                    case.id
                );
            }
            (Err(error), None, Some(expected)) => {
                assert_eq!(expected.kind, "ValueError", "{} error type", case.id);
                assert_eq!(
                    error.to_string(),
                    expected.message,
                    "{} error message",
                    case.id
                );
            }
            _ => panic!("{} length contract mismatch", case.id),
        }

        let (width, height, pixels) =
            pillow_rs::font_get_transposed_mask(&font, &case.text, orientation)
                .expect("transposed mask");
        assert_eq!(case.expected.mask.mode, "L", "{} mask mode", case.id);
        assert_eq!(
            [width, height],
            case.expected.mask.size,
            "{} mask size",
            case.id
        );
        assert_eq!(
            pixels,
            decode_hex(&case.expected.mask.pixels_hex),
            "{} exact mask bytes",
            case.id
        );
    }
}
