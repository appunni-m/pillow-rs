#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::mem::{align_of, offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use fontdone::Face;
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
enum RuntimeReadiness {
    Runnable { key: String },
    Pending { reason: String },
}

struct ProfileStage {
    name: &'static str,
    start: Instant,
    enabled: bool,
}

impl ProfileStage {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            enabled: profile_enabled(),
        }
    }
}

impl Drop for ProfileStage {
    fn drop(&mut self) {
        if self.enabled {
            let elapsed = self.start.elapsed();
            eprintln!(
                "profile_stage: name={} elapsed_ns={} elapsed_ms={:.3}",
                self.name,
                duration_ns(elapsed),
                duration_ms(elapsed)
            );
        }
    }
}

fn profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FONTDONE_UNIFIED_PROFILE").is_ok())
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn duration_ns(duration: Duration) -> u128 {
    duration.as_nanos()
}

#[test]
fn unified_fixture_parity() {
    let _profile = ProfileStage::new("unified_fixture_parity.total");
    if let Some(limit) = variability_axis_limit() {
        eprintln!("profile_variability_limit: axis_values={limit}");
    }
    let input_cases = {
        let _profile = ProfileStage::new("load_global_input_cache");
        read_all_case_files()
    };
    {
        let _profile = ProfileStage::new("preload_global_fixture_caches");
        preload_global_fixture_caches(input_cases);
    }
    {
        let _profile = ProfileStage::new("assert_single_aggregate_model");
        assert_unified_inputs_use_single_aggregate_model(input_cases);
    }
    {
        let _profile = ProfileStage::new("assert_manifest_cases_cover_fixture_inputs");
        assert_manifest_cases_cover_fixture_inputs(input_cases);
    }
    {
        let _profile = ProfileStage::new(
            "assert_manifest_font_variability_cases_cover_declared_fixture_folder",
        );
        assert_manifest_font_variability_cases_cover_declared_fixture_folder(input_cases);
    }
    {
        let _profile = ProfileStage::new("assert_unified_variability_cases_have_single_model");
        assert_unified_variability_cases_have_single_model(input_cases);
    }
    {
        let _profile = ProfileStage::new("assert_unified_fixture_cases_match_runtime_c_oracle");
        assert_unified_fixture_cases_match_runtime_c_oracle(input_cases);
    }
}

fn assert_unified_fixture_cases_match_runtime_c_oracle(all_cases: &[InputCase]) {
    let manifest = {
        let _profile = ProfileStage::new("runtime.load_manifest_cache");
        read_manifest()
    };
    let runtime_selection = {
        let _profile = ProfileStage::new("runtime.select_runtime_cases");
        select_runtime_cases(all_cases)
    };
    let cases = runtime_selection.executable;
    let mut passed = 0usize;
    let mut failures = Vec::new();
    let mut runtime_failures = FailureSummary::default();
    let mut backend_profile = BackendProfile::default();
    let mut face_prewarm_profile = FacePrewarmProfile::default();
    let mut slow_cases = SlowCaseSummary::default();
    let mut axis_profile = AxisProfileSummary::default();
    let mut covered = BTreeSet::new();
    let mut valid_cases = Vec::new();

    {
        let _profile = ProfileStage::new("runtime.validate_cases_and_assets");
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
            valid_cases.push(case);
        }
    }

    if failures.is_empty() {
        match compare_backend_outputs_with_oracle_cache(&valid_cases) {
            Ok(result) => {
                passed = passed.saturating_add(result.passed);
                covered.extend(result.covered);
                runtime_failures.extend(result.failures);
                backend_profile.extend(result.profile);
                face_prewarm_profile.extend(result.face_prewarm);
                slow_cases.extend(result.slow_cases);
                axis_profile.extend(result.axis_profile);
            }
            Err(err) => {
                failures.push(format!("runtime oracle comparison failed: {err}"));
            }
        }
    }

    let total_failures = failures.len().saturating_add(runtime_failures.count);
    eprintln!(
        "runtime_cases: runnable={} pending={} pending_reasons={}",
        cases.len(),
        runtime_selection.model_only,
        format_operation_counts(&runtime_selection.unsupported_operations)
    );
    eprintln!(
        "runtime_parity: passed={} failed={} total={} covered_manifest_cases={} failure_buckets={}",
        passed,
        total_failures,
        cases.len(),
        covered.len(),
        format_operation_counts(&runtime_failures.buckets)
    );
    if profile_enabled() {
        eprintln!(
            "profile_backend_totals: rust_ns={} c_abi_ns={} wasm_ns={} compare_ns={} rust_ms={:.3} c_abi_ms={:.3} wasm_ms={:.3} compare_ms={:.3}",
            duration_ns(backend_profile.rust_ffi),
            duration_ns(backend_profile.c_abi),
            duration_ns(backend_profile.wasm_abi),
            duration_ns(backend_profile.compare),
            duration_ms(backend_profile.rust_ffi),
            duration_ms(backend_profile.c_abi),
            duration_ms(backend_profile.wasm_abi),
            duration_ms(backend_profile.compare)
        );
        if face_prewarm_profile.cases > 0 {
            eprintln!(
                "profile_face_warmup: chunks={} cases={} opened_face_handles={} total_ns={} rust_ns={} c_abi_ns={} wasm_ns={} total_ms={:.3} rust_ms={:.3} c_abi_ms={:.3} wasm_ms={:.3}",
                face_prewarm_profile.chunks,
                face_prewarm_profile.cases,
                face_prewarm_profile.opened_face_handles,
                duration_ns(face_prewarm_profile.total),
                duration_ns(face_prewarm_profile.rust_ffi),
                duration_ns(face_prewarm_profile.c_abi),
                duration_ns(face_prewarm_profile.wasm_abi),
                duration_ms(face_prewarm_profile.total),
                duration_ms(face_prewarm_profile.rust_ffi),
                duration_ms(face_prewarm_profile.c_abi),
                duration_ms(face_prewarm_profile.wasm_abi)
            );
        }
        for sample in &slow_cases.samples {
            eprintln!(
                "profile_slow_case: case_id={} operation={} total_ns={} rust_ns={} c_abi_ns={} wasm_ns={} compare_ns={}",
                sample.case_id,
                sample.operation,
                duration_ns(sample.total),
                duration_ns(sample.rust_ffi),
                duration_ns(sample.c_abi),
                duration_ns(sample.wasm_abi),
                duration_ns(sample.compare)
            );
        }
        let mut axis_entries = axis_profile.groups.iter().collect::<Vec<_>>();
        axis_entries.sort_by(|(left_group, left_stats), (right_group, right_stats)| {
            right_stats
                .total
                .cmp(&left_stats.total)
                .then_with(|| left_group.cmp(right_group))
        });
        axis_entries.truncate(axis_profile_sample_limit());
        for (group, stats) in axis_entries {
            eprintln!(
                "profile_axis_group: group={} count={} total_ns={} avg_ns={} min_ns={} max_ns={} rust_ns={} c_abi_ns={} wasm_ns={} compare_ns={}",
                group,
                stats.count,
                duration_ns(stats.total),
                stats.avg_ns(),
                duration_ns(stats.min),
                duration_ns(stats.max),
                duration_ns(stats.rust_ffi),
                duration_ns(stats.c_abi),
                duration_ns(stats.wasm_abi),
                duration_ns(stats.compare)
            );
        }
    }
    if !failures.is_empty() || !runtime_failures.is_empty() {
        for failure in &failures {
            eprintln!("{failure}");
        }
        for failure in &runtime_failures.samples {
            eprintln!("{failure}");
        }
        if runtime_failures.count > runtime_failures.samples.len() {
            eprintln!(
                "... omitted {} additional runtime parity failures",
                runtime_failures
                    .count
                    .saturating_sub(runtime_failures.samples.len())
            );
        }
        panic!("{total_failures} unified fixture cases failed");
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

struct RuntimeSelection {
    executable: Vec<InputCase>,
    model_only: usize,
    unsupported_operations: BTreeMap<String, usize>,
}

const DEFAULT_VARIABILITY_SIZES: &[u32] = &[10, 20];
const DEFAULT_VARIABILITY_CODEPOINTS: &[u64] = &[33, 65, 103, 109];
const DEFAULT_VARIABILITY_LOAD_FLAGS: &[i32] = &[FT_LOAD_DEFAULT];
const DEFAULT_VARIABILITY_RENDER_MODES: &[i32] = &[FT_RENDER_MODE_NORMAL];

#[derive(Clone, Default)]
struct ExpansionPlan {
    fonts: Vec<Option<Asset>>,
    sizes: Vec<Option<u32>>,
    codepoints: Vec<Option<u64>>,
    glyph_indices_enabled: bool,
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
            && !self.glyph_indices_enabled
            && self.load_flags.len() == 1
            && self.load_flags[0].is_none()
            && self.render_modes.len() == 1
            && self.render_modes[0].is_none()
    }
}

