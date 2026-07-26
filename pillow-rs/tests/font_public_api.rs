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
    negative_operations: BTreeSet<String>,
    public_method_parameters: BTreeMap<String, ParameterCoverage>,
    required_operations: BTreeSet<String>,
}

#[derive(Debug)]
struct ParameterCoverage {
    blocked: BTreeSet<String>,
    covered: BTreeSet<String>,
}

const EXPECTED_FONT_PUBLIC_OPERATIONS: [&str; 23] = [
    "draw_text",
    "font_size",
    "font_variant",
    "get_transposed_mask",
    "get_variation_axes",
    "get_variation_names",
    "getbbox",
    "getbbox_binary",
    "getlength",
    "getmask",
    "getmask2",
    "getmask2_with_start",
    "getmetrics",
    "getname",
    "has_variations",
    "load_default",
    "render_text_binary",
    "set_variation_by_axes",
    "set_variation_by_name",
    "text_bbox",
    "transposed_bbox",
    "truetype",
    "validate_transposed_length",
];

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
    let negative_operations = manifest_list(&text, "negative_operations");
    let public_method_parameters = manifest_nested_list_map(&text, "public_method_parameters");
    let required_operations = manifest_list(&text, "required_operations");
    let expected_operations = EXPECTED_FONT_PUBLIC_OPERATIONS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        required_operations,
        expected_operations,
        "{} required_operations must exactly enumerate the current implemented Font public parity surface",
        path.display()
    );
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
        negative_operations,
        public_method_parameters,
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

