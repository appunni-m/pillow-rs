#![cfg(feature = "image-codecs-all")]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::{Image, PilError};
use pillow_rs_image::{ImageError, ImageFormat};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    decode: Vec<DecodeCase>,
    errors: Vec<ErrorCase>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
    source: String,
}

#[derive(Deserialize)]
struct DecodeCase {
    id: String,
    feature: String,
    input: String,
    pixels: String,
    format: String,
    mode: String,
    width: u32,
    height: u32,
    palette_hex: Option<String>,
    palette_alpha_hex: Option<String>,
}

#[derive(Deserialize)]
struct ErrorCase {
    id: String,
    feature: String,
    input: String,
    stage: String,
    kind: String,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/image_backend")
}

fn manifest() -> Manifest {
    let path = fixture_root().join("manifest.json");
    let bytes = fs::read(path).expect("migration fixture manifest must be readable");
    serde_json::from_slice(&bytes).expect("migration fixture manifest must be valid JSON")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn manifest_decode_rows_preserve_oracle_state_before_and_after_load() {
    let manifest = manifest();
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    assert_eq!(
        manifest.oracle.source,
        "image-slash-star/pillow-oracle.lock.yaml"
    );

    for row in manifest.decode {
        if row.feature == "image-avif" && !cfg!(feature = "image-avif") {
            continue;
        }
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        let expected_pixels = fs::read(fixture_root().join(&row.pixels))
            .unwrap_or_else(|error| panic!("{} pixel fixture: {error}", row.id));
        let mut image = Image::open_bytes(input)
            .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));

        assert!(!image.is_materialized(), "{} opened eagerly", row.id);
        let info_before = image
            .image_info()
            .unwrap_or_else(|| panic!("{} missing cached ImageInfo", row.id));
        assert_eq!(
            image.size().expect("fixture metadata size"),
            (row.width, row.height),
            "{} size",
            row.id
        );
        assert_eq!(
            image.mode().expect("fixture metadata mode"),
            row.mode,
            "{} mode",
            row.id
        );
        assert_eq!(image.format_name().as_deref(), Some(row.format.as_str()));
        assert_eq!(
            (info_before.width, info_before.height),
            (row.width, row.height)
        );
        assert!(
            !image.is_materialized(),
            "{} metadata decoded pixels",
            row.id
        );

        image
            .verify()
            .unwrap_or_else(|error| panic!("{} verify failed: {error}", row.id));
        assert!(!image.is_materialized(), "{} verify changed state", row.id);
        image
            .load()
            .unwrap_or_else(|error| panic!("{} load failed: {error}", row.id));

        assert!(
            image.is_materialized(),
            "{} load was not persistent",
            row.id
        );
        assert_eq!(
            image.size().expect("loaded fixture size"),
            (row.width, row.height),
            "{} loaded size",
            row.id
        );
        assert_eq!(
            image.mode().expect("loaded fixture mode"),
            row.mode,
            "{} loaded mode",
            row.id
        );
        assert_eq!(image.format_name().as_deref(), Some(row.format.as_str()));
        assert_eq!(image.image_info(), Some(info_before));
        assert_eq!(
            image
                .tobytes()
                .unwrap_or_else(|error| panic!("{} pixels failed: {error}", row.id)),
            expected_pixels,
            "{} exact Pillow pixels",
            row.id
        );

        if let Some(expected) = row.palette_hex {
            let palette = image
                .getpalette_trimmed()
                .unwrap_or_else(|| panic!("{} missing palette", row.id));
            assert_eq!(hex(&palette), expected, "{} exact Pillow palette", row.id);
        }
        if let Some(expected) = row.palette_alpha_hex {
            let alpha = image
                .palette_alpha()
                .unwrap_or_else(|| panic!("{} missing palette alpha", row.id));
            assert_eq!(hex(&alpha), expected, "{} exact palette alpha", row.id);
        }
    }
}

#[test]
fn manifest_error_rows_preserve_structured_failures() {
    for row in manifest().errors {
        assert_eq!(row.feature, "image-png", "{} feature", row.id);
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        match (row.stage.as_str(), row.kind.as_str()) {
            ("open", "unidentified") => {
                let error = Image::open_bytes(input).expect_err("fixture must fail during open");
                assert!(
                    matches!(error, PilError::UnidentifiedImageError(_)),
                    "{} returned {error:?}",
                    row.id
                );
            }
            ("verify", "malformed_png") => {
                let image = Image::open_bytes(input)
                    .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));
                let error = image
                    .verify()
                    .expect_err("fixture must fail during verification");
                assert!(
                    matches!(
                        error,
                        PilError::ImageError(ImageError::Malformed {
                            format: ImageFormat::Png,
                            ..
                        })
                    ),
                    "{} returned {error:?}",
                    row.id
                );
            }
            _ => panic!("{} has an unsupported error expectation", row.id),
        }
    }
}