fn for_each_expanded_input_case(
    case: &InputCase,
    mut visit: impl FnMut(InputCase) -> bool,
) -> bool {
    let plan = expansion_plan(case);
    if plan.is_identity() {
        return visit(case.clone());
    }

    for font in &plan.fonts {
        let glyph_indices = glyph_indices_axis(case, font.as_ref(), plan.glyph_indices_enabled);
        for size in &plan.sizes {
            for codepoint in &plan.codepoints {
                for glyph_index in &glyph_indices {
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
                            if !visit(case) {
                                return false;
                            }
                        }
                    }
                }
            }
        }
    }
    true
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
        glyph_indices_enabled: axes.contains("glyph_indices"),
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
    apply_variability_axis_limit(&mut plan);
    plan
}

fn expansion_count(case: &InputCase) -> usize {
    let plan = expansion_plan(case);
    let other_axes = plan
        .sizes
        .len()
        .saturating_mul(plan.codepoints.len())
        .saturating_mul(plan.load_flags.len())
        .saturating_mul(plan.render_modes.len());
    if !plan.glyph_indices_enabled {
        return plan.fonts.len().saturating_mul(other_axes);
    }
    plan.fonts
        .iter()
        .map(|font| glyph_indices_axis(case, font.as_ref(), true).len())
        .sum::<usize>()
        .saturating_mul(other_axes)
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
    if plan.load_flags.is_empty() {
        plan.load_flags.push(None);
    }
    if plan.render_modes.is_empty() {
        plan.render_modes.push(None);
    }
}

fn apply_variability_axis_limit(plan: &mut ExpansionPlan) {
    let Some(limit) = variability_axis_limit() else {
        return;
    };
    truncate_axis(&mut plan.fonts, limit);
    truncate_axis(&mut plan.sizes, limit);
    truncate_axis(&mut plan.codepoints, limit);
    truncate_axis(&mut plan.load_flags, limit);
    truncate_axis(&mut plan.render_modes, limit);
}

fn truncate_axis<T>(values: &mut Vec<T>, limit: usize) {
    if values.len() > limit {
        values.truncate(limit);
    }
}

fn variability_axis_limit() -> Option<usize> {
    static LIMIT: OnceLock<Option<usize>> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::env::var("FONTDONE_UNIFIED_VARIABILITY_LIMIT")
            .ok()
            .map(|value| {
                value.parse::<usize>().unwrap_or_else(|err| {
                    panic!("FONTDONE_UNIFIED_VARIABILITY_LIMIT must be usize: {err}")
                })
            })
            .map(|value| value.max(1))
    })
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
    static CACHE: OnceLock<Mutex<BTreeMap<String, Vec<Asset>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(assets) = cache
        .lock()
        .expect("font asset cache lock")
        .get(folder)
        .cloned()
    {
        return assets;
    }

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
    cache
        .lock()
        .expect("font asset cache lock")
        .insert(folder.to_string(), assets.clone());
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

fn glyph_indices_axis(
    case: &InputCase,
    font_override: Option<&Asset>,
    enabled: bool,
) -> Vec<Option<u32>> {
    if !enabled {
        return vec![None];
    }
    if let Some(indices) = runtime_glyph_indices(case, font_override) {
        if !indices.is_empty() {
            let mut values = indices.iter().copied().map(Some).collect::<Vec<_>>();
            if let Some(limit) = variability_axis_limit() {
                truncate_axis(&mut values, limit);
            }
            return values;
        }
    }
    let explicit = u32_axis(&case.inputs.params, &["glyph_indices"], true, &[]);
    let mut values = if explicit.is_empty() {
        vec![None]
    } else {
        explicit.into_iter().map(Some).collect()
    };
    if let Some(limit) = variability_axis_limit() {
        truncate_axis(&mut values, limit);
    }
    values
}

fn runtime_glyph_indices(case: &InputCase, font_override: Option<&Asset>) -> Option<Arc<[u32]>> {
    let font = font_override.or_else(|| runtime_font_asset(case))?;
    let face_index = usize::try_from(face_index_param(&case.inputs.params).ok()?).ok()?;
    let key = glyph_domain_cache_key(font, face_index)?;

    static CACHE: OnceLock<Mutex<BTreeMap<String, Arc<[u32]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(indices) = cache.lock().ok()?.get(&key).map(Arc::clone) {
        return Some(indices);
    }

    let bytes = font_asset_bytes(font).ok()?;
    let face = Face::from_memory(bytes.as_ref(), face_index, 20.0).ok()?;
    let indices = Arc::<[u32]>::from((0..u32::from(face.info().num_glyphs)).collect::<Vec<_>>());
    cache.lock().ok()?.insert(key, Arc::clone(&indices));
    Some(indices)
}

