//! Exact direct-Rust execution of the shared Pillow apply-transparency fixtures.
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::Image;
use pillow_rs::PaletteTransparency;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    apply_transparency_cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    input: String,
    prepare_alpha: Option<u8>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
    palette_rgba_hex: String,
    before_info: Value,
    before_palette_mode: String,
    before_has_transparency_data: bool,
    info: Value,
    palette_mode: String,
    has_transparency_data: bool,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("image_backend")
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex is ASCII");
            u8::from_str_radix(pair, 16).expect("fixture contains valid hex")
        })
        .collect()
}

fn transparency_info(image: &Image) -> Value {
    match image.pending_palette_transparency() {
        Some(PaletteTransparency::Index(value)) => {
            serde_json::json!({"transparency": {"kind": "index", "value": value}})
        }
        Some(PaletteTransparency::Table(value)) => {
            serde_json::json!({
                "transparency": {
                    "kind": "table",
                    "value_hex": value.iter().map(|byte| format!("{byte:02x}")).collect::<String>()
                }
            })
        }
        None => serde_json::json!({}),
    }
}

#[test]
fn shared_apply_transparency_cases_match_pillow_exactly() {
    let root = fixture_root();
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(root.join("backend_parity.json")).expect("backend parity manifest exists"),
    )
    .expect("backend parity manifest parses");
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    assert_eq!(manifest.apply_transparency_cases.len(), 3);

    for case in manifest.apply_transparency_cases {
        let mut image = Image::open_bytes(
            fs::read(root.join(&case.input))
                .unwrap_or_else(|error| panic!("{} input: {error}", case.id)),
        )
        .unwrap_or_else(|error| panic!("{} opens: {error}", case.id));
        if let Some(alpha) = case.prepare_alpha {
            image
                .putalpha(alpha)
                .unwrap_or_else(|error| panic!("{} prepares PA: {error}", case.id));
        }

        let expected = case.expected;
        assert_eq!(
            transparency_info(&image),
            expected.before_info,
            "{}",
            case.id
        );
        assert_eq!(
            image.palette_mode(),
            Some(expected.before_palette_mode.as_str()),
            "{}",
            case.id
        );
        assert_eq!(
            image.has_transparency_data(),
            expected.before_has_transparency_data,
            "{}",
            case.id
        );

        image
            .apply_transparency()
            .unwrap_or_else(|error| panic!("{} applies: {error}", case.id));

        assert_eq!(transparency_info(&image), expected.info, "{}", case.id);
        assert_eq!(
            image.palette_mode(),
            Some(expected.palette_mode.as_str()),
            "{}",
            case.id
        );
        assert_eq!(
            image.has_transparency_data(),
            expected.has_transparency_data,
            "{}",
            case.id
        );
        assert_eq!(image.mode().expect("mode"), expected.mode, "{}", case.id);
        assert_eq!(
            image.size().expect("size"),
            (expected.size[0], expected.size[1]),
            "{}",
            case.id
        );
        assert_eq!(
            image.tobytes().expect("pixels"),
            decode_hex(&expected.pixels_hex),
            "{}",
            case.id
        );
        assert_eq!(
            image.getpalette_rgba(),
            Some(decode_hex(&expected.palette_rgba_hex)),
            "{}",
            case.id
        );
    }
}
