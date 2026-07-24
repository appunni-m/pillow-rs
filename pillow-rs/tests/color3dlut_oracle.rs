//! Exact Rust API execution of the shared Pillow Color3DLUT oracle cases.
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::error::PilError;
use pillow_rs::image::Image;
use serde::Deserialize;

#[derive(Deserialize)]
struct InputFixture {
    cases: Vec<InputCase>,
}

#[derive(Deserialize)]
struct OutputFixture {
    cases: Vec<OutputCase>,
}

#[derive(Deserialize)]
struct InputCase {
    id: String,
    mode: String,
    input: InputSpec,
    params: LutParams,
}

#[derive(Deserialize)]
struct InputSpec {
    source: String,
    size: [u32; 2],
}

#[derive(Deserialize)]
#[serde(untagged)]
enum LutSize {
    Uniform(u32),
    Dimensions([u32; 3]),
}

impl LutSize {
    fn dimensions(&self) -> (u32, u32, u32) {
        match self {
            Self::Uniform(size) => (*size, *size, *size),
            Self::Dimensions([x, y, z]) => (*x, *y, *z),
        }
    }
}

#[derive(Deserialize)]
struct LutParams {
    size: LutSize,
    channels: u32,
    target_mode: Option<String>,
    #[serde(rename = "_table_pattern")]
    table_pattern: String,
}

#[derive(Deserialize)]
struct OutputCase {
    id: String,
    assert: OracleAssertion,
}

#[derive(Deserialize)]
#[serde(tag = "method")]
enum OracleAssertion {
    #[serde(rename = "image")]
    Image {
        reference: String,
        raw_kind: Option<String>,
        mode: Option<String>,
        size: Option<[u32; 2]>,
    },
    #[serde(rename = "error")]
    Error { exception: String, message: String },
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pillow-rs crate has a workspace parent")
        .to_owned()
}

fn identity_table(size: (u32, u32, u32), channels: u32) -> Vec<f64> {
    let mut table = Vec::with_capacity((size.0 * size.1 * size.2 * channels) as usize);
    for z in 0..size.2 {
        for y in 0..size.1 {
            for x in 0..size.0 {
                let values = [
                    f64::from(x) / f64::from(size.0 - 1),
                    f64::from(y) / f64::from(size.1 - 1),
                    f64::from(z) / f64::from(size.2 - 1),
                    f64::from(x + 2 * y + 3 * z) / f64::from(6 * (size.0 - 1)),
                ];
                table.extend_from_slice(&values[..channels as usize]);
            }
        }
    }
    table
}

fn create_input(spec: &InputSpec, mode: &str) -> Image {
    assert_eq!(spec.source, "reference_rgb");
    let encoded = fs::read(repo_root().join("tests/test_reference.png"))
        .expect("shared reference image is present");
    Image::open_bytes(encoded)
        .expect("shared reference image opens")
        .resize((spec.size[0], spec.size[1]), Some("LANCZOS"))
        .expect("reference image resizes")
        .convert(mode, None, None, None, None)
        .expect("reference image converts")
}

fn assert_expected_image(
    base: &Path,
    assertion: &OracleAssertion,
    actual: &Image,
    case_id: &str,
) {
    let OracleAssertion::Image {
        reference,
        raw_kind,
        mode,
        size,
    } = assertion
    else {
        panic!("{case_id}: expected an image assertion");
    };
    let actual_mode = actual.mode().expect("actual mode resolves");
    let actual_size = actual.size().expect("actual dimensions resolve");
    let actual_bytes = actual.tobytes().expect("actual pixels materialize");

    if raw_kind.as_deref() == Some("image") {
        assert_eq!(Some(actual_mode.as_str()), mode.as_deref(), "{case_id}: mode");
        assert_eq!(
            Some([actual_size.0, actual_size.1]),
            *size,
            "{case_id}: dimensions"
        );
        assert_eq!(
            actual_bytes,
            fs::read(base.join("outputs").join(reference)).expect("raw oracle is present"),
            "{case_id}: pixel bytes"
        );
        return;
    }

    let expected = Image::open_bytes(
        fs::read(base.join("outputs").join(reference)).expect("PNG oracle is present"),
    )
    .expect("PNG oracle opens");
    assert_eq!(
        actual_mode,
        expected.mode().expect("expected mode resolves"),
        "{case_id}: mode"
    );
    assert_eq!(
        actual_size,
        expected.size().expect("expected dimensions resolve"),
        "{case_id}: dimensions"
    );
    assert_eq!(
        actual_bytes,
        expected.tobytes().expect("expected pixels materialize"),
        "{case_id}: pixel bytes"
    );
}

fn assert_expected_error(assertion: &OracleAssertion, error: &PilError, case_id: &str) {
    let OracleAssertion::Error { exception, message } = assertion else {
        panic!("{case_id}: expected an error assertion");
    };
    assert_eq!(exception, "ValueError", "{case_id}: exception category");
    assert!(
        matches!(error, PilError::ValueError(_)),
        "{case_id}: Rust error category was {error:?}"
    );
    assert_eq!(&error.to_string(), message, "{case_id}: error message");
}

#[test]
fn shared_color3dlut_corpus_matches_pillow_exactly() {
    let root = repo_root().join("tests");
    for suite in ["fixtures", "fixtures_2"] {
        let base = root.join(suite);
        let input: InputFixture = serde_json::from_slice(
            &fs::read(base.join("input/jsons/ImageFilter.Color3DLUT.json"))
                .expect("input fixture is present"),
        )
        .expect("input fixture parses");
        let output: OutputFixture = serde_json::from_slice(
            &fs::read(base.join("outputs/jsons/ImageFilter.Color3DLUT.json"))
                .expect("output fixture is present"),
        )
        .expect("output fixture parses");

        for case in input.cases {
            let assertion = &output
                .cases
                .iter()
                .find(|candidate| candidate.id == case.id)
                .expect("every input case has an oracle")
                .assert;
            assert_eq!(case.params.table_pattern, "identity");
            let size = case.params.size.dimensions();
            let table = identity_table(size, case.params.channels);
            let image = create_input(&case.input, &case.mode);
            match image.color3dlut(
                size,
                table,
                case.params.channels,
                case.params.target_mode.as_deref(),
            ) {
                Ok(actual) => assert_expected_image(&base, assertion, &actual, &case.id),
                Err(error) => assert_expected_error(assertion, &error, &case.id),
            }
        }
    }
}
