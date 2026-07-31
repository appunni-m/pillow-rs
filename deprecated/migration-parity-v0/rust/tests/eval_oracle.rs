//! Exact Rust API execution of the shared Pillow Image.eval oracle cases.
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::Image;

use serde::Deserialize;
use serde_json::Value;

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
    params: EvalParams,
}

#[derive(Deserialize)]
struct InputSpec {
    source: String,
    size: [u32; 2],
    reference: Option<String>,
    color: Option<Value>,
}

#[derive(Deserialize)]
struct EvalParams {
    function: String,
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
        palette: Option<Vec<u8>>,
    },
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pillow-rs crate has a workspace parent")
        .to_owned()
}

fn input_color(mode: &str, value: Option<&Value>) -> (u8, u8, u8, u8) {
    let Some(value) = value else {
        return (0, 0, 0, 0);
    };
    if let Some(single) = value.as_u64() {
        return (single as u8, 0, 0, 0);
    }
    let values = value.as_array().expect("fixture color is scalar or array");
    let channel = |index: usize, default: u8| {
        values
            .get(index)
            .and_then(Value::as_u64)
            .map_or(default, |item| item as u8)
    };
    match values.len() {
        2 => {
            let l = channel(0, 0);
            (l, l, l, channel(1, 0))
        }
        3 => (channel(0, 0), channel(1, 0), channel(2, 0), 255),
        4 => (channel(0, 0), channel(1, 0), channel(2, 0), channel(3, 0)),
        _ => panic!("unsupported {mode} fixture color"),
    }
}

fn create_input(base: &Path, spec: &InputSpec, mode: &str) -> Image {
    match spec.source.as_str() {
        "reference_rgb" => {
            let path = if let Some(reference) = spec.reference.as_deref() {
                base.join("input/images").join(format!("{reference}.png"))
            } else {
                repo_root().join("tests/test_reference.png")
            };
            let source = Image::open_bytes(fs::read(path).expect("reference image is present"))
                .expect("reference image opens");
            let size = source.size().expect("reference dimensions resolve");
            let resized = if size == (spec.size[0], spec.size[1]) {
                source
            } else {
                source
                    .resize((spec.size[0], spec.size[1]), Some("LANCZOS"))
                    .expect("reference image resizes")
            };
            resized
                .convert(mode, None, None, None, None)
                .expect("reference image converts")
        }
        "constant" if mode == "P" && spec.color.as_ref().is_some_and(Value::is_number) => {
            Image::new_palette_index(
                spec.size[0],
                spec.size[1],
                spec.color
                    .as_ref()
                    .and_then(Value::as_u64)
                    .expect("P scalar exists") as u8,
            )
        }
        "constant" => Image::new(
            spec.size[0],
            spec.size[1],
            mode,
            input_color(mode, spec.color.as_ref()),
        )
        .expect("constant image constructs"),
        source => panic!("unsupported fixture input source {source}"),
    }
}

fn assert_expected(base: &Path, assertion: &OracleAssertion, actual: &Image, case_id: &str) {
    let OracleAssertion::Image {
        reference,
        raw_kind,
        mode,
        size,
        palette,
    } = assertion;
    let actual_mode = actual.mode().expect("actual mode resolves");
    let actual_size = actual.size().expect("actual dimensions resolve");
    let actual_bytes = actual.tobytes().expect("actual pixels materialize");

    if raw_kind.as_deref() == Some("image") {
        assert_eq!(
            Some(actual_mode.as_str()),
            mode.as_deref(),
            "{case_id}: mode"
        );
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
        if let Some(expected_palette) = palette {
            assert_eq!(
                actual.getpalette_trimmed().unwrap_or_default(),
                *expected_palette,
                "{case_id}: palette"
            );
        }
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

#[test]
fn shared_eval_corpus_matches_pillow_exactly() {
    let root = repo_root().join("tests");
    let mut case_count = 0;
    for suite in ["fixtures", "fixtures_2"] {
        let base = root.join(suite);
        let input: InputFixture = serde_json::from_slice(
            &fs::read(base.join("input/jsons/ImageModule.eval.json"))
                .expect("input fixture is present"),
        )
        .expect("input fixture parses");
        let output: OutputFixture = serde_json::from_slice(
            &fs::read(base.join("outputs/jsons/ImageModule.eval.json"))
                .expect("output fixture is present"),
        )
        .expect("output fixture parses");

        for case in input.cases {
            case_count += 1;
            assert_eq!(case.params.function, "add_10");
            let assertion = &output
                .cases
                .iter()
                .find(|candidate| candidate.id == case.id)
                .expect("every input case has an oracle")
                .assert;
            let image = create_input(&base, &case.input, &case.mode);
            let lut: Vec<u8> = (0..=255u8).map(|value| value.saturating_add(10)).collect();
            let actual = pillow_rs::image_eval_replicated_for_image(&image, &lut)
                .expect("Image.eval succeeds");
            assert_expected(&base, assertion, &actual, &case.id);
        }
    }
    assert_eq!(case_count, 14, "shared Image.eval case count");
    println!("{case_count} exact Image.eval Pillow oracle cases passed");
}
