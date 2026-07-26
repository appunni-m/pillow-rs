use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;

#[path = "support/font_runner.rs"]
mod font_runner;

const FORBIDDEN_INPUT_KEYS: [&str; 9] = [
    "error",
    "expect_error",
    "expected",
    "hash",
    "oracle",
    "output",
    "outputs",
    "pixels_hex",
    "status",
];

#[derive(Debug)]
struct FontManifest {
    input_dir: String,
    input_files: BTreeSet<String>,
    required_operations: BTreeSet<String>,
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font")
}

fn oracle_site_packages() -> Vec<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let lib_dir = repo_root.join(".oracle-venv/lib");
    if !lib_dir.exists() {
        return vec![];
    }

    let mut paths = Vec::new();
    let entries = match fs::read_dir(lib_dir) {
        Ok(entries) => entries,
        Err(_) => return vec![],
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.starts_with("python"))
        {
            continue;
        }
        let site = path.join("site-packages");
        if site.is_dir() {
            paths.push(site);
        }
    }
    paths
}

fn oracle_python() -> PathBuf {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let expected = repo_root.join(".oracle-venv/bin/python");

    let candidate = if let Some(env_path) = std::env::var_os("FONT_ORACLE_PYTHON") {
        let path = PathBuf::from(env_path);
        if !path.exists() {
            panic!("FONT_ORACLE_PYTHON does not exist: {path:?}");
        }
        if !path.starts_with(&repo_root) || !path.ends_with(".oracle-venv/bin/python") {
            panic!("FONT_ORACLE_PYTHON must point to .oracle-venv/bin/python: {path:?}");
        }
        path
    } else {
        expected.clone()
    };

    if !candidate.exists() {
        panic!("FONT_ORACLE_PYTHON must exist: {candidate:?}");
    }
    assert!(
        candidate.ends_with(".oracle-venv/bin/python"),
        "FONT_ORACLE_PYTHON must point to .oracle-venv/bin/python: {candidate:?}"
    );
    candidate
}

fn load_manifest(root: &Path) -> FontManifest {
    let path = root.join("font_manifest.yaml");
    let text = fs::read_to_string(&path).expect("font public-api manifest must be readable");
    assert_manifest_has_no_embedded_expectations(&path, &text);
    let input_dir = manifest_scalar(&text, "input_dir")
        .unwrap_or_else(|| panic!("{} must define input_dir", path.display()));
    let input_files = manifest_list(&text, "input_files");
    let required_operations = manifest_list(&text, "required_operations");
    assert!(
        !input_files.is_empty(),
        "{} input_files must not be empty",
        path.display()
    );
    assert!(
        !required_operations.is_empty(),
        "{} required_operations must not be empty",
        path.display()
    );
    FontManifest {
        input_dir,
        input_files,
        required_operations,
    }
}

fn manifest_scalar(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(&format!("{key}:"))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn manifest_list(text: &str, key: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let mut in_list = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == format!("{key}:") {
            in_list = true;
            continue;
        }
        if in_list && !trimmed.starts_with('-') {
            break;
        }
        if in_list {
            let value = trimmed
                .strip_prefix('-')
                .expect("manifest list item starts with dash")
                .trim()
                .trim_matches('"')
                .to_owned();
            assert!(
                !value.is_empty(),
                "font public-api manifest list item must not be empty"
            );
            values.insert(value);
        }
    }
    values
}

fn load_input_cases(directory: &Path, manifest_files: &BTreeSet<String>) -> Vec<Value> {
    let discovered_files = fs::read_dir(directory)
        .expect("public-api font input directory must be readable")
        .map(|entry| entry.expect("input entry must be readable").path())
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| name.starts_with("font.") && name.ends_with(".json"))
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        *manifest_files, discovered_files,
        "font public-api manifest input_files must exactly match raw input JSON files"
    );

    let mut cases = Vec::new();
    for file in manifest_files {
        let path = directory.join(file.as_str());
        let document: Value = serde_json::from_slice(
            &fs::read(&path).expect("font public-api input must be readable"),
        )
        .expect("font public-api input must be valid JSON");
        assert_input_only_case(&path, &document);
        let rows = document
            .get("cases")
            .and_then(Value::as_array)
            .expect("font public-api input must contain a cases array");
        for case in rows {
            let object = case
                .as_object()
                .expect("each font public-api case must be an object");
            let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
            let required_keys = BTreeSet::from(["case_id", "inputs", "operation"]);
            assert!(
                keys == required_keys,
                "{} must contain only case_id, inputs, and operation",
                path.display()
            );
            assert_input_only_case(&path, case);
            cases.push(case.clone());
        }
    }
    cases
}

