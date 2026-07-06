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
    cases: Vec<InputCase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InputCase {
    case_id: String,
    subject: String,
    case: String,
    #[serde(default)]
    covers_manifest_cases: Vec<String>,
    operation: String,
    schema: String,
    #[serde(default)]
    expect_error: bool,
    inputs: Inputs,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Inputs {
    #[serde(default)]
    assets: BTreeMap<String, Asset>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    variability: Option<VariabilitySpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct VariabilitySpec {
    #[serde(default)]
    axes: Vec<String>,
    #[serde(default)]
    fonts_folder: Option<String>,
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

#[derive(Debug)]
struct UnifiedCase {
    input: InputCase,
    runtime: RuntimeReadiness,
}

#[derive(Debug)]
enum RuntimeReadiness {
    Runnable { key: String },
    Pending { reason: String },
}

#[test]
fn unified_fixture_parity() {
    eprintln!("unified_fixture_parity: loading unified input cases");
    let input_cases = read_all_case_files();
    assert_unified_inputs_use_single_aggregate_model(&input_cases);
    let unified_cases = prepare_unified_cases(&input_cases);
    let unique_case_keys = unified_cases
        .iter()
        .map(|case| {
            (
                case.input.subject.clone(),
                case.input.case.clone(),
                case.input.case_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    eprintln!(
        "unified_cases: total={} deduped_case_keys={} runnable={} pending={}",
        unified_cases.len(),
        unique_case_keys.len(),
        unified_cases
            .iter()
            .filter(|case| matches!(case.runtime, RuntimeReadiness::Runnable { .. }))
            .count(),
        unified_cases
            .iter()
            .filter(|case| matches!(case.runtime, RuntimeReadiness::Pending { .. }))
            .count()
    );
    assert_manifest_cases_cover_fixture_inputs(&unified_cases);
    assert_manifest_font_variability_cases_cover_declared_fixture_folder(&unified_cases);
    assert_unified_variability_cases_have_single_model(&unified_cases);
    assert_unified_fixture_cases_match_runtime_c_oracle(&unified_cases);
}

fn assert_unified_fixture_cases_match_runtime_c_oracle(all_cases: &[UnifiedCase]) {
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
        "runtime_cases: runnable={} pending={} pending_reasons={}",
        cases.len(),
        runtime_selection.model_only,
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

fn assert_unified_inputs_use_single_aggregate_model(cases: &[InputCase]) {
    let mut failures = Vec::new();
    for case in cases {
        if case.schema == "scalar" {
            failures.push(format!("{} uses migration schema scalar", case.case_id));
        }
        if case.schema.ends_with("_matrix") {
            failures.push(format!(
                "{} uses legacy matrix schema {}",
                case.case_id, case.schema
            ));
        }
        if case.operation.ends_with("_matrix") {
            failures.push(format!(
                "{} uses legacy matrix operation {}",
                case.case_id, case.operation
            ));
        }
        if case
            .inputs
            .params
            .as_object()
            .is_some_and(|params| params.contains_key("load_flags_matrix"))
        {
            failures.push(format!(
                "{} uses load_flags_matrix; use load_flag_sets",
                case.case_id
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "legacy fixture model entries:\n{}",
        failures.join("\n")
    );
}

struct RuntimeSelection<'a> {
    executable: Vec<&'a InputCase>,
    model_only: usize,
    unsupported_operations: BTreeMap<String, usize>,
}

fn prepare_unified_cases(cases: &[InputCase]) -> Vec<UnifiedCase> {
    let mut unified = Vec::new();
    for case in cases {
        for expanded in expand_input_case(case) {
            let canonical_operation = canonical_operation(&expanded.operation);
            let runtime = classify_runtime_case(&expanded, canonical_operation);
            unified.push(UnifiedCase {
                input: expanded,
                runtime,
            });
        }
    }
    unified
}

const DEFAULT_VARIABILITY_SIZES: &[u32] = &[10, 20];
const DEFAULT_VARIABILITY_CODEPOINTS: &[u64] = &[33, 65, 103, 109];
const DEFAULT_VARIABILITY_GLYPH_INDICES: &[u32] = &[0, 1, 36, 57];
const DEFAULT_VARIABILITY_LOAD_FLAGS: &[i32] = &[FT_LOAD_DEFAULT];
const DEFAULT_VARIABILITY_RENDER_MODES: &[i32] = &[FT_RENDER_MODE_NORMAL];

#[derive(Clone, Default)]
struct ExpansionPlan {
    fonts: Vec<Option<Asset>>,
    sizes: Vec<Option<u32>>,
    codepoints: Vec<Option<u64>>,
    glyph_indices: Vec<Option<u32>>,
    load_flags: Vec<Option<i32>>,
    render_modes: Vec<Option<i32>>,
}

impl ExpansionPlan {
    fn is_identity(&self) -> bool {
        self.fonts.len() == 1
            && self.fonts[0].is_none()
            && self.sizes.len() == 1
            && self.sizes[0].is_none()
            && self.codepoints.len() == 1
            && self.codepoints[0].is_none()
            && self.glyph_indices.len() == 1
            && self.glyph_indices[0].is_none()
            && self.load_flags.len() == 1
            && self.load_flags[0].is_none()
            && self.render_modes.len() == 1
            && self.render_modes[0].is_none()
    }
}

fn expand_input_case(case: &InputCase) -> Vec<InputCase> {
    let plan = expansion_plan(case);
    if plan.is_identity() {
        return vec![case.clone()];
    }

    let mut expanded = Vec::new();
    for font in &plan.fonts {
        for size in &plan.sizes {
            for codepoint in &plan.codepoints {
                for glyph_index in &plan.glyph_indices {
                    for load_flags in &plan.load_flags {
                        for render_mode in &plan.render_modes {
                            let mut case = case.clone();
                            apply_expansion_axes(
                                &mut case,
                                font.clone(),
                                *size,
                                *codepoint,
                                *glyph_index,
                                *load_flags,
                                *render_mode,
                            );
                            expanded.push(case);
                        }
                    }
                }
            }
        }
    }
    expanded
}

fn expansion_plan(case: &InputCase) -> ExpansionPlan {
    let axes = variability_axes(case);
    let params = &case.inputs.params;
    let mut plan = ExpansionPlan {
        fonts: font_axis(case, &axes),
        sizes: u32_axis(
            params,
            &["sizes", "pixel_sizes"],
            axes.contains("sizes"),
            DEFAULT_VARIABILITY_SIZES,
        )
        .into_iter()
        .map(Some)
        .collect(),
        codepoints: u64_axis(
            params,
            &["codepoints", "char_codes"],
            axes.contains("codepoints"),
            DEFAULT_VARIABILITY_CODEPOINTS,
        )
        .into_iter()
        .map(Some)
        .collect(),
        glyph_indices: u32_axis(
            params,
            &["glyph_indices"],
            axes.contains("glyph_indices"),
            DEFAULT_VARIABILITY_GLYPH_INDICES,
        )
        .into_iter()
        .map(Some)
        .collect(),
        load_flags: load_flags_axis(params, axes.contains("load_flags"))
            .into_iter()
            .map(Some)
            .collect(),
        render_modes: render_modes_axis(
            params,
            axes.contains("render_modes"),
            DEFAULT_VARIABILITY_RENDER_MODES,
        )
        .into_iter()
        .map(Some)
        .collect(),
    };
    normalize_empty_axes(&mut plan);
    plan
}

fn normalize_empty_axes(plan: &mut ExpansionPlan) {
    if plan.fonts.is_empty() {
        plan.fonts.push(None);
    }
    if plan.sizes.is_empty() {
        plan.sizes.push(None);
    }
    if plan.codepoints.is_empty() {
        plan.codepoints.push(None);
    }
    if plan.glyph_indices.is_empty() {
        plan.glyph_indices.push(None);
    }
    if plan.load_flags.is_empty() {
        plan.load_flags.push(None);
    }
    if plan.render_modes.is_empty() {
        plan.render_modes.push(None);
    }
}

fn variability_axes(case: &InputCase) -> BTreeSet<&str> {
    case.inputs
        .variability
        .as_ref()
        .map(|variability| {
            variability
                .axes
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn font_axis(case: &InputCase, axes: &BTreeSet<&str>) -> Vec<Option<Asset>> {
    let folder = case
        .inputs
        .variability
        .as_ref()
        .and_then(|variability| variability.fonts_folder.as_deref())
        .map(ToString::to_string)
        .or_else(|| {
            file_asset_path(case.inputs.assets.get("font_folder")).map(ToString::to_string)
        });
    if !axes.contains("fonts") && folder.is_none() {
        return Vec::new();
    }
    let folder = folder.unwrap_or_else(|| "input/fonts".to_string());
    fixture_font_assets(&folder).into_iter().map(Some).collect()
}

fn fixture_font_assets(folder: &str) -> Vec<Asset> {
    let folder_path = fixture_dir().join(folder);
    let Ok(entries) = fs::read_dir(&folder_path) else {
        return Vec::new();
    };
    let mut assets = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension().is_some_and(|extension| {
                matches!(
                    extension.to_string_lossy().as_ref(),
                    "ttf" | "otf" | "ttc" | "pfb" | "bdf" | "otb"
                )
            })
        })
        .filter_map(|path| {
            let relative = path
                .strip_prefix(fixture_dir())
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some(Asset::File {
                path: relative,
                sha256: None,
                length: None,
            })
        })
        .collect::<Vec<_>>();
    assets.sort_by_key(asset_label);
    assets
}

fn file_asset_path(asset: Option<&Asset>) -> Option<&str> {
    match asset {
        Some(Asset::File { path, .. }) => Some(path),
        Some(Asset::Ref { id: Some(id), .. }) => Some(id),
        Some(Asset::Ref {
            path: Some(path), ..
        }) => Some(path),
        _ => None,
    }
}

fn u32_axis(params: &Value, keys: &[&str], enabled: bool, defaults: &[u32]) -> Vec<u32> {
    for key in keys {
        if let Some(values) = numeric_array(params, key) {
            return values
                .iter()
                .map(|value| u32_value(value, key))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
        }
    }
    if enabled {
        defaults.to_vec()
    } else {
        Vec::new()
    }
}

fn u64_axis(params: &Value, keys: &[&str], enabled: bool, defaults: &[u64]) -> Vec<u64> {
    for key in keys {
        if let Some(values) = numeric_array(params, key) {
            return values
                .iter()
                .map(|value| u64_value(value, key))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
        }
    }
    if enabled {
        defaults.to_vec()
    } else {
        Vec::new()
    }
}

fn load_flags_axis(params: &Value, enabled: bool) -> Vec<i32> {
    if let Some(values) = params.get("load_flag_sets").and_then(Value::as_array) {
        let base_flags = values
            .iter()
            .map(|value| flag_value(value, "load_flag_sets"))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        return combine_load_targets(base_flags, target_mode_axis(params));
    }
    if let Some(values) = params.get("load_flags").and_then(Value::as_array) {
        if enabled || values.iter().all(|value| value.is_number()) {
            let base_flags = values
                .iter()
                .map(|value| flag_value(value, "load_flags"))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
            return combine_load_targets(base_flags, target_mode_axis(params));
        }
    }
    if enabled {
        combine_load_targets(
            DEFAULT_VARIABILITY_LOAD_FLAGS.to_vec(),
            target_mode_axis(params),
        )
    } else if params.get("target_modes").is_some() {
        combine_load_targets(vec![FT_LOAD_DEFAULT], target_mode_axis(params))
    } else {
        Vec::new()
    }
}

fn combine_load_targets(base_flags: Vec<i32>, target_modes: Vec<i32>) -> Vec<i32> {
    if target_modes.is_empty() {
        return base_flags;
    }
    let bases = if base_flags.is_empty() {
        vec![FT_LOAD_DEFAULT]
    } else {
        base_flags
    };
    let mut combined = Vec::new();
    for base in bases {
        for target in &target_modes {
            let value = base | target;
            if !combined.contains(&value) {
                combined.push(value);
            }
        }
    }
    combined
}

fn target_mode_axis(params: &Value) -> Vec<i32> {
    let Some(values) = params.get("target_modes").and_then(Value::as_array) else {
        return Vec::new();
    };
    values
        .iter()
        .map(load_target_mode_value)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
}

fn render_modes_axis(params: &Value, enabled: bool, defaults: &[i32]) -> Vec<i32> {
    if let Some(values) = params.get("render_modes").and_then(Value::as_array) {
        return values
            .iter()
            .map(render_mode_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
    }
    if enabled {
        defaults.to_vec()
    } else {
        Vec::new()
    }
}

fn numeric_array<'a>(params: &'a Value, key: &str) -> Option<&'a [Value]> {
    params.get(key)?.as_array().map(Vec::as_slice)
}

fn flag_value(value: &Value, key: &str) -> Result<i32, String> {
    let raw = match value {
        Value::Array(items) => {
            let mut flags = 0i64;
            for item in items {
                flags |= i64_value(item, key)?;
            }
            flags
        }
        _ => i64_value(value, key)?,
    };
    i32::try_from(raw).map_err(|err| format!("{key} does not fit i32: {err}"))
}

fn load_target_mode_value(value: &Value) -> Result<i32, String> {
    if let Some(text) = value.as_str() {
        return match text {
            "NORMAL" | "FT_LOAD_TARGET_NORMAL" => Ok(FT_LOAD_TARGET_NORMAL),
            "LIGHT" | "FT_LOAD_TARGET_LIGHT" => Ok(FT_LOAD_TARGET_LIGHT),
            "MONO" | "FT_LOAD_TARGET_MONO" => Ok(FT_LOAD_TARGET_MONO),
            "LCD" | "FT_LOAD_TARGET_LCD" => Ok(FT_LOAD_TARGET_LCD),
            "LCD_V" | "FT_LOAD_TARGET_LCD_V" => Ok(FT_LOAD_TARGET_LCD_V),
            _ => i64_value(value, "target_modes").and_then(|value| {
                i32::try_from(value).map_err(|err| format!("target_modes does not fit i32: {err}"))
            }),
        };
    }
    let raw = i64_value(value, "target_modes")?;
    i32::try_from(raw).map_err(|err| format!("target_modes does not fit i32: {err}"))
}

fn render_mode_value(value: &Value) -> Result<i32, String> {
    if let Some(text) = value.as_str() {
        return match text {
            "NORMAL" | "FT_RENDER_MODE_NORMAL" => Ok(FT_RENDER_MODE_NORMAL),
            "LIGHT" | "FT_RENDER_MODE_LIGHT" => Ok(FT_RENDER_MODE_LIGHT),
            "MONO" | "FT_RENDER_MODE_MONO" => Ok(FT_RENDER_MODE_MONO),
            "LCD" | "FT_RENDER_MODE_LCD" => Ok(FT_RENDER_MODE_LCD),
            "LCD_V" | "FT_RENDER_MODE_LCD_V" => Ok(FT_RENDER_MODE_LCD_V),
            "SDF" | "FT_RENDER_MODE_SDF" => Ok(FT_RENDER_MODE_SDF),
            _ => i64_value(value, "render_modes").and_then(|value| {
                i32::try_from(value).map_err(|err| format!("render_modes does not fit i32: {err}"))
            }),
        };
    }
    let raw = i64_value(value, "render_modes")?;
    i32::try_from(raw).map_err(|err| format!("render_modes does not fit i32: {err}"))
}

fn apply_expansion_axes(
    case: &mut InputCase,
    font: Option<Asset>,
    size: Option<u32>,
    codepoint: Option<u64>,
    glyph_index: Option<u32>,
    load_flags: Option<i32>,
    render_mode: Option<i32>,
) {
    let mut suffix = Vec::new();
    if let Some(font) = font {
        suffix.push(format!("font={}", asset_label(&font)));
        case.inputs.assets.insert("font".to_string(), font);
        case.inputs.assets.remove("font_folder");
    }
    let params = case
        .inputs
        .params
        .as_object_mut()
        .expect("fixture input params must be a JSON object");
    remove_aggregate_params(params);
    if let Some(size) = size {
        suffix.push(format!("size={size}"));
        params.insert("pixel_size".to_string(), json!({"x": 0, "y": size}));
    }
    if let Some(codepoint) = codepoint {
        suffix.push(format!("cp={codepoint}"));
        params.insert("char_code".to_string(), Value::from(codepoint));
    }
    if let Some(glyph_index) = glyph_index {
        suffix.push(format!("gid={glyph_index}"));
        params.insert("glyph_index".to_string(), Value::from(glyph_index));
    }
    if let Some(load_flags) = load_flags {
        suffix.push(format!("flags={load_flags}"));
        params.insert("load_flags".to_string(), Value::from(load_flags));
    }
    if let Some(render_mode) = render_mode {
        suffix.push(format!("mode={render_mode}"));
        params.insert("render_mode".to_string(), Value::from(render_mode));
    }
    if !suffix.is_empty() {
        case.case_id = format!("{}#{}", case.case_id, suffix.join(";"));
    }
}

fn remove_aggregate_params(params: &mut serde_json::Map<String, Value>) {
    for key in [
        "sizes",
        "pixel_sizes",
        "codepoints",
        "char_codes",
        "glyph_indices",
        "load_flag_sets",
        "render_modes",
        "target_modes",
    ] {
        params.remove(key);
    }
}

fn classify_runtime_case(case: &InputCase, canonical_operation: &str) -> RuntimeReadiness {
    if !is_supported_runtime_operation(case, canonical_operation) {
        return RuntimeReadiness::Pending {
            reason: canonical_operation.to_string(),
        };
    }
    match oracle_args(case) {
        Ok(args) => RuntimeReadiness::Runnable {
            key: args.join("\t"),
        },
        Err(err) => RuntimeReadiness::Pending {
            reason: format!("{canonical_operation}:{err}"),
        },
    }
}

fn select_runtime_cases(cases: &[UnifiedCase]) -> RuntimeSelection<'_> {
    let filter = case_filter();
    let limit = case_limit();
    let mut executable = Vec::new();
    let mut model_only = 0usize;
    let mut unsupported_operations = BTreeMap::new();
    let mut seen_executable = BTreeSet::new();

    for case in cases {
        if !case_matches_filter(&case.input, filter.as_deref()) {
            continue;
        }
        match &case.runtime {
            RuntimeReadiness::Runnable { key } => {
                if seen_executable.insert(key.clone()) {
                    executable.push(&case.input);
                    if limit.is_some_and(|limit| executable.len() >= limit) {
                        break;
                    }
                }
            }
            RuntimeReadiness::Pending { reason } => {
                model_only = model_only.saturating_add(1);
                *unsupported_operations.entry(reason.clone()).or_default() += 1;
            }
        }
    }

    RuntimeSelection {
        executable,
        model_only,
        unsupported_operations,
    }
}

fn is_supported_runtime_operation(case: &InputCase, canonical_operation: &str) -> bool {
    match canonical_operation {
        "constant" => case
            .inputs
            .params
            .get("symbol")
            .and_then(Value::as_str)
            .is_some_and(is_supported_runtime_constant),
        "record_layout" => record_param(&case.inputs.params).is_ok_and(is_supported_runtime_layout),
        "new_memory_face" | "set_pixel_sizes" | "set_char_size" | "size_metrics"
        | "get_char_index" | "load_char" | "load_glyph" | "render_glyph" => {
            has_runtime_font_source(case) && assets_are_runtime_resolved(case)
        }
        _ => false,
    }
}

fn has_runtime_font_source(case: &InputCase) -> bool {
    runtime_font_asset(case).is_some_and(|font| match font {
        Asset::File { path, .. } => fixture_dir().join(path).is_file(),
        Asset::InlineBytes { encoding, .. } => encoding == "hex",
        Asset::Ref { .. } | Asset::Other(_) => false,
    })
}

fn assets_are_runtime_resolved(case: &InputCase) -> bool {
    case.inputs.assets.values().all(|asset| match asset {
        Asset::File { path, .. } => fixture_dir().join(path).is_file(),
        Asset::InlineBytes { encoding, .. } => {
            encoding == "hex" && inline_bytes_hex(asset).is_some()
        }
        Asset::Ref { .. } | Asset::Other(_) => false,
    })
}

fn canonical_operation(operation: &str) -> &str {
    match operation {
        "constant"
        | "constant.value"
        | "constant.import"
        | "constant_eval"
        | "error_constant.value"
        | "public_api.constant_value"
        | "abi.constant_eval"
        | "abi.constant_value"
        | "abi.error_code_value_and_import"
        | "abi.enum_variant_value"
        | "abi.macro_value_and_import"
        | "constant.alias_value" => "constant",
        "record_layout" | "abi.record_layout" | "abi.layout_probe" => "record_layout",
        "new_memory_face"
        | "freetype.new_memory_face"
        | "freetype.open_face"
        | "FT_New_Memory_Face" => "new_memory_face",
        "set_pixel_sizes" | "freetype.set_pixel_sizes" | "FT_Set_Pixel_Sizes" => "set_pixel_sizes",
        "set_char_size" | "freetype.set_char_size" | "FT_Set_Char_Size" => "set_char_size",
        "size_metrics" | "freetype.size_metrics" | "FT_Size_Metrics" => "size_metrics",
        "get_char_index" | "freetype.get_char_index" | "FT_Get_Char_Index" => "get_char_index",
        "load_char" | "freetype.load_char" | "FT_Load_Char" => "load_char",
        "load_glyph" | "freetype.load_glyph" | "FT_Load_Glyph" => "load_glyph",
        "render_glyph" | "freetype.render_glyph" | "FT_Render_Glyph" => "render_glyph",
        _ => operation,
    }
}

fn is_supported_runtime_layout(record: &str) -> bool {
    rust_layout(record).is_ok()
}

fn is_supported_runtime_constant(symbol: &str) -> bool {
    rust_constant(symbol).is_ok()
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
        case.case_id.contains(needle) || case.subject.contains(needle) || case.case.contains(needle)
    })
}

fn assert_manifest_cases_cover_fixture_inputs(cases: &[UnifiedCase]) {
    let manifest = read_manifest();
    let mut covered = BTreeSet::new();

    for case in cases {
        assert!(
            manifest.has_case(&case.input.subject, &case.input.case),
            "{} references unknown manifest case {}::{}",
            case.input.case_id,
            case.input.subject,
            case.input.case
        );
        covered.insert((case.input.subject.clone(), case.input.case.clone()));
        for covered_case in &case.input.covers_manifest_cases {
            assert!(
                manifest.has_case(&case.input.subject, covered_case),
                "{} references unknown covered manifest case {}::{}",
                case.input.case_id,
                case.input.subject,
                covered_case
            );
            covered.insert((case.input.subject.clone(), covered_case.clone()));
        }
    }
    eprintln!(
        "manifest_coverage: checked_cases={} covered_manifest_cases={}",
        cases.len(),
        covered.len()
    );
}

fn assert_manifest_font_variability_cases_cover_declared_fixture_folder(cases: &[UnifiedCase]) {
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
                                if !cases.iter().any(|input| {
                                    input_covers_font_variability(&input.input, &probe)
                                }) {
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

fn assert_unified_variability_cases_have_single_model(cases: &[UnifiedCase]) {
    let expanded = cases
        .iter()
        .filter(|case| case.input.case_id.contains('#'))
        .count();
    let aggregate_subjects = cases
        .iter()
        .filter(|case| case.input.case_id.contains('#'))
        .map(|case| case.input.subject.as_str())
        .collect::<BTreeSet<_>>();
    eprintln!(
        "variability_expansion: expanded_cases={} aggregate_subjects={}",
        expanded,
        aggregate_subjects.len()
    );
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
    read_case_files(None, None)
}

fn read_case_files(filter: Option<&str>, limit: Option<usize>) -> Vec<InputCase> {
    let input_dir = fixture_dir().join("inputs").join("public-api");
    let mut paths = input_case_paths(&input_dir);
    paths.sort();

    let mut cases = Vec::new();
    for path in paths {
        let text = fs::read_to_string(&path).expect("read input case file");
        let raw: Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
        assert!(
            raw.get("matrix_cases").is_none(),
            "{} uses legacy matrix_cases; move coverage into cases[].inputs.variability",
            path.display()
        );
        let parsed: CaseFile = serde_json::from_value(raw)
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

fn resolve_case_assets(
    _path: &Path,
    shared_assets: &BTreeMap<String, Asset>,
    case: &mut InputCase,
) {
    for asset in case.inputs.assets.values_mut() {
        let Asset::Ref { id, path } = asset else {
            continue;
        };
        if let Some(id) = id.as_deref() {
            if let Some(resolved) = shared_assets.get(id) {
                *asset = resolved.clone();
                continue;
            }
        }
        let reference = id.as_deref().or(path.as_deref());
        if let Some(reference) = reference {
            if let Some(path) = resolve_fixture_asset_ref(reference) {
                *asset = Asset::File {
                    path,
                    sha256: None,
                    length: None,
                };
            }
        }
    }
}

fn resolve_fixture_asset_ref(reference: &str) -> Option<String> {
    let candidates = [
        reference.to_string(),
        format!("input/{reference}"),
        reference
            .strip_prefix("fixtures/assets/")
            .map_or_else(|| reference.to_string(), ToString::to_string),
    ];
    candidates
        .into_iter()
        .find(|candidate| fixture_dir().join(candidate).is_file())
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
    pixel_size_param(&input.inputs.params).ok().map(|(_x, y)| y)
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
    if key == "load_flags" {
        return load_flags_param(&input.inputs.params).ok();
    }
    if key == "render_mode" {
        return render_mode_param(&input.inputs.params).ok();
    }
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
    match canonical_operation(&case.operation) {
        "constant" => Ok(vec![
            "--constant".to_string(),
            string_param(params, "symbol")?.to_string(),
        ]),
        "record_layout" => Ok(vec![
            "--layout".to_string(),
            record_param(params)?.to_string(),
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
            args.push(face_index_param(params)?.to_string());
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
            args.push(load_flags_param(params)?.to_string());
            Ok(args)
        }
        "load_glyph" => {
            let mut args = vec!["--load-glyph".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(glyph_index_param(params)?.to_string());
            args.push(load_flags_param(params)?.to_string());
            Ok(args)
        }
        "render_glyph" => {
            let mut args = vec!["--render-glyph".to_string()];
            push_font_source(case, &mut args)?;
            push_face_size(params, &mut args)?;
            args.push(u64_param(params, "char_code")?.to_string());
            args.push(load_flags_param(params)?.to_string());
            args.push(render_mode_param(params)?.to_string());
            Ok(args)
        }
        other => Err(format!("unsupported oracle operation {other}")),
    }
}

fn push_font_source(case: &InputCase, args: &mut Vec<String>) -> Result<(), String> {
    let font = runtime_font_asset(case).ok_or_else(|| "missing font asset".to_string())?;
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
    args.push(face_index_param(params)?.to_string());
    let (x, y) = pixel_size_param(params)?;
    args.push(x.to_string());
    args.push(y.to_string());
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
    match canonical_operation(&case.operation) {
        "constant" => Ok(ok(json!({
            "value": rust_constant(string_param(&case.inputs.params, "symbol")?)?
        }))),
        "record_layout" => Ok(ok(rust_layout(record_param(&case.inputs.params)?)?)),
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
                load_flags_param(&case.inputs.params)?,
            ) {
                Ok(slot) => Ok(ok(slot_json(&slot))),
                Err(err) => Ok(error(err)),
            }
        }
        "load_glyph" => {
            let face = open_face(case)?;
            match FT_Load_Glyph(
                &face,
                glyph_index_param(&case.inputs.params)?,
                load_flags_param(&case.inputs.params)?,
            ) {
                Ok(slot) => Ok(ok(slot_json(&slot))),
                Err(err) => Ok(error(err)),
            }
        }
        "render_glyph" => {
            let face = open_face(case)?;
            let render_mode = render_mode_param(&case.inputs.params)?;
            let loaded = FT_Load_Char(
                &face,
                u64_param(&case.inputs.params, "char_code")?,
                load_flags_param(&case.inputs.params)?,
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
    match canonical_operation(&case.operation) {
        "constant" | "record_layout" => run_rust_ffi(case),
        "new_memory_face" => {
            let bytes = font_bytes(case)?;
            let mut library = std::ptr::null_mut();
            let err = c_abi::FT_Init_FreeType(&mut library);
            if err != FT_Err_Ok {
                return Ok(error(err));
            }
            let mut face = std::ptr::null_mut();
            let file_size = i64::try_from(bytes.len()).map_err(|err| err.to_string())?;
            let err = c_abi::FT_New_Memory_Face(
                library,
                bytes.as_ptr(),
                file_size,
                face_index_param(&case.inputs.params)?,
                &mut face,
            );
            if err != FT_Err_Ok {
                c_done_library(library);
                return Ok(error(err));
            }
            let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
            let err = c_abi::FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
            c_done_face(face);
            c_done_library(library);
            if err == FT_Err_Ok {
                Ok(ok(json!({"opened": true})))
            } else {
                Ok(error(err))
            }
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
                load_flags_param(&case.inputs.params)?,
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
                glyph_index_param(&case.inputs.params)?,
                load_flags_param(&case.inputs.params)?,
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
                load_flags_param(&case.inputs.params)?,
            );
            let err = if load_err == FT_Err_Ok {
                c_abi::fontdone_test_render_glyph(face, render_mode_param(&case.inputs.params)?)
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
    match canonical_operation(&case.operation) {
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
                load_flags_param(&case.inputs.params)?,
            );
            let output = wasm_slot_output(handle, err);
            wasm_done_face(handle);
            output
        }
        "load_glyph" => {
            let handle = wasm_open_face(case)?;
            let err = wasm_abi::fontdone_wasm_load_glyph(
                handle,
                glyph_index_param(&case.inputs.params)?,
                load_flags_param(&case.inputs.params)?,
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
                load_flags_param(&case.inputs.params)?,
            );
            let err = if load_err == FT_Err_Ok {
                wasm_abi::fontdone_wasm_render_glyph(
                    handle,
                    render_mode_param(&case.inputs.params)?,
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
        face_index_param(&case.inputs.params)?,
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
    let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
    let err = c_abi::FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
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
        face_index_param(&case.inputs.params)?,
        20.0,
    );
    if status.error != FT_Err_Ok {
        return Err(format!("fontdone_wasm_open_face returned {}", status.error));
    }
    let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
    let err = wasm_abi::fontdone_wasm_set_pixel_sizes(status.handle, pixel_width, pixel_height);
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
        face_index_param(&case.inputs.params)?,
        20.0,
    ) {
        Ok(mut face) => {
            let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
            let err = FT_Set_Pixel_Sizes(&mut face, pixel_width, pixel_height);
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
        face_index_param(&case.inputs.params)?,
        20.0,
    )
    .map_err(|err| format!("FT_New_Memory_Face returned {err}"))?;
    let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
    let err = FT_Set_Pixel_Sizes(&mut face, pixel_width, pixel_height);
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
        face_index_param(&case.inputs.params)?,
        20.0,
    ) {
        Ok(mut face) => {
            let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
            let err = FT_Set_Pixel_Sizes(&mut face, pixel_width, pixel_height);
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
        face_index_param(&case.inputs.params)?,
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
    let font = runtime_font_asset(case).ok_or_else(|| "missing font asset".to_string())?;
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

fn runtime_font_asset(case: &InputCase) -> Option<&Asset> {
    ["font", "font_bytes", "fixture", "blob"]
        .into_iter()
        .find_map(|key| case.inputs.assets.get(key))
        .or_else(|| {
            let mut assets = case.inputs.assets.values();
            let first = assets.next()?;
            if assets.next().is_none() {
                Some(first)
            } else {
                None
            }
        })
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
    if oracle.status.kind == StatusKind::Error {
        return if oracle.output == actual.output {
            Ok(())
        } else {
            Err(format!(
                "{} error output mismatch: oracle={} actual={}",
                case.case_id, oracle.output, actual.output
            ))
        };
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
    match comparison_schema(case) {
        "constant" => require_path(output, "/value", label, case),
        "record_layout" => {
            require_path(output, "/record", label, case)?;
            require_path(output, "/size", label, case)?;
            require_path(output, "/align", label, case)?;
            require_path(output, "/fields", label, case)
        }
        "value" => require_path(output, "/value", label, case),
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

fn comparison_schema(case: &InputCase) -> &str {
    match case.schema.as_str() {
        "constant" | "api_constant" | "abi_constant" | "constant_value" => "constant",
        "record_layout" | "abi_layout" | "abi_record_layout" | "abi_record" | "api_record"
        | "c_abi_record" | "c_abi_layout" => "record_layout",
        "face_open" | "face_result" | "face_handle" => "face_open",
        "glyph_slot" | "glyph_slot_bitmap" | "glyph_render" | "bitmap_result" => "glyph_slot",
        "api_result" => match canonical_operation(&case.operation) {
            "constant" => "value",
            "new_memory_face" => "face_open",
            "set_pixel_sizes" | "set_char_size" => "set_status",
            "size_metrics" => "size_metrics",
            "get_char_index" => "value",
            "load_char" | "load_glyph" | "render_glyph" => "glyph_slot",
            _ => case.schema.as_str(),
        },
        other => other,
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

fn record_param(value: &Value) -> Result<&str, String> {
    ["record", "type", "symbol"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .ok_or_else(|| "missing record/type/symbol param".to_string())
}

fn face_index_param(value: &Value) -> Result<i64, String> {
    value
        .get("face_index")
        .map_or(Ok(0), |raw| i64_value(raw, "face_index"))
}

fn pixel_size_param(value: &Value) -> Result<(u32, u32), String> {
    if let Some(size) = value.get("pixel_size") {
        if let Some(object) = size.as_object() {
            return Ok((
                u32_param_object(object, "x")?,
                u32_param_object(object, "y")?,
            ));
        }
        let y = u32_value(size, "pixel_size")?;
        return Ok((0, y));
    }
    for key in ["size", "size_ppem"] {
        if let Some(size) = value.get(key) {
            return Ok((0, u32_value(size, key)?));
        }
    }
    Ok((0, 20))
}

fn load_flags_param(value: &Value) -> Result<i32, String> {
    let Some(raw) = value.get("load_flags") else {
        return Ok(FT_LOAD_DEFAULT);
    };
    let flags = match raw {
        Value::Array(items) => {
            let mut flags = 0i64;
            for item in items {
                if item.is_array() {
                    return Err("load_flags contains aggregate flag sets".to_string());
                }
                flags |= i64_value(item, "load_flags")?;
            }
            flags
        }
        _ => i64_value(raw, "load_flags")?,
    };
    i32::try_from(flags).map_err(|err| format!("load_flags does not fit i32: {err}"))
}

fn render_mode_param(value: &Value) -> Result<i32, String> {
    value
        .get("render_mode")
        .map_or(Ok(FT_RENDER_MODE_NORMAL), |raw| {
            let mode = i64_value(raw, "render_mode")?;
            i32::try_from(mode).map_err(|err| format!("render_mode does not fit i32: {err}"))
        })
}

fn glyph_index_param(value: &Value) -> Result<u32, String> {
    let raw = value
        .get("glyph_index")
        .ok_or_else(|| "missing glyph_index".to_string())?;
    u32_value(raw, "glyph_index")
}

fn i64_param(value: &Value, key: &str) -> Result<i64, String> {
    let raw = value
        .get(key)
        .ok_or_else(|| format!("missing i64 param {key}"))?;
    i64_value(raw, key)
}

fn u64_param(value: &Value, key: &str) -> Result<u64, String> {
    let raw = value
        .get(key)
        .ok_or_else(|| format!("missing u64 param {key}"))?;
    u64_value(raw, key)
}

fn u32_param(value: &Value, key: &str) -> Result<u32, String> {
    let raw = u64_param(value, key)?;
    u32::try_from(raw).map_err(|err| format!("{key} does not fit u32: {err}"))
}

fn u32_param_object(object: &serde_json::Map<String, Value>, key: &str) -> Result<u32, String> {
    let raw = object
        .get(key)
        .ok_or_else(|| format!("missing u32 param {key}"))?;
    u32_value(raw, key)
}

fn u32_value(value: &Value, key: &str) -> Result<u32, String> {
    let raw = u64_value(value, key)?;
    u32::try_from(raw).map_err(|err| format!("{key} does not fit u32: {err}"))
}

fn u64_value(value: &Value, key: &str) -> Result<u64, String> {
    if let Some(value) = value.as_u64() {
        return Ok(value);
    }
    if let Some(value) = value.as_i64() {
        return u64::try_from(value).map_err(|err| format!("{key} is negative: {err}"));
    }
    if let Some(text) = value.as_str() {
        let value = symbolic_i64(text)?;
        return u64::try_from(value).map_err(|err| format!("{key} is negative: {err}"));
    }
    Err(format!("missing u64 param {key}"))
}

fn i64_value(value: &Value, key: &str) -> Result<i64, String> {
    if let Some(value) = value.as_i64() {
        return Ok(value);
    }
    if let Some(value) = value.as_u64() {
        return i64::try_from(value).map_err(|err| format!("{key} does not fit i64: {err}"));
    }
    if let Some(text) = value.as_str() {
        return symbolic_i64(text);
    }
    Err(format!("missing i64 param {key}"))
}

fn symbolic_i64(text: &str) -> Result<i64, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("empty integer expression".to_string());
    }
    if text.contains('|') {
        let mut value = 0i64;
        for part in text.split('|') {
            value |= symbolic_i64(part)?;
        }
        return Ok(value);
    }
    let text = text
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(text)
        .trim();
    if let Some((left, right)) = text.split_once("<<") {
        let left = symbolic_i64(left)?;
        let right = symbolic_i64(right)?;
        let shift = u32::try_from(right).map_err(|err| format!("negative shift: {err}"))?;
        return left
            .checked_shl(shift)
            .ok_or_else(|| format!("shift overflows i64: {text}"));
    }
    if let Some(hex) = text.strip_prefix("0x") {
        return i64::from_str_radix(hex, 16).map_err(|err| format!("parse hex {text}: {err}"));
    }
    if let Some(hex) = text.strip_prefix("U+") {
        return i64::from_str_radix(hex, 16)
            .map_err(|err| format!("parse codepoint {text}: {err}"));
    }
    if let Ok(value) = text.parse::<i64>() {
        return Ok(value);
    }
    rust_constant(text).map_err(|_| format!("unsupported integer expression {text}"))
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
