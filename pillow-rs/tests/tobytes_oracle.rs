//! Exact Rust API execution of the shared Pillow Image.tobytes corpus.
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::image::Image;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct InputFixture {
    cases: Vec<InputCase>,
}

#[derive(Deserialize)]
struct InputCase {
    id: String,
    mode: String,
    input: InputSpec,
    params: EncoderParams,
}

#[derive(Deserialize)]
struct InputSpec {
    source: String,
    size: [u32; 2],
    color: Option<Value>,
}

#[derive(Deserialize)]
struct EncoderParams {
    #[serde(rename = "_args", default)]
    args: Vec<String>,
}

#[derive(Deserialize)]
struct OutputFixture {
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
    reference: String,
    raw_kind: String,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pillow-rs crate has a workspace parent")
        .to_owned()
}

fn constant_color(value: &Value) -> (u8, u8, u8, u8) {
    let values = value.as_array().expect("constant color is an array");
    let component = |index: usize, default: u8| {
        values
            .get(index)
            .and_then(Value::as_u64)
            .map_or(default, |item| item as u8)
    };
    (
        component(0, 0),
        component(1, 0),
        component(2, 0),
        component(3, 255),
    )
}

fn create_input(root: &Path, case: &InputCase) -> Image {
    match case.input.source.as_str() {
        "constant" => Image::new(
            case.input.size[0],
            case.input.size[1],
            &case.mode,
            constant_color(case.input.color.as_ref().expect("constant color exists")),
        )
        .expect("constant image constructs"),
        "reference_rgb" => {
            let source = Image::open_bytes(
                fs::read(root.join("tests/test_reference.png")).expect("reference image exists"),
            )
            .expect("reference image opens");
            source
                .resize((case.input.size[0], case.input.size[1]), Some("LANCZOS"))
                .expect("reference image resizes")
                .convert(&case.mode, None, None, None, None)
                .expect("reference image converts")
        }
        source => panic!("unsupported fixture input source {source}"),
    }
}

#[test]
fn shared_tobytes_corpus_matches_pillow_exactly() {
    let root = repo_root();
    let input: InputFixture = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/input/jsons/Image.tobytes.json"))
            .expect("Image.tobytes inputs exist"),
    )
    .expect("Image.tobytes inputs parse");
    let output: OutputFixture = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/outputs/jsons/Image.tobytes.json"))
            .expect("Image.tobytes oracles exist"),
    )
    .expect("Image.tobytes oracles parse");

    assert_eq!(input.cases.len(), 8);
    for case in input.cases {
        let assertion = &output
            .cases
            .iter()
            .find(|candidate| candidate.id == case.id)
            .unwrap_or_else(|| panic!("{}: oracle exists", case.id))
            .assert;
        assert_eq!(assertion.method, "image", "{}: assertion method", case.id);
        assert_eq!(
            assertion.raw_kind, "bytes",
            "{}: exact byte assertion",
            case.id
        );

        let image = create_input(&root, &case);
        let actual = if case.params.args.is_empty() {
            image.tobytes().expect("public bytes materialize")
        } else {
            image
                .tobytes_encoded(&case.mode, &case.params.args[0], &case.params.args[1..])
                .expect("encoded public bytes materialize")
        };
        assert_eq!(
            actual,
            fs::read(
                root.join("tests/fixtures/outputs")
                    .join(&assertion.reference)
            )
            .expect("raw Pillow oracle exists"),
            "{}: exact public bytes",
            case.id
        );
    }
}
