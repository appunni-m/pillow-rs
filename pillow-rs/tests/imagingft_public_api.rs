use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;

#[path = "support/imagingft_runner.rs"]
mod imagingft_runner;

const REQUIRED_PUBLIC_OPS: [&str; 14] = [
    "getname",
    "getmetrics",
    "getlength",
    "has_variations",
    "getbbox",
    "getbbox_binary",
    "getmask",
    "getmask2",
    "getmask2_with_start",
    "get_transposed_mask",
    "transposed_bbox",
    "validate_transposed_length",
    "draw_text",
    "render_text_binary",
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/imagingft")
}

fn oracle_python() -> PathBuf {
    let path = std::env::var_os("IMAGINGFT_ORACLE_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../.oracle-venv/bin/python")
        });
    assert!(path.exists(), "repo-local IMAGINGFT_ORACLE_PYTHON must exist: {path:?}");
    path
}

fn load_input_cases(directory: &Path) -> Vec<Value> {
    let mut paths = fs::read_dir(directory)
        .expect("public-api imagingft input directory must be readable")
        .map(|entry| entry.expect("input entry must be readable").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("imagingft.") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut cases = Vec::new();
    for path in paths {
        let document: Value = serde_json::from_slice(
            &fs::read(&path).expect("imagingft public-api input must be readable"),
        )
        .expect("imagingft public-api input must be valid JSON");
        let rows = document
            .get("cases")
            .and_then(Value::as_array)
            .expect("imagingft public-api input must contain a cases array");
        for case in rows {
            let object = case
                .as_object()
                .expect("each imagingft public-api case must be an object");
            let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
            assert_eq!(
                keys,
                BTreeSet::from(["case_id", "inputs", "operation"]),
                "{} must contain input fields only",
                path.display()
            );
            cases.push(case.clone());
        }
    }
    cases
}

fn run_oracle(cases: &[Value]) -> BTreeMap<String, Value> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/imagingft_oracle.py");
    let mut child = Command::new(oracle_python())
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the pinned Pillow imagingft oracle must start");

    child
        .stdin
        .take()
        .expect("oracle stdin must be available")
        .write_all(&serde_json::to_vec(cases).expect("input-only imagingft cases must serialize"))
        .expect("input-only imagingft cases must be sent to the oracle");

    let output = child
        .wait_with_output()
        .expect("the Pillow imagingft oracle must finish");
    assert!(
        output.status.success(),
        "Pillow imagingft oracle failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle output must be a case-id result map")
}

#[test]
fn every_input_matches_the_live_pillow_imagingft_oracle_exactly() {
    let root = fixture_root();
    let cases = load_input_cases(&root.join("inputs/public-api"));
    assert!(
        !cases.is_empty(),
        "imagingft public-api input corpus must not be empty"
    );

    let observed = cases
        .iter()
        .map(|case| imagingft_runner::operation(case).expect("case operation must be valid"))
        .collect::<BTreeSet<_>>();
    let missing = REQUIRED_PUBLIC_OPS
        .into_iter()
        .filter(|operation| !observed.contains(*operation))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "required imagingft public operations missing from inputs: {missing:?}"
    );

    let oracle = run_oracle(&cases);
    assert_eq!(
        oracle.len(),
        cases.len(),
        "oracle must return exactly one result per input"
    );

    for case in &cases {
        let case_id = case["case_id"].as_str().expect("case_id must be a string");
        let expected = oracle
            .get(case_id)
            .unwrap_or_else(|| panic!("{case_id}: live Pillow oracle result missing"));
        let actual = imagingft_runner::run(case, &root);
        assert_eq!(
            &actual, expected,
            "{case_id}: Rust result differs from live Pillow/_imagingft oracle"
        );
    }
}
