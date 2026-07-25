#![allow(missing_docs, unused_crate_dependencies)]

use pillow_rs::error::PilError;
use pillow_rs::ops::array::resolve_array_layout;
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    shape: Vec<usize>,
    typestr: String,
    mode: Option<String>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    mode: Option<String>,
    size: Option<[usize; 2]>,
    error: Option<ExpectedError>,
}

#[derive(Deserialize)]
struct ExpectedError {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

#[test]
fn descriptor_resolution_matches_pillow_oracle() {
    let manifest: Manifest =
        serde_json::from_str(include_str!("fixtures/fromarray/descriptor.json"))
            .expect("valid Pillow descriptor oracle");
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");

    for case in manifest.cases {
        let actual = resolve_array_layout(&case.shape, &case.typestr, case.mode.as_deref());
        match (actual, case.expected.error) {
            (Ok(layout), None) => {
                assert_eq!(Some(layout.mode), case.expected.mode, "{} mode", case.id);
                assert_eq!(
                    Some([layout.width, layout.height]),
                    case.expected.size,
                    "{} size",
                    case.id
                );
            }
            (Err(error), Some(expected)) => {
                let kind = match error {
                    PilError::TypeError(_) => "TypeError",
                    PilError::ValueError(_) => "ValueError",
                    _ => panic!("{} unexpected error category", case.id),
                };
                assert_eq!(kind, expected.kind, "{} error type", case.id);
                assert_eq!(
                    error.to_string(),
                    expected.message,
                    "{} error message",
                    case.id
                );
            }
            _ => panic!("{} descriptor contract mismatch", case.id),
        }
    }
}
