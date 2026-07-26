use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
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

fn oracle_site_packages() -> Vec<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for imagingft tests must be discoverable");
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
        .expect("repo root for imagingft tests must be discoverable");
    let expected = repo_root.join(".oracle-venv/bin/python");

    let candidate = if let Some(env_path) = std::env::var_os("IMAGINGFT_ORACLE_PYTHON") {
        let path = PathBuf::from(env_path);
        if path.starts_with(&repo_root)
            && path.ends_with(".oracle-venv/bin/python")
            && path.exists()
        {
            path
        } else {
            expected.clone()
        }
    } else {
        expected.clone()
    };

    if !candidate.exists() {
        panic!("IMAGINGFT_ORACLE_PYTHON must exist: {candidate:?}");
    }
    assert!(
        candidate.ends_with(".oracle-venv/bin/python"),
        "IMAGINGFT_ORACLE_PYTHON must point to .oracle-venv/bin/python: {candidate:?}"
    );
    candidate
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
            let required_keys = BTreeSet::from(["case_id", "inputs", "operation"]);
            let supported_keys = {
                let mut allowed = required_keys.clone();
                allowed.insert("expect_error");
                allowed
            };
            assert!(
                keys == required_keys || keys == supported_keys,
                "{} must contain only case_id, inputs, and operation",
                path.display()
            );
            cases.push(case.clone());
        }
    }
    cases
}

fn run_oracle(cases: &[Value]) -> BTreeMap<String, Value> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/imagingft_oracle.py");
    let oracle = oracle_python();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for imagingft tests must be discoverable");
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

fn assert_exact_oracle_match(case_id: &str, input_case: &Value, expected: &Value, actual: &Value) {
    let expect_error = input_case
        .get("expect_error")
        .and_then(Value::as_bool);
    let expected_status = expected
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{case_id}: missing status in oracle payload"));
    let actual_status = actual
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{case_id}: missing status in rust payload"));

    if let Some(expect_error) = expect_error {
        if expect_error {
            assert_eq!(
                expected_status, "error",
                "{case_id}: expect_error=true must map to an error status"
            );
        } else {
            assert_eq!(
                expected_status, "ok",
                "{case_id}: expect_error=false must map to ok status"
            );
        }
    }

    assert_eq!(
        expected_status, actual_status,
        "{case_id}: status mismatch between rust and live oracle"
    );

    if expected_status == "error" {
        let expected_error = expected
            .get("error")
            .expect("oracle error payload must include error");
        let actual_error = actual
            .get("error")
            .expect("rust error payload must include error");
        let expected_kind = expected_error
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: oracle error missing kind"));
        let actual_kind = actual_error
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: rust error missing kind"));
        assert_eq!(expected_kind, actual_kind, "{case_id}: error kind mismatch");

        let expected_message = expected_error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: oracle error missing message"));
        let actual_message = actual_error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{case_id}: rust error missing message"));
        assert_eq!(
            expected_message, actual_message,
            "{case_id}: error message mismatch"
        );
        return;
    }

    assert_eq!(
        expected, actual,
        "{case_id}: Rust result differs from live Pillow/_imagingft oracle"
    );
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
        assert_exact_oracle_match(case_id, case, expected, &actual);
    }
}
