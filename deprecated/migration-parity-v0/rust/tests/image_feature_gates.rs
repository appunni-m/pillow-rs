//! Manifest-driven verification of downstream codec feature forwarding.

#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use image_slash_star::{ImageError, ImageFormat};
use pillow_rs::{Image, PilError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    decode: Vec<DecodeCase>,
}

#[derive(Deserialize)]
struct DecodeCase {
    feature: String,
    input: String,
    format: String,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/image_backend")
}

fn enabled(feature: &str) -> bool {
    match feature {
        "image-jpeg" => cfg!(feature = "image-jpeg"),
        "image-png" => cfg!(feature = "image-png"),
        "image-gif" => cfg!(feature = "image-gif"),
        "image-bmp" => cfg!(feature = "image-bmp"),
        "image-tiff" => cfg!(feature = "image-tiff"),
        "image-webp" => cfg!(feature = "image-webp"),
        "image-ico" => cfg!(feature = "image-ico"),
        "image-avif" => cfg!(feature = "image-avif"),
        other => panic!("unknown manifest feature {other}"),
    }
}

#[test]
fn disabled_codec_rows_preserve_backend_feature_errors() {
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(fixture_root().join("manifest.json")).expect("fixture manifest must be readable"),
    )
    .expect("fixture manifest must be valid JSON");

    for row in manifest.decode {
        if enabled(&row.feature) {
            continue;
        }
        let bytes =
            fs::read(fixture_root().join(row.input)).expect("input fixture must be readable");
        let expected_format = ImageFormat::from_name(&row.format).expect("known fixture format");
        let error = Image::open_bytes(bytes).expect_err("disabled codec must reject during open");
        assert!(
            matches!(
                &error,
                PilError::ImageError(ImageError::FeatureDisabled { format, feature })
                    if *format == expected_format && *feature == row.feature.trim_start_matches("image-")
            ),
            "{} returned {error:?}",
            row.feature
        );
    }
}