fn manifest_nested_list_map(text: &str, section: &str) -> BTreeMap<String, ParameterCoverage> {
    let mut values: BTreeMap<String, ParameterCoverage> = BTreeMap::new();
    let mut in_section = false;
    let mut current_method: Option<String> = None;
    let mut current_key: Option<String> = None;

    for line in text.lines() {
        let without_comment = line.split_once('#').map_or(line, |(line, _)| line);
        if without_comment.trim().is_empty() {
            continue;
        }
        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let trimmed = without_comment.trim();
        if trimmed == format!("{section}:") {
            in_section = true;
            current_method = None;
            current_key = None;
            continue;
        }
        if in_section && indent == 0 && !trimmed.starts_with('-') {
            break;
        }
        if !in_section {
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            let method = trimmed.trim_end_matches(':').to_owned();
            values
                .entry(method.clone())
                .or_insert_with(|| ParameterCoverage {
                    blocked: BTreeSet::new(),
                    covered: BTreeSet::new(),
                });
            current_method = Some(method);
            current_key = None;
            continue;
        }
        if indent == 4 && trimmed.ends_with(':') {
            current_key = Some(trimmed.trim_end_matches(':').to_owned());
            continue;
        }
        if indent == 4 && trimmed.ends_with("[]") {
            let Some(method) = current_method.as_ref() else {
                panic!("{section} list key without method: {trimmed}");
            };
            let key = trimmed
                .split_once(':')
                .map(|(key, _)| key.trim())
                .unwrap_or_else(|| panic!("{section} malformed inline empty list: {trimmed}"));
            match key {
                "blocked" | "covered" => {}
                other => panic!("{section}.{method} has unsupported key {other}"),
            }
            current_key = None;
            continue;
        }
        if indent == 6 && trimmed.starts_with('-') {
            let Some(method) = current_method.as_ref() else {
                panic!("{section} list item without method: {trimmed}");
            };
            let Some(key) = current_key.as_deref() else {
                panic!("{section}.{method} list item without key: {trimmed}");
            };
            let value = trimmed
                .strip_prefix('-')
                .expect("nested manifest list item starts with dash")
                .trim()
                .trim_matches('"')
                .to_owned();
            assert!(!value.is_empty(), "{section}.{method}.{key} item is empty");
            let coverage = values
                .get_mut(method)
                .expect("current method must have parameter coverage");
            match key {
                "blocked" => {
                    coverage.blocked.insert(value);
                }
                "covered" => {
                    coverage.covered.insert(value);
                }
                other => panic!("{section}.{method} has unsupported key {other}"),
            }
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

fn assert_case_ids_are_unique(cases: &[Value]) {
    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();
    for case in cases {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .expect("font public-api case_id must be a string");
        if !seen.insert(case_id.to_owned()) {
            duplicates.push(case_id.to_owned());
        }
    }
    assert!(
        duplicates.is_empty(),
        "font public-api case_id values must be unique: {duplicates:?}"
    );
}

fn assert_referenced_assets_exist(fixture_root: &Path, cases: &[Value]) {
    let canonical_root = fixture_root
        .canonicalize()
        .expect("font fixture root must be canonicalizable");
    for case in cases {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .expect("font public-api case_id must be a string");
        let Some(assets) = case
            .get("inputs")
            .and_then(|inputs| inputs.get("assets"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (asset_name, asset) in assets {
            let Some(kind) = asset.get("kind").and_then(Value::as_str) else {
                panic!("{case_id}.{asset_name}: font asset kind must be a string");
            };
            match kind {
                "load_default" => {
                    assert!(
                        asset.get("id").is_none(),
                        "{case_id}.{asset_name}: load_default assets must not have an id"
                    );
                }
                "ref" => {
                    let id = asset
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| panic!("{case_id}.{asset_name}: ref asset id missing"));
                    let path = fixture_root.join(id);
                    if case_id == "font.load_failure.missing_font_asset" {
                        assert!(
                            !path.exists(),
                            "{case_id}.{asset_name}: missing-asset negative row must reference an absent file"
                        );
                        continue;
                    }
                    let canonical_path = path.canonicalize().unwrap_or_else(|error| {
                        panic!(
                            "{case_id}.{asset_name}: referenced asset `{}` must exist: {error}",
                            path.display()
                        )
                    });
                    assert!(
                        canonical_path.starts_with(&canonical_root),
                        "{case_id}.{asset_name}: referenced asset must stay under fixture root: {}",
                        path.display()
                    );
                    assert!(
                        canonical_path.is_file(),
                        "{case_id}.{asset_name}: referenced asset must be a file: {}",
                        path.display()
                    );
                }
                other => panic!("{case_id}.{asset_name}: unsupported font asset kind {other}"),
            }
        }
    }
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

fn pillow_freetypefont_public_methods() -> BTreeSet<String> {
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
        .arg("--public-methods")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("VIRTUAL_ENV", venv_root)
        .env("PYTHONNOUSERSITE", "1")
        .env(
            "PYTHONPATH",
            env::join_paths(oracle_site_packages()).expect("valid PYTHONPATH join"),
        );
    let output = command
        .output()
        .expect("the pinned Pillow font oracle method query must finish");
    assert!(
        output.status.success(),
        "Pillow font oracle method query failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Vec<String>>(&output.stdout)
        .expect("oracle public method output must be a string list")
        .into_iter()
        .collect()
}

fn pillow_freetypefont_public_signatures() -> BTreeMap<String, BTreeSet<String>> {
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
        .arg("--public-signatures")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("VIRTUAL_ENV", venv_root)
        .env("PYTHONNOUSERSITE", "1")
        .env(
            "PYTHONPATH",
            env::join_paths(oracle_site_packages()).expect("valid PYTHONPATH join"),
        );
    let output = command
        .output()
        .expect("the pinned Pillow font oracle signature query must finish");
    assert!(
        output.status.success(),
        "Pillow font oracle signature query failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<BTreeMap<String, Vec<String>>>(&output.stdout)
        .expect("oracle public signature output must be a method-to-parameter map")
        .into_iter()
        .map(|(method, parameters)| (method, parameters.into_iter().collect()))
        .collect()
}

fn observed_public_method_parameters(cases: &[Value]) -> BTreeMap<String, BTreeSet<String>> {
    let mut observed = BTreeMap::new();
    for case in cases {
        let operation = font_runner::operation(case).expect("case operation must be valid");
        let Some(params) = case
            .get("inputs")
            .and_then(|inputs| inputs.get("params"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        let entry = observed
            .entry(operation.to_owned())
            .or_insert_with(BTreeSet::new);
        for key in params.keys() {
            if let Some(parameter) = canonical_pillow_parameter(operation, key) {
                entry.insert(parameter);
            }
        }
        if operation == "font_variant"
            && case
                .get("inputs")
                .and_then(|inputs| inputs.get("assets"))
                .and_then(|assets| assets.get("variant_font"))
                .is_some()
        {
            entry.insert("font".to_owned());
        }
    }
    observed
}

fn canonical_pillow_parameter(operation: &str, fixture_key: &str) -> Option<String> {
    let parameter = match fixture_key {
        "size" => return None,
        "text" | "text_bytes_hex"
            if matches!(operation, "getbbox" | "getlength" | "getmask" | "getmask2") =>
        {
            "text"
        }
        "text" | "text_bytes_hex" => return None,
        "variant_size" if operation == "font_variant" => "size",
        "variant_index" if operation == "font_variant" => "index",
        "variant_encoding" if operation == "font_variant" => "encoding",
        "variant_layout_engine" if operation == "font_variant" => "layout_engine",
        other => other,
    };
    Some(parameter.to_owned())
}

fn assert_manifest_covers_pillow_public_signatures(
    manifest: &FontManifest,
    cases: &[Value],
    pillow_signatures: &BTreeMap<String, BTreeSet<String>>,
) {
    let observed_parameters = observed_public_method_parameters(cases);
    assert_eq!(
        manifest
            .public_method_parameters
            .keys()
            .collect::<BTreeSet<_>>(),
        pillow_signatures.keys().collect::<BTreeSet<_>>(),
        "font_manifest.yaml public_method_parameters must enumerate every live Pillow FreeTypeFont public method exactly"
    );
    for (method, pillow_parameters) in pillow_signatures {
        let coverage = manifest
            .public_method_parameters
            .get(method)
            .unwrap_or_else(|| panic!("{method}: missing public_method_parameters entry"));
        assert!(
            coverage.covered.is_disjoint(&coverage.blocked),
            "{method}: covered and blocked parameter sets must be disjoint"
        );
        let classified = coverage
            .covered
            .union(&coverage.blocked)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            &classified, pillow_parameters,
            "{method}: manifest must classify every live Pillow public parameter as covered or blocked"
        );

        let observed = observed_parameters.get(method).cloned().unwrap_or_default();
        let non_pillow_observed = observed.difference(pillow_parameters).collect::<Vec<_>>();
        assert!(
            non_pillow_observed.is_empty(),
            "{method}: active input rows contain canonical parameters that are not in the live Pillow public signature: {non_pillow_observed:?}"
        );
        let missing_rows = coverage.covered.difference(&observed).collect::<Vec<_>>();
        assert!(
            missing_rows.is_empty(),
            "{method}: manifest marks parameters as covered but active input rows do not exercise them: {missing_rows:?}"
        );
        let blocked_rows = coverage.blocked.intersection(&observed).collect::<Vec<_>>();
        assert!(
            blocked_rows.is_empty(),
            "{method}: active input rows exercise parameters still marked blocked in font_manifest.yaml: {blocked_rows:?}"
        );
    }
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
    assert_case_ids_are_unique(&cases);
    assert_referenced_assets_exist(&root, &cases);

    let pillow_methods = pillow_freetypefont_public_methods();
    let missing_pillow_methods = pillow_methods
        .iter()
        .filter(|operation| !manifest.required_operations.contains(operation.as_str()))
        .collect::<Vec<_>>();
    assert!(
        missing_pillow_methods.is_empty(),
        "font_manifest.yaml required_operations must include every live Pillow FreeTypeFont public method: {missing_pillow_methods:?}"
    );
    let pillow_signatures = pillow_freetypefont_public_signatures();
    assert_manifest_covers_pillow_public_signatures(&manifest, &cases, &pillow_signatures);

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
    let allowed_operations = manifest
        .required_operations
        .union(&manifest.negative_operations)
        .cloned()
        .collect::<BTreeSet<_>>();
    let unclassified = observed
        .iter()
        .filter(|operation| !allowed_operations.contains(**operation))
        .collect::<Vec<_>>();
    assert!(
        unclassified.is_empty(),
        "font public-api inputs contain operations missing from required_operations/negative_operations: {unclassified:?}"
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