fn glyph_domain_cache_key(font: &Asset, face_index: usize) -> Option<String> {
    match font {
        Asset::File { path, .. } => Some(format!("file:{path}:face:{face_index}")),
        Asset::InlineBytes { .. } => {
            let value = inline_bytes_hex(font)?;
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            Some(format!(
                "inline:{}:face:{face_index}",
                hex_bytes(&hasher.finalize())
            ))
        }
        Asset::Ref { .. } | Asset::Other(_) => None,
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

fn select_runtime_cases(cases: &[InputCase]) -> RuntimeSelection {
    let filter = case_filter();
    let operation_filter = operation_filter();
    let limit = case_limit();
    let mut executable = Vec::new();
    let mut model_only = 0usize;
    let mut unsupported_operations: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_executable = BTreeSet::new();

    for case in cases {
        if !operation_matches_filter(case, operation_filter.as_deref()) {
            continue;
        }
        let mut should_continue = true;
        for_each_expanded_input_case(case, |expanded| {
            if !case_matches_filter(&expanded, filter.as_deref()) {
                return true;
            }
            match classify_runtime_case(&expanded, canonical_operation(&expanded.operation)) {
                RuntimeReadiness::Runnable { key } => {
                    if seen_executable.insert(key) {
                        executable.push(expanded);
                        if limit.is_some_and(|limit| executable.len() >= limit) {
                            should_continue = false;
                            return false;
                        }
                    }
                }
                RuntimeReadiness::Pending { reason } => {
                    model_only = model_only.saturating_add(1);
                    let count = unsupported_operations.entry(reason).or_default();
                    *count = count.saturating_add(1);
                }
            }
            true
        });
        if !should_continue {
            break;
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
    failures: FailureSummary,
    profile: BackendProfile,
    face_prewarm: FacePrewarmProfile,
    slow_cases: SlowCaseSummary,
    axis_profile: AxisProfileSummary,
}

#[derive(Clone, Copy, Default)]
struct BackendProfile {
    rust_ffi: Duration,
    c_abi: Duration,
    wasm_abi: Duration,
    compare: Duration,
}

impl BackendProfile {
    fn extend(&mut self, other: Self) {
        self.rust_ffi = self.rust_ffi.saturating_add(other.rust_ffi);
        self.c_abi = self.c_abi.saturating_add(other.c_abi);
        self.wasm_abi = self.wasm_abi.saturating_add(other.wasm_abi);
        self.compare = self.compare.saturating_add(other.compare);
    }
}

#[derive(Clone, Copy, Default)]
struct FacePrewarmProfile {
    chunks: usize,
    cases: usize,
    opened_face_handles: usize,
    total: Duration,
    rust_ffi: Duration,
    c_abi: Duration,
    wasm_abi: Duration,
}

impl FacePrewarmProfile {
    fn extend(&mut self, other: Self) {
        self.chunks = self.chunks.saturating_add(other.chunks);
        self.cases = self.cases.saturating_add(other.cases);
        self.opened_face_handles = self
            .opened_face_handles
            .saturating_add(other.opened_face_handles);
        self.total = self.total.saturating_add(other.total);
        self.rust_ffi = self.rust_ffi.saturating_add(other.rust_ffi);
        self.c_abi = self.c_abi.saturating_add(other.c_abi);
        self.wasm_abi = self.wasm_abi.saturating_add(other.wasm_abi);
    }
}

struct SlowCaseSummary {
    samples: Vec<SlowCaseSample>,
    limit: usize,
}

struct SlowCaseSample {
    case_id: String,
    operation: String,
    total: Duration,
    rust_ffi: Duration,
    c_abi: Duration,
    wasm_abi: Duration,
    compare: Duration,
}

impl SlowCaseSummary {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            limit: slow_case_sample_limit(),
        }
    }

    fn push(&mut self, sample: SlowCaseSample) {
        if self.limit == 0 {
            return;
        }
        self.samples.push(sample);
        self.samples.sort_by_key(|sample| Reverse(sample.total));
        if self.samples.len() > self.limit {
            self.samples.truncate(self.limit);
        }
    }

    fn extend(&mut self, other: Self) {
        for sample in other.samples {
            self.push(sample);
        }
    }
}

impl Default for SlowCaseSummary {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct AxisProfileSummary {
    groups: BTreeMap<String, AxisProfileStats>,
}

#[derive(Clone, Copy)]
struct AxisProfileStats {
    count: usize,
    total: Duration,
    min: Duration,
    max: Duration,
    rust_ffi: Duration,
    c_abi: Duration,
    wasm_abi: Duration,
    compare: Duration,
}

impl Default for AxisProfileStats {
    fn default() -> Self {
        Self {
            count: 0,
            total: Duration::ZERO,
            min: Duration::MAX,
            max: Duration::ZERO,
            rust_ffi: Duration::ZERO,
            c_abi: Duration::ZERO,
            wasm_abi: Duration::ZERO,
            compare: Duration::ZERO,
        }
    }
}

impl AxisProfileSummary {
    fn record(&mut self, case: &InputCase, sample: &SlowCaseSample) {
        if !profile_enabled() {
            return;
        }
        let operation = canonical_operation(&case.operation);
        let mut groups = vec![
            format!("operation={operation}"),
            format!("operation={operation}|subject={}", case.subject),
        ];
        if let Some(font) = input_font_file_name(case) {
            groups.push(format!("operation={operation}|font={font}"));
        }
        if let Some(size) = input_pixel_y(case) {
            groups.push(format!("operation={operation}|size_y={size}"));
        }
        if let Some(glyph_index) = input_u32_param(case, "glyph_index") {
            groups.push(format!("operation={operation}|glyph_index={glyph_index}"));
        }
        if let Some(char_code) = input_u64_param(case, "char_code") {
            groups.push(format!("operation={operation}|char_code={char_code}"));
        }
        if let Ok(load_flags) = load_flags_param(&case.inputs.params) {
            groups.push(format!("operation={operation}|load_flags={load_flags}"));
        }
        if let Ok(render_mode) = render_mode_param(&case.inputs.params) {
            groups.push(format!("operation={operation}|render_mode={render_mode}"));
        }

        for group in groups {
            self.groups.entry(group).or_default().record(sample);
        }
    }

    fn extend(&mut self, other: Self) {
        for (group, stats) in other.groups {
            self.groups.entry(group).or_default().extend(stats);
        }
    }
}

impl AxisProfileStats {
    fn record(&mut self, sample: &SlowCaseSample) {
        self.count = self.count.saturating_add(1);
        self.total = self.total.saturating_add(sample.total);
        self.min = self.min.min(sample.total);
        self.max = self.max.max(sample.total);
        self.rust_ffi = self.rust_ffi.saturating_add(sample.rust_ffi);
        self.c_abi = self.c_abi.saturating_add(sample.c_abi);
        self.wasm_abi = self.wasm_abi.saturating_add(sample.wasm_abi);
        self.compare = self.compare.saturating_add(sample.compare);
    }

    fn extend(&mut self, other: Self) {
        self.count = self.count.saturating_add(other.count);
        self.total = self.total.saturating_add(other.total);
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.rust_ffi = self.rust_ffi.saturating_add(other.rust_ffi);
        self.c_abi = self.c_abi.saturating_add(other.c_abi);
        self.wasm_abi = self.wasm_abi.saturating_add(other.wasm_abi);
        self.compare = self.compare.saturating_add(other.compare);
    }

    fn avg_ns(&self) -> u128 {
        if self.count == 0 {
            0
        } else {
            let count = u128::try_from(self.count).unwrap_or(u128::MAX);
            duration_ns(self.total).checked_div(count).unwrap_or(0)
        }
    }
}

fn axis_profile_sample_limit() -> usize {
    40
}

fn slow_case_sample_limit() -> usize {
    20
}

struct FailureSummary {
    count: usize,
    samples: Vec<String>,
    buckets: BTreeMap<String, usize>,
    sample_limit: usize,
}

impl Default for FailureSummary {
    fn default() -> Self {
        Self {
            count: 0,
            samples: Vec::new(),
            buckets: BTreeMap::new(),
            sample_limit: failure_sample_limit(),
        }
    }
}

impl FailureSummary {
    fn push(&mut self, failure: String) {
        self.count = self.count.saturating_add(1);
        let count = self.buckets.entry(failure_bucket(&failure)).or_default();
        *count = count.saturating_add(1);
        if self.samples.len() < self.sample_limit {
            self.samples.push(failure);
        }
    }

    fn extend(&mut self, other: Self) {
        self.count = self.count.saturating_add(other.count);
        for (bucket, count) in other.buckets {
            let existing = self.buckets.entry(bucket).or_default();
            *existing = existing.saturating_add(count);
        }
        for sample in other.samples {
            if self.samples.len() >= self.sample_limit {
                break;
            }
            self.samples.push(sample);
        }
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }
}

fn failure_sample_limit() -> usize {
    200
}

fn failure_bucket(failure: &str) -> String {
    let Some((backend, detail)) = failure.split_once(": ") else {
        return "setup".to_string();
    };
    let kind = if detail.contains("status mismatch") {
        "status".to_string()
    } else if let Some((_, rest)) = detail.split_once(" field=") {
        let field = rest.split_whitespace().next().unwrap_or("value");
        format!("field:{field}")
    } else if detail.contains("backend failed") {
        "backend_error".to_string()
    } else {
        "value".to_string()
    };
    format!("{backend}:{kind}")
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
        let pairs = cases
            .iter()
            .copied()
            .zip(oracle_outputs.iter())
            .collect::<Vec<_>>();
        return compare_backend_output_pairs(&pairs);
    }

    let partitions = partition_backend_pairs(cases, oracle_outputs, workers);
    let mut result = BackendComparisonResult::default();
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for partition in partitions {
            handles.push(scope.spawn(move || compare_backend_output_pairs(&partition)));
        }
        for handle in handles {
            let partial = handle.join().expect("backend comparison worker panicked");
            result.passed = result.passed.saturating_add(partial.passed);
            result.covered.extend(partial.covered);
            result.failures.extend(partial.failures);
            result.profile.extend(partial.profile);
            result.face_prewarm.extend(partial.face_prewarm);
            result.slow_cases.extend(partial.slow_cases);
            result.axis_profile.extend(partial.axis_profile);
        }
    });
    result
}

