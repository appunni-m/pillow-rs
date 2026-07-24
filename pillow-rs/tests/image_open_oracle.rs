//! Exact Rust API execution of the shared Pillow Image.open decoder corpus.
#![allow(unused_crate_dependencies)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::image::Image;
use serde::Deserialize;

const PILLOW_VERSION: &str = "12.2.0";

#[derive(Deserialize)]
struct InputManifest {
    pillow_version: String,
    inputs: BTreeMap<String, EncodedInput>,
}

#[derive(Deserialize)]
struct EncodedInput {
    hex: String,
}

#[derive(Deserialize)]
struct OutputFixture {
    pillow_version: String,
    cases: Vec<OutputCase>,
}

#[derive(Deserialize)]
struct OutputCase {
    id: String,
    assert: OracleAssertion,
}

#[derive(Deserialize)]
struct OracleAssertion {
    method: String,
    reference: Option<String>,
    raw_kind: Option<String>,
    mode: Option<String>,
    size: Option<[u32; 2]>,
    palette: Option<Vec<u8>>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pillow-rs crate has a workspace parent")
        .to_owned()
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert!(
        value.len().is_multiple_of(2),
        "hex input has complete bytes"
    );
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex input is ASCII");
            u8::from_str_radix(text, 16).expect("hex input contains byte pairs")
        })
        .collect()
}

#[test]
fn shared_image_open_corpus_matches_pillow_exactly() {
    let root = repo_root();
    let manifest: InputManifest = serde_json::from_slice(
        &fs::read(root.join("tests/oracles/image_open_inputs.json"))
            .expect("shared encoded inputs are present"),
    )
    .expect("shared encoded inputs parse");
    let output: OutputFixture = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/outputs/jsons/ImageModule.open.json"))
            .expect("Pillow Image.open oracle is present"),
    )
    .expect("Pillow Image.open oracle parses");

    assert_eq!(manifest.pillow_version, PILLOW_VERSION);
    assert_eq!(output.pillow_version, PILLOW_VERSION);
    assert_eq!(manifest.inputs.len(), 9);

    for (mode, encoded) in manifest.inputs {
        let case_id = if mode == "RGB" {
            "rgb_10x10".to_owned()
        } else {
            format!("ImageModule.open_{mode}")
        };
        let assertion = &output
            .cases
            .iter()
            .find(|candidate| candidate.id == case_id)
            .unwrap_or_else(|| panic!("{case_id}: Pillow oracle is present"))
            .assert;
        assert_eq!(assertion.method, "image", "{case_id}: assertion method");
        assert_eq!(assertion.raw_kind.as_deref(), Some("image"));

        let actual = Image::open_bytes(decode_hex(&encoded.hex))
            .unwrap_or_else(|error| panic!("{case_id}: encoded input opens: {error}"));
        assert_eq!(
            actual.mode().expect("mode resolves"),
            assertion.mode.as_deref().expect("oracle mode is present"),
            "{case_id}: mode"
        );
        let expected_size = assertion.size.expect("oracle dimensions are present");
        assert_eq!(
            actual.size().expect("dimensions resolve"),
            (expected_size[0], expected_size[1]),
            "{case_id}: dimensions"
        );
        assert_eq!(
            actual.tobytes().expect("public bytes materialize"),
            fs::read(
                root.join("tests/fixtures/outputs").join(
                    assertion
                        .reference
                        .as_deref()
                        .expect("oracle reference is present")
                )
            )
            .expect("raw Pillow oracle is present"),
            "{case_id}: exact public bytes"
        );
        if let Some(expected_palette) = &assertion.palette {
            assert_eq!(
                actual.palette().unwrap_or_default(),
                *expected_palette,
                "{case_id}: exact palette"
            );
        }
    }
}
