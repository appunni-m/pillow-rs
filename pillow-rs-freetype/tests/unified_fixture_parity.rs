#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::mem::{align_of, offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::Command;

use fontdone::ffi::*;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug)]
struct Manifest {
    subjects: BTreeMap<String, BTreeSet<String>>,
    font_variability: BTreeMap<(String, String), FontVariability>,
}

#[derive(Debug, Default)]
struct FontVariability {
    folder: String,
    sizes: Vec<u32>,
    char_codes: Vec<u64>,
    load_flags: Vec<i32>,
    render_modes: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct CaseFile {
    cases: Vec<InputCase>,
}

#[derive(Debug, Deserialize)]
struct InputCase {
    case_id: String,
    subject: String,
    case: String,
    operation: String,
    schema: String,
    #[serde(default)]
    expect_error: bool,
    inputs: Inputs,
}

#[derive(Debug, Deserialize)]
struct Inputs {
    #[serde(default)]
    assets: BTreeMap<String, Asset>,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum Asset {
    #[serde(rename = "file")]
    File {
        path: String,
        sha256: String,
        length: u64,
    },
    #[serde(rename = "inline_bytes")]
    InlineBytes { encoding: String, value: String },
}

#[derive(Debug)]
struct RunOutput {
    status: Status,
    output: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Status {
    kind: StatusKind,
    error_code: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    Ok,
    Error,
}

#[test]
fn unified_fixture_cases_match_runtime_c_oracle() {
    let manifest = read_manifest();
    let cases = read_all_case_files();
    let mut passed = 0usize;
    let mut failures = Vec::new();
    let mut covered = BTreeSet::new();

    for case in &cases {
        if !manifest.has_case(&case.subject, &case.case) {
            failures.push(format!(
                "{} references unknown manifest case {}::{}",
                case.case_id, case.subject, case.case
            ));
            continue;
        }
        if let Err(err) = validate_assets(case) {
            failures.push(format!("{} asset validation failed: {err}", case.case_id));
            continue;
        }

        let oracle = match run_oracle(case) {
            Ok(output) => output,
            Err(err) => {
                failures.push(format!("{} oracle failed: {err}", case.case_id));
                continue;
            }
        };
        let actual = match run_rust_ffi(case) {
            Ok(output) => output,
            Err(err) => {
                failures.push(format!("{} rust backend failed: {err}", case.case_id));
                continue;
            }
        };

        match compare_case(case, &oracle, &actual) {
            Ok(()) => {
                passed += 1;
                covered.insert((case.subject.clone(), case.case.clone()));
            }
            Err(err) => failures.push(err),
        }
    }

    eprintln!(
        "unified_fixture_cases: {}/{} passed, {} manifest cases covered",
        passed,
        cases.len(),
        covered.len()
    );
    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("{failure}");
        }
        panic!("{} unified fixture cases failed", failures.len());
    }
}

#[test]
fn manifest_exhaustively_lists_current_ffi_surface() {
    let manifest = read_manifest();
    let expected = discover_current_ffi_surface();

    let manifest_subjects = manifest.subjects.keys().cloned().collect::<BTreeSet<_>>();
    let missing = expected
        .difference(&manifest_subjects)
        .cloned()
        .collect::<Vec<_>>();
    let extra = manifest_subjects
        .difference(&expected)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "manifest is missing FFI subjects: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "manifest has subjects not exported by fontdone::ffi: {extra:?}"
    );

    for case in read_all_case_files() {
        assert!(
            manifest.has_case(&case.subject, &case.case),
            "{} references unknown manifest case {}::{}",
            case.case_id,
            case.subject,
            case.case
        );
    }
}