fn partition_backend_pairs<'a>(
    cases: &[&'a InputCase],
    oracle_outputs: &'a [RunOutput],
    workers: usize,
) -> Vec<Vec<(&'a InputCase, &'a RunOutput)>> {
    let worker_count = workers.clamp(1, cases.len().max(1));
    let partition_capacity = cases
        .len()
        .checked_div(worker_count)
        .and_then(|value| value.checked_add(1))
        .unwrap_or(1);
    let mut partitions = (0..worker_count)
        .map(|_| Vec::with_capacity(partition_capacity))
        .collect::<Vec<_>>();
    for (index, (case, oracle)) in cases.iter().copied().zip(oracle_outputs.iter()).enumerate() {
        let partition_index = index.checked_rem(worker_count).unwrap_or(0);
        partitions[partition_index].push((case, oracle));
    }
    partitions
        .into_iter()
        .filter(|partition| !partition.is_empty())
        .collect()
}

fn compare_backend_outputs_with_oracle_cache(
    cases: &[&InputCase],
) -> Result<BackendComparisonResult, String> {
    if cases.is_empty() {
        return Ok(BackendComparisonResult::default());
    }
    let cache_path = {
        let _profile = ProfileStage::new("runtime.ensure_oracle_cache");
        ensure_oracle_cache(cases)?
    };
    let oracle_outputs = {
        let _profile = ProfileStage::new("runtime.load_oracle_cache_outputs");
        read_oracle_cache_outputs(cases, &cache_path)?
    };
    let result = {
        let _profile = ProfileStage::new("runtime.compare_backend_outputs");
        compare_backend_outputs(cases, oracle_outputs.as_ref())
    };
    eprintln!(
        "runtime_parity_progress: compared={} total={} passed={} failed={} rust_ns={} c_abi_ns={} wasm_ns={} compare_ns={} rust_ms={:.3} c_abi_ms={:.3} wasm_ms={:.3} compare_ms={:.3}",
        cases.len(),
        cases.len(),
        result.passed,
        result.failures.count,
        duration_ns(result.profile.rust_ffi),
        duration_ns(result.profile.c_abi),
        duration_ns(result.profile.wasm_abi),
        duration_ns(result.profile.compare),
        duration_ms(result.profile.rust_ffi),
        duration_ms(result.profile.c_abi),
        duration_ms(result.profile.wasm_abi),
        duration_ms(result.profile.compare)
    );
    Ok(result)
}

