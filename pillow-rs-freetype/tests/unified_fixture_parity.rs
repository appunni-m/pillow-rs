#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::mem::{align_of, offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;

use fontdone::ffi::*;
use fontdone_ffi_c as c_abi;
use fontdone_ffi_wasm as wasm_abi;
use serde::{Deserialize, Deserializer, Serialize};
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
    glyph_indices: Vec<u32>,
    load_flags: Vec<i32>,
    render_modes: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct CaseFile {
    #[serde(default)]
    assets: BTreeMap<String, Asset>,
    #[serde(default)]
    matrix_cases: Vec<MatrixCaseSpec>,
    cases: Vec<InputCase>,
}

#[derive(Debug, Deserialize, Serialize)]
struct InputCase {
    case_id: String,
    subject: String,
    case: String,
    operation: String,
    schema: String,
    #[serde(default)]
    expect_error: bool,
    inputs: Inputs,
    #[serde(default)]
    source: Option<CaseSource>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Inputs {
    #[serde(default)]
    assets: BTreeMap<String, Asset>,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Clone, Serialize)]
enum Asset {
    Ref {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    #[serde(rename = "file")]
    File {
        path: String,
        #[serde(default)]
        sha256: Option<String>,
        #[serde(default)]
        length: Option<u64>,
    },
    #[serde(rename = "inline_bytes")]
    InlineBytes {
        encoding: String,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        data: Option<String>,
    },
    Other(Value),
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Some(object) = value.as_object() else {
            return Ok(Asset::Other(value));
        };
        let kind = object.get("kind").and_then(Value::as_str);
        match kind {
            Some("file") => {
                let Some(path) = object.get("path").and_then(Value::as_str) else {
                    return Ok(Asset::Other(value));
                };
                Ok(Asset::File {
                    path: path.to_string(),
                    sha256: object
                        .get("sha256")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    length: object.get("length").and_then(Value::as_u64),
                })
            }
            Some("ref") => Ok(Asset::Ref {
                id: object
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                path: object
                    .get("path")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            }),
            Some("inline_bytes") => {
                let Some(encoding) = object.get("encoding").and_then(Value::as_str) else {
                    return Ok(Asset::Other(value));
                };
                Ok(Asset::InlineBytes {
                    encoding: encoding.to_string(),
                    value: object
                        .get("value")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    data: object
                        .get("data")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                })
            }
            _ => Ok(Asset::Other(value)),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CaseSource {
    matrix: String,
    row_id: String,
    row_operation: String,
}

#[derive(Debug, Deserialize)]
struct MatrixCaseSpec {
    id: String,
    subject: String,
    case: String,
    operation: String,
    schema: String,
    source: MatrixCaseSource,
    #[serde(default)]
    classifiers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MatrixCaseSource {
    matrix: String,
    row_operation: String,
    #[serde(default)]
    requires_glyph_index: bool,
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
fn unified_fixture_parity() {
    eprintln!("unified_fixture_parity: loading expanded input cases");
    let all_cases = read_all_case_files();
    let unique_case_keys = all_cases
        .iter()
        .map(|case| {
            (
                case.subject.clone(),
                case.case.clone(),
                case.case_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    eprintln!(
        "unified_fixture_parity: loaded_cases={} deduped_case_keys={}",
        all_cases.len(),
        unique_case_keys.len()
    );
    assert_manifest_cases_cover_fixture_inputs(&all_cases);
    assert_manifest_font_variability_cases_cover_declared_fixture_folder(&all_cases);
    assert_matrix_derived_inputs_cover_supported_source_rows(&all_cases);
    assert_unified_fixture_cases_match_runtime_c_oracle(&all_cases);
}

fn assert_unified_fixture_cases_match_runtime_c_oracle(all_cases: &[InputCase]) {
    let manifest = read_manifest();
    let runtime_selection = select_runtime_cases(all_cases);
    let cases = runtime_selection.executable;
    let mut passed = 0usize;
    let mut failures = Vec::new();
    let mut covered = BTreeSet::new();
    let mut valid_cases = Vec::new();

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
        valid_cases.push(*case);
    }

    let oracle_outputs = if failures.is_empty() {
        match run_oracles_with_cache(&valid_cases) {
            Ok(outputs) => outputs,
            Err(err) => {
                failures.push(format!("batch oracle failed: {err}"));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    if failures.is_empty() {
        let result = compare_backend_outputs(&valid_cases, &oracle_outputs);
        passed += result.passed;
        covered.extend(result.covered);
        failures.extend(result.failures);
    }

    eprintln!(
        "runtime_selection: executable={} model_only={} direct_model_only={} matrix_model_only={} matrix_runtime_enabled={} unsupported_ops={}",
        cases.len(),
        runtime_selection.model_only,
        runtime_selection
            .model_only
            .saturating_sub(runtime_selection.matrix_model_only),
        runtime_selection.matrix_model_only,
        include_matrix_runtime_cases(),
        format_operation_counts(&runtime_selection.unsupported_operations)
    );
    eprintln!(
        "runtime_parity: passed={} failed={} total={} covered_manifest_cases={}",
        passed,
        failures.len(),
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

struct RuntimeSelection<'a> {
    executable: Vec<&'a InputCase>,
    model_only: usize,
    matrix_model_only: usize,
    unsupported_operations: BTreeMap<String, usize>,
}

fn select_runtime_cases(cases: &[InputCase]) -> RuntimeSelection<'_> {
    let filter = case_filter();
    let limit = case_limit();
    let mut executable = Vec::new();
    let mut model_only = 0usize;
    let mut matrix_model_only = 0usize;
    let mut unsupported_operations = BTreeMap::new();
    let mut seen_executable = BTreeSet::new();

    for case in cases {
        if !case_matches_filter(case, filter.as_deref()) {
            continue;
        }
        if case.source.is_some() && !include_matrix_runtime_cases() {
            model_only = model_only.saturating_add(1);
            matrix_model_only = matrix_model_only.saturating_add(1);
            *unsupported_operations
                .entry(format!("matrix:{}", case.operation))
                .or_default() += 1;
            continue;
        }
        if !is_runtime_executable_case(case) {
            model_only = model_only.saturating_add(1);
            if case.source.is_some() {
                matrix_model_only = matrix_model_only.saturating_add(1);
            }
            *unsupported_operations
                .entry(case.operation.clone())
                .or_default() += 1;
            continue;
        }
        let key = runtime_case_key(case);
        if seen_executable.insert(key) {
            executable.push(case);
            if limit.is_some_and(|limit| executable.len() >= limit) {
                break;
            }
        }
    }

    RuntimeSelection {
        executable,
        model_only,
        matrix_model_only,
        unsupported_operations,
    }
}

fn include_matrix_runtime_cases() -> bool {
    std::env::var("FONTDONE_UNIFIED_MATRIX_RUNTIME")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn is_runtime_executable_case(case: &InputCase) -> bool {
    match case.operation.as_str() {
        "constant" => case
            .inputs
            .params
            .get("symbol")
            .and_then(Value::as_str)
            .is_some_and(is_supported_runtime_constant),
        "record_layout" => case
            .inputs
            .params
            .get("record")
            .and_then(Value::as_str)
            .is_some_and(is_supported_runtime_layout),
        "new_memory_face" | "set_pixel_sizes" | "set_char_size" | "size_metrics"
        | "get_char_index" | "load_char" | "load_glyph" | "render_glyph" => {
            has_runtime_font_source(case)
        }
        _ => false,
    }
}

fn has_runtime_font_source(case: &InputCase) -> bool {
    case.inputs
        .assets
        .get("font")
        .is_some_and(|font| match font {
            Asset::File { .. } => true,
            Asset::InlineBytes { encoding, .. } => encoding == "hex",
            Asset::Ref { .. } | Asset::Other(_) => false,
        })
}

fn is_supported_runtime_layout(record: &str) -> bool {
    matches!(
        record,
        "FT_Vector" | "FT_BBox" | "FT_Glyph_Metrics" | "FT_Size_Metrics"
    )
}

fn is_supported_runtime_constant(symbol: &str) -> bool {
    matches!(
        symbol,
        "FT_Err_Ok"
            | "FT_Err_Cannot_Open_Resource"
            | "FT_Err_Unknown_File_Format"
            | "FT_Err_Invalid_File_Format"
            | "FT_Err_Invalid_Argument"
            | "FT_Err_Unimplemented_Feature"
            | "FT_Err_Invalid_Table"
            | "FT_Err_Invalid_Glyph_Index"
            | "FT_Err_Invalid_Character_Code"
            | "FT_Err_Invalid_Glyph_Format"
            | "FT_Err_Cannot_Render_Glyph"
            | "FT_Err_Invalid_Outline"
            | "FT_Err_Invalid_Pixel_Size"
            | "FT_Err_Invalid_CharMap_Handle"
            | "FT_Err_Out_Of_Memory"
            | "FT_Err_Raster_Overflow"
            | "FT_Err_Invalid_CharMap_Format"
            | "FT_LOAD_DEFAULT"
            | "FT_LOAD_NO_SCALE"
            | "FT_LOAD_NO_HINTING"
            | "FT_LOAD_RENDER"
            | "FT_LOAD_NO_BITMAP"
            | "FT_LOAD_VERTICAL_LAYOUT"
            | "FT_LOAD_FORCE_AUTOHINT"
            | "FT_LOAD_CROP_BITMAP"
            | "FT_LOAD_PEDANTIC"
            | "FT_LOAD_ADVANCE_ONLY"
            | "FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH"
            | "FT_LOAD_NO_RECURSE"
            | "FT_LOAD_IGNORE_TRANSFORM"
            | "FT_LOAD_MONOCHROME"
            | "FT_LOAD_LINEAR_DESIGN"
            | "FT_LOAD_SBITS_ONLY"
            | "FT_LOAD_NO_AUTOHINT"
            | "FT_LOAD_COLOR"
            | "FT_LOAD_COMPUTE_METRICS"
            | "FT_LOAD_BITMAP_METRICS_ONLY"
            | "FT_LOAD_SVG_ONLY"
            | "FT_LOAD_NO_SVG"
            | "FT_RENDER_MODE_NORMAL"
            | "FT_RENDER_MODE_LIGHT"
            | "FT_RENDER_MODE_MONO"
            | "FT_RENDER_MODE_LCD"
            | "FT_RENDER_MODE_LCD_V"
            | "FT_RENDER_MODE_SDF"
            | "FT_RENDER_MODE_MAX"
            | "FT_LOAD_TARGET_NORMAL"
            | "FT_LOAD_TARGET_LIGHT"
            | "FT_LOAD_TARGET_MONO"
            | "FT_LOAD_TARGET_LCD"
            | "FT_LOAD_TARGET_LCD_V"
            | "FT_PIXEL_MODE_NONE"
            | "FT_PIXEL_MODE_MONO"
            | "FT_PIXEL_MODE_GRAY"
            | "FT_PIXEL_MODE_GRAY2"
            | "FT_PIXEL_MODE_GRAY4"
            | "FT_PIXEL_MODE_LCD"
            | "FT_PIXEL_MODE_LCD_V"
            | "FT_PIXEL_MODE_BGRA"
            | "FT_PIXEL_MODE_MAX"
            | "FT_GLYPH_FORMAT_NONE"
            | "FT_GLYPH_FORMAT_COMPOSITE"
            | "FT_GLYPH_FORMAT_BITMAP"
            | "FT_GLYPH_FORMAT_OUTLINE"
            | "FT_GLYPH_FORMAT_PLOTTER"
            | "FT_GLYPH_FORMAT_SVG"
    )
}

fn runtime_case_key(case: &InputCase) -> String {
    serde_json::to_string(&json!({
        "operation": case.operation,
        "schema": case.schema,
        "expect_error": case.expect_error,
        "inputs": case.inputs,
    }))
    .expect("runtime case key serializes")
}

fn format_operation_counts(counts: &BTreeMap<String, usize>) -> String {
    let mut entries = counts
        .iter()
        .map(|(operation, count)| (*count, operation.as_str()))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    let mut entries = entries
        .into_iter()
        .map(|(count, operation)| format!("{operation}:{count}"))
        .collect::<Vec<_>>();
    if entries.len() > 12 {
        entries.truncate(12);
        entries.push("...".to_string());
    }
    entries.join(",")
}

fn compare_named_output(
    case: &InputCase,
    backend: &str,
    oracle: &RunOutput,
    actual: &RunOutput,
) -> Result<(), String> {
    compare_case(case, oracle, actual).map_err(|err| format!("{backend}: {err}"))
}

#[derive(Default)]
struct BackendComparisonResult {
    passed: usize,
    covered: BTreeSet<(String, String)>,
    failures: Vec<String>,
}

fn compare_backend_outputs(
    cases: &[&InputCase],
    oracle_outputs: &[RunOutput],
) -> BackendComparisonResult {
    assert_eq!(
        cases.len(),
        oracle_outputs.len(),
        "oracle output count must match case count"
    );
    let workers = unified_worker_count(cases.len());
    if workers <= 1 {
        return compare_backend_output_range(cases, oracle_outputs);
    }

    let chunk_size = cases.len().div_ceil(workers);
    let mut result = BackendComparisonResult::default();
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for (case_chunk, oracle_chunk) in cases
            .chunks(chunk_size)
            .zip(oracle_outputs.chunks(chunk_size))
        {
            handles
                .push(scope.spawn(move || compare_backend_output_range(case_chunk, oracle_chunk)));
        }
        for handle in handles {
            let partial = handle.join().expect("backend comparison worker panicked");
            result.passed = result.passed.saturating_add(partial.passed);
            result.covered.extend(partial.covered);
            result.failures.extend(partial.failures);
        }
    });
    result
}

fn unified_worker_count(case_count: usize) -> usize {
    if case_count < 2 {
        return 1;
    }
    let default_workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 8);
    let requested =
        std::env::var("FONTDONE_UNIFIED_WORKERS")
            .ok()
            .map_or(default_workers, |value| {
                value
                    .parse::<usize>()
                    .unwrap_or_else(|err| panic!("FONTDONE_UNIFIED_WORKERS must be usize: {err}"))
            });
    requested.clamp(1, case_count)
}

fn compare_backend_output_range(
    cases: &[&InputCase],
    oracle_outputs: &[RunOutput],
) -> BackendComparisonResult {
    let mut result = BackendComparisonResult::default();
    for (case, oracle) in cases.iter().zip(oracle_outputs.iter()) {
        match compare_backend_output_case(case, oracle) {
            Ok(()) => {
                result.passed = result.passed.saturating_add(1);
                result
                    .covered
                    .insert((case.subject.clone(), case.case.clone()));
            }
            Err(err) => result.failures.push(err),
        }
    }
    result
}

fn compare_backend_output_case(case: &InputCase, oracle: &RunOutput) -> Result<(), String> {
    let rust_actual =
        run_rust_ffi(case).map_err(|err| format!("{} rust backend failed: {err}", case.case_id))?;
    let c_actual =
        run_c_abi(case).map_err(|err| format!("{} c abi backend failed: {err}", case.case_id))?;
    let wasm_actual = run_wasm_abi(case)
        .map_err(|err| format!("{} wasm abi backend failed: {err}", case.case_id))?;

    compare_named_output(case, "rust ffi", oracle, &rust_actual)
        .and_then(|()| compare_named_output(case, "c abi", oracle, &c_actual))
        .and_then(|()| compare_named_output(case, "wasm abi", oracle, &wasm_actual))
}

fn case_filter() -> Option<String> {
    std::env::var("FONTDONE_UNIFIED_CASE_FILTER").ok()
}

fn case_limit() -> Option<usize> {
    std::env::var("FONTDONE_UNIFIED_CASE_LIMIT")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .unwrap_or_else(|err| panic!("FONTDONE_UNIFIED_CASE_LIMIT must be usize: {err}"))
        })
}

fn case_matches_filter(case: &InputCase, filter: Option<&str>) -> bool {
    filter.is_none_or(|needle| {
        case.case_id.contains(needle)
            || case.subject.contains(needle)
            || case.case.contains(needle)
            || case
                .source
                .as_ref()
                .is_some_and(|source| source.matrix.contains(needle))
    })
}

fn assert_manifest_cases_cover_fixture_inputs(cases: &[InputCase]) {
    let manifest = read_manifest();
    let mut covered = BTreeSet::new();

    for case in cases {
        assert!(
            manifest.has_case(&case.subject, &case.case),
            "{} references unknown manifest case {}::{}",
            case.case_id,
            case.subject,
            case.case
        );
        covered.insert((case.subject.clone(), case.case.clone()));
    }
    eprintln!(
        "manifest_coverage: checked_cases={} covered_manifest_cases={}",
        cases.len(),
        covered.len()
    );
}

fn assert_manifest_font_variability_cases_cover_declared_fixture_folder(cases: &[InputCase]) {
    let manifest = read_manifest();
    let mut checked = 0usize;
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
                let glyph_indices = coverage_values(&variability.glyph_indices);
                let load_flags = coverage_values(&variability.load_flags);
                let render_modes = coverage_values(&variability.render_modes);
                for char_code in &char_codes {
                    for glyph_index in &glyph_indices {
                        for load_flag in &load_flags {
                            for render_mode in &render_modes {
                                checked = checked.saturating_add(1);
                                let probe = CoverageProbe {
                                    subject,
                                    case_id,
                                    font,
                                    size: *size,
                                    char_code: *char_code,
                                    glyph_index: *glyph_index,
                                    load_flag: *load_flag,
                                    render_mode: *render_mode,
                                };
                                if !cases
                                    .iter()
                                    .any(|input| input_covers_font_variability(input, &probe))
                                {
                                    failures.push(format!(
                                        "{}::{} missing font={} size={} char_code={:?} glyph_index={:?} load_flags={:?} render_mode={:?}",
                                        subject,
                                        case_id,
                                        font,
                                        size,
                                        char_code,
                                        glyph_index,
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
    }

    assert!(
        !manifest.font_variability.is_empty(),
        "manifest declares no font variability coverage requirements"
    );
    eprintln!(
        "font_variability_coverage: checked_probes={} missing={} requirements={}",
        checked,
        failures.len(),
        manifest.font_variability.len()
    );
    assert!(
        failures.is_empty(),
        "font variability coverage gaps:\n{}",
        failures.join("\n")
    );
}

fn assert_matrix_derived_inputs_cover_supported_source_rows(cases: &[InputCase]) {
    let matrix_sources = cases
        .iter()
        .filter_map(|case| {
            case.source.as_ref().map(|source| {
                (
                    source.matrix.clone(),
                    source.row_id.clone(),
                    source.row_operation.clone(),
                    case.subject.clone(),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    let expected = expected_matrix_subject_coverage();

    assert!(
        matrix_sources.len() >= 100_000,
        "expected matrix-derived unified FFI coverage to be at least 100k subject cases, got {}",
        matrix_sources.len()
    );

    let mut missing = Vec::new();
    for expectation in &expected {
        if !matrix_sources.contains(expectation) {
            missing.push(format!(
                "{} row={} operation={} subject={}",
                expectation.0, expectation.1, expectation.2, expectation.3
            ));
        }
    }
    eprintln!(
        "matrix_coverage: covered_subject_rows={} expected_subject_rows={} missing={}",
        matrix_sources.len(),
        expected.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "matrix-derived unified input gaps:\n{}",
        missing.join("\n")
    );
}

fn expected_matrix_subject_coverage() -> BTreeSet<(String, String, String, String)> {
    let mut expected = BTreeSet::new();
    for (matrix, mappings) in matrix_subject_mappings() {
        let rows = read_source_matrix_rows(matrix);
        for row in rows {
            let operation = string_param(&row, "operation")
                .unwrap_or_else(|err| panic!("{matrix} row operation: {err}"))
                .to_string();
            let row_id = string_param(&row, "id")
                .unwrap_or_else(|err| panic!("{matrix} row id: {err}"))
                .to_string();
            if string_param(&row, "status").unwrap_or("active") != "active" {
                continue;
            }
            for (mapped_operation, subjects) in &mappings {
                if operation != *mapped_operation {
                    continue;
                }
                for subject in subjects {
                    if *subject == "freetype.FT_Load_Glyph"
                        && row.get("glyph_index").and_then(Value::as_u64).is_none()
                    {
                        continue;
                    }
                    expected.insert((
                        matrix.to_string(),
                        row_id.clone(),
                        operation.clone(),
                        (*subject).to_string(),
                    ));
                }
            }
        }
    }
    expected
}

fn matrix_subject_mappings() -> BTreeMap<&'static str, BTreeMap<&'static str, Vec<&'static str>>> {
    let mut mappings = BTreeMap::new();
    mappings.insert(
        "native_tt_default_matrix.json",
        BTreeMap::from([
            (
                "getmask",
                vec![
                    "freetype.FT_Get_Char_Index",
                    "freetype.FT_Load_Char",
                    "freetype.FT_Load_Glyph",
                ],
            ),
            ("getmetrics", vec!["freetype.FT_Size_Metrics"]),
        ]),
    );
    mappings.insert(
        "force_autohint_matrix.json",
        BTreeMap::from([(
            "getmask",
            vec!["freetype.FT_Get_Char_Index", "freetype.FT_Load_Char"],
        )]),
    );
    mappings.insert(
        "no_hinting_matrix.json",
        BTreeMap::from([(
            "getmask",
            vec![
                "freetype.FT_Get_Char_Index",
                "freetype.FT_Load_Char",
                "freetype.FT_Load_Glyph",
            ],
        )]),
    );
    for matrix in ["render_mono_matrix.json", "render_lcd_matrix.json"] {
        mappings.insert(
            matrix,
            BTreeMap::from([(
                "getmask",
                vec![
                    "freetype.FT_Get_Char_Index",
                    "freetype.FT_Load_Char",
                    "freetype.FT_Load_Glyph",
                    "freetype.FT_Render_Glyph",
                ],
            )]),
        );
    }
    mappings.insert(
        "metrics_only_matrix.json",
        BTreeMap::from([(
            "metrics_only",
            vec![
                "freetype.FT_Get_Char_Index",
                "freetype.FT_Load_Char",
                "freetype.FT_Load_Glyph",
            ],
        )]),
    );
    mappings
}

fn read_source_matrix_rows(matrix: &str) -> Vec<Value> {
    let path = fixture_dir().join(matrix);
    let value: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read source matrix"))
            .unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
    if let Some(rows) = value.get("rows").and_then(Value::as_array) {
        return rows.clone();
    }
    if let Some(rows) = value.as_array() {
        return rows.clone();
    }
    panic!("{} has no rows array", path.display());
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
    let mut symbols = BTreeMap::<String, String>::new();
    let mut font_variability = BTreeMap::<(String, String), FontVariability>::new();
    let mut current_subject: Option<String> = None;
    let mut current_case: Option<String> = None;
    let mut in_cases = false;
    let mut in_font_variability = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if raw_line.starts_with("  - id: ") {
            let id = line.trim_start_matches("- id: ").to_string();
            assert!(
                !subjects.contains_key(&id),
                "manifest has duplicate subject id {id}"
            );
            subjects.insert(id.clone(), BTreeSet::new());
            current_subject = Some(id);
            current_case = None;
            in_cases = false;
            in_font_variability = false;
        } else if raw_line.starts_with("    symbol: ") {
            let symbol = line.trim_start_matches("symbol: ").to_string();
            let subject = current_subject
                .as_ref()
                .expect("symbol entry appears before subject");
            if let Some(existing_subject) = symbols.insert(symbol.clone(), subject.clone()) {
                panic!("manifest symbol {symbol} appears in both {existing_subject} and {subject}");
            }
        } else if raw_line.starts_with("    cases:") {
            in_cases = true;
            in_font_variability = false;
        } else if in_cases && raw_line.starts_with("      - id: ") {
            let case = line.trim_start_matches("- id: ").to_string();
            let subject = current_subject
                .as_ref()
                .expect("case entry appears before subject");
            let inserted = subjects
                .entry(subject.clone())
                .or_default()
                .insert(case.clone());
            assert!(inserted, "{subject} has duplicate manifest case id {case}");
            current_case = Some(case);
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
            } else if let Some(value) = line.strip_prefix("glyph_indices: ") {
                requirement.glyph_indices = parse_u32_array(value);
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
    read_case_files(None, None, true)
}

fn read_case_files(
    filter: Option<&str>,
    limit: Option<usize>,
    include_matrix_cases: bool,
) -> Vec<InputCase> {
    let input_dir = fixture_dir().join("inputs");
    let mut paths = input_case_paths(&input_dir);
    paths.sort();

    let mut cases = Vec::new();
    let mut matrix_asset_cache = BTreeMap::new();
    for path in paths {
        let parsed: CaseFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("read input case file"))
                .unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
        for mut case in parsed.cases {
            resolve_case_assets(&path, &parsed.assets, &mut case);
            if case_matches_filter(&case, filter) {
                cases.push(case);
                if limit.is_some_and(|limit| cases.len() >= limit) {
                    return cases;
                }
            }
        }
        if include_matrix_cases {
            for spec in parsed.matrix_cases {
                for case in expand_matrix_case_spec(&spec, &mut matrix_asset_cache) {
                    if case_matches_filter(&case, filter) {
                        cases.push(case);
                        if limit.is_some_and(|limit| cases.len() >= limit) {
                            return cases;
                        }
                    }
                }
            }
        }
    }
    cases
}

fn input_case_paths(input_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_input_case_paths(input_dir, &mut paths);
    paths
}

fn collect_input_case_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let path = entry.expect("read input entry").path();
        if path.is_dir() {
            collect_input_case_paths(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
}

fn expand_matrix_case_spec(
    spec: &MatrixCaseSpec,
    matrix_asset_cache: &mut BTreeMap<String, Asset>,
) -> Vec<InputCase> {
    let mut expanded = Vec::new();
    for row in read_source_matrix_rows(&spec.source.matrix) {
        let operation = string_param(&row, "operation")
            .unwrap_or_else(|err| panic!("{} row operation: {err}", spec.source.matrix));
        if operation != spec.source.row_operation {
            continue;
        }
        if string_param(&row, "status").unwrap_or("active") != "active" {
            continue;
        }
        if spec.source.requires_glyph_index
            && row.get("glyph_index").and_then(Value::as_u64).is_none()
        {
            continue;
        }
        expanded.push(matrix_row_to_input_case(spec, &row, matrix_asset_cache));
    }
    expanded
}

fn matrix_row_to_input_case(
    spec: &MatrixCaseSpec,
    row: &Value,
    matrix_asset_cache: &mut BTreeMap<String, Asset>,
) -> InputCase {
    let matrix = &spec.source.matrix;
    let row_id = string_param(row, "id").unwrap_or_else(|err| panic!("{matrix} row id: {err}"));
    let row_operation = string_param(row, "operation")
        .unwrap_or_else(|err| panic!("{matrix} row operation: {err}"));
    let font_name = string_param(row, "font").unwrap_or_else(|err| panic!("{row_id} font: {err}"));
    let font_file = row
        .get("font_file")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{font_name}.ttf"), ToString::to_string);
    let size = row
        .get("size_pt")
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("{row_id} size_pt missing"))
        .round() as u32;
    let mut params = json!({
        "face_index": 0,
        "pixel_size": {"x": 0, "y": size}
    });
    if matches!(
        spec.operation.as_str(),
        "get_char_index" | "load_char" | "render_glyph"
    ) {
        params["char_code"] = Value::from(
            row.get("codepoint")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{row_id} codepoint missing")),
        );
    }
    if matches!(
        spec.operation.as_str(),
        "load_char" | "load_glyph" | "render_glyph"
    ) {
        params["load_flags"] = Value::from(matrix_load_flags_value(row));
    }
    if spec.operation == "load_glyph" {
        params["glyph_index"] = Value::from(
            row.get("glyph_index")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{row_id} glyph_index missing")),
        );
    }
    if spec.operation == "render_glyph" {
        params["render_mode"] = Value::from(matrix_render_mode_value(row));
    }

    let mut classifiers = spec.classifiers.clone();
    if let Some(script) = row.get("script").and_then(Value::as_str) {
        classifiers.push(format!("script:{script}"));
    }

    InputCase {
        case_id: format!("{}.{}", spec.id, row_id),
        subject: spec.subject.clone(),
        case: spec.case.clone(),
        operation: spec.operation.clone(),
        schema: spec.schema.clone(),
        expect_error: false,
        inputs: Inputs {
            assets: BTreeMap::from([(
                "font".to_string(),
                matrix_font_asset(&font_file, matrix_asset_cache),
            )]),
            params,
        },
        source: Some(CaseSource {
            matrix: matrix.clone(),
            row_id: row_id.to_string(),
            row_operation: row_operation.to_string(),
        }),
    }
}

fn matrix_font_asset(font_file: &str, cache: &mut BTreeMap<String, Asset>) -> Asset {
    if let Some(asset) = cache.get(font_file) {
        return asset.clone();
    }
    let relative_path = format!("input/fonts_autohint/{font_file}");
    let bytes = fs::read(fixture_dir().join(&relative_path))
        .unwrap_or_else(|err| panic!("read matrix font {relative_path}: {err}"));
    let asset = Asset::File {
        path: relative_path,
        sha256: Some(sha256_hex(&bytes)),
        length: Some(u64::try_from(bytes.len()).expect("font length fits u64")),
    };
    cache.insert(font_file.to_string(), asset.clone());
    asset
}

fn matrix_load_flags_value(row: &Value) -> i64 {
    if let Some(value) = row.get("load_flags_value").and_then(Value::as_i64) {
        return value;
    }
    row.get("load_flags")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("matrix row missing load_flags"))
        .iter()
        .map(|flag| match flag.as_str().expect("load flag is string") {
            "FT_LOAD_DEFAULT" => 0,
            "FT_LOAD_RENDER" => 4,
            "FT_LOAD_NO_BITMAP" => 8,
            "FT_LOAD_NO_HINTING" => 2,
            "FT_LOAD_FORCE_AUTOHINT" => 32,
            "FT_LOAD_TARGET_MONO" => 2 << 16,
            "FT_LOAD_TARGET_LCD" => 3 << 16,
            "FT_LOAD_TARGET_LCD_V" => 4 << 16,
            other => panic!("unsupported matrix load flag {other}"),
        })
        .sum()
}

fn matrix_render_mode_value(row: &Value) -> i64 {
    match row
        .get("render_mode")
        .and_then(Value::as_str)
        .unwrap_or("FT_RENDER_MODE_NORMAL")
    {
        "none" | "FT_RENDER_MODE_NORMAL" => 0,
        "FT_RENDER_MODE_LIGHT" => 1,
        "FT_RENDER_MODE_MONO" => 2,
        "FT_RENDER_MODE_LCD" => 3,
        "FT_RENDER_MODE_LCD_V" => 4,
        other => panic!("unsupported matrix render mode {other}"),
    }
}

fn resolve_case_assets(
    _path: &Path,
    shared_assets: &BTreeMap<String, Asset>,
    case: &mut InputCase,
) {
    for asset in case.inputs.assets.values_mut() {
        let Asset::Ref { id: Some(id), .. } = asset else {
            continue;
        };
        if let Some(resolved) = shared_assets.get(id) {
            *asset = resolved.clone();
        }
    }
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
    glyph_index: Option<u32>,
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
            .glyph_index
            .is_none_or(|value| input_u32_param(input, "glyph_index") == Some(value))
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
        Asset::Ref { .. } => None,
        Asset::Other(_) => None,
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

fn input_u32_param(input: &InputCase, key: &str) -> Option<u32> {
    input
        .inputs
        .params
        .get(key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
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
                let (actual_length, digest) = validated_file_asset(path)
                    .map_err(|err| format!("{name} read {path}: {err}"))?;
                if let Some(length) = length {
                    if actual_length != *length {
                        return Err(format!("{name} length mismatch for {path}"));
                    }
                }
                if let Some(sha256) = sha256 {
                    if digest != *sha256 {
                        return Err(format!(
                            "{name} sha256 mismatch for {path}: actual={digest} expected={sha256}"
                        ));
                    }
                }
            }
            Asset::InlineBytes { encoding, .. } => {
                if encoding != "hex" {
                    return Err(format!(
                        "{name} uses unsupported inline encoding {encoding}"
                    ));
                }
                let value = inline_bytes_hex(asset)
                    .ok_or_else(|| format!("{name} missing inline hex bytes"))?;
                decode_hex(value).map_err(|err| format!("{name} invalid hex: {err}"))?;
            }
            Asset::Ref { .. } => {
                return Err(format!(
                    "{name} unresolved shared asset ref {}",
                    asset_label(asset)
                ));
            }
            Asset::Other(_) => {
                return Err(format!("{name} uses unsupported asset shape"));
            }
        }
    }
    Ok(())
}

fn validated_file_asset(path: &str) -> Result<(u64, String), String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, (u64, String)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut cache = cache.lock().map_err(|err| err.to_string())?;
    if let Some(entry) = cache.get(path) {
        return Ok(entry.clone());
    }
    let bytes = cached_file_bytes(path)?;
    let entry = (
        u64::try_from(bytes.len()).map_err(|err| err.to_string())?,
        sha256_hex(&bytes),
    );
    cache.insert(path.to_string(), entry.clone());
    Ok(entry)
}

fn run_oracles_with_cache(cases: &[&InputCase]) -> Result<Vec<RunOutput>, String> {
    if cases.is_empty() {
        return Ok(Vec::new());
    }
    let batch_input = oracle_batch_input(cases)?;
    let cache_key = oracle_cache_key(cases, &batch_input)?;
    let cache_path = oracle_cache_path(&cache_key);

    if std::env::var("FONTDONE_UNIFIED_ORACLE_REFRESH").is_err() && cache_path.exists() {
        let cached = fs::read_to_string(&cache_path)
            .map_err(|err| format!("read oracle cache {}: {err}", cache_path.display()))?;
        eprintln!(
            "unified_oracle_cache: hit {} cases key={}",
            cases.len(),
            cache_key
        );
        return parse_oracle_lines(cases, &cached)
            .map_err(|err| format!("oracle cache {} invalid: {err}", cache_path.display()));
    }

    let stdout = run_oracles_batch(cases, &batch_input)?;
    let outputs = parse_oracle_lines(cases, &stdout)?;
    write_oracle_cache(&cache_path, &stdout)?;
    eprintln!(
        "unified_oracle_cache: wrote {} cases key={}",
        cases.len(),
        cache_key
    );
    Ok(outputs)
}

fn oracle_batch_input(cases: &[&InputCase]) -> Result<String, String> {
    let mut input = String::new();
    for case in cases {
        let args = oracle_args(case)?;
        if args
            .iter()
            .any(|arg| arg.contains('\t') || arg.contains('\n'))
        {
            return Err(format!(
                "{} contains unsupported batch argument",
                case.case_id
            ));
        }
        input.push_str(&args.join("\t"));
        input.push('\n');
    }
    Ok(input)
}

fn oracle_cache_key(cases: &[&InputCase], batch_input: &str) -> Result<String, String> {
    let canonical_cases = serde_json::to_string(cases).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(b"fontdone-unified-oracle-cache-v1\n");
    hasher.update(canonical_cases.as_bytes());
    hasher.update(b"\n--argv--\n");
    hasher.update(batch_input.as_bytes());
    Ok(hex_bytes(&hasher.finalize()))
}

fn oracle_cache_path(cache_key: &str) -> PathBuf {
    fixture_dir()
        .join("outputs")
        .join("unified_oracle_cache")
        .join(format!("{cache_key}.jsonl"))
}

fn write_oracle_cache(path: &Path, stdout: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("oracle cache path {} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("create oracle cache dir {}: {err}", parent.display()))?;
    let tmp = path.with_extension(format!("jsonl.tmp.{}", std::process::id()));
    fs::write(&tmp, stdout)
        .map_err(|err| format!("write oracle cache {}: {err}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|err| format!("install oracle cache {}: {err}", path.display()))
}

fn run_oracles_batch(cases: &[&InputCase], batch_input: &str) -> Result<String, String> {
    if cases.is_empty() {
        return Ok(String::new());
    }
    let oracle = oracle_bin()?;

    let batch_path = manifest_dir()
        .join("target")
        .join("unified-fixtures")
        .join(format!("oracle_batch_{}.argv", std::process::id()));
    fs::write(&batch_path, batch_input)
        .map_err(|err| format!("write batch oracle input {}: {err}", batch_path.display()))?;

    let output = Command::new(&oracle)
        .arg("--batch-argv")
        .env(
            "LD_LIBRARY_PATH",
            manifest_dir().join("freetype").join("build"),
        )
        .stdin(Stdio::from(fs::File::open(&batch_path).map_err(|err| {
            format!("open batch oracle input {}: {err}", batch_path.display())
        })?))
        .output()
        .map_err(|err| format!("spawn {}: {err}", oracle.display()))?;
    let _ = fs::remove_file(&batch_path);
    if !output.status.success() {
        return Err(format!(
            "exit={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).map_err(|err| err.to_string())
}

fn parse_oracle_lines(cases: &[&InputCase], stdout: &str) -> Result<Vec<RunOutput>, String> {
    let lines = stdout.lines().collect::<Vec<_>>();
    if lines.len() != cases.len() {
        return Err(format!(
            "batch oracle returned {} lines for {} cases",
            lines.len(),
            cases.len()
        ));
    }
    cases
        .iter()
        .zip(lines)
        .map(|(case, line)| {
            parse_run_output(line).map_err(|err| format!("{} oracle failed: {err}", case.case_id))
        })
        .collect()
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
        "set_pixel_sizes" => {
            let mut args = vec!["--set-pixel-sizes".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            Ok(args)
        }
        "set_char_size" => {
            let mut args = vec!["--set-char-size".to_string()];
            push_font_source(case, &mut args)?;
            args.push(i64_param(params, "face_index")?.to_string());
            args.push(i64_param(params, "char_width")?.to_string());
            args.push(i64_param(params, "char_height")?.to_string());
            args.push(u64_param(params, "horz_resolution")?.to_string());
            args.push(u64_param(params, "vert_resolution")?.to_string());
            Ok(args)
        }
        "size_metrics" => {
            let mut args = vec!["--size-metrics".to_string()];
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
        Asset::InlineBytes { .. } => {
            args.push("hex".to_string());
            args.push(
                inline_bytes_hex(font)
                    .ok_or_else(|| "missing inline hex bytes".to_string())?
                    .to_string(),
            );
        }
        Asset::Ref { .. } => {
            return Err(format!("unresolved shared asset ref {}", asset_label(font)));
        }
        Asset::Other(_) => return Err("unsupported font asset shape".to_string()),
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
        "set_pixel_sizes" => rust_set_pixel_sizes(case),
        "set_char_size" => rust_set_char_size(case),
        "size_metrics" => {
            let face = open_face(case)?;
            Ok(ok(size_metrics_json(&FT_Size_Metrics(&face))))
        }
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

fn run_c_abi(case: &InputCase) -> Result<RunOutput, String> {
    match case.operation.as_str() {
        "constant" | "record_layout" => run_rust_ffi(case),
        "new_memory_face" => {
            let (_library, face) = c_open_face(case)?;
            c_done_face(face);
            Ok(ok(json!({"opened": true})))
        }
        "set_pixel_sizes" => {
            let (_library, face) = c_open_face(case)?;
            c_done_face(face);
            Ok(ok(json!({"set": true})))
        }
        "set_char_size" => {
            let (library, face) = c_new_face_without_size(case)?;
            let err = c_abi::FT_Set_Char_Size(
                face,
                i64_param(&case.inputs.params, "char_width")?,
                i64_param(&case.inputs.params, "char_height")?,
                u32_param(&case.inputs.params, "horz_resolution")?,
                u32_param(&case.inputs.params, "vert_resolution")?,
            );
            c_done_face(face);
            c_done_library(library);
            if err == FT_Err_Ok {
                Ok(ok(json!({"set": true})))
            } else {
                Ok(error(err))
            }
        }
        "size_metrics" => {
            let (_library, face) = c_open_face(case)?;
            let metrics = c_abi::FT_Size_Metrics_Get(face);
            let output = json!({
                "x_ppem": metrics.x_ppem,
                "y_ppem": metrics.y_ppem,
                "x_scale": metrics.x_scale,
                "y_scale": metrics.y_scale,
                "ascender": metrics.ascender,
                "descender": metrics.descender,
                "height": metrics.height,
                "max_advance": metrics.max_advance
            });
            c_done_face(face);
            Ok(ok(output))
        }
        "get_char_index" => {
            let (_library, face) = c_open_face(case)?;
            let value =
                c_abi::FT_Get_Char_Index(face, u64_param(&case.inputs.params, "char_code")?);
            c_done_face(face);
            Ok(ok(json!({"value": value})))
        }
        "load_char" => {
            let (_library, face) = c_open_face(case)?;
            let err = c_abi::FT_Load_Char(
                face,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            if err == FT_Err_Ok {
                let value = c_slot_json(face)?;
                c_done_face(face);
                Ok(ok(value))
            } else {
                c_done_face(face);
                Ok(error(err))
            }
        }
        "load_glyph" => {
            let (_library, face) = c_open_face(case)?;
            let err = c_abi::FT_Load_Glyph(
                face,
                u32_param(&case.inputs.params, "glyph_index")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            if err == FT_Err_Ok {
                let value = c_slot_json(face)?;
                c_done_face(face);
                Ok(ok(value))
            } else {
                c_done_face(face);
                Ok(error(err))
            }
        }
        "render_glyph" => {
            let (_library, face) = c_open_face(case)?;
            let load_err = c_abi::FT_Load_Char(
                face,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            let err = if load_err == FT_Err_Ok {
                c_abi::fontdone_test_render_glyph(
                    face,
                    i32_param(&case.inputs.params, "render_mode")?,
                )
            } else {
                load_err
            };
            if err == FT_Err_Ok {
                let value = c_slot_json(face)?;
                c_done_face(face);
                Ok(ok(value))
            } else {
                c_done_face(face);
                Ok(error(err))
            }
        }
        other => Err(format!("unsupported c abi operation {other}")),
    }
}

fn run_wasm_abi(case: &InputCase) -> Result<RunOutput, String> {
    match case.operation.as_str() {
        "constant" | "record_layout" | "set_char_size" => run_rust_ffi(case),
        "new_memory_face" => {
            let handle = wasm_open_face(case)?;
            wasm_done_face(handle);
            Ok(ok(json!({"opened": true})))
        }
        "set_pixel_sizes" => {
            let handle = wasm_open_face(case)?;
            wasm_done_face(handle);
            Ok(ok(json!({"set": true})))
        }
        "size_metrics" => {
            let handle = wasm_open_face(case)?;
            let mut metrics = wasm_abi::FontdoneWasmSizeMetrics::default();
            let err = wasm_abi::fontdone_wasm_size_metrics(handle, &mut metrics);
            let output = if err == FT_Err_Ok {
                Ok(ok(json!({
                    "x_ppem": metrics.x_ppem,
                    "y_ppem": metrics.y_ppem,
                    "x_scale": metrics.x_scale,
                    "y_scale": metrics.y_scale,
                    "ascender": metrics.ascender,
                    "descender": metrics.descender,
                    "height": metrics.height,
                    "max_advance": metrics.max_advance
                })))
            } else {
                Ok(error(err))
            };
            wasm_done_face(handle);
            output
        }
        "get_char_index" => {
            let handle = wasm_open_face(case)?;
            let value = wasm_abi::fontdone_wasm_get_char_index(
                handle,
                u64_param(&case.inputs.params, "char_code")?,
            );
            wasm_done_face(handle);
            Ok(ok(json!({"value": value})))
        }
        "load_char" => {
            let handle = wasm_open_face(case)?;
            let err = wasm_abi::fontdone_wasm_load_char(
                handle,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            let output = wasm_slot_output(handle, err);
            wasm_done_face(handle);
            output
        }
        "load_glyph" => {
            let handle = wasm_open_face(case)?;
            let err = wasm_abi::fontdone_wasm_load_glyph(
                handle,
                u32_param(&case.inputs.params, "glyph_index")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            let output = wasm_slot_output(handle, err);
            wasm_done_face(handle);
            output
        }
        "render_glyph" => {
            let handle = wasm_open_face(case)?;
            let load_err = wasm_abi::fontdone_wasm_load_char(
                handle,
                u64_param(&case.inputs.params, "char_code")?,
                i32_param(&case.inputs.params, "load_flags")?,
            );
            let err = if load_err == FT_Err_Ok {
                wasm_abi::fontdone_wasm_render_glyph(
                    handle,
                    i32_param(&case.inputs.params, "render_mode")?,
                )
            } else {
                load_err
            };
            let output = wasm_slot_output(handle, err);
            wasm_done_face(handle);
            output
        }
        other => Err(format!("unsupported wasm abi operation {other}")),
    }
}

fn c_new_face_without_size(
    case: &InputCase,
) -> Result<(c_abi::FT_Library, c_abi::FT_Face), String> {
    let bytes = font_bytes(case)?;
    let mut library = std::ptr::null_mut();
    let err = c_abi::FT_Init_FreeType(&mut library);
    if err != FT_Err_Ok {
        return Err(format!("FT_Init_FreeType returned {err}"));
    }
    let mut face = std::ptr::null_mut();
    let file_size = i64::try_from(bytes.len()).map_err(|err| err.to_string())?;
    let err = c_abi::FT_New_Memory_Face(
        library,
        bytes.as_ptr(),
        file_size,
        i64_param(&case.inputs.params, "face_index")?,
        &mut face,
    );
    if err != FT_Err_Ok {
        c_done_library(library);
        return Err(format!("FT_New_Memory_Face returned {err}"));
    }
    Ok((library, face))
}

fn c_open_face(case: &InputCase) -> Result<(c_abi::FT_Library, c_abi::FT_Face), String> {
    let (library, face) = c_new_face_without_size(case)?;
    let size = case
        .inputs
        .params
        .get("pixel_size")
        .ok_or_else(|| "missing pixel_size".to_string())?;
    let err = c_abi::FT_Set_Pixel_Sizes(face, u32_param(size, "x")?, u32_param(size, "y")?);
    if err == FT_Err_Ok {
        Ok((library, face))
    } else {
        c_done_face(face);
        c_done_library(library);
        Err(format!("FT_Set_Pixel_Sizes returned {err}"))
    }
}

fn c_slot_json(face: c_abi::FT_Face) -> Result<Value, String> {
    let slot = c_abi::fontdone_test_slot_snapshot(face)
        .ok_or_else(|| "missing c glyph slot snapshot".to_string())?;
    let bitmap = slot.bitmap.as_ref().map_or(Value::Null, |bitmap| {
        json!({
            "width": bitmap.width,
            "rows": bitmap.rows,
            "pitch": bitmap.pitch,
            "pixel_mode": bitmap.pixel_mode,
            "num_grays": bitmap.num_grays,
            "left": bitmap.left,
            "top": bitmap.top,
            "buffer_hex": hex_bytes(&bitmap.buffer)
        })
    });
    Ok(json!({
        "glyph_index": slot.glyph_index,
        "format": slot.format,
        "advance": {"x": slot.advance.x, "y": slot.advance.y},
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
        "bitmap": bitmap
    }))
}

fn c_done_face(face: c_abi::FT_Face) {
    if !face.is_null() {
        let _ = c_abi::FT_Done_Face(face);
    }
}

fn c_done_library(library: c_abi::FT_Library) {
    if !library.is_null() {
        let _ = c_abi::FT_Done_FreeType(library);
    }
}

fn wasm_open_face(case: &InputCase) -> Result<usize, String> {
    let bytes = font_bytes(case)?;
    let status = wasm_abi::fontdone_wasm_open_face(
        bytes.as_ptr(),
        bytes.len(),
        i64_param(&case.inputs.params, "face_index")?,
        20.0,
    );
    if status.error != FT_Err_Ok {
        return Err(format!("fontdone_wasm_open_face returned {}", status.error));
    }
    let size = case
        .inputs
        .params
        .get("pixel_size")
        .ok_or_else(|| "missing pixel_size".to_string())?;
    let err = wasm_abi::fontdone_wasm_set_pixel_sizes(
        status.handle,
        u32_param(size, "x")?,
        u32_param(size, "y")?,
    );
    if err == FT_Err_Ok {
        Ok(status.handle)
    } else {
        wasm_done_face(status.handle);
        Err(format!("fontdone_wasm_set_pixel_sizes returned {err}"))
    }
}

fn wasm_slot_output(handle: usize, err: i32) -> Result<RunOutput, String> {
    if err != FT_Err_Ok {
        return Ok(error(err));
    }
    let slot = wasm_abi::fontdone_test_slot_snapshot(handle)
        .ok_or_else(|| "missing wasm glyph slot snapshot".to_string())?;
    let bitmap = slot.bitmap.as_ref().map_or(Value::Null, |bitmap| {
        json!({
            "width": bitmap.width,
            "rows": bitmap.rows,
            "pitch": bitmap.pitch,
            "pixel_mode": bitmap.pixel_mode,
            "num_grays": bitmap.num_grays,
            "left": bitmap.left,
            "top": bitmap.top,
            "buffer_hex": hex_bytes(&bitmap.buffer)
        })
    });
    Ok(ok(json!({
        "glyph_index": slot.glyph_index,
        "format": slot.format,
        "advance": {"x": slot.advance.x, "y": slot.advance.y},
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
        "bitmap": bitmap
    })))
}

fn wasm_done_face(handle: usize) {
    if handle != 0 {
        let _ = wasm_abi::fontdone_wasm_done_face(handle);
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

fn rust_set_pixel_sizes(case: &InputCase) -> Result<RunOutput, String> {
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
                Ok(ok(json!({"set": true})))
            } else {
                Ok(error(err))
            }
        }
        Err(err) => Ok(error(err)),
    }
}

fn rust_set_char_size(case: &InputCase) -> Result<RunOutput, String> {
    let data = font_bytes(case)?;
    let library = FT_Init_FreeType();
    match FT_New_Memory_Face(
        &library,
        &data,
        i64_param(&case.inputs.params, "face_index")?,
        20.0,
    ) {
        Ok(mut face) => {
            let err = FT_Set_Char_Size(
                &mut face,
                i64_param(&case.inputs.params, "char_width")?,
                i64_param(&case.inputs.params, "char_height")?,
                u32_param(&case.inputs.params, "horz_resolution")?,
                u32_param(&case.inputs.params, "vert_resolution")?,
            );
            if err == FT_Err_Ok {
                Ok(ok(json!({"set": true})))
            } else {
                Ok(error(err))
            }
        }
        Err(err) => Ok(error(err)),
    }
}

fn font_bytes(case: &InputCase) -> Result<Vec<u8>, String> {
    let font = case
        .inputs
        .assets
        .get("font")
        .ok_or_else(|| "missing font asset".to_string())?;
    match font {
        Asset::File { path, .. } => cached_file_bytes(path),
        Asset::InlineBytes { encoding, .. } => {
            if encoding != "hex" {
                return Err(format!("unsupported inline byte encoding {encoding}"));
            }
            let value =
                inline_bytes_hex(font).ok_or_else(|| "missing inline hex bytes".to_string())?;
            decode_hex(value)
        }
        Asset::Ref { .. } => Err(format!("unresolved shared asset ref {}", asset_label(font))),
        Asset::Other(_) => Err("unsupported font asset shape".to_string()),
    }
}

fn asset_label(asset: &Asset) -> String {
    match asset {
        Asset::Ref { id: Some(id), .. } => id.clone(),
        Asset::Ref {
            path: Some(path), ..
        } => path.clone(),
        Asset::Ref { .. } => "<anonymous>".to_string(),
        Asset::File { path, .. } => path.clone(),
        Asset::InlineBytes { .. } => "<inline-bytes>".to_string(),
        Asset::Other(_) => "<other-asset>".to_string(),
    }
}

fn inline_bytes_hex(asset: &Asset) -> Option<&str> {
    match asset {
        Asset::InlineBytes {
            value: Some(value), ..
        } => Some(value),
        Asset::InlineBytes {
            data: Some(data), ..
        } => Some(data),
        _ => None,
    }
}

fn cached_file_bytes(path: &str) -> Result<Vec<u8>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Vec<u8>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut cache = cache.lock().map_err(|err| err.to_string())?;
    if let Some(bytes) = cache.get(path) {
        return Ok(bytes.clone());
    }
    let bytes = fs::read(fixture_dir().join(path)).map_err(|err| format!("read {path}: {err}"))?;
    cache.insert(path.to_string(), bytes.clone());
    Ok(bytes)
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
        "FT_Err_Ok" => Ok(i64::from(FT_Err_Ok)),
        "FT_Err_Cannot_Open_Resource" => Ok(i64::from(FT_Err_Cannot_Open_Resource)),
        "FT_Err_Unknown_File_Format" => Ok(i64::from(FT_Err_Unknown_File_Format)),
        "FT_Err_Invalid_File_Format" => Ok(i64::from(FT_Err_Invalid_File_Format)),
        "FT_Err_Invalid_Argument" => Ok(i64::from(FT_Err_Invalid_Argument)),
        "FT_Err_Unimplemented_Feature" => Ok(i64::from(FT_Err_Unimplemented_Feature)),
        "FT_Err_Invalid_Table" => Ok(i64::from(FT_Err_Invalid_Table)),
        "FT_Err_Invalid_Glyph_Index" => Ok(i64::from(FT_Err_Invalid_Glyph_Index)),
        "FT_Err_Invalid_Character_Code" => Ok(i64::from(FT_Err_Invalid_Character_Code)),
        "FT_Err_Invalid_Glyph_Format" => Ok(i64::from(FT_Err_Invalid_Glyph_Format)),
        "FT_Err_Cannot_Render_Glyph" => Ok(i64::from(FT_Err_Cannot_Render_Glyph)),
        "FT_Err_Invalid_Outline" => Ok(i64::from(FT_Err_Invalid_Outline)),
        "FT_Err_Invalid_Pixel_Size" => Ok(i64::from(FT_Err_Invalid_Pixel_Size)),
        "FT_Err_Invalid_CharMap_Handle" => Ok(i64::from(FT_Err_Invalid_CharMap_Handle)),
        "FT_Err_Out_Of_Memory" => Ok(i64::from(FT_Err_Out_Of_Memory)),
        "FT_Err_Raster_Overflow" => Ok(i64::from(FT_Err_Raster_Overflow)),
        "FT_Err_Invalid_CharMap_Format" => Ok(i64::from(FT_Err_Invalid_CharMap_Format)),
        "FT_LOAD_DEFAULT" => Ok(i64::from(FT_LOAD_DEFAULT)),
        "FT_LOAD_NO_SCALE" => Ok(i64::from(FT_LOAD_NO_SCALE)),
        "FT_LOAD_NO_HINTING" => Ok(i64::from(FT_LOAD_NO_HINTING)),
        "FT_LOAD_RENDER" => Ok(i64::from(FT_LOAD_RENDER)),
        "FT_LOAD_NO_BITMAP" => Ok(i64::from(FT_LOAD_NO_BITMAP)),
        "FT_LOAD_VERTICAL_LAYOUT" => Ok(i64::from(FT_LOAD_VERTICAL_LAYOUT)),
        "FT_LOAD_FORCE_AUTOHINT" => Ok(i64::from(FT_LOAD_FORCE_AUTOHINT)),
        "FT_LOAD_CROP_BITMAP" => Ok(i64::from(FT_LOAD_CROP_BITMAP)),
        "FT_LOAD_PEDANTIC" => Ok(i64::from(FT_LOAD_PEDANTIC)),
        "FT_LOAD_ADVANCE_ONLY" => Ok(i64::from(FT_LOAD_ADVANCE_ONLY)),
        "FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH" => Ok(i64::from(FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH)),
        "FT_LOAD_NO_RECURSE" => Ok(i64::from(FT_LOAD_NO_RECURSE)),
        "FT_LOAD_IGNORE_TRANSFORM" => Ok(i64::from(FT_LOAD_IGNORE_TRANSFORM)),
        "FT_LOAD_MONOCHROME" => Ok(i64::from(FT_LOAD_MONOCHROME)),
        "FT_LOAD_LINEAR_DESIGN" => Ok(i64::from(FT_LOAD_LINEAR_DESIGN)),
        "FT_LOAD_SBITS_ONLY" => Ok(i64::from(FT_LOAD_SBITS_ONLY)),
        "FT_LOAD_NO_AUTOHINT" => Ok(i64::from(FT_LOAD_NO_AUTOHINT)),
        "FT_LOAD_COLOR" => Ok(i64::from(FT_LOAD_COLOR)),
        "FT_LOAD_COMPUTE_METRICS" => Ok(i64::from(FT_LOAD_COMPUTE_METRICS)),
        "FT_LOAD_BITMAP_METRICS_ONLY" => Ok(i64::from(FT_LOAD_BITMAP_METRICS_ONLY)),
        "FT_LOAD_SVG_ONLY" => Ok(i64::from(FT_LOAD_SVG_ONLY)),
        "FT_LOAD_NO_SVG" => Ok(i64::from(FT_LOAD_NO_SVG)),
        "FT_RENDER_MODE_NORMAL" => Ok(i64::from(FT_RENDER_MODE_NORMAL)),
        "FT_RENDER_MODE_LIGHT" => Ok(i64::from(FT_RENDER_MODE_LIGHT)),
        "FT_RENDER_MODE_MONO" => Ok(i64::from(FT_RENDER_MODE_MONO)),
        "FT_RENDER_MODE_LCD" => Ok(i64::from(FT_RENDER_MODE_LCD)),
        "FT_RENDER_MODE_LCD_V" => Ok(i64::from(FT_RENDER_MODE_LCD_V)),
        "FT_RENDER_MODE_SDF" => Ok(i64::from(FT_RENDER_MODE_SDF)),
        "FT_RENDER_MODE_MAX" => Ok(i64::from(FT_RENDER_MODE_MAX)),
        "FT_LOAD_TARGET_NORMAL" => Ok(i64::from(FT_LOAD_TARGET_NORMAL)),
        "FT_LOAD_TARGET_LIGHT" => Ok(i64::from(FT_LOAD_TARGET_LIGHT)),
        "FT_LOAD_TARGET_MONO" => Ok(i64::from(FT_LOAD_TARGET_MONO)),
        "FT_LOAD_TARGET_LCD" => Ok(i64::from(FT_LOAD_TARGET_LCD)),
        "FT_LOAD_TARGET_LCD_V" => Ok(i64::from(FT_LOAD_TARGET_LCD_V)),
        "FT_PIXEL_MODE_NONE" => Ok(i64::from(FT_PIXEL_MODE_NONE)),
        "FT_PIXEL_MODE_MONO" => Ok(i64::from(FT_PIXEL_MODE_MONO)),
        "FT_PIXEL_MODE_GRAY" => Ok(i64::from(FT_PIXEL_MODE_GRAY)),
        "FT_PIXEL_MODE_GRAY2" => Ok(i64::from(FT_PIXEL_MODE_GRAY2)),
        "FT_PIXEL_MODE_GRAY4" => Ok(i64::from(FT_PIXEL_MODE_GRAY4)),
        "FT_PIXEL_MODE_LCD" => Ok(i64::from(FT_PIXEL_MODE_LCD)),
        "FT_PIXEL_MODE_LCD_V" => Ok(i64::from(FT_PIXEL_MODE_LCD_V)),
        "FT_PIXEL_MODE_BGRA" => Ok(i64::from(FT_PIXEL_MODE_BGRA)),
        "FT_PIXEL_MODE_MAX" => Ok(i64::from(FT_PIXEL_MODE_MAX)),
        "FT_GLYPH_FORMAT_NONE" => Ok(i64::from(FT_GLYPH_FORMAT_NONE)),
        "FT_GLYPH_FORMAT_COMPOSITE" => Ok(i64::from(FT_GLYPH_FORMAT_COMPOSITE)),
        "FT_GLYPH_FORMAT_BITMAP" => Ok(i64::from(FT_GLYPH_FORMAT_BITMAP)),
        "FT_GLYPH_FORMAT_OUTLINE" => Ok(i64::from(FT_GLYPH_FORMAT_OUTLINE)),
        "FT_GLYPH_FORMAT_PLOTTER" => Ok(i64::from(FT_GLYPH_FORMAT_PLOTTER)),
        "FT_GLYPH_FORMAT_SVG" => Ok(i64::from(FT_GLYPH_FORMAT_SVG)),
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

fn size_metrics_json(metrics: &FT_Size_Metrics) -> Value {
    json!({
        "x_ppem": metrics.x_ppem,
        "y_ppem": metrics.y_ppem,
        "x_scale": metrics.x_scale,
        "y_scale": metrics.y_scale,
        "ascender": metrics.ascender,
        "descender": metrics.descender,
        "height": metrics.height,
        "max_advance": metrics.max_advance
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
    validate_schema_output(case, &oracle.output, "oracle")?;
    validate_schema_output(case, &actual.output, "actual")?;

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
        "face_open" => require_path(output, "/opened", label, case),
        "set_status" => require_path(output, "/set", label, case),
        "size_metrics" => {
            require_path(output, "/x_ppem", label, case)?;
            require_path(output, "/y_ppem", label, case)?;
            require_path(output, "/x_scale", label, case)?;
            require_path(output, "/y_scale", label, case)?;
            require_path(output, "/ascender", label, case)?;
            require_path(output, "/descender", label, case)?;
            require_path(output, "/height", label, case)?;
            require_path(output, "/max_advance", label, case)
        }
        "error" => {
            if output.is_null() {
                Ok(())
            } else {
                Err(format!(
                    "{} {label} error output must be null, got {output}",
                    case.case_id
                ))
            }
        }
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