#[test]
fn manifest_font_variability_cases_cover_declared_fixture_folder() {
    let manifest = read_manifest();
    let cases = read_all_case_files();
    let mut failures = Vec::new();

    for ((subject, case_id), variability) in &manifest.font_variability {
        let fonts = fixture_fonts(&variability.folder);
        assert!(
            !fonts.is_empty(),
            "{}::{} declares empty font variability folder {}",
            subject,
            case_id,
            variability.folder
        );

        for font in &fonts {
            for size in &variability.sizes {
                let char_codes = coverage_values(&variability.char_codes);
                let load_flags = coverage_values(&variability.load_flags);
                let render_modes = coverage_values(&variability.render_modes);
                for char_code in &char_codes {
                    for load_flag in &load_flags {
                        for render_mode in &render_modes {
                            let probe = CoverageProbe {
                                subject,
                                case_id,
                                font,
                                size: *size,
                                char_code: *char_code,
                                load_flag: *load_flag,
                                render_mode: *render_mode,
                            };
                            if !cases
                                .iter()
                                .any(|input| input_covers_font_variability(input, &probe))
                            {
                                failures.push(format!(
                                    "{}::{} missing font={} size={} char_code={:?} load_flags={:?} render_mode={:?}",
                                    subject,
                                    case_id,
                                    font,
                                    size,
                                    char_code,
                                    load_flag,
                                    render_mode
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        !manifest.font_variability.is_empty(),
        "manifest declares no font variability coverage requirements"
    );
    assert!(
        failures.is_empty(),
        "font variability coverage gaps:\n{}",
        failures.join("\n")
    );
}

fn discover_current_ffi_surface() -> BTreeSet<String> {
    let src = manifest_dir().join("src").join("ffi");
    let mut subjects = BTreeSet::new();
    collect_pub_items(&src.join("constants.rs"), "freetype", &mut subjects);
    collect_pub_items(&src.join("handles.rs"), "freetype", &mut subjects);
    collect_pub_items(&src.join("types.rs"), "freetype", &mut subjects);
    collect_pub_items(&src.join("convert.rs"), "fontdone.ffi", &mut subjects);
    subjects
}

fn collect_pub_items(path: &Path, prefix: &str, subjects: &mut BTreeSet<String>) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    for line in text.lines().map(str::trim) {
        let name = if let Some(rest) = line.strip_prefix("pub const ") {
            rest.split(':').next()
        } else if let Some(rest) = line.strip_prefix("pub type ") {
            rest.split('=').next()
        } else if let Some(rest) = line.strip_prefix("pub struct ") {
            rest.split([' ', '{', '(']).next()
        } else if let Some(rest) = line.strip_prefix("pub fn ") {
            rest.split('(').next()
        } else {
            None
        };
        if let Some(name) = name {
            subjects.insert(format!("{prefix}.{}", name.trim()));
        }
    }
}

impl Manifest {
    fn has_case(&self, subject: &str, case: &str) -> bool {
        self.subjects
            .get(subject)
            .is_some_and(|cases| cases.contains(case))
    }
}

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn fixture_dir() -> PathBuf {
    manifest_dir().join("tests").join("fixtures")
}

fn read_manifest() -> Manifest {
    let text = fs::read_to_string(manifest_dir().join("tests").join("manifest.yaml"))
        .expect("read manifest.yaml");
    parse_manifest(&text)
}

fn parse_manifest(text: &str) -> Manifest {
    let mut subjects = BTreeMap::<String, BTreeSet<String>>::new();
    let mut font_variability = BTreeMap::<(String, String), FontVariability>::new();
    let mut current_subject: Option<String> = None;
    let mut current_case: Option<String> = None;
    let mut in_cases = false;
    let mut in_font_variability = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if raw_line.starts_with("  - id: ") {
            let id = line.trim_start_matches("- id: ").to_string();
            subjects.entry(id.clone()).or_default();
            current_subject = Some(id);
            current_case = None;
            in_cases = false;
            in_font_variability = false;
        } else if raw_line.starts_with("    cases:") {
            in_cases = true;
            in_font_variability = false;
        } else if in_cases && raw_line.starts_with("      - id: ") {
            let case = line.trim_start_matches("- id: ").to_string();
            let subject = current_subject
                .as_ref()
                .expect("case entry appears before subject");
            subjects.entry(subject.clone()).or_default().insert(case);
            current_case = Some(line.trim_start_matches("- id: ").to_string());
            in_font_variability = false;
        } else if in_cases && raw_line.starts_with("          font_variability:") {
            let subject = current_subject
                .as_ref()
                .expect("font_variability appears before subject");
            let case = current_case
                .as_ref()
                .expect("font_variability appears before case");
            font_variability
                .entry((subject.clone(), case.clone()))
                .or_default();
            in_font_variability = true;
        } else if in_font_variability && raw_line.starts_with("            ") {
            let subject = current_subject
                .as_ref()
                .expect("font_variability field appears before subject");
            let case = current_case
                .as_ref()
                .expect("font_variability field appears before case");
            let requirement = font_variability
                .get_mut(&(subject.clone(), case.clone()))
                .expect("font_variability entry exists");
            if let Some(value) = line.strip_prefix("folder: ") {
                requirement.folder = value.to_string();
            } else if let Some(value) = line.strip_prefix("sizes: ") {
                requirement.sizes = parse_u32_array(value);
            } else if let Some(value) = line.strip_prefix("char_codes: ") {
                requirement.char_codes = parse_u64_array(value);
            } else if let Some(value) = line.strip_prefix("load_flags: ") {
                requirement.load_flags = parse_i32_array(value);
            } else if let Some(value) = line.strip_prefix("render_modes: ") {
                requirement.render_modes = parse_i32_array(value);
            }
        } else if in_font_variability && !raw_line.starts_with("            ") {
            in_font_variability = false;
        }
    }

    assert!(!subjects.is_empty(), "manifest has no subjects");
    for (subject, cases) in &subjects {
        assert!(!cases.is_empty(), "{subject} has no cases");
    }
    for ((subject, case), requirement) in &font_variability {
        assert!(
            !requirement.folder.is_empty(),
            "{subject}::{case} font_variability requires folder"
        );
        assert!(
            !requirement.sizes.is_empty(),
            "{subject}::{case} font_variability requires at least one size"
        );
    }
    Manifest {
        subjects,
        font_variability,
    }
}

fn read_all_case_files() -> Vec<InputCase> {
    let input_dir = fixture_dir().join("inputs");
    let mut paths = fs::read_dir(&input_dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", input_dir.display()))
        .map(|entry| entry.expect("read input entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut cases = Vec::new();
    for path in paths {
        let parsed: CaseFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("read input case file"))
                .unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
        cases.extend(parsed.cases);
    }
    cases
}

fn parse_u32_array(value: &str) -> Vec<u32> {
    parse_array(value, |item| item.parse::<u32>())
}

fn parse_u64_array(value: &str) -> Vec<u64> {
    parse_array(value, |item| item.parse::<u64>())
}

fn parse_i32_array(value: &str) -> Vec<i32> {
    parse_array(value, |item| item.parse::<i32>())
}

fn parse_array<T, E>(value: &str, parse: impl Fn(&str) -> Result<T, E>) -> Vec<T>
where
    E: std::fmt::Display,
{
    let trimmed = value.trim();
    assert!(
        trimmed.starts_with('[') && trimmed.ends_with(']'),
        "manifest array must use inline [..] form: {value}"
    );
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .expect("manifest array uses checked inline form");
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|item| {
            let item = item.trim();
            parse(item).unwrap_or_else(|err| panic!("parse manifest array item {item}: {err}"))
        })
        .collect()
}

fn fixture_fonts(folder: &str) -> Vec<String> {
    let folder_path = fixture_dir().join(folder);
    let mut fonts = fs::read_dir(&folder_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", folder_path.display()))
        .map(|entry| entry.expect("read font fixture entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttf"))
        .map(|path| {
            path.file_name()
                .expect("fixture font has file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    fonts.sort();
    fonts
}

fn coverage_values<T: Copy>(values: &[T]) -> Vec<Option<T>> {
    if values.is_empty() {
        vec![None]
    } else {
        values.iter().copied().map(Some).collect()
    }
}

struct CoverageProbe<'a> {
    subject: &'a str,
    case_id: &'a str,
    font: &'a str,
    size: u32,
    char_code: Option<u64>,
    load_flag: Option<i32>,
    render_mode: Option<i32>,
}

fn input_covers_font_variability(input: &InputCase, probe: &CoverageProbe<'_>) -> bool {
    input.subject == probe.subject
        && input.case == probe.case_id
        && input_font_file_name(input).is_some_and(|file_name| file_name == probe.font)
        && input_pixel_y(input) == Some(probe.size)
        && probe
            .char_code
            .is_none_or(|value| input_u64_param(input, "char_code") == Some(value))
        && probe
            .load_flag
            .is_none_or(|value| input_i32_param(input, "load_flags") == Some(value))
        && probe
            .render_mode
            .is_none_or(|value| input_i32_param(input, "render_mode") == Some(value))
}

fn input_font_file_name(input: &InputCase) -> Option<String> {
    match input.inputs.assets.get("font")? {
        Asset::File { path, .. } => Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        Asset::InlineBytes { .. } => None,
    }
}

fn input_pixel_y(input: &InputCase) -> Option<u32> {
    input
        .inputs
        .params
        .get("pixel_size")?
        .get("y")?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

fn input_u64_param(input: &InputCase, key: &str) -> Option<u64> {
    input.inputs.params.get(key)?.as_u64()
}

fn input_i32_param(input: &InputCase, key: &str) -> Option<i32> {
    input
        .inputs
        .params
        .get(key)?
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
}

fn validate_assets(case: &InputCase) -> Result<(), String> {
    for (name, asset) in &case.inputs.assets {
        match asset {
            Asset::File {
                path,
                sha256,
                length,
            } => {
                let bytes = fs::read(fixture_dir().join(path))
                    .map_err(|err| format!("{name} read {path}: {err}"))?;
                if u64::try_from(bytes.len()).map_err(|err| err.to_string())? != *length {
                    return Err(format!("{name} length mismatch for {path}"));
                }
                let digest = sha256_hex(&bytes);
                if digest != *sha256 {
                    return Err(format!(
                        "{name} sha256 mismatch for {path}: actual={digest} expected={sha256}"
                    ));
                }
            }
            Asset::InlineBytes { encoding, value } => {
                if encoding != "hex" {
                    return Err(format!(
                        "{name} uses unsupported inline encoding {encoding}"
                    ));
                }
                decode_hex(value).map_err(|err| format!("{name} invalid hex: {err}"))?;
            }
        }
    }
    Ok(())
}

fn run_oracle(case: &InputCase) -> Result<RunOutput, String> {
    let oracle = oracle_bin()?;
    let args = oracle_args(case)?;
    let output = Command::new(&oracle)
        .args(&args)
        .env(
            "LD_LIBRARY_PATH",
            manifest_dir().join("freetype").join("build"),
        )
        .output()
        .map_err(|err| format!("spawn {}: {err}", oracle.display()))?;
    if !output.status.success() {
        return Err(format!(
            "exit={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    parse_run_output(&String::from_utf8(output.stdout).map_err(|err| err.to_string())?)
}

fn oracle_bin() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("FONTDONE_UNIFIED_ORACLE") {
        return Ok(PathBuf::from(path));
    }
    let path = manifest_dir()
        .join("target")
        .join("unified-fixtures")
        .join("gen_unified_oracle");
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "missing oracle helper {}; run `make -C pillow-rs-freetype unified-oracle`",
            path.display()
        ))
    }
}

fn oracle_args(case: &InputCase) -> Result<Vec<String>, String> {
    let params = &case.inputs.params;
    match case.operation.as_str() {
        "constant" => Ok(vec![
            "--constant".to_string(),
            string_param(params, "symbol")?.to_string(),
        ]),
        "record_layout" => Ok(vec![
            "--layout".to_string(),
            string_param(params, "record")?.to_string(),
        ]),
        "new_memory_face" => {
            let mut args = vec!["--new-memory-face".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            Ok(args)
        }
        "get_char_index" => {
            let mut args = vec!["--get-char-index".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(u64_param(params, "char_code")?.to_string());
            Ok(args)
        }
        "load_char" => {
            let mut args = vec!["--load-char".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(u64_param(params, "char_code")?.to_string());
            args.push(i64_param(params, "load_flags")?.to_string());
            Ok(args)
        }
        "load_glyph" => {
            let mut args = vec!["--load-glyph".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(u64_param(params, "glyph_index")?.to_string());
            args.push(i64_param(params, "load_flags")?.to_string());
            Ok(args)
        }
        "render_glyph" => {
            let mut args = vec!["--render-glyph".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(u64_param(params, "char_code")?.to_string());
            args.push(i64_param(params, "load_flags")?.to_string());
            args.push(i64_param(params, "render_mode")?.to_string());
            Ok(args)
        }
        other => Err(format!("unsupported oracle operation {other}")),
    }
}

fn push_font_source(case: &InputCase, args: &mut Vec<String>) -> Result<(), String> {
    let font = case
        .inputs
        .assets
        .get("font")
        .ok_or_else(|| "missing font asset".to_string())?;
    match font {
        Asset::File { path, .. } => {
            args.push("file".to_string());
            args.push(fixture_dir().join(path).display().to_string());
        }
        Asset::InlineBytes { value, .. } => {
            args.push("hex".to_string());
            args.push(value.clone());
        }
    }
    Ok(())
}

fn push_face_size(params: &Value, args: &mut Vec<String>) -> Result<(), String> {
    args.push(i64_param(params, "face_index")?.to_string());
    let size = params
        .get("pixel_size")
        .ok_or_else(|| "missing pixel_size".to_string())?;
    args.push(u64_param(size, "x")?.to_string());
    args.push(u64_param(size, "y")?.to_string());
    Ok(())
}

fn parse_run_output(text: &str) -> Result<RunOutput, String> {
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|err| format!("parse runtime output: {err}; output={}", text.trim()))?;
    let status_value = value
        .get("status")
        .ok_or_else(|| "missing status".to_string())?;
    let kind = match string_param(status_value, "kind")? {
        "ok" => StatusKind::Ok,
        "error" => StatusKind::Error,
        other => return Err(format!("unknown status kind {other}")),
    };
    let status = Status {
        kind,
        error_code: i64_param(status_value, "error_code")?,
    };
    let output = value.get("output").cloned().unwrap_or(Value::Null);
    if status.kind == StatusKind::Ok && output.is_null() {
        return Err("ok output must not be null".to_string());
    }
    Ok(RunOutput { status, output })
}

fn run_rust_ffi(case: &InputCase) -> Result<RunOutput, String> {
    match case.operation.as_str() {
        "constant" => Ok(ok(json!({
            "value": rust_constant(string_param(&case.inputs.params, "symbol")?)?
        }))),
        "record_layout" => Ok(ok(rust_layout(string_param(
            &case.inputs.params,
            "record",
        )?)?)),
        "new_memory_face" => rust_new_memory_face(case),
        "get_char_index" => {
            let face = open_face(case)?;
            Ok(ok(json!({
                "value": FT_Get_Char_Index(&face, u64_param(&case.inputs.params, "char_code")?)
            })))
        }
        "load_char" => {
            let face = open_face(case)?;
            match FT_Load_Char(
                &face,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            ) {
                Ok(slot) => Ok(ok(slot_json(&slot))),
                Err(err) => Ok(error(err)),
            }
        }
        "load_glyph" => {
            let face = open_face(case)?;
            match FT_Load_Glyph(
                &face,
                u32_param(&case.inputs.params, "glyph_index")?,
                i32_param(&case.inputs.params, "load_flags")?,
            ) {
                Ok(slot) => Ok(ok(slot_json(&slot))),
                Err(err) => Ok(error(err)),
            }
        }
        "render_glyph" => {
            let face = open_face(case)?;
            let render_mode = i32_param(&case.inputs.params, "render_mode")?;
            let loaded = FT_Load_Char(
                &face,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            match loaded.and_then(|slot| FT_Render_Glyph(slot, render_mode)) {
                Ok(slot) => Ok(ok(slot_json(&slot))),
                Err(err) => Ok(error(err)),
            }
        }
        other => Err(format!("unsupported rust operation {other}")),
    }
}

fn rust_new_memory_face(case: &InputCase) -> Result<RunOutput, String> {
    let data = font_bytes(case)?;
    let library = FT_Init_FreeType();
    match FT_New_Memory_Face(
        &library,
        &data,
        i64_param(&case.inputs.params, "face_index")?,
        20.0,
    ) {
        Ok(mut face) => {
            let size = case
                .inputs
                .params
                .get("pixel_size")
                .ok_or_else(|| "missing pixel_size".to_string())?;
            let err = FT_Set_Pixel_Sizes(&mut face, u32_param(size, "x")?, u32_param(size, "y")?);
            if err == FT_Err_Ok {
                Ok(ok(json!({"opened": true})))
            } else {
                Ok(error(err))
            }
        }
        Err(err) => Ok(error(err)),
    }
}

fn open_face(case: &InputCase) -> Result<FT_Face, String> {
    let data = font_bytes(case)?;
    let library = FT_Init_FreeType();
    let mut face = FT_New_Memory_Face(
        &library,
        &data,
        i64_param(&case.inputs.params, "face_index")?,
        20.0,
    )
    .map_err(|err| format!("FT_New_Memory_Face returned {err}"))?;
    let size = case
        .inputs
        .params
        .get("pixel_size")
        .ok_or_else(|| "missing pixel_size".to_string())?;
    let err = FT_Set_Pixel_Sizes(&mut face, u32_param(size, "x")?, u32_param(size, "y")?);
    if err == FT_Err_Ok {
        Ok(face)
    } else {
        Err(format!("FT_Set_Pixel_Sizes returned {err}"))
    }
}

fn font_bytes(case: &InputCase) -> Result<Vec<u8>, String> {
    let font = case
        .inputs
        .assets
        .get("font")
        .ok_or_else(|| "missing font asset".to_string())?;
    match font {
        Asset::File { path, .. } => {
            fs::read(fixture_dir().join(path)).map_err(|err| format!("read {path}: {err}"))
        }
        Asset::InlineBytes { encoding, value } => {
            if encoding != "hex" {
                return Err(format!("unsupported inline byte encoding {encoding}"));
            }
            decode_hex(value)
        }
    }
}

fn ok(output: Value) -> RunOutput {
    RunOutput {
        status: Status {
            kind: StatusKind::Ok,
            error_code: 0,
        },
        output,
    }
}

fn error(error_code: i32) -> RunOutput {
    RunOutput {
        status: Status {
            kind: StatusKind::Error,
            error_code: i64::from(error_code),
        },
        output: Value::Null,
    }
}

fn rust_constant(symbol: &str) -> Result<i64, String> {
    match symbol {
        "FT_LOAD_RENDER" => Ok(i64::from(FT_LOAD_RENDER)),
        "FT_RENDER_MODE_MONO" => Ok(i64::from(FT_RENDER_MODE_MONO)),
        "FT_PIXEL_MODE_GRAY" => Ok(i64::from(FT_PIXEL_MODE_GRAY)),
        other => Err(format!("unsupported rust constant {other}")),
    }
}

fn rust_layout(record: &str) -> Result<Value, String> {
    match record {
        "FT_Vector" => Ok(json!({
            "record": "FT_Vector",
            "size": size_of::<FT_Vector>(),
            "align": align_of::<FT_Vector>(),
            "fields": [
                {"name": "x", "offset": offset_of!(FT_Vector, x), "size": size_of::<FT_Pos>()},
                {"name": "y", "offset": offset_of!(FT_Vector, y), "size": size_of::<FT_Pos>()}
            ]
        })),
        "FT_BBox" => Ok(json!({
            "record": "FT_BBox",
            "size": size_of::<FT_BBox>(),
            "align": align_of::<FT_BBox>(),
            "fields": [
                {"name": "xMin", "offset": offset_of!(FT_BBox, xMin), "size": size_of::<FT_Pos>()},
                {"name": "yMin", "offset": offset_of!(FT_BBox, yMin), "size": size_of::<FT_Pos>()},
                {"name": "xMax", "offset": offset_of!(FT_BBox, xMax), "size": size_of::<FT_Pos>()},
                {"name": "yMax", "offset": offset_of!(FT_BBox, yMax), "size": size_of::<FT_Pos>()}
            ]
        })),
        "FT_Glyph_Metrics" => Ok(json!({
            "record": "FT_Glyph_Metrics",
            "size": size_of::<FT_Glyph_Metrics>(),
            "align": align_of::<FT_Glyph_Metrics>(),
            "fields": [
                {"name": "width", "offset": offset_of!(FT_Glyph_Metrics, width), "size": size_of::<FT_Pos>()},
                {"name": "height", "offset": offset_of!(FT_Glyph_Metrics, height), "size": size_of::<FT_Pos>()},
                {"name": "horiBearingX", "offset": offset_of!(FT_Glyph_Metrics, horiBearingX), "size": size_of::<FT_Pos>()},
                {"name": "horiBearingY", "offset": offset_of!(FT_Glyph_Metrics, horiBearingY), "size": size_of::<FT_Pos>()},
                {"name": "horiAdvance", "offset": offset_of!(FT_Glyph_Metrics, horiAdvance), "size": size_of::<FT_Pos>()},
                {"name": "vertBearingX", "offset": offset_of!(FT_Glyph_Metrics, vertBearingX), "size": size_of::<FT_Pos>()},
                {"name": "vertBearingY", "offset": offset_of!(FT_Glyph_Metrics, vertBearingY), "size": size_of::<FT_Pos>()},
                {"name": "vertAdvance", "offset": offset_of!(FT_Glyph_Metrics, vertAdvance), "size": size_of::<FT_Pos>()}
            ]
        })),
        "FT_Size_Metrics" => Ok(json!({
            "record": "FT_Size_Metrics",
            "size": size_of::<FT_Size_Metrics>(),
            "align": align_of::<FT_Size_Metrics>(),
            "fields": [
                {"name": "x_ppem", "offset": offset_of!(FT_Size_Metrics, x_ppem), "size": size_of::<FT_UShort>()},
                {"name": "y_ppem", "offset": offset_of!(FT_Size_Metrics, y_ppem), "size": size_of::<FT_UShort>()},
                {"name": "x_scale", "offset": offset_of!(FT_Size_Metrics, x_scale), "size": size_of::<FT_Fixed>()},
                {"name": "y_scale", "offset": offset_of!(FT_Size_Metrics, y_scale), "size": size_of::<FT_Fixed>()},
                {"name": "ascender", "offset": offset_of!(FT_Size_Metrics, ascender), "size": size_of::<FT_Pos>()},
                {"name": "descender", "offset": offset_of!(FT_Size_Metrics, descender), "size": size_of::<FT_Pos>()},
                {"name": "height", "offset": offset_of!(FT_Size_Metrics, height), "size": size_of::<FT_Pos>()},
                {"name": "max_advance", "offset": offset_of!(FT_Size_Metrics, max_advance), "size": size_of::<FT_Pos>()}
            ]
        })),
        other => Err(format!("unsupported rust layout {other}")),
    }
}

fn slot_json(slot: &FT_GlyphSlot) -> Value {
    json!({
        "glyph_index": slot.glyph_index,
        "format": slot.format,
        "advance": {
            "x": slot.advance.x,
            "y": slot.advance.y
        },
        "metrics": {
            "width": slot.metrics.width,
            "height": slot.metrics.height,
            "horiBearingX": slot.metrics.horiBearingX,
            "horiBearingY": slot.metrics.horiBearingY,
            "horiAdvance": slot.metrics.horiAdvance,
            "vertBearingX": slot.metrics.vertBearingX,
            "vertBearingY": slot.metrics.vertBearingY,
            "vertAdvance": slot.metrics.vertAdvance
        },
        "bitmap": slot.bitmap.as_ref().map(|bitmap| {
            json!({
                "width": bitmap.width,
                "rows": bitmap.rows,
                "pitch": bitmap.pitch,
                "pixel_mode": bitmap.pixel_mode,
                "num_grays": bitmap.num_grays,
                "left": slot.bitmap_left,
                "top": slot.bitmap_top,
                "buffer_hex": hex_bytes(&bitmap.buffer)
            })
        })
    })
}

fn compare_case(case: &InputCase, oracle: &RunOutput, actual: &RunOutput) -> Result<(), String> {
    if oracle.status.kind == StatusKind::Ok && case.expect_error {
        return Err(format!(
            "{} expected a C error but oracle returned ok",
            case.case_id
        ));
    }
    if oracle.status.kind == StatusKind::Error && !case.expect_error {
        return Err(format!(
            "{} oracle returned unexpected error {}",
            case.case_id, oracle.status.error_code
        ));
    }
    if oracle.status != actual.status {
        return Err(format!(
            "{} status mismatch: oracle={:?} actual={:?}",
            case.case_id, oracle.status, actual.status
        ));
    }
    if oracle.status.kind == StatusKind::Error {
        return Ok(());
    }

    validate_schema_output(case, &oracle.output, "oracle")?;
    validate_schema_output(case, &actual.output, "actual")?;

    if case.schema == "face_open" {
        return Ok(());
    }
    if oracle.output == actual.output {
        Ok(())
    } else {
        let path =
            first_json_diff("", &oracle.output, &actual.output).unwrap_or_else(|| JsonDiff {
                path: "/".to_string(),
                expected: oracle.output.clone(),
                actual: actual.output.clone(),
            });
        Err(format!(
            "{} schema={} field={} expected={} actual={}",
            case.case_id, case.schema, path.path, path.expected, path.actual
        ))
    }
}

fn validate_schema_output(case: &InputCase, output: &Value, label: &str) -> Result<(), String> {
    match case.schema.as_str() {
        "constant" => require_path(output, "/value", label, case),
        "record_layout" => {
            require_path(output, "/record", label, case)?;
            require_path(output, "/size", label, case)?;
            require_path(output, "/align", label, case)?;
            require_path(output, "/fields", label, case)
        }
        "scalar" => require_path(output, "/value", label, case),
        "glyph_slot" => {
            require_path(output, "/glyph_index", label, case)?;
            require_path(output, "/format", label, case)?;
            require_path(output, "/advance", label, case)?;
            require_path(output, "/metrics", label, case)?;
            if let Some(bitmap) = output.get("bitmap").filter(|value| !value.is_null()) {
                require_path(bitmap, "/width", label, case)?;
                require_path(bitmap, "/rows", label, case)?;
                require_path(bitmap, "/pitch", label, case)?;
                require_path(bitmap, "/pixel_mode", label, case)?;
                require_path(bitmap, "/buffer_hex", label, case)?;
            }
            Ok(())
        }
        "face_open" => Ok(()),
        "error" => Ok(()),
        other => Err(format!("{} uses unknown schema {other}", case.case_id)),
    }
}

fn require_path(
    output: &Value,
    pointer: &str,
    label: &str,
    case: &InputCase,
) -> Result<(), String> {
    if output.pointer(pointer).is_some() {
        Ok(())
    } else {
        Err(format!(
            "{} {label} output missing required path {pointer}",
            case.case_id
        ))
    }
}

struct JsonDiff {
    path: String,
    expected: Value,
    actual: Value,
}

fn first_json_diff(path: &str, expected: &Value, actual: &Value) -> Option<JsonDiff> {
    match (expected, actual) {
        (Value::Object(expected_map), Value::Object(actual_map)) => {
            for (key, expected_value) in expected_map {
                let child_path = format!("{path}/{key}");
                let Some(actual_value) = actual_map.get(key) else {
                    return Some(JsonDiff {
                        path: child_path,
                        expected: expected_value.clone(),
                        actual: Value::Null,
                    });
                };
                if let Some(diff) = first_json_diff(&child_path, expected_value, actual_value) {
                    return Some(diff);
                }
            }
            for (key, actual_value) in actual_map {
                if !expected_map.contains_key(key) {
                    return Some(JsonDiff {
                        path: format!("{path}/{key}"),
                        expected: Value::Null,
                        actual: actual_value.clone(),
                    });
                }
            }
            None
        }
        (Value::Array(expected_array), Value::Array(actual_array)) => {
            let max_len = expected_array.len().max(actual_array.len());
            for index in 0..max_len {
                let child_path = format!("{path}/{index}");
                match (expected_array.get(index), actual_array.get(index)) {
                    (Some(expected_value), Some(actual_value)) => {
                        if let Some(diff) =
                            first_json_diff(&child_path, expected_value, actual_value)
                        {
                            return Some(diff);
                        }
                    }
                    (Some(expected_value), None) => {
                        return Some(JsonDiff {
                            path: child_path,
                            expected: expected_value.clone(),
                            actual: Value::Null,
                        });
                    }
                    (None, Some(actual_value)) => {
                        return Some(JsonDiff {
                            path: child_path,
                            expected: Value::Null,
                            actual: actual_value.clone(),
                        });
                    }
                    (None, None) => {}
                }
            }
            None
        }
        _ if expected == actual => None,
        _ => Some(JsonDiff {
            path: path.to_string(),
            expected: expected.clone(),
            actual: actual.clone(),
        }),
    }
}

fn string_param<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string param {key}"))
}

fn i64_param(value: &Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing i64 param {key}"))
}

fn u64_param(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing u64 param {key}"))
}

fn i32_param(value: &Value, key: &str) -> Result<i32, String> {
    let raw = i64_param(value, key)?;
    i32::try_from(raw).map_err(|err| format!("{key} does not fit i32: {err}"))
}

fn u32_param(value: &Value, key: &str) -> Result<u32, String> {
    let raw = u64_param(value, key)?;
    u32::try_from(raw).map_err(|err| format!("{key} does not fit u32: {err}"))
}

fn sha256_hex(data: &[u8]) -> String {
    hex_bytes(&Sha256::digest(data))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("hex has odd length".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chars = value.as_bytes().chunks_exact(2);
    for pair in &mut chars {
        let text = std::str::from_utf8(pair).map_err(|err| err.to_string())?;
        bytes.push(u8::from_str_radix(text, 16).map_err(|err| err.to_string())?);
    }
    Ok(bytes)
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