fn assert_manifest_has_no_embedded_expectations(path: &Path, text: &str) {
    for (index, line) in text.lines().enumerate() {
        let Some((key, _)) = line.trim().split_once(':') else {
            continue;
        };
        assert!(
            !matches!(
                key,
                "error"
                    | "expect_error"
                    | "expectation"
                    | "expected"
                    | "hash"
                    | "output"
                    | "outputs"
                    | "pixels_hex"
                    | "raw_path"
                    | "sha256"
                    | "status"
            ),
            "{}:{} must not embed oracle output/error expectation key `{}`",
            path.display(),
            index + 1,
            key
        );
    }
}

fn assert_input_only_case(path: &Path, value: &Value) {
    match value {
        Value::Object(object) => {
            for key in object.keys() {
                assert!(
                    !FORBIDDEN_INPUT_KEYS.contains(&key.as_str()),
                    "{} must be input-only; forbidden oracle/output key `{}` found",
                    path.display(),
                    key
                );
            }
            for child in object.values() {
                assert_input_only_case(path, child);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_input_only_case(path, child);
            }
        }
        _ => {}
    }
}

fn run_oracle(cases: &[Value]) -> BTreeMap<String, Value> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/font_oracle.py");
    let oracle = oracle_python();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let venv_root = repo_root.join(".oracle-venv");
    let mut command = Command::new(oracle.as_os_str());
    command
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("VIRTUAL_ENV", venv_root);
    command.env("PYTHONNOUSERSITE", "1");
    command.env(
        "PYTHONPATH",
        env::join_paths(oracle_site_packages()).expect("valid PYTHONPATH join"),
    );
    let mut child = command
        .spawn()
        .expect("the pinned Pillow font oracle must start");

    child
        .stdin
        .take()
        .expect("oracle stdin must be available")
        .write_all(&serde_json::to_vec(cases).expect("input-only font cases must serialize"))
        .expect("input-only font cases must be sent to the oracle");

    let output = child
        .wait_with_output()
        .expect("the Pillow font oracle must finish");
    assert!(
        output.status.success(),
        "Pillow font oracle failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle output must be a case-id result map")
}

fn assert_exact_oracle_match(case_id: &str, expected: &Value, actual: &Value) {
    let expected_status = expected
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{case_id}: missing status in oracle payload"));
    let actual_status = actual
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{case_id}: missing status in rust payload"));

    assert_eq!(
        expected_status, actual_status,
        "{case_id}: status mismatch between rust and live oracle"
    );

    assert_eq!(
        expected, actual,
        "{case_id}: Rust result differs from live Pillow Font oracle"
    );
}

#[test]
fn every_input_matches_the_live_pillow_font_oracle_exactly() {
    let root = fixture_root();
    let manifest = load_manifest(&root);
    let input_dir = root.join(&manifest.input_dir);
    let cases = load_input_cases(&input_dir, &manifest.input_files);
    assert!(
        !cases.is_empty(),
        "font public-api input corpus must not be empty"
    );

    let observed = cases
        .iter()
        .map(|case| font_runner::operation(case).expect("case operation must be valid"))
        .collect::<BTreeSet<_>>();
    let missing = manifest
        .required_operations
        .iter()
        .filter(|operation| !observed.contains(operation.as_str()))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "required font public operations missing from inputs: {missing:?}"
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
        let actual = font_runner::run(case, &root);
        assert_exact_oracle_match(case_id, expected, &actual);
    }
}