fn read_oracle_cache_outputs(
    cases: &[&InputCase],
    cache_path: &Path,
) -> Result<Arc<[RunOutput]>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Arc<[RunOutput]>>>> = OnceLock::new();
    let key = cache_path.to_string_lossy().into_owned();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(outputs) = cache
        .lock()
        .map_err(|err| err.to_string())?
        .get(&key)
        .map(Arc::clone)
    {
        if outputs.len() == cases.len() {
            return Ok(outputs);
        }
    }

    let text = fs::read_to_string(cache_path)
        .map_err(|err| format!("read oracle cache {}: {err}", cache_path.display()))?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() != cases.len() {
        return Err(format!(
            "oracle cache {} has {} lines for {} cases",
            cache_path.display(),
            lines.len(),
            cases.len()
        ));
    }
    let outputs = cases
        .iter()
        .zip(lines)
        .map(|(case, line)| {
            parse_run_output(line).map_err(|err| format!("{} oracle failed: {err}", case.case_id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = Arc::<[RunOutput]>::from(outputs);
    cache
        .lock()
        .map_err(|err| err.to_string())?
        .insert(key, Arc::clone(&outputs));
    Ok(outputs)
}

fn unified_worker_count(case_count: usize) -> usize {
    if case_count < 2 {
        return 1;
    }
    thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 8)
        .min(case_count)
}

fn compare_backend_output_pairs(pairs: &[(&InputCase, &RunOutput)]) -> BackendComparisonResult {
    let mut result = BackendComparisonResult::default();
    let mut worker = BackendComparisonWorker::default();
    let cases = pairs
        .iter()
        .map(|(case, _oracle)| *case)
        .collect::<Vec<_>>();
    match worker.prewarm_faces(&cases) {
        Ok(profile) => result.face_prewarm.extend(profile),
        Err(err) => {
            result.failures.push(err);
            return result;
        }
    }
    for (case, oracle) in pairs {
        match worker.compare_case(case, oracle) {
            Ok(()) => {
                result.passed = result.passed.saturating_add(1);
                result
                    .covered
                    .insert((case.subject.clone(), case.case.clone()));
            }
            Err(err) => result.failures.push(err),
        }
    }
    result.profile = worker.profile;
    result.slow_cases = worker.slow_cases;
    result.axis_profile = worker.axis_profile;
    result
}

#[derive(Default)]
struct BackendComparisonWorker {
    rust_faces: BTreeMap<String, FT_Face>,
    c_faces: BTreeMap<String, CachedCAbiFace>,
    wasm_faces: BTreeMap<String, CachedWasmFace>,
    profile: BackendProfile,
    slow_cases: SlowCaseSummary,
    axis_profile: AxisProfileSummary,
}

struct CachedCAbiFace {
    library: c_abi::FT_Library,
    face: c_abi::FT_Face,
}

impl Drop for CachedCAbiFace {
    fn drop(&mut self) {
        c_done_face(self.face);
        c_done_library(self.library);
    }
}

struct CachedWasmFace {
    handle: usize,
}

impl Drop for CachedWasmFace {
    fn drop(&mut self) {
        wasm_done_face(self.handle);
    }
}

impl BackendComparisonWorker {
    fn prewarm_faces(&mut self, cases: &[&InputCase]) -> Result<FacePrewarmProfile, String> {
        let total_start = Instant::now();
        let opened_before = self.opened_face_handle_count();
        let mut profile = FacePrewarmProfile {
            chunks: 1,
            ..FacePrewarmProfile::default()
        };

        for case in cases {
            if !case_uses_cached_face(case) {
                continue;
            }
            profile.cases = profile.cases.saturating_add(1);
            let start = Instant::now();
            self.rust_face(case)
                .map_err(|err| format!("{} rust face warmup failed: {err}", case.case_id))?;
            profile.rust_ffi = profile.rust_ffi.saturating_add(start.elapsed());
        }
        for case in cases {
            if !case_uses_cached_face(case) {
                continue;
            }
            let start = Instant::now();
            self.c_face(case)
                .map_err(|err| format!("{} c abi face warmup failed: {err}", case.case_id))?;
            profile.c_abi = profile.c_abi.saturating_add(start.elapsed());
        }
        for case in cases {
            if !case_uses_cached_face(case) {
                continue;
            }
            let start = Instant::now();
            self.wasm_face(case)
                .map_err(|err| format!("{} wasm abi face warmup failed: {err}", case.case_id))?;
            profile.wasm_abi = profile.wasm_abi.saturating_add(start.elapsed());
        }

        profile.opened_face_handles = self
            .opened_face_handle_count()
            .saturating_sub(opened_before);
        profile.total = total_start.elapsed();
        Ok(profile)
    }

    fn opened_face_handle_count(&self) -> usize {
        self.rust_faces
            .len()
            .saturating_add(self.c_faces.len())
            .saturating_add(self.wasm_faces.len())
    }

    fn compare_case(&mut self, case: &InputCase, oracle: &RunOutput) -> Result<(), String> {
        let case_start = Instant::now();
        let start = Instant::now();
        let rust_actual = self
            .run_rust_ffi(case)
            .map_err(|err| format!("{} rust backend failed: {err}", case.case_id))?;
        let rust_duration = start.elapsed();
        self.profile.rust_ffi = self.profile.rust_ffi.saturating_add(rust_duration);

        let start = Instant::now();
        let c_actual = self
            .run_c_abi(case)
            .map_err(|err| format!("{} c abi backend failed: {err}", case.case_id))?;
        let c_duration = start.elapsed();
        self.profile.c_abi = self.profile.c_abi.saturating_add(c_duration);

        let start = Instant::now();
        let wasm_actual = self
            .run_wasm_abi(case)
            .map_err(|err| format!("{} wasm abi backend failed: {err}", case.case_id))?;
        let wasm_duration = start.elapsed();
        self.profile.wasm_abi = self.profile.wasm_abi.saturating_add(wasm_duration);

        let start = Instant::now();
        let result = compare_named_output(case, "rust ffi", oracle, &rust_actual)
            .and_then(|()| compare_named_output(case, "c abi", oracle, &c_actual))
            .and_then(|()| compare_named_output(case, "wasm abi", oracle, &wasm_actual));
        let compare_duration = start.elapsed();
        self.profile.compare = self.profile.compare.saturating_add(compare_duration);
        if profile_enabled() {
            let sample = SlowCaseSample {
                case_id: case.case_id.clone(),
                operation: canonical_operation(&case.operation).to_string(),
                total: case_start.elapsed(),
                rust_ffi: rust_duration,
                c_abi: c_duration,
                wasm_abi: wasm_duration,
                compare: compare_duration,
            };
            self.axis_profile.record(case, &sample);
            self.slow_cases.push(sample);
        }
        result
    }

    fn run_rust_ffi(&mut self, case: &InputCase) -> Result<RunOutput, String> {
        match canonical_operation(&case.operation) {
            "size_metrics" => {
                let face = self.rust_face(case)?;
                Ok(ok(size_metrics_json(&FT_Size_Metrics(face))))
            }
            "get_char_index" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let face = self.rust_face(case)?;
                Ok(ok(json!({"value": FT_Get_Char_Index(face, char_code)})))
            }
            "load_char" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let face = self.rust_face(case)?;
                match FT_Load_Char(face, char_code, load_flags) {
                    Ok(slot) => Ok(ok(slot_json(&slot))),
                    Err(err) => Ok(error(err)),
                }
            }
            "load_glyph" => {
                let glyph_index = glyph_index_param(&case.inputs.params)?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let face = self.rust_face(case)?;
                match FT_Load_Glyph(face, glyph_index, load_flags) {
                    Ok(slot) => Ok(ok(slot_json(&slot))),
                    Err(err) => Ok(error(err)),
                }
            }
            "render_glyph" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let render_mode = render_mode_param(&case.inputs.params)?;
                let face = self.rust_face(case)?;
                match FT_Load_Char(face, char_code, load_flags)
                    .and_then(|slot| FT_Render_Glyph(slot, render_mode))
                {
                    Ok(slot) => Ok(ok(slot_json(&slot))),
                    Err(err) => Ok(error(err)),
                }
            }
            _ => run_rust_ffi(case),
        }
    }

    fn run_c_abi(&mut self, case: &InputCase) -> Result<RunOutput, String> {
        match canonical_operation(&case.operation) {
            "size_metrics" => {
                let face = self.c_face(case)?;
                c_size_metrics_json(face).map(ok)
            }
            "get_char_index" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let face = self.c_face(case)?;
                let value = c_abi::FT_Get_Char_Index(face, char_code);
                Ok(ok(json!({"value": value})))
            }
            "load_char" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let face = self.c_face(case)?;
                let err = c_abi::FT_Load_Char(face, char_code, load_flags);
                if err == FT_Err_Ok {
                    c_slot_json(face).map(ok)
                } else {
                    Ok(error(err))
                }
            }
            "load_glyph" => {
                let glyph_index = glyph_index_param(&case.inputs.params)?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let face = self.c_face(case)?;
                let err = c_abi::FT_Load_Glyph(face, glyph_index, load_flags);
                if err == FT_Err_Ok {
                    c_slot_json(face).map(ok)
                } else {
                    Ok(error(err))
                }
            }
            "render_glyph" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let render_mode = render_mode_param(&case.inputs.params)?;
                let face = self.c_face(case)?;
                let load_err = c_abi::FT_Load_Char(face, char_code, load_flags);
                let err = if load_err == FT_Err_Ok {
                    c_abi::abi_render_glyph_from_face(face, render_mode)
                } else {
                    load_err
                };
                if err == FT_Err_Ok {
                    c_slot_json(face).map(ok)
                } else {
                    Ok(error(err))
                }
            }
            _ => run_c_abi(case),
        }
    }

    fn run_wasm_abi(&mut self, case: &InputCase) -> Result<RunOutput, String> {
        match canonical_operation(&case.operation) {
            "size_metrics" => {
                let handle = self.wasm_face(case)?;
                let mut metrics = wasm_abi::FontdoneWasmSizeMetrics::default();
                let err = wasm_abi::fontdone_wasm_size_metrics(handle, &mut metrics);
                if err == FT_Err_Ok {
                    Ok(ok(wasm_size_metrics_json(&metrics)))
                } else {
                    Ok(error(err))
                }
            }
            "get_char_index" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let handle = self.wasm_face(case)?;
                let value = wasm_abi::fontdone_wasm_get_char_index(handle, char_code);
                Ok(ok(json!({"value": value})))
            }
            "load_char" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let handle = self.wasm_face(case)?;
                let err = wasm_abi::fontdone_wasm_load_char(handle, char_code, load_flags);
                wasm_slot_output(handle, err)
            }
            "load_glyph" => {
                let glyph_index = glyph_index_param(&case.inputs.params)?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let handle = self.wasm_face(case)?;
                let err = wasm_abi::fontdone_wasm_load_glyph(handle, glyph_index, load_flags);
                wasm_slot_output(handle, err)
            }
            "render_glyph" => {
                let char_code = u64_param(&case.inputs.params, "char_code")?;
                let load_flags = load_flags_param(&case.inputs.params)?;
                let render_mode = render_mode_param(&case.inputs.params)?;
                let handle = self.wasm_face(case)?;
                let load_err = wasm_abi::fontdone_wasm_load_char(handle, char_code, load_flags);
                let err = if load_err == FT_Err_Ok {
                    wasm_abi::fontdone_wasm_render_glyph(handle, render_mode)
                } else {
                    load_err
                };
                wasm_slot_output(handle, err)
            }
            _ => run_wasm_abi(case),
        }
    }

    fn rust_face(&mut self, case: &InputCase) -> Result<&FT_Face, String> {
        let key = runtime_face_cache_key(case)?;
        if !self.rust_faces.contains_key(&key) {
            self.rust_faces.insert(key.clone(), open_face(case)?);
        }
        self.rust_faces
            .get(&key)
            .ok_or_else(|| format!("missing cached rust face {key}"))
    }

    fn c_face(&mut self, case: &InputCase) -> Result<c_abi::FT_Face, String> {
        let key = runtime_face_cache_key(case)?;
        if !self.c_faces.contains_key(&key) {
            let (library, face) = c_open_face(case)?;
            self.c_faces
                .insert(key.clone(), CachedCAbiFace { library, face });
        }
        self.c_faces
            .get(&key)
            .map(|cached| cached.face)
            .ok_or_else(|| format!("missing cached c abi face {key}"))
    }

    fn wasm_face(&mut self, case: &InputCase) -> Result<usize, String> {
        let key = runtime_face_cache_key(case)?;
        if !self.wasm_faces.contains_key(&key) {
            let handle = wasm_open_face(case)?;
            self.wasm_faces
                .insert(key.clone(), CachedWasmFace { handle });
        }
        self.wasm_faces
            .get(&key)
            .map(|cached| cached.handle)
            .ok_or_else(|| format!("missing cached wasm face {key}"))
    }
}

fn runtime_face_cache_key(case: &InputCase) -> Result<String, String> {
    let font = runtime_font_asset(case).ok_or_else(|| "missing font asset".to_string())?;
    let face_index =
        usize::try_from(face_index_param(&case.inputs.params)?).map_err(|err| err.to_string())?;
    let font_key = glyph_domain_cache_key(font, face_index)
        .ok_or_else(|| format!("unsupported cached font asset {}", asset_label(font)))?;
    let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
    Ok(format!("{font_key}:pixel:{pixel_width}:{pixel_height}"))
}

fn case_uses_cached_face(case: &InputCase) -> bool {
    matches!(
        canonical_operation(&case.operation),
        "size_metrics" | "get_char_index" | "load_char" | "load_glyph" | "render_glyph"
    )
}

fn case_filter() -> Option<String> {
    std::env::var("FONTDONE_UNIFIED_CASE_FILTER").ok()
}

fn operation_filter() -> Option<String> {
    std::env::var("FONTDONE_UNIFIED_OPERATION_FILTER").ok()
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

fn operation_matches_filter(case: &InputCase, filter: Option<&str>) -> bool {
    filter.is_none_or(|needle| {
        canonical_operation(&case.operation).contains(needle) || case.operation.contains(needle)
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
        for covered_case in &case.covers_manifest_cases {
            assert!(
                manifest.has_case(&case.subject, covered_case),
                "{} references unknown covered manifest case {}::{}",
                case.case_id,
                case.subject,
                covered_case
            );
            covered.insert((case.subject.clone(), covered_case.clone()));
        }
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
    if variability_axis_limit().is_some() && !failures.is_empty() {
        eprintln!(
            "font_variability_coverage: diagnostic variability limit is active; missing probes are reported but not asserted"
        );
        for failure in failures.iter().take(20) {
            eprintln!("{failure}");
        }
        if failures.len() > 20 {
            eprintln!(
                "... omitted {} additional font variability coverage gaps",
                failures.len().saturating_sub(20)
            );
        }
        return;
    }
    assert!(
        failures.is_empty(),
        "font variability coverage gaps:\n{}",
        failures.join("\n")
    );
}

fn assert_unified_variability_cases_have_single_model(cases: &[InputCase]) {
    let mut expanded = 0usize;
    let mut aggregate_subjects = BTreeSet::new();
    for case in cases {
        let count = expansion_count(case);
        if count > 1 {
            expanded = expanded.saturating_add(count);
            aggregate_subjects.insert(case.subject.as_str());
        }
    }
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

fn read_manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let text = fs::read_to_string(manifest_dir().join("tests").join("manifest.yaml"))
            .expect("read manifest.yaml");
        parse_manifest(&text)
    })
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

fn read_all_case_files() -> &'static [InputCase] {
    static CASES: OnceLock<Vec<InputCase>> = OnceLock::new();
    CASES.get_or_init(load_all_case_files)
}

fn preload_global_fixture_caches(cases: &[InputCase]) {
    let manifest = read_manifest();
    let mut folders = BTreeSet::new();
    let mut file_assets = BTreeSet::new();
    let mut inline_assets = BTreeSet::new();

    for variability in manifest.font_variability.values() {
        folders.insert(variability.folder.clone());
    }

    for case in cases {
        collect_case_cache_inputs(case, &mut folders, &mut file_assets, &mut inline_assets);
    }

    for folder in &folders {
        for asset in fixture_font_assets(folder) {
            collect_asset_cache_input(&asset, &mut file_assets, &mut inline_assets);
        }
    }

    for path in &file_assets {
        if !fixture_dir().join(path).is_file() {
            continue;
        }
        cached_file_bytes(path).unwrap_or_else(|err| panic!("preload fixture bytes {path}: {err}"));
        validated_file_asset(path)
            .unwrap_or_else(|err| panic!("preload fixture validation {path}: {err}"));
    }
    for inline_hex in &inline_assets {
        cached_inline_bytes(inline_hex).unwrap_or_else(|err| panic!("preload inline bytes: {err}"));
    }

    preload_glyph_domains(cases);

    if profile_enabled() {
        eprintln!(
            "profile_global_cache_warmup: input_cases={} font_folders={} file_assets={} inline_assets={}",
            cases.len(),
            folders.len(),
            file_assets.len(),
            inline_assets.len()
        );
    }
}

fn collect_case_cache_inputs(
    case: &InputCase,
    folders: &mut BTreeSet<String>,
    file_assets: &mut BTreeSet<String>,
    inline_assets: &mut BTreeSet<String>,
) {
    if let Some(folder) = case
        .inputs
        .variability
        .as_ref()
        .and_then(|variability| variability.fonts_folder.as_deref())
    {
        folders.insert(folder.to_string());
    }
    if let Some(folder) = file_asset_path(case.inputs.assets.get("font_folder")) {
        folders.insert(folder.to_string());
    }
    for asset in case.inputs.assets.values() {
        collect_asset_cache_input(asset, file_assets, inline_assets);
    }
}

fn collect_asset_cache_input(
    asset: &Asset,
    file_assets: &mut BTreeSet<String>,
    inline_assets: &mut BTreeSet<String>,
) {
    match asset {
        Asset::File { path, .. } => {
            file_assets.insert(path.clone());
        }
        Asset::InlineBytes { .. } => {
            if let Some(value) = inline_bytes_hex(asset) {
                inline_assets.insert(value.to_string());
            }
        }
        Asset::Ref { .. } | Asset::Other(_) => {}
    }
}

fn preload_glyph_domains(cases: &[InputCase]) {
    for case in cases {
        let axes = variability_axes(case);
        if !axes.contains("glyph_indices") {
            continue;
        }
        let fonts = font_axis(case, &axes);
        if fonts.is_empty() {
            let _ = runtime_glyph_indices(case, None);
            continue;
        }
        for font in &fonts {
            let _ = runtime_glyph_indices(case, font.as_ref());
        }
    }
}

fn load_all_case_files() -> Vec<InputCase> {
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
            cases.push(case);
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
    if input.subject != probe.subject || input.case != probe.case_id {
        return false;
    }
    let plan = expansion_plan(input);
    input_covers_font_name(input, &plan, probe.font)
        && option_axis_covers_u32(&plan.sizes, input_pixel_y(input), probe.size)
        && probe.char_code.is_none_or(|value| {
            option_axis_covers_u64(&plan.codepoints, input_u64_param(input, "char_code"), value)
        })
        && probe.glyph_index.is_none_or(|value| {
            if plan.glyph_indices_enabled {
                plan.fonts.iter().any(|font| {
                    glyph_indices_axis(input, font.as_ref(), true)
                        .into_iter()
                        .flatten()
                        .any(|glyph_index| glyph_index == value)
                })
            } else {
                input_u32_param(input, "glyph_index") == Some(value)
            }
        })
        && probe.load_flag.is_none_or(|value| {
            option_axis_covers_i32(
                &plan.load_flags,
                input_i32_param(input, "load_flags"),
                value,
            )
        })
        && probe.render_mode.is_none_or(|value| {
            option_axis_covers_i32(
                &plan.render_modes,
                input_i32_param(input, "render_mode"),
                value,
            )
        })
}

fn input_covers_font_name(input: &InputCase, plan: &ExpansionPlan, font_name: &str) -> bool {
    if plan.fonts.iter().any(|font| {
        font.as_ref()
            .and_then(asset_file_name)
            .is_some_and(|candidate| candidate == font_name)
    }) {
        return true;
    }
    input_font_file_name(input).is_some_and(|candidate| candidate == font_name)
}

fn option_axis_covers_u32(axis: &[Option<u32>], base: Option<u32>, value: u32) -> bool {
    axis.iter()
        .any(|candidate| candidate.map_or(base == Some(value), |candidate| candidate == value))
}

fn option_axis_covers_u64(axis: &[Option<u64>], base: Option<u64>, value: u64) -> bool {
    axis.iter()
        .any(|candidate| candidate.map_or(base == Some(value), |candidate| candidate == value))
}

fn option_axis_covers_i32(axis: &[Option<i32>], base: Option<i32>, value: i32) -> bool {
    axis.iter()
        .any(|candidate| candidate.map_or(base == Some(value), |candidate| candidate == value))
}

fn asset_file_name(asset: &Asset) -> Option<String> {
    match asset {
        Asset::File { path, .. } => Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        Asset::InlineBytes { .. } | Asset::Ref { .. } | Asset::Other(_) => None,
    }
}

fn input_font_file_name(input: &InputCase) -> Option<String> {
    asset_file_name(input.inputs.assets.get("font")?)
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
                cached_inline_bytes(value).map_err(|err| format!("{name} invalid hex: {err}"))?;
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
    if let Some(entry) = cache
        .lock()
        .map_err(|err| err.to_string())?
        .get(path)
        .cloned()
    {
        return Ok(entry);
    }
    let bytes = cached_file_bytes(path)?;
    let entry = (
        u64::try_from(bytes.len()).map_err(|err| err.to_string())?,
        sha256_hex(bytes.as_ref()),
    );
    let mut cache = cache.lock().map_err(|err| err.to_string())?;
    let entry = cache.entry(path.to_string()).or_insert(entry).clone();
    Ok(entry)
}

fn ensure_oracle_cache(cases: &[&InputCase]) -> Result<PathBuf, String> {
    if cases.is_empty() {
        return Err("cannot cache empty oracle case set".to_string());
    }
    let batch_input = {
        let _profile = ProfileStage::new("oracle_cache.build_batch_input");
        oracle_batch_input(cases)?
    };
    let cache_key = {
        let _profile = ProfileStage::new("oracle_cache.compute_key");
        oracle_cache_key(cases, &batch_input)?
    };
    let cache_path = oracle_cache_path(&cache_key);

    if std::env::var("FONTDONE_UNIFIED_ORACLE_REFRESH").is_err() && cache_path.exists() {
        eprintln!(
            "unified_oracle_cache: hit {} cases key={}",
            cases.len(),
            cache_key
        );
        return Ok(cache_path);
    }

    let stdout = {
        let _profile = ProfileStage::new("oracle_cache.run_c_oracle_batch");
        run_oracles_batch(cases, &batch_input)?
    };
    {
        let _profile = ProfileStage::new("oracle_cache.write_cache");
        write_oracle_cache(&cache_path, &stdout)?;
    }
    eprintln!(
        "unified_oracle_cache: wrote {} cases key={}",
        cases.len(),
        cache_key
    );
    Ok(cache_path)
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
    hasher.update(b"fontdone-unified-oracle-cache-v2\n");
    hasher.update(b"\n--oracle--\n");
    hasher.update(oracle_identity_hash()?.as_bytes());
    hasher.update(canonical_cases.as_bytes());
    hasher.update(b"\n--argv--\n");
    hasher.update(batch_input.as_bytes());
    Ok(hex_bytes(&hasher.finalize()))
}

fn oracle_identity_hash() -> Result<String, String> {
    let mut hasher = Sha256::new();
    for path in [
        oracle_bin()?,
        manifest_dir()
            .join("freetype")
            .join("build")
            .join("libfreetype.so"),
    ] {
        let bytes = fs::read(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(sha256_hex(&bytes).as_bytes());
        hasher.update(b"\n");
    }
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
            if is_face_probe(case)? {
                c_done_face(face);
                c_done_library(library);
                return Ok(ok(json!({"opened": true})));
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
            let (library, face) = c_open_face(case)?;
            let output = c_size_metrics_json(face);
            c_done_face(face);
            c_done_library(library);
            output.map(ok)
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
            let (library, face) = c_open_face(case)?;
            let output = c_size_metrics_json(face);
            c_done_face(face);
            c_done_library(library);
            output.map(ok)
        }
        "get_char_index" => {
            let (library, face) = c_open_face(case)?;
            let value =
                c_abi::FT_Get_Char_Index(face, u64_param(&case.inputs.params, "char_code")?);
            c_done_face(face);
            c_done_library(library);
            Ok(ok(json!({"value": value})))
        }
        "load_char" => {
            let (library, face) = c_open_face(case)?;
            let err = c_abi::FT_Load_Char(
                face,
                u64_param(&case.inputs.params, "char_code")?,
                load_flags_param(&case.inputs.params)?,
            );
            if err == FT_Err_Ok {
                let output = c_slot_json(face).map(ok);
                c_done_face(face);
                c_done_library(library);
                output
            } else {
                c_done_face(face);
                c_done_library(library);
                Ok(error(err))
            }
        }
        "load_glyph" => {
            let (library, face) = c_open_face(case)?;
            let err = c_abi::FT_Load_Glyph(
                face,
                glyph_index_param(&case.inputs.params)?,
                load_flags_param(&case.inputs.params)?,
            );
            if err == FT_Err_Ok {
                let output = c_slot_json(face).map(ok);
                c_done_face(face);
                c_done_library(library);
                output
            } else {
                c_done_face(face);
                c_done_library(library);
                Ok(error(err))
            }
        }
        "render_glyph" => {
            let (library, face) = c_open_face(case)?;
            let load_err = c_abi::FT_Load_Char(
                face,
                u64_param(&case.inputs.params, "char_code")?,
                load_flags_param(&case.inputs.params)?,
            );
            let err = if load_err == FT_Err_Ok {
                c_abi::abi_render_glyph_from_face(face, render_mode_param(&case.inputs.params)?)
            } else {
                load_err
            };
            if err == FT_Err_Ok {
                let output = c_slot_json(face).map(ok);
                c_done_face(face);
                c_done_library(library);
                output
            } else {
                c_done_face(face);
                c_done_library(library);
                Ok(error(err))
            }
        }
        other => Err(format!("unsupported c abi operation {other}")),
    }
}

fn run_wasm_abi(case: &InputCase) -> Result<RunOutput, String> {
    match canonical_operation(&case.operation) {
        "constant" | "record_layout" | "set_char_size" => run_rust_ffi(case),
        "new_memory_face" => wasm_new_memory_face(case),
        "set_pixel_sizes" => {
            let handle = wasm_open_face(case)?;
            let mut metrics = wasm_abi::FontdoneWasmSizeMetrics::default();
            let err = wasm_abi::fontdone_wasm_size_metrics(handle, &mut metrics);
            wasm_done_face(handle);
            if err == FT_Err_Ok {
                Ok(ok(wasm_size_metrics_json(&metrics)))
            } else {
                Ok(error(err))
            }
        }
        "size_metrics" => {
            let handle = wasm_open_face(case)?;
            let mut metrics = wasm_abi::FontdoneWasmSizeMetrics::default();
            let err = wasm_abi::fontdone_wasm_size_metrics(handle, &mut metrics);
            let output = if err == FT_Err_Ok {
                Ok(ok(wasm_size_metrics_json(&metrics)))
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
    let slot = c_abi::abi_slot_snapshot(face)
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

fn c_size_metrics_json(face: c_abi::FT_Face) -> Result<Value, String> {
    let metrics =
        c_abi::abi_size_metrics(face).ok_or_else(|| "missing c size metrics".to_string())?;
    Ok(json!({
        "x_ppem": metrics.x_ppem,
        "y_ppem": metrics.y_ppem,
        "x_scale": metrics.x_scale,
        "y_scale": metrics.y_scale,
        "ascender": metrics.ascender,
        "descender": metrics.descender,
        "height": metrics.height,
        "max_advance": metrics.max_advance
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

fn wasm_new_memory_face(case: &InputCase) -> Result<RunOutput, String> {
    let bytes = font_bytes(case)?;
    let status = wasm_abi::fontdone_wasm_open_face(
        bytes.as_ptr(),
        bytes.len(),
        face_index_param(&case.inputs.params)?,
        20.0,
    );
    if status.error == FT_Err_Ok {
        wasm_done_face(status.handle);
        Ok(ok(json!({"opened": true})))
    } else {
        Ok(error(status.error))
    }
}

fn wasm_slot_output(handle: usize, err: i32) -> Result<RunOutput, String> {
    if err != FT_Err_Ok {
        return Ok(error(err));
    }
    let slot = wasm_abi::abi_slot_snapshot(handle)
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
        data.as_ref(),
        face_index_param(&case.inputs.params)?,
        20.0,
    ) {
        Ok(mut face) => {
            if is_face_probe(case)? {
                return Ok(ok(json!({"opened": true})));
            }
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
        data.as_ref(),
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
        data.as_ref(),
        face_index_param(&case.inputs.params)?,
        20.0,
    ) {
        Ok(mut face) => {
            let (pixel_width, pixel_height) = pixel_size_param(&case.inputs.params)?;
            let err = FT_Set_Pixel_Sizes(&mut face, pixel_width, pixel_height);
            if err == FT_Err_Ok {
                Ok(ok(size_metrics_json(&FT_Size_Metrics(&face))))
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
        data.as_ref(),
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

fn font_bytes(case: &InputCase) -> Result<Arc<[u8]>, String> {
    let font = runtime_font_asset(case).ok_or_else(|| "missing font asset".to_string())?;
    font_asset_bytes(font)
}

fn font_asset_bytes(font: &Asset) -> Result<Arc<[u8]>, String> {
    match font {
        Asset::File { path, .. } => cached_file_bytes(path),
        Asset::InlineBytes { encoding, .. } => {
            if encoding != "hex" {
                return Err(format!("unsupported inline byte encoding {encoding}"));
            }
            let value =
                inline_bytes_hex(font).ok_or_else(|| "missing inline hex bytes".to_string())?;
            cached_inline_bytes(value)
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

fn cached_inline_bytes(value: &str) -> Result<Arc<[u8]>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Arc<[u8]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(bytes) = cache
        .lock()
        .map_err(|err| err.to_string())?
        .get(value)
        .map(Arc::clone)
    {
        return Ok(bytes);
    }
    let bytes = Arc::<[u8]>::from(decode_hex(value)?);
    let mut cache = cache.lock().map_err(|err| err.to_string())?;
    let bytes = cache
        .entry(value.to_string())
        .or_insert_with(|| Arc::clone(&bytes));
    Ok(Arc::clone(bytes))
}

fn cached_file_bytes(path: &str) -> Result<Arc<[u8]>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Arc<[u8]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(bytes) = cache
        .lock()
        .map_err(|err| err.to_string())?
        .get(path)
        .map(Arc::clone)
    {
        return Ok(bytes);
    }
    let bytes = Arc::<[u8]>::from(
        fs::read(fixture_dir().join(path)).map_err(|err| format!("read {path}: {err}"))?,
    );
    let mut cache = cache.lock().map_err(|err| err.to_string())?;
    let bytes = cache
        .entry(path.to_string())
        .or_insert_with(|| Arc::clone(&bytes));
    Ok(Arc::clone(bytes))
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

fn wasm_size_metrics_json(metrics: &wasm_abi::FontdoneWasmSizeMetrics) -> Value {
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

fn is_face_probe(case: &InputCase) -> Result<bool, String> {
    Ok(face_index_param(&case.inputs.params)? < 0)
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
