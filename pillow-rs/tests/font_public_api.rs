use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::Value;

#[path = "support/font_runner.rs"]
mod font_runner;
#[path = "support/migration_parity.rs"]
mod migration_parity;

use migration_parity::{Case, OutputShape, ResultEnvelope, compare_results};

const NO_LIBRAQM_MESSAGE: &str =
    "'setting text direction, language or font features is not supported without libraqm'";

const FORBIDDEN_INPUT_KEYS: [&str; 20] = [
    "actual",
    "baseline",
    "encoded_ref_bytes",
    "encoded_ref_path",
    "error",
    "expect_error",
    "expectation",
    "expected",
    "golden",
    "hash",
    "oracle",
    "output",
    "outputs",
    "pixels",
    "pixels_hex",
    "raw_path",
    "ref_bytes",
    "ref_path",
    "sha256",
    "status",
];

#[derive(Debug)]
struct FontManifest {
    asset_dir: String,
    input_dir: String,
    input_files: BTreeSet<String>,
    negative_operations: BTreeSet<String>,
    operation_shapes: BTreeMap<String, OutputShape>,
    public_method_parameters: BTreeMap<String, ParameterCoverage>,
    required_operations: BTreeSet<String>,
    source_name: String,
    target_name: String,
    case_count: usize,
}

#[derive(Debug)]
struct ParameterCoverage {
    blocked: BTreeSet<String>,
    covered: BTreeSet<String>,
    required_values: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationManifest {
    version: u64,
    source: SourceIdentity,
    target: TargetIdentity,
    policy: Policy,
    migration: Migration,
    accounting: Accounting,
    surfaces: Vec<Surface>,
    evidence: Evidence,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceIdentity {
    name: String,
    version: String,
    runtime: String,
    contract: String,
    identity: SourceRuntimeIdentity,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRuntimeIdentity {
    module: String,
    native_core: String,
    freetype_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetIdentity {
    name: String,
    version: String,
    runtime: String,
    contract: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    input_only: bool,
    live_oracle: bool,
    result_comparison: bool,
    coverage_required_for_claims: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    source: String,
    source_status: String,
    case_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Accounting {
    inventory_source: String,
    inventory_version: String,
    surface_total: usize,
    surface_accounted: usize,
    surface_accounting_percent: u64,
    public_name_total: usize,
    public_name_accounted: usize,
    public_name_accounting_percent: u64,
    active_surface_count: usize,
    pending_surface_count: usize,
    active_operation_count: usize,
    unsupported_operation_count: usize,
    pending_operation_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Surface {
    id: String,
    source_path: String,
    target_path: String,
    input_root: Option<String>,
    asset_root: String,
    status: String,
    reason: Option<String>,
    blocker: Option<String>,
    exclusions: Vec<String>,
    inventory: SurfaceInventory,
    public_names: PublicNames,
    operations: Vec<ManifestOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceInventory {
    source: String,
    public_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicNames {
    active: Vec<String>,
    pending: Vec<String>,
    non_endpoint: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestOperation {
    id: String,
    kind: String,
    status: String,
    legacy_status: Option<String>,
    input: Option<String>,
    output_shape: Option<OutputShape>,
    required_parameter_values: BTreeMap<String, Vec<String>>,
    branches: Vec<String>,
    coverage_regions: Vec<String>,
    case_count: usize,
    reason: Option<String>,
    blocker: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    parity_command: String,
    coverage_command: String,
    coverage_artifact: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OraclePayload {
    identity: OracleIdentity,
    results: Vec<ResultEnvelope>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OracleIdentity {
    pillow_version: String,
    python_executable: String,
    pillow_module: String,
    native_core: String,
    freetype_version: String,
}

const EXPECTED_FONT_PUBLIC_OPERATIONS: [&str; 41] = [
    "ImageFont.getbbox",
    "ImageFont.info",
    "ImageFont.getlength",
    "ImageFont.getmask",
    "TransposedFont.getbbox",
    "TransposedFont.getlength",
    "TransposedFont.getmask",
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
    "load",
    "load_default",
    "load_default_imagefont",
    "load_path",
    "native_face_attrs",
    "native_getlength_26dot6",
    "native_getsize",
    "native_render",
    "native_getvaraxes",
    "native_getvarnames",
    "native_setvaraxes",
    "native_setvarname",
    "render_text_binary",
    "set_variation_by_axes",
    "set_variation_by_name",
    "text_bbox",
    "transposed_bbox",
    "truetype",
    "validate_transposed_length",
];

const EXPECTED_PROJECT_SURFACES: [&str; 12] = [
    "Image",
    "ImageModule",
    "ImageDraw",
    "ImageFilter",
    "ImageEnhance",
    "ImageOps",
    "ImageChops",
    "ImageColor",
    "ImagePalette",
    "font",
    "ImageStat",
    "ImageSequence",
];

const EXPECTED_PROJECT_PUBLIC_NAME_COUNT: usize = 173;
const EXPECTED_PROJECT_PENDING_OPERATION_COUNT: usize = 164;

const EXPECTED_REPO_FONT_HELPER_OPERATIONS: [&str; 18] = [
    "draw_text",
    "font_size",
    "get_transposed_mask",
    "getbbox_binary",
    "getmask2_with_start",
    "has_variations",
    "native_face_attrs",
    "native_getlength_26dot6",
    "native_getsize",
    "native_render",
    "native_getvaraxes",
    "native_getvarnames",
    "native_setvaraxes",
    "native_setvarname",
    "render_text_binary",
    "text_bbox",
    "transposed_bbox",
    "validate_transposed_length",
];

const EXPECTED_OUT_OF_SCOPE: [&str; 1] = [
    "libraqm successful shaping; direction/features/language rows must match Pillow's no-libraqm errors",
];

const EXPECTED_IMAGEFONT_LAYOUT_MEMBERS: [&str; 2] = ["BASIC", "RAQM"];

const EXPECTED_IMAGEFONT_BEHAVIORAL_PUBLIC_NAMES: [&str; 9] = [
    "FreeTypeFont",
    "ImageFont",
    "Layout",
    "TransposedFont",
    "load",
    "load_default",
    "load_default_imagefont",
    "load_path",
    "truetype",
];

const EXPECTED_IMAGEFONT_NON_ENDPOINT_PUBLIC_NAMES: [&str; 21] = [
    "Any",
    "Axis",
    "BinaryIO",
    "BytesIO",
    "DeferredError",
    "IO",
    "Image",
    "IntEnum",
    "MAX_STRING_LENGTH",
    "ModuleType",
    "StrOrBytesPath",
    "TYPE_CHECKING",
    "TypedDict",
    "annotations",
    "base64",
    "cast",
    "core",
    "is_path",
    "os",
    "sys",
    "warnings",
];

const EXPECTED_BLOCKED_PUBLIC_PARAMETERS: [(&str, &str); 0] = [];

const EXPECTED_FREETYPE_STROKE_BLOCKING_CASES: [&str; 0] = [];

const EXPECTED_PARTIAL_STROKER_SYMBOLS: [&str; 17] = [
    "FT_Outline_GetInsideBorder",
    "FT_Outline_GetOutsideBorder",
    "FT_Stroker_New",
    "FT_Stroker_Set",
    "FT_Stroker_Rewind",
    "FT_Stroker_ParseOutline",
    "FT_Stroker_BeginSubPath",
    "FT_Stroker_EndSubPath",
    "FT_Stroker_LineTo",
    "FT_Stroker_ConicTo",
    "FT_Stroker_CubicTo",
    "FT_Stroker_GetBorderCounts",
    "FT_Stroker_ExportBorder",
    "FT_Stroker_GetCounts",
    "FT_Stroker_Export",
    "FT_Stroker_Done",
    "FT_Glyph_StrokeBorder",
];

const DEFAULT_PARAMETER_VALUE: &str = "<default>";

const REQUIRED_PUBLIC_PARAMETER_VALUES: &[(&str, &str, &str)] = &[
    ("font_variant", "layout_engine", "BASIC"),
    ("font_variant", "layout_engine", "RAQM"),
    ("getbbox", "anchor", DEFAULT_PARAMETER_VALUE),
    ("getbbox", "anchor", "a"),
    ("getbbox", "anchor", "la"),
    ("getbbox", "anchor", "lb"),
    ("getbbox", "anchor", "ls"),
    ("getbbox", "anchor", "lt"),
    ("getbbox", "anchor", "lx"),
    ("getbbox", "anchor", "mm"),
    ("getbbox", "anchor", "rd"),
    ("getbbox", "anchor", "xy"),
    ("getbbox", "direction", "rtl"),
    ("getbbox", "features", "[]"),
    ("getbbox", "language", "en"),
    ("getbbox", "mode", DEFAULT_PARAMETER_VALUE),
    ("getbbox", "mode", "1"),
    ("getbbox", "mode", "bad"),
    ("getbbox", "stroke_width", DEFAULT_PARAMETER_VALUE),
    ("getbbox", "stroke_width", "0.5"),
    ("getbbox", "stroke_width", "1"),
    ("getlength", "direction", "rtl"),
    ("getlength", "features", "[]"),
    ("getlength", "features", "[\"-kern\"]"),
    ("getlength", "language", "en"),
    ("getlength", "mode", DEFAULT_PARAMETER_VALUE),
    ("getlength", "mode", "1"),
    ("getlength", "mode", "bad"),
    ("getmask", "anchor", DEFAULT_PARAMETER_VALUE),
    ("getmask", "anchor", "mm"),
    ("getmask", "direction", "rtl"),
    ("getmask", "features", "[]"),
    ("getmask", "ink", DEFAULT_PARAMETER_VALUE),
    ("getmask", "ink", "123"),
    ("getmask", "ink", "[1,2,3]"),
    ("getmask", "language", "en"),
    ("getmask", "mode", DEFAULT_PARAMETER_VALUE),
    ("getmask", "mode", "1"),
    ("getmask", "mode", "RGBA"),
    ("getmask", "start", DEFAULT_PARAMETER_VALUE),
    ("getmask", "start", "[-10,0]"),
    ("getmask", "start", "[-100,0]"),
    ("getmask", "start", "[0,-10]"),
    ("getmask", "start", "[0.0,0.0]"),
    ("getmask", "start", "[1.25,2.5]"),
    ("getmask", "stroke_width", DEFAULT_PARAMETER_VALUE),
    ("getmask", "stroke_width", "-1.5"),
    ("getmask", "stroke_width", "1.5"),
    ("getmask2", "anchor", DEFAULT_PARAMETER_VALUE),
    ("getmask2", "anchor", "mm"),
    ("getmask2", "args", DEFAULT_PARAMETER_VALUE),
    (
        "getmask2",
        "args",
        "[\"L\",null,null,null,0,null,123,null,\"ignored\"]",
    ),
    ("getmask2", "direction", "rtl"),
    ("getmask2", "features", "[]"),
    ("getmask2", "ink", DEFAULT_PARAMETER_VALUE),
    ("getmask2", "ink", "[1,2,3]"),
    ("getmask2", "kwargs", DEFAULT_PARAMETER_VALUE),
    (
        "getmask2",
        "kwargs",
        "{\"stroke_filled\":true,\"unknown\":1}",
    ),
    ("getmask2", "language", "en"),
    ("getmask2", "mode", DEFAULT_PARAMETER_VALUE),
    ("getmask2", "mode", "1"),
    ("getmask2", "mode", "RGBA"),
    ("getmask2", "start", DEFAULT_PARAMETER_VALUE),
    ("getmask2", "start", "[-10,0]"),
    ("getmask2", "start", "[0,-10]"),
    ("getmask2", "start", "[-100.0,0.0]"),
    ("getmask2", "start", "[0.0,-100.0]"),
    ("getmask2", "start", "[0.0,0.0]"),
    ("getmask2", "start", "[1.25,2.5]"),
    ("getmask2", "start", "[100.0,0.0]"),
    ("getmask2", "stroke_width", DEFAULT_PARAMETER_VALUE),
    ("getmask2", "stroke_width", "-1.5"),
    ("getmask2", "stroke_width", "1.5"),
    ("truetype", "encoding", ""),
    ("truetype", "index", "0"),
    ("truetype", "layout_engine", "RAQM"),
];

const ROOT_FONT_API_TO_OPERATION: [(&str, &str); 46] = [
    ("imagefont_from_bytes", "truetype"),
    ("imagefont_from_bytes_with_options", "truetype"),
    ("imagefont_get_variation_axes", "get_variation_axes"),
    ("imagefont_get_variation_names", "get_variation_names"),
    ("imagefont_get_transposed_mask", "get_transposed_mask"),
    ("imagefont_getbbox", "getbbox"),
    ("imagefont_getbbox_binary", "getbbox_binary"),
    ("imagefont_getbbox_binary_bytes", "getbbox_binary"),
    ("imagefont_getbbox_bytes", "getbbox"),
    ("imagefont_getbbox_bytes_with_options", "getbbox"),
    ("imagefont_getbbox_with_options", "getbbox"),
    ("imagefont_getlength", "getlength"),
    ("imagefont_getlength_bytes", "getlength"),
    ("imagefont_getlength_bytes_with_options", "getlength"),
    ("imagefont_getlength_with_options", "getlength"),
    ("imagefont_getmask", "getmask"),
    ("imagefont_getmask2", "getmask2"),
    ("imagefont_getmask2_bytes", "getmask2"),
    ("imagefont_getmask2_bytes_with_options", "getmask2"),
    ("imagefont_getmask2_bytes_with_start", "getmask2_with_start"),
    ("imagefont_getmask2_with_options", "getmask2"),
    ("imagefont_getmask2_with_start", "getmask2_with_start"),
    ("imagefont_getmask_bytes", "getmask"),
    ("imagefont_getmask_bytes_with_options", "getmask"),
    ("imagefont_getmask_with_options", "getmask"),
    ("imagefont_getmetrics", "getmetrics"),
    ("imagefont_getname", "getname"),
    ("imagefont_getname_optional", "getname"),
    ("imagefont_has_variations", "has_variations"),
    ("imagefont_load_default", "load_default"),
    ("imagefont_native_face_attrs", "native_face_attrs"),
    (
        "imagefont_native_getlength_26dot6",
        "native_getlength_26dot6",
    ),
    ("imagefont_native_getsize", "native_getsize"),
    ("imagefont_native_render", "native_render"),
    ("imagefont_native_getvaraxes", "native_getvaraxes"),
    ("imagefont_native_getvarnames", "native_getvarnames"),
    ("imagefont_native_setvaraxes", "native_setvaraxes"),
    ("imagefont_native_setvarname", "native_setvarname"),
    ("imagefont_render_text_binary", "render_text_binary"),
    ("imagefont_set_variation_by_axes", "set_variation_by_axes"),
    ("imagefont_set_variation_by_name", "set_variation_by_name"),
    ("imagefont_size", "font_size"),
    ("imagefont_text_bbox", "text_bbox"),
    ("imagefont_text_bbox_bytes", "text_bbox"),
    ("imagefont_variant", "font_variant"),
    ("imagefont_variant_with_options", "font_variant"),
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
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

fn assert_present(value: &Option<String>, context: &str) {
    assert!(
        value.as_ref().is_some_and(|text| !text.is_empty()),
        "{context} must be present and non-empty"
    );
}

fn assert_project_surface_accounting(path: &Path, manifest: &MigrationManifest) {
    let accounting = &manifest.accounting;
    assert_eq!(
        accounting.inventory_source,
        "tests/deprecated/project_manifest_v0/manifest.yaml"
    );
    assert_eq!(accounting.inventory_version, "0.2.0");
    assert_eq!(accounting.surface_total, EXPECTED_PROJECT_SURFACES.len());
    assert_eq!(
        accounting.surface_accounted,
        EXPECTED_PROJECT_SURFACES.len()
    );
    assert_eq!(accounting.surface_accounting_percent, 100);
    assert_eq!(
        accounting.public_name_total,
        EXPECTED_PROJECT_PUBLIC_NAME_COUNT
    );
    assert_eq!(
        accounting.public_name_accounted,
        EXPECTED_PROJECT_PUBLIC_NAME_COUNT
    );
    assert_eq!(accounting.public_name_accounting_percent, 100);
    assert_eq!(accounting.active_surface_count, 1);
    assert_eq!(
        accounting.pending_surface_count,
        EXPECTED_PROJECT_SURFACES.len() - 1
    );
    assert_eq!(
        accounting.active_operation_count,
        EXPECTED_FONT_PUBLIC_OPERATIONS.len()
    );
    assert_eq!(accounting.unsupported_operation_count, 1);
    assert_eq!(
        accounting.pending_operation_count,
        EXPECTED_PROJECT_PENDING_OPERATION_COUNT
    );

    let expected_surface_ids = EXPECTED_PROJECT_SURFACES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let observed_surface_ids = manifest
        .surfaces
        .iter()
        .map(|surface| surface.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_surface_ids,
        expected_surface_ids,
        "{} must classify every deprecated project-manifest surface exactly",
        path.display()
    );
    assert_eq!(
        observed_surface_ids.len(),
        manifest.surfaces.len(),
        "{} contains duplicate surface IDs",
        path.display()
    );

    let mut inventory_name_count = 0usize;
    let mut active_surface_count = 0usize;
    let mut pending_surface_count = 0usize;
    let mut active_operation_count = 0usize;
    let mut unsupported_operation_count = 0usize;
    let mut pending_operation_count = 0usize;

    for surface in &manifest.surfaces {
        assert!(
            !surface.source_path.is_empty() && !surface.target_path.is_empty(),
            "{} surface {} must declare source and target paths",
            path.display(),
            surface.id
        );
        assert_eq!(
            surface.inventory.source,
            "deprecated-project-manifest-v0",
            "{} surface {} has an unknown inventory source",
            path.display(),
            surface.id
        );
        let inventory_names = surface
            .inventory
            .public_names
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            inventory_names.len(),
            surface.inventory.public_names.len(),
            "{} surface {} contains duplicate inventory names",
            path.display(),
            surface.id
        );
        assert!(
            !inventory_names.is_empty(),
            "{} surface {} has an empty legacy inventory",
            path.display(),
            surface.id
        );
        inventory_name_count += inventory_names.len();

        let active_names = surface
            .public_names
            .active
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let pending_names = surface
            .public_names
            .pending
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let non_endpoint_names = surface
            .public_names
            .non_endpoint
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(active_names.len(), surface.public_names.active.len());
        assert_eq!(pending_names.len(), surface.public_names.pending.len());
        assert_eq!(
            non_endpoint_names.len(),
            surface.public_names.non_endpoint.len()
        );
        assert!(
            active_names.is_disjoint(&pending_names)
                && active_names.is_disjoint(&non_endpoint_names)
                && pending_names.is_disjoint(&non_endpoint_names),
            "{} surface {} classifies a public name more than once",
            path.display(),
            surface.id
        );
        let classified_names = active_names
            .union(&pending_names)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&non_endpoint_names)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unclassified = inventory_names
            .difference(&classified_names)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            unclassified.is_empty(),
            "{} surface {} leaves legacy public names unclassified: {unclassified:?}",
            path.display(),
            surface.id
        );

        match surface.status.as_str() {
            "active" => {
                active_surface_count += 1;
                assert_eq!(
                    surface.id,
                    "font",
                    "{} only the proven Font surface may be active",
                    path.display()
                );
                assert!(surface.reason.is_none() && surface.blocker.is_none());
                assert!(surface.public_names.pending.is_empty());
                assert_present(
                    &surface.input_root,
                    &format!("active surface {} input_root", surface.id),
                );
            }
            "pending" => {
                pending_surface_count += 1;
                assert_present(
                    &surface.reason,
                    &format!("pending surface {} reason", surface.id),
                );
                assert_present(
                    &surface.blocker,
                    &format!("pending surface {} blocker", surface.id),
                );
                assert!(
                    surface.input_root.is_none(),
                    "{} pending surface {} must not claim an active input root",
                    path.display(),
                    surface.id
                );
                assert!(surface.public_names.active.is_empty());
                assert_eq!(
                    pending_names,
                    inventory_names,
                    "{} pending surface {} must expose its full legacy inventory as pending",
                    path.display(),
                    surface.id
                );
            }
            other => panic!(
                "{} surface {} has unclassified status {other}",
                path.display(),
                surface.id
            ),
        }

        let mut operation_ids = BTreeSet::new();
        for operation in &surface.operations {
            assert!(
                operation_ids.insert(operation.id.clone()),
                "{} surface {} duplicates operation {}",
                path.display(),
                surface.id,
                operation.id
            );
            assert!(
                !operation.kind.is_empty(),
                "{} surface {} operation {} must declare a kind",
                path.display(),
                surface.id,
                operation.id
            );
            match operation.status.as_str() {
                "active" | "unsupported" => {
                    assert_eq!(
                        surface.status,
                        "active",
                        "{} executable operation {} cannot belong to pending surface {}",
                        path.display(),
                        operation.id,
                        surface.id
                    );
                    assert_present(
                        &operation.input,
                        &format!("operation {} input", operation.id),
                    );
                    assert!(
                        operation.output_shape.is_some(),
                        "{} operation {} must declare an output shape",
                        path.display(),
                        operation.id
                    );
                    assert!(
                        !operation.branches.is_empty() && !operation.coverage_regions.is_empty(),
                        "{} operation {} must declare branches and coverage regions",
                        path.display(),
                        operation.id
                    );
                    assert!(
                        operation.reason.is_none()
                            && operation.blocker.is_none()
                            && operation.legacy_status.is_none(),
                        "{} executable operation {} must not carry pending metadata",
                        path.display(),
                        operation.id
                    );
                    if operation.status == "active" {
                        active_operation_count += 1;
                    } else {
                        unsupported_operation_count += 1;
                    }
                }
                "pending" => {
                    pending_operation_count += 1;
                    assert_eq!(
                        surface.status,
                        "pending",
                        "{} pending operation {} must belong to a pending surface",
                        path.display(),
                        operation.id
                    );
                    assert!(
                        operation.input.is_none()
                            && operation.output_shape.is_none()
                            && operation.branches.is_empty()
                            && operation.coverage_regions.is_empty()
                            && operation.case_count == 0,
                        "{} pending operation {} must not claim runnable evidence",
                        path.display(),
                        operation.id
                    );
                    assert_present(
                        &operation.reason,
                        &format!("pending operation {} reason", operation.id),
                    );
                    assert_present(
                        &operation.blocker,
                        &format!("pending operation {} blocker", operation.id),
                    );
                    assert!(
                        operation
                            .legacy_status
                            .as_deref()
                            .is_some_and(|status| matches!(
                                status,
                                "implemented" | "ignored" | "unclassified"
                            )),
                        "{} pending operation {} has an invalid legacy status",
                        path.display(),
                        operation.id
                    );
                }
                other => panic!(
                    "{} surface {} operation {} has unclassified status {other}",
                    path.display(),
                    surface.id,
                    operation.id
                ),
            }
        }
        if surface.status == "pending" {
            assert_eq!(
                operation_ids,
                inventory_names,
                "{} pending surface {} operation rows must exactly match its legacy inventory",
                path.display(),
                surface.id
            );
        }
    }

    assert_eq!(inventory_name_count, accounting.public_name_accounted);
    assert_eq!(active_surface_count, accounting.active_surface_count);
    assert_eq!(pending_surface_count, accounting.pending_surface_count);
    assert_eq!(active_operation_count, accounting.active_operation_count);
    assert_eq!(
        unsupported_operation_count,
        accounting.unsupported_operation_count
    );
    assert_eq!(pending_operation_count, accounting.pending_operation_count);
}

fn load_manifest(root: &Path) -> FontManifest {
    let path = root.join("manifest.yaml");
    let text = fs::read_to_string(&path).expect("migration parity manifest must be readable");
    let manifest: MigrationManifest = serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!(
            "{} must be strict JSON-compatible YAML: {error}",
            path.display()
        )
    });
    assert_eq!(manifest.version, 1, "{} must use version 1", path.display());
    assert_eq!(manifest.source.name, "Pillow");
    assert_eq!(manifest.source.version, "12.2.0");
    assert_eq!(manifest.source.runtime, ".oracle-venv/bin/python");
    assert_eq!(manifest.source.contract, "PIL.ImageFont public behavior");
    assert_eq!(manifest.source.identity.module, "PIL.ImageFont");
    assert_eq!(manifest.source.identity.native_core, "PIL._imagingft");
    assert_eq!(manifest.source.identity.freetype_version, "2.14.3");
    assert_eq!(manifest.target.name, "pillow-rs");
    assert_eq!(manifest.target.version, "current-checkout");
    assert_eq!(
        manifest.target.runtime,
        "Rust integration test calling pillow_rs root public API"
    );
    assert_eq!(
        manifest.target.contract,
        "Result-style public Font behavior"
    );
    assert!(
        manifest.policy.input_only
            && manifest.policy.live_oracle
            && manifest.policy.result_comparison
            && manifest.policy.coverage_required_for_claims,
        "{} must require the full parity evidence policy",
        path.display()
    );
    assert_eq!(
        manifest.migration.source,
        "tests/deprecated/font_public_api_v0"
    );
    assert_eq!(manifest.migration.source_status, "deprecated");
    assert_eq!(
        manifest.evidence.parity_command,
        "make migration-parity-test"
    );
    assert_eq!(
        manifest.evidence.coverage_command,
        "make coverage-font-rust-with-freetype"
    );
    assert_eq!(
        manifest.evidence.coverage_artifact,
        "coverage/font-rust-with-freetype"
    );
    assert_project_surface_accounting(&path, &manifest);
    let surface = manifest
        .surfaces
        .into_iter()
        .find(|surface| surface.id == "font")
        .expect("project accounting must contain the active Font surface");
    assert_eq!(surface.id, "font");
    assert_eq!(surface.source_path, "PIL.ImageFont");
    assert_eq!(surface.target_path, "pillow_rs::imagefont_*");
    assert_eq!(surface.status, "active");
    assert!(surface.public_names.pending.is_empty());
    assert_eq!(
        surface
            .public_names
            .active
            .into_iter()
            .collect::<BTreeSet<_>>(),
        EXPECTED_IMAGEFONT_BEHAVIORAL_PUBLIC_NAMES
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        surface
            .public_names
            .non_endpoint
            .into_iter()
            .collect::<BTreeSet<_>>(),
        EXPECTED_IMAGEFONT_NON_ENDPOINT_PUBLIC_NAMES
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    );

    let out_of_scope = surface.exclusions.into_iter().collect::<BTreeSet<_>>();
    let mut input_files = BTreeSet::new();
    let mut negative_operations = BTreeSet::new();
    let mut operation_shapes = BTreeMap::new();
    let mut public_method_parameters = BTreeMap::new();
    let mut required_operations = BTreeSet::new();
    let mut declared_case_count = 0usize;
    for operation in surface.operations {
        assert!(!operation.kind.is_empty());
        assert!(
            !operation.branches.is_empty() && !operation.coverage_regions.is_empty(),
            "{} operation {} must declare intended branches and coverage regions",
            path.display(),
            operation.id
        );
        let input = operation
            .input
            .expect("validated active Font operation must have an input");
        assert!(
            input_files.insert(input.clone()),
            "{} input file {} is assigned twice",
            path.display(),
            input
        );
        let output_shape = operation
            .output_shape
            .expect("validated active Font operation must have an output shape");
        assert!(
            operation_shapes
                .insert(operation.id.clone(), output_shape)
                .is_none(),
            "{} operation {} is duplicated",
            path.display(),
            operation.id
        );
        match operation.status.as_str() {
            "active" => {
                required_operations.insert(operation.id.clone());
            }
            "unsupported" => {
                negative_operations.insert(operation.id.clone());
            }
            other => panic!(
                "{} Font operation {} has unclassified status {other}",
                path.display(),
                operation.id
            ),
        }
        let required_values = operation
            .required_parameter_values
            .into_iter()
            .map(|(parameter, values)| (parameter, values.into_iter().collect()))
            .collect::<BTreeMap<_, _>>();
        if !EXPECTED_REPO_FONT_HELPER_OPERATIONS.contains(&operation.id.as_str())
            && operation.status == "active"
        {
            public_method_parameters.insert(
                operation.id,
                ParameterCoverage {
                    blocked: BTreeSet::new(),
                    covered: required_values.keys().cloned().collect(),
                    required_values,
                },
            );
        }
        declared_case_count += operation.case_count;
    }
    let expected_operations = EXPECTED_FONT_PUBLIC_OPERATIONS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        required_operations,
        expected_operations,
        "{} required_operations must exactly enumerate the current implemented ImageFont public parity surface",
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
    let expected_out_of_scope = EXPECTED_OUT_OF_SCOPE
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        out_of_scope,
        expected_out_of_scope,
        "{} out_of_scope must be exact; libraqm successful shaping is the only excluded PIL.ImageFont behavior",
        path.display()
    );
    assert_eq!(declared_case_count, manifest.migration.case_count);
    FontManifest {
        asset_dir: surface.asset_root,
        input_dir: surface
            .input_root
            .expect("validated active Font surface must have an input root"),
        input_files,
        negative_operations,
        operation_shapes,
        public_method_parameters,
        required_operations,
        source_name: manifest.source.name,
        target_name: manifest.target.name,
        case_count: declared_case_count,
    }
}

fn load_input_cases(directory: &Path, manifest: &FontManifest) -> Vec<Value> {
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
        manifest.input_files, discovered_files,
        "font public-api manifest input_files must exactly match raw input JSON files"
    );

    let allowed_document_operations = manifest
        .required_operations
        .union(&manifest.negative_operations)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut cases = Vec::new();
    for file in &manifest.input_files {
        let path = directory.join(file.as_str());
        let document: Value = serde_json::from_slice(
            &fs::read(&path).expect("font public-api input must be readable"),
        )
        .expect("font public-api input must be valid JSON");
        assert_input_document_envelope(&path, &document, &allowed_document_operations);
        let document_operation = document
            .get("operation")
            .and_then(Value::as_str)
            .map(normalize_font_operation)
            .expect("font public-api input operation must be a string");
        assert_input_only_case(&path, &document);
        let rows = document
            .get("cases")
            .and_then(Value::as_array)
            .expect("font public-api input must contain a cases array");
        for case in rows {
            assert_input_only_case(&path, case);
            let typed: Case = serde_json::from_value(case.clone()).unwrap_or_else(|error| {
                panic!(
                    "{} contains an invalid strict Case: {error}",
                    path.display()
                )
            });
            let case = serde_json::to_value(typed).expect("strict Case must serialize");
            let case_operation = font_runner::operation(&case)
                .expect("font public-api case operation must be a string");
            assert!(
                document_operation == case_operation,
                "{} declares operation `{document_operation}` but contains case operation `{case_operation}`",
                path.display()
            );
            cases.push(case);
        }
    }
    assert_eq!(
        cases.len(),
        manifest.case_count,
        "manifest operation case counts must equal the loaded corpus"
    );
    cases
}

fn normalize_font_operation(operation: &str) -> &str {
    operation.strip_prefix("font.").unwrap_or(operation)
}

fn assert_input_document_envelope(
    path: &Path,
    document: &Value,
    allowed_document_operations: &BTreeSet<String>,
) {
    let object = document
        .as_object()
        .unwrap_or_else(|| panic!("{} must be a JSON object", path.display()));
    let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let required_keys = BTreeSet::from(["cases", "operation", "surface", "version"]);
    assert_eq!(
        keys,
        required_keys,
        "{} must contain only version, surface, operation, and cases",
        path.display()
    );
    assert_eq!(
        document.get("version").and_then(Value::as_i64),
        Some(1),
        "{} must use version 1",
        path.display()
    );
    let operation = document
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} operation must be a string", path.display()));
    assert_eq!(
        document.get("surface").and_then(Value::as_str),
        Some("font"),
        "{} surface must be `font`",
        path.display()
    );
    let operation = normalize_font_operation(operation);
    assert!(
        allowed_document_operations.contains(operation),
        "{} top-level operation `{operation}` must be listed in the manifest",
        path.display()
    );
    let cases = document
        .get("cases")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} cases must be an array", path.display()));
    assert!(
        !cases.is_empty(),
        "{} cases must not be empty",
        path.display()
    );
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

fn assert_result_ids_match(expected_ids: &BTreeSet<String>, actual_ids: &BTreeSet<String>) {
    assert_eq!(
        actual_ids, expected_ids,
        "Result IDs must exactly match input Case IDs"
    );
}

fn assert_case_ids_match_operations(cases: &[Value]) {
    for case in cases {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .expect("font public-api case_id must be a string");
        let operation =
            font_runner::operation(case).expect("font public-api operation must be a string");
        let operation_prefix = format!("font.{operation}.");
        assert!(
            case_id.starts_with(&operation_prefix),
            "{case_id}: case_id prefix must match normalized operation `{operation}`"
        );
    }
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
                "builtin" => {
                    assert!(
                        matches!(
                            asset.get("name").and_then(Value::as_str),
                            Some("load_default" | "pilfont_default")
                        ),
                        "{case_id}.{asset_name}: builtin font name must be classified"
                    );
                }
                "ref" | "missing_ref" => {
                    let relative = asset
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| panic!("{case_id}.{asset_name}: asset path missing"));
                    let path = fixture_root.join(relative);
                    if kind == "missing_ref" {
                        assert!(
                            !path.exists(),
                            "{case_id}.{asset_name}: missing_ref must reference an absent file"
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

fn root_font_public_functions() -> BTreeSet<String> {
    let lib_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let text = fs::read_to_string(&lib_rs).expect("src/lib.rs must be readable");
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix("pub fn imagefont_"))
        .map(|suffix| {
            let name = suffix
                .split_once('(')
                .map(|(name, _)| name)
                .unwrap_or_else(|| {
                    panic!("malformed root ImageFont API line in {}", lib_rs.display())
                });
            format!("imagefont_{name}")
        })
        .collect()
}

fn assert_manifest_covers_root_font_api(manifest: &FontManifest) {
    let observed = root_font_public_functions();
    let expected = ROOT_FONT_API_TO_OPERATION
        .iter()
        .map(|(function, _)| (*function).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, expected,
        "ImageFont manifest test must explicitly map every root pillow_rs::imagefont_* public function to a manifest operation"
    );

    let missing_operations = ROOT_FONT_API_TO_OPERATION
        .iter()
        .filter(|(_, operation)| !manifest.required_operations.contains(*operation))
        .collect::<Vec<_>>();
    assert!(
        missing_operations.is_empty(),
        "root pillow_rs::imagefont_* functions map to operations missing from font_manifest.yaml required_operations: {missing_operations:?}"
    );
}

fn runner_root_font_api_references() -> BTreeSet<String> {
    let runner = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/font_runner.rs");
    let text = fs::read_to_string(&runner).expect("font runner must be readable");
    let marker = "pillow_rs::imagefont_";
    let mut references = BTreeSet::new();

    for line in text.lines() {
        let mut rest = line;
        while let Some(index) = rest.find(marker) {
            let suffix = &rest[index + "pillow_rs::".len()..];
            let name = suffix
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<String>();
            references.insert(name);
            rest = &suffix[marker.len() - "pillow_rs::".len()..];
        }
    }

    assert!(
        !references.is_empty(),
        "font runner must call root pillow_rs::imagefont_* APIs directly"
    );
    references
}

fn assert_runner_exercises_root_font_api() {
    let root_functions = root_font_public_functions();
    let runner_references = runner_root_font_api_references();
    assert_eq!(
        runner_references, root_functions,
        "font_public_api runner must reference every root pillow_rs::imagefont_* function exactly; otherwise the manifest may map an API that no fixture can execute"
    );
}

fn runner_public_operations() -> BTreeSet<String> {
    let runner = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/font_runner.rs");
    let text = fs::read_to_string(&runner).expect("font runner must be readable");
    let mut operations = BTreeSet::new();
    let mut in_operation_match = false;

    for line in text.lines() {
        let indent = line
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let trimmed = line.trim();
        if trimmed == "match operation {" {
            in_operation_match = true;
            continue;
        }
        if !in_operation_match {
            continue;
        }
        if indent == 8 && trimmed.starts_with("other =>") {
            break;
        }
        if indent != 8 || !trimmed.contains("=>") {
            continue;
        }

        let Some((patterns, _)) = trimmed.split_once("=>") else {
            continue;
        };
        for pattern in patterns.split('|') {
            let operation = pattern
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches('"');
            if !operation.is_empty() && operation != "_" {
                operations.insert(operation.to_owned());
            }
        }
    }

    assert!(
        !operations.is_empty(),
        "font runner public operation match must be discoverable"
    );
    operations
}

fn assert_manifest_operations_have_runner_arms(manifest: &FontManifest) {
    let runner_operations = runner_public_operations();
    assert_eq!(
        runner_operations, manifest.required_operations,
        "active manifest operations must exactly match explicit font_runner public operation arms"
    );
    let registered_operations = font_runner::SUPPORTED_OPERATIONS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let manifest_operations = manifest
        .required_operations
        .union(&manifest.negative_operations)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        registered_operations, manifest_operations,
        "manifest operations and the target operation registry must map both ways"
    );
}

fn assert_manifest_operations_match_live_pillow_plus_repo_helpers(
    manifest: &FontManifest,
    pillow_operations: &BTreeSet<String>,
) {
    let helper_operations = EXPECTED_REPO_FONT_HELPER_OPERATIONS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let overlap = pillow_operations
        .intersection(&helper_operations)
        .collect::<Vec<_>>();
    assert!(
        overlap.is_empty(),
        "repo ImageFont helper operations must not duplicate live Pillow public operations: {overlap:?}"
    );
    let expected = pillow_operations
        .union(&helper_operations)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        manifest.required_operations, expected,
        "font_manifest.yaml required_operations must exactly equal live PIL.ImageFont public operations plus the explicit repo helper/consumer operations"
    );
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

fn run_oracle(cases: &[Value]) -> BTreeMap<String, ResultEnvelope> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/font_oracle.py");
    let oracle = oracle_python();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let venv_root = repo_root.join(".oracle-venv");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    let capture_root = env::temp_dir().join(format!(
        "pillow-rs-font-oracle-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&capture_root).expect("oracle capture directory must be creatable");
    let stdout_path = capture_root.join("stdout.json");
    let stderr_path = capture_root.join("stderr.txt");
    let mut command = Command::new(oracle.as_os_str());
    command
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(
            File::create(&stdout_path).expect("oracle stdout capture must be creatable"),
        ))
        .stderr(Stdio::from(
            File::create(&stderr_path).expect("oracle stderr capture must be creatable"),
        ))
        .env_clear()
        .env("VIRTUAL_ENV", &venv_root);
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

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .expect("the Pillow font oracle status must be observable")
        {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(60) {
            child
                .kill()
                .expect("timed-out Pillow font oracle must be killable");
            child
                .wait()
                .expect("timed-out Pillow font oracle must be reaped");
            let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
            let _ = fs::remove_dir_all(&capture_root);
            panic!("Pillow font oracle timed out after 60 seconds:\n{stderr}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = fs::read(&stdout_path).expect("oracle stdout must be readable");
    let stderr = fs::read(&stderr_path).expect("oracle stderr must be readable");
    fs::remove_dir_all(&capture_root).expect("oracle capture directory must be removable");
    assert!(
        status.success(),
        "Pillow font oracle failed:\n{}",
        String::from_utf8_lossy(&stderr)
    );
    let payload: OraclePayload = serde_json::from_slice(&stdout)
        .expect("oracle output must be a strict identity/results payload");
    assert_eq!(payload.identity.pillow_version, "12.2.0");
    assert_eq!(payload.identity.freetype_version, "2.14.3");
    assert_eq!(
        Path::new(&payload.identity.python_executable)
            .canonicalize()
            .expect("oracle identity Python path must canonicalize"),
        oracle
            .canonicalize()
            .expect("configured oracle Python path must canonicalize")
    );
    let canonical_venv = venv_root
        .canonicalize()
        .expect("oracle venv must canonicalize");
    for identity_path in [
        &payload.identity.pillow_module,
        &payload.identity.native_core,
    ] {
        assert!(
            Path::new(identity_path)
                .canonicalize()
                .expect("oracle module identity path must canonicalize")
                .starts_with(&canonical_venv),
            "oracle module identity must remain beneath .oracle-venv: {identity_path}"
        );
    }

    let expected_ids = cases
        .iter()
        .map(|case| {
            case.get("case_id")
                .and_then(Value::as_str)
                .expect("input case_id must be a string")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let mut results = BTreeMap::new();
    for result in payload.results {
        result
            .validate()
            .unwrap_or_else(|error| panic!("oracle emitted malformed Result: {error}"));
        let case_id = result.case_id.clone();
        assert!(
            results.insert(case_id.clone(), result).is_none(),
            "oracle emitted duplicate Result for {case_id}"
        );
    }
    assert_result_ids_match(
        &expected_ids,
        &results.keys().cloned().collect::<BTreeSet<_>>(),
    );
    results
}

fn pillow_imagefont_public_methods() -> BTreeSet<String> {
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

fn pillow_imagefont_public_signatures() -> BTreeMap<String, BTreeSet<String>> {
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

fn pillow_imagefont_layout_members() -> BTreeSet<String> {
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
        .arg("--public-surface")
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
        .expect("the pinned Pillow font oracle public-surface query must finish");
    assert!(
        output.status.success(),
        "Pillow font oracle public-surface query failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let surface = serde_json::from_slice::<Value>(&output.stdout)
        .expect("oracle public surface output must be JSON");
    surface
        .get("layout")
        .and_then(Value::as_array)
        .expect("oracle public surface must include ImageFont.Layout members")
        .iter()
        .map(|member| {
            member
                .as_str()
                .expect("ImageFont.Layout member must be a string")
                .to_owned()
        })
        .collect()
}

fn pillow_imagefont_public_names() -> BTreeSet<String> {
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
        .arg("--public-surface")
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
        .expect("the pinned Pillow font oracle public-surface query must finish");
    assert!(
        output.status.success(),
        "Pillow font oracle public-surface query failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let surface = serde_json::from_slice::<Value>(&output.stdout)
        .expect("oracle public surface output must be JSON");
    surface
        .get("module_public_names")
        .and_then(Value::as_array)
        .expect("oracle public surface must include module_public_names")
        .iter()
        .map(|member| {
            member
                .as_str()
                .expect("ImageFont public module name must be a string")
                .to_owned()
        })
        .collect()
}

fn assert_pillow_imagefont_public_names_are_classified() {
    let live_names = pillow_imagefont_public_names();
    let classified = EXPECTED_IMAGEFONT_BEHAVIORAL_PUBLIC_NAMES
        .into_iter()
        .chain(EXPECTED_IMAGEFONT_NON_ENDPOINT_PUBLIC_NAMES)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        live_names, classified,
        "every live PIL.ImageFont public module name must be explicitly classified as a behavioral endpoint/class/enum or a non-endpoint import/constant/type"
    );
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
        if matches!(operation, "load" | "load_path") {
            if case
                .get("inputs")
                .and_then(|inputs| inputs.get("assets"))
                .and_then(|assets| assets.get("font"))
                .and_then(|font| font.get("path"))
                .is_some()
            {
                entry.insert("filename".to_owned());
            }
        }
        if operation == "truetype"
            && case
                .get("inputs")
                .and_then(|inputs| inputs.get("assets"))
                .and_then(|assets| assets.get("font"))
                .and_then(|font| font.get("path"))
                .is_some()
        {
            entry.insert("font".to_owned());
        }
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
        "loader" | "orientation" | "text_repeat" => return None,
        "size" if matches!(operation, "load_default" | "truetype") => "size",
        "size" => return None,
        "text" | "text_bytes_hex"
            if matches!(
                operation,
                "getbbox"
                    | "getlength"
                    | "getmask"
                    | "getmask2"
                    | "ImageFont.getbbox"
                    | "ImageFont.getlength"
                    | "ImageFont.getmask"
                    | "TransposedFont.getbbox"
                    | "TransposedFont.getlength"
                    | "TransposedFont.getmask"
            ) =>
        {
            "text"
        }
        "text" | "text_bytes_hex" => return None,
        "variant_size" if operation == "font_variant" => "size",
        "variant_index" if operation == "font_variant" => "index",
        "variant_encoding" if operation == "font_variant" => "encoding",
        "variant_layout_engine" if operation == "font_variant" => "layout_engine",
        "name_bytes_hex" if operation == "set_variation_by_name" => "name",
        "repeat_count" if operation == "set_variation_by_name" => return None,
        other => other,
    };
    Some(parameter.to_owned())
}

fn observed_public_parameter_values(
    cases: &[Value],
) -> BTreeMap<(String, String), BTreeSet<String>> {
    let required_keys = REQUIRED_PUBLIC_PARAMETER_VALUES
        .iter()
        .map(|&(operation, parameter, _)| (operation.to_owned(), parameter.to_owned()))
        .collect::<BTreeSet<_>>();
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

        for (required_operation, required_parameter) in &required_keys {
            if operation != required_operation {
                continue;
            }
            let value = match required_parameter.as_str() {
                "mode" => params
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PARAMETER_VALUE)
                    .to_owned(),
                "anchor" | "direction" | "language" => params
                    .get(required_parameter)
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PARAMETER_VALUE)
                    .to_owned(),
                "args" | "ink" | "kwargs" | "start" | "stroke_width" => params
                    .get(required_parameter)
                    .map(|value| {
                        serde_json::to_string(value)
                            .expect("font parameter value must serialize for coverage")
                    })
                    .unwrap_or_else(|| DEFAULT_PARAMETER_VALUE.to_owned()),
                "features" => params
                    .get("features")
                    .map(|value| {
                        serde_json::to_string(value)
                            .expect("font feature-list value must serialize for coverage")
                    })
                    .unwrap_or_else(|| DEFAULT_PARAMETER_VALUE.to_owned()),
                "layout_engine" if operation == "font_variant" => params
                    .get("variant_layout_engine")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PARAMETER_VALUE)
                    .to_owned(),
                "layout_engine" if operation == "truetype" => params
                    .get("layout_engine")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PARAMETER_VALUE)
                    .to_owned(),
                "encoding" if operation == "truetype" => params
                    .get("encoding")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PARAMETER_VALUE)
                    .to_owned(),
                "index" if operation == "truetype" => params
                    .get("index")
                    .map(|value| {
                        serde_json::to_string(value)
                            .expect("font index value must serialize for coverage")
                    })
                    .unwrap_or_else(|| DEFAULT_PARAMETER_VALUE.to_owned()),
                other => panic!("unsupported required public value-coverage parameter: {other}"),
            };
            observed
                .entry((required_operation.clone(), required_parameter.clone()))
                .or_insert_with(BTreeSet::new)
                .insert(value);
        }
    }

    observed
}

fn assert_manifest_covers_required_public_parameter_values(
    manifest: &FontManifest,
    cases: &[Value],
) {
    let observed_values = observed_public_parameter_values(cases);

    for (operation, coverage) in &manifest.public_method_parameters {
        for (parameter, required_values) in &coverage.required_values {
            assert!(
                coverage.covered.contains(parameter) && !required_values.is_empty(),
                "{operation}.{parameter}: manifest required values must be non-empty and classified as covered"
            );
        }
    }

    for &(operation, parameter, required_value) in REQUIRED_PUBLIC_PARAMETER_VALUES {
        let coverage = manifest
            .public_method_parameters
            .get(operation)
            .unwrap_or_else(|| panic!("{operation}: missing public_method_parameters entry"));
        assert!(
            coverage.covered.contains(parameter),
            "{operation}.{parameter}: required value coverage is only valid for parameters marked covered in font_manifest.yaml"
        );

        let key = (operation.to_owned(), parameter.to_owned());
        let observed = observed_values.get(&key).cloned().unwrap_or_default();
        assert!(
            observed.contains(required_value),
            "{operation}.{parameter}: font_manifest.yaml marks parameter covered but active input rows do not exercise required value `{required_value}`; observed values: {observed:?}"
        );
    }
}

fn assert_documented_blocked_public_parameters() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let status_path = repo_root.join("docs/font-parity-status.md");
    let status = fs::read_to_string(&status_path)
        .expect("ImageFont parity status document must be readable");

    assert!(
        status.contains("pure-Rust FreeType stroker"),
        "{} must document the implementation dependency for currently blocked ImageFont public parameters",
        status_path.display()
    );

    let documented = documented_current_blocked_public_parameters(&status, &status_path);
    let expected = EXPECTED_BLOCKED_PUBLIC_PARAMETERS
        .into_iter()
        .map(|(method, parameter)| format!("{method}.{parameter}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        documented,
        expected,
        "{} Current blocked public parameters section must exactly match the pinned ImageFont manifest blocker allow-list",
        status_path.display()
    );
}

fn assert_gap_analysis_tracks_stroke_filled_status() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let analysis_path = repo_root.join("docs/imagefont-parity-gap-analysis.md");
    let analysis = fs::read_to_string(&analysis_path)
        .expect("ImageFont parity gap analysis document must be readable");

    assert!(
        !analysis
            .contains("`stroke_filled=true` is wired but not proven by successful fixture rows"),
        "{} must not regress to the pre-outside-border blocker status for stroke_filled=true",
        analysis_path.display()
    );
    assert!(
        analysis.contains("font.getmask2.dejavusans24_a_stroke_1_5_filled_l"),
        "{} must name the live Pillow oracle row that proves the maintained stroke_filled=true route",
        analysis_path.display()
    );
    assert!(
        analysis.contains("FT_Glyph_StrokeBorder.outside_border_success"),
        "{} must tie the Font stroke_filled=true row to the exact lower FreeType outside-border proof",
        analysis_path.display()
    );
}

fn assert_blocked_public_parameters_have_active_dependency_blockers() {
    let interface_map_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fontdone/tests/data/interface_map.json");
    let interface_map_text = fs::read_to_string(&interface_map_path).unwrap_or_else(|err| {
        panic!(
            "{} must be readable to justify ImageFont stroke_width blockers: {err}",
            interface_map_path.display()
        )
    });
    let interface_map: Value = serde_json::from_str(&interface_map_text).unwrap_or_else(|err| {
        panic!(
            "{} must be valid JSON to justify ImageFont stroke_width blockers: {err}",
            interface_map_path.display()
        )
    });

    for symbol in EXPECTED_PARTIAL_STROKER_SYMBOLS {
        let entry = freetype_interface_symbol(&interface_map, symbol).unwrap_or_else(|| {
            panic!(
                "{} must classify {symbol}; Pillow ImageFont stroke rendering depends on the lower FreeType stroker path",
                interface_map_path.display()
            )
        });
        assert_eq!(
            entry.get("status").and_then(Value::as_str),
            Some("partial"),
            "{symbol} must stay marked partial until its maintained and pending stroker rows all have exact parity"
        );
        assert!(
            entry.get("rust").is_some_and(|rust| !rust.is_null()),
            "{symbol} must name its Rust endpoint while it is a partial ImageFont stroke dependency"
        );
    }

    let glyph_stroke = freetype_interface_symbol(&interface_map, "FT_Glyph_Stroke")
        .unwrap_or_else(|| {
            panic!(
                "{} must classify FT_Glyph_Stroke; Pillow ImageFont stroke_width uses it through _imagingft.c",
                interface_map_path.display()
            )
        });
    assert_eq!(
        glyph_stroke.get("status").and_then(Value::as_str),
        Some("partial"),
        "FT_Glyph_Stroke must stay partial until every lower-level glyph-stroke success case passes"
    );
    assert!(
        glyph_stroke.get("rust").is_some_and(|rust| !rust.is_null()),
        "FT_Glyph_Stroke must name its partial Rust endpoint while ImageFont stroke parity is incomplete"
    );

    let stroke_border = freetype_interface_symbol(&interface_map, "FT_Glyph_StrokeBorder")
        .unwrap_or_else(|| {
            panic!(
                "{} must classify FT_Glyph_StrokeBorder; Pillow ImageFont stroke_width uses it through _imagingft.c",
                interface_map_path.display()
            )
    });
    assert_eq!(
        stroke_border.get("status").and_then(Value::as_str),
        Some("partial"),
        "FT_Glyph_StrokeBorder must stay marked partial while broader stroke-border geometry remains guarded"
    );
    assert!(
        stroke_border
            .get("rust")
            .is_some_and(|rust| !rust.is_null()),
        "FT_Glyph_StrokeBorder must name its partial Rust endpoint while ImageFont stroke_filled parity is incomplete"
    );

    assert_freetype_stroke_fixture_has_success_case(
        "ftstroke.FT_Glyph_Stroke.json",
        "ftstroke.FT_Glyph_Stroke",
    );
    assert_freetype_stroke_fixture_has_success_case(
        "ftstroke.FT_Glyph_StrokeBorder.json",
        "ftstroke.FT_Glyph_StrokeBorder",
    );
    assert_freetype_stroke_blocking_cases_are_exact();
}

fn freetype_interface_symbol<'a>(interface_map: &'a Value, symbol: &str) -> Option<&'a Value> {
    interface_map
        .get("paths")?
        .as_array()?
        .iter()
        .find_map(|group| group.get("symbols")?.get(symbol))
}

fn assert_freetype_stroke_fixture_has_success_case(file_name: &str, subject: &str) {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fontdone/tests/fixtures/inputs/public-api")
        .join(file_name);
    let fixture_text = fs::read_to_string(&fixture_path).unwrap_or_else(|err| {
        panic!(
            "{} must be readable to prove {subject} has a lower-level parity fixture: {err}",
            fixture_path.display()
        )
    });
    let fixture: Value = serde_json::from_str(&fixture_text).unwrap_or_else(|err| {
        panic!(
            "{} must be valid JSON to prove {subject} has a lower-level parity fixture: {err}",
            fixture_path.display()
        )
    });
    let has_success_case = fixture
        .get("cases")
        .and_then(Value::as_array)
        .is_some_and(|cases| {
            cases.iter().any(|case| {
                case.get("subject").and_then(Value::as_str) == Some(subject)
                    && case.get("expect_error").and_then(Value::as_bool) == Some(false)
            })
        });
    assert!(
        has_success_case,
        "{} must retain at least one success fixture for {subject}; otherwise ImageFont stroke_width is blocked without a lower-level parity target",
        fixture_path.display()
    );
}

fn assert_freetype_stroke_blocking_cases_are_exact() {
    let observed = [
        "ftstroke.FT_Glyph_Stroke.json",
        "ftstroke.FT_Glyph_StrokeBorder.json",
    ]
    .into_iter()
    .flat_map(freetype_stroke_success_case_ids)
    .collect::<BTreeSet<_>>();
    let expected = EXPECTED_FREETYPE_STROKE_BLOCKING_CASES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, expected,
        "ImageFont stroke_width blockers must stay tied to the exact lower-level FreeType glyph-stroke success rows that currently need implementation"
    );
}

fn freetype_stroke_success_case_ids(file_name: &str) -> BTreeSet<String> {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fontdone/tests/fixtures/inputs/public-api")
        .join(file_name);
    let fixture_text = fs::read_to_string(&fixture_path).unwrap_or_else(|err| {
        panic!(
            "{} must be readable to classify lower-level ImageFont stroke blockers: {err}",
            fixture_path.display()
        )
    });
    let fixture: Value = serde_json::from_str(&fixture_text).unwrap_or_else(|err| {
        panic!(
            "{} must be valid JSON to classify lower-level ImageFont stroke blockers: {err}",
            fixture_path.display()
        )
    });
    fixture
        .get("cases")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} must contain cases", fixture_path.display()))
        .iter()
        .filter(|case| case.get("expect_error").and_then(Value::as_bool) == Some(false))
        .map(|case| {
            case.get("case_id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "{} contains a non-error stroke case without case_id",
                        fixture_path.display()
                    )
                })
                .to_owned()
        })
        .filter(|case_id| {
            !matches!(
                case_id.as_str(),
                "ftstroke.FT_Glyph_Stroke.outline_glyph_stroked_success"
                    | "ftstroke.FT_Glyph_Stroke.destroy_original_option"
                    | "ftstroke.FT_Glyph_StrokeBorder.outside_border_success"
                    | "ftstroke.FT_Glyph_StrokeBorder.inside_border_success"
                    | "ftstroke.FT_Glyph_StrokeBorder.destroy_original_option"
            )
        })
        .collect()
}

fn documented_current_blocked_public_parameters(
    status: &str,
    status_path: &Path,
) -> BTreeSet<String> {
    let mut in_section = false;
    let mut documented = BTreeSet::new();

    for line in status.lines() {
        let trimmed = line.trim();
        if trimmed == "Current blocked public parameters:" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.is_empty() {
            if documented.is_empty() {
                continue;
            }
            break;
        }
        if !trimmed.starts_with("- `") {
            break;
        }
        let value = trimmed
            .strip_prefix("- `")
            .and_then(|value| value.strip_suffix('`'))
            .unwrap_or_else(|| {
                panic!(
                    "{} has malformed Current blocked public parameters row: {trimmed}",
                    status_path.display()
                )
            });
        documented.insert(value.to_owned());
    }

    documented
}

fn assert_manifest_covers_pillow_public_signatures(
    manifest: &FontManifest,
    cases: &[Value],
    pillow_signatures: &BTreeMap<String, BTreeSet<String>>,
) {
    let observed_parameters = observed_public_method_parameters(cases);
    let expected_blocked = EXPECTED_BLOCKED_PUBLIC_PARAMETERS
        .into_iter()
        .map(|(method, parameter)| (method.to_owned(), parameter.to_owned()))
        .collect::<BTreeSet<_>>();
    let actual_blocked = manifest
        .public_method_parameters
        .iter()
        .flat_map(|(method, coverage)| {
            coverage
                .blocked
                .iter()
                .map(|parameter| (method.clone(), parameter.clone()))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_blocked, expected_blocked,
        "font_manifest.yaml must not hide public ImageFont parity gaps; update EXPECTED_BLOCKED_PUBLIC_PARAMETERS only with a documented implementation blocker"
    );
    assert_eq!(
        manifest
            .public_method_parameters
            .keys()
            .collect::<BTreeSet<_>>(),
        pillow_signatures.keys().collect::<BTreeSet<_>>(),
        "font_manifest.yaml public_method_parameters must enumerate every live Pillow ImageFont public method exactly"
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

fn assert_gap_analysis_live_corpus_matches_inputs(input_dir: &Path, manifest: &FontManifest) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let analysis_path = repo_root.join("docs/imagefont-parity-gap-analysis.md");
    let analysis = fs::read_to_string(&analysis_path)
        .expect("ImageFont parity gap analysis document must be readable");

    let (documented_counts, documented_total) =
        documented_live_corpus_counts(&analysis, &analysis_path);
    let actual_counts = actual_live_corpus_counts(input_dir, manifest);
    let actual_total = actual_counts.values().sum::<usize>();

    assert_eq!(
        documented_counts,
        actual_counts,
        "{} Live fixture corpus table must exactly match manifest input JSON case counts",
        analysis_path.display()
    );
    assert_eq!(
        documented_total,
        actual_total,
        "{} Live fixture corpus total must match active input JSON case count",
        analysis_path.display()
    );
}

fn documented_live_corpus_counts(
    analysis: &str,
    analysis_path: &Path,
) -> (BTreeMap<String, usize>, usize) {
    let mut in_section = false;
    let mut counts = BTreeMap::new();
    let mut total = None;

    for line in analysis.lines() {
        let trimmed = line.trim();
        if trimmed == "## Live fixture corpus" {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with("## ") {
            break;
        }
        if !in_section || !trimmed.starts_with('|') {
            continue;
        }
        if trimmed.starts_with("|---") || trimmed.starts_with("| Input file ") {
            continue;
        }
        if let Some(file_name) = markdown_backtick_value(trimmed) {
            let case_count = markdown_table_second_cell_usize(trimmed, analysis_path);
            counts.insert(file_name, case_count);
        } else if trimmed.starts_with("| total |") {
            total = Some(markdown_table_second_cell_usize(trimmed, analysis_path));
        }
    }

    assert!(
        !counts.is_empty(),
        "{} must document the active Font input file counts under Live fixture corpus",
        analysis_path.display()
    );
    let total = total.unwrap_or_else(|| {
        panic!(
            "{} must document the active Font input total under Live fixture corpus",
            analysis_path.display()
        )
    });
    (counts, total)
}

fn markdown_backtick_value(line: &str) -> Option<String> {
    let start = line.find('`')?;
    let after_start = &line[start + 1..];
    let end = after_start.find('`')?;
    Some(after_start[..end].to_owned())
}

fn markdown_table_second_cell_usize(line: &str, path: &Path) -> usize {
    line.split('|')
        .nth(2)
        .unwrap_or_else(|| {
            panic!(
                "{} has malformed markdown table row: {line}",
                path.display()
            )
        })
        .trim()
        .parse::<usize>()
        .unwrap_or_else(|error| {
            panic!(
                "{} has non-numeric Live fixture corpus count in row `{line}`: {error}",
                path.display()
            )
        })
}

fn actual_live_corpus_counts(input_dir: &Path, manifest: &FontManifest) -> BTreeMap<String, usize> {
    manifest
        .input_files
        .iter()
        .map(|file_name| {
            let path = input_dir.join(file_name);
            let document: Value = serde_json::from_slice(
                &fs::read(&path).expect("font public-api input must be readable"),
            )
            .expect("font public-api input must be valid JSON");
            let count = document
                .get("cases")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{} must contain a cases array", path.display()))
                .len();
            (file_name.clone(), count)
        })
        .collect()
}

fn assert_exact_oracle_match(
    case_id: &str,
    source_system: &str,
    target_system: &str,
    expected: &ResultEnvelope,
    actual: &ResultEnvelope,
    output_shape: OutputShape,
) {
    let diffs = compare_results(source_system, target_system, expected, actual, output_shape)
        .unwrap_or_else(|error| panic!("{case_id}: invalid comparison: {error}"));
    assert!(
        diffs.is_empty(),
        "{case_id}: Rust result differs from live Pillow ImageFont oracle: {diffs:#?}"
    );
}

fn assert_raqm_rows_use_dedicated_core_error(cases: &[Value], fixture_root: &Path) {
    let mut checked = 0usize;
    for case in cases {
        let operation = font_runner::operation(case).expect("case operation must be valid");
        if !matches!(operation, "getbbox" | "getlength" | "getmask" | "getmask2") {
            continue;
        }
        let Some(params) = case
            .get("inputs")
            .and_then(|inputs| inputs.get("params"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        if !(params.contains_key("direction")
            || params.contains_key("features")
            || params.contains_key("language"))
        {
            continue;
        }

        let case_id = case["case_id"].as_str().expect("case_id must be a string");
        let core_kind = font_runner::core_error_kind(case, fixture_root)
            .unwrap_or_else(|| panic!("{case_id}: RAQM row must fail in Rust core"));
        assert_eq!(
            core_kind, "UnsupportedLibraqm",
            "{case_id}: direction/features/language must be hard-coded to the dedicated PilError::UnsupportedLibraqm variant before Pillow-visible KeyError mapping"
        );
        checked += 1;
    }
    assert!(
        checked >= 12,
        "font public-api corpus must keep explicit no-libraqm rows for getbbox/getlength/getmask/getmask2"
    );
}

fn assert_stroke_filled_rows_do_not_fake_branch_coverage(cases: &[Value]) {
    let allowed_rows = BTreeSet::from([String::from(
        "font.getmask2.dejavusans24_a_stroke_1_5_filled_l",
    )]);
    let false_coverage_rows = cases
        .iter()
        .filter_map(|case| {
            let operation = font_runner::operation(case).ok()?;
            if operation != "getmask2" {
                return None;
            }
            let params = case.get("inputs").and_then(|inputs| inputs.get("params"))?;
            let stroke_filled = params
                .get("kwargs")
                .and_then(|kwargs| kwargs.get("stroke_filled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !stroke_filled {
                return None;
            }
            let stroke_width = params
                .get("stroke_width")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if stroke_width <= 0.0 {
                return None;
            }
            let case_id = case
                .get("case_id")
                .and_then(Value::as_str)
                .unwrap_or("<missing case_id>")
                .to_owned();
            (!allowed_rows.contains(&case_id)).then_some(case_id)
        })
        .collect::<Vec<_>>();

    assert!(
        false_coverage_rows.is_empty(),
        "active Font rows must not claim stroke_filled=true branch coverage unless the exact lower FT_Glyph_StrokeBorder success route is implemented; unapproved rows with stroke_width > 0: {false_coverage_rows:?}"
    );
}

fn assert_imagingft_has_no_coverage_or_oracle_shortcuts() {
    let imagingft =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/font/imagingft.rs"))
            .expect("imagingft source must be readable");
    let forbidden = [
        "#[coverage",
        "coverage(off)",
        "cfg(coverage",
        "cfg_attr(coverage",
        "#[cfg(test)]",
        "Command::new",
        "std::process",
        ".oracle-venv",
        "tests/fixtures",
    ];
    let present = forbidden
        .into_iter()
        .filter(|needle| imagingft.contains(needle))
        .collect::<Vec<_>>();
    assert!(
        present.is_empty(),
        "imagingft.rs must not use coverage exclusions, test cfg shortcuts, subprocess or fixture/oracle paths to fake Font parity or region coverage: {present:?}"
    );
}

fn assert_libraqm_error_contract_is_hard_coded() {
    assert_eq!(
        pillow_rs::PilError::UnsupportedLibraqm.to_string(),
        NO_LIBRAQM_MESSAGE,
        "PilError::UnsupportedLibraqm must keep Pillow's no-libraqm message hard-coded in core"
    );

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");

    let core_error =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/error.rs"))
            .expect("core error source must be readable");
    assert!(
        !core_error.contains("UnsupportedLibraqm("),
        "PilError::UnsupportedLibraqm must remain a unit variant; core must not attach ad-hoc libraqm text"
    );
    assert_eq!(
        core_error.matches(NO_LIBRAQM_MESSAGE).count(),
        1,
        "the no-libraqm message must be hard-coded exactly once in core error.rs"
    );

    let imagingft =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/font/imagingft.rs"))
            .expect("imagingft source must be readable");
    assert_eq!(
        imagingft.matches("PilError::unsupported_libraqm()").count(),
        1,
        "imagingft must route libraqm-dependent options through the dedicated PilError::UnsupportedLibraqm constructor exactly once"
    );
    assert!(
        !imagingft.contains("KeyError") && !imagingft.contains(NO_LIBRAQM_MESSAGE),
        "imagingft must not encode Pillow-visible KeyError text directly; bindings own public exception category mapping"
    );

    let py_binding = fs::read_to_string(repo_root.join("pillow-rs-py/src/lib.rs"))
        .expect("Python binding source must be readable");
    assert!(
        py_binding.contains("PilError::UnsupportedLibraqm")
            && py_binding.contains("pyo3::exceptions::PyKeyError::new_err"),
        "Python binding must expose PilError::UnsupportedLibraqm as Pillow-compatible KeyError"
    );
    assert!(
        py_binding.contains("text_with_options"),
        "Python ImageDraw binding must route text options through Rust core instead of bypassing PilError::UnsupportedLibraqm"
    );

    let py_imagedraw =
        fs::read_to_string(repo_root.join("pillow-rs-py/python/pillow_rs/imagedraw.py"))
            .expect("Python ImageDraw facade source must be readable");
    assert!(
        py_imagedraw.contains("direction")
            && py_imagedraw.contains("features")
            && py_imagedraw.contains("language")
            && py_imagedraw.contains("self._draw.text(")
            && py_imagedraw.contains("self._draw.textbbox(")
            && py_imagedraw.contains("self._draw.textlength("),
        "Python ImageDraw facade must pass libraqm-dependent text options into the Rust core binding"
    );

    let js_binding = fs::read_to_string(repo_root.join("pillow-rs-js/src/lib.rs"))
        .expect("JavaScript binding source must be readable");
    assert!(
        js_binding.contains("pillow_rs::PilError::UnsupportedLibraqm")
            && js_binding.contains("\"KeyError\""),
        "JavaScript binding must classify PilError::UnsupportedLibraqm as KeyError"
    );
}

fn assert_python_imagefont_facade_delegates_variation_apis() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root for font tests must be discoverable");
    let py_imagefont =
        fs::read_to_string(repo_root.join("pillow-rs-py/python/pillow_rs/imagefont.py"))
            .expect("Python ImageFont facade source must be readable");

    for expected in [
        "self._rust_font.get_variation_names()",
        "self._rust_font.set_variation_by_name(name)",
        "self._rust_font.get_variation_axes()",
        "self._rust_font.set_variation_by_axes(axes)",
    ] {
        assert!(
            py_imagefont.contains(expected),
            "Python ImageFont facade must delegate variation public API to Rust core: missing {expected}"
        );
    }
    assert!(
        !py_imagefont.contains("variable-font metadata and mutation are not implemented")
            && !py_imagefont.contains("def _require_variation_font"),
        "Python ImageFont facade must not replace Rust/Pillow variation behavior with local NotImplementedError gating"
    );
}

#[test]
fn migration_parity_schema_and_anti_cheat_guards_reject_invalid_inputs() {
    let forbidden = serde_json::json!({
        "case_id": "font.getname.forbidden",
        "operation": "getname",
        "inputs": {
            "assets": {},
            "params": {"expected": "answer"}
        }
    });
    assert!(
        std::panic::catch_unwind(|| {
            assert_input_only_case(Path::new("forbidden.json"), &forbidden);
        })
        .is_err(),
        "input-only validation must reject recursively embedded expectations"
    );

    let unknown_case_field = serde_json::json!({
        "case_id": "font.getname.unknown_field",
        "operation": "getname",
        "inputs": {"assets": {}, "params": {}},
        "extra": true
    });
    assert!(
        serde_json::from_value::<Case>(unknown_case_field).is_err(),
        "strict Case loading must reject unknown fields"
    );

    let duplicate = serde_json::json!({
        "case_id": "font.getname.duplicate",
        "operation": "getname",
        "inputs": {"assets": {}, "params": {}}
    });
    assert!(
        std::panic::catch_unwind(|| {
            assert_case_ids_are_unique(&[duplicate.clone(), duplicate.clone()]);
        })
        .is_err(),
        "duplicate Case IDs must fail"
    );

    let expected_ids = BTreeSet::from(["font.getname.missing_result".to_owned()]);
    assert!(
        std::panic::catch_unwind(|| {
            assert_result_ids_match(&expected_ids, &BTreeSet::new());
        })
        .is_err(),
        "missing Results must fail"
    );
}

#[test]
fn migration_parity_comparator_rejects_self_comparison_and_has_no_case_branches() {
    let result: ResultEnvelope = serde_json::from_value(serde_json::json!({
        "case_id": "font.getname.self_compare",
        "status": "ok",
        "value": {"type": "name", "value": ["Family", "Style"]}
    }))
    .expect("test Result must parse");
    assert!(
        compare_results("same", "same", &result, &result, OutputShape::Object).is_err(),
        "source/target self-comparison must be rejected"
    );

    let comparator_source = include_str!("support/migration_parity.rs");
    assert!(
        !comparator_source.contains("\"font."),
        "generic comparator must not contain Font case-ID branches"
    );
}

#[test]
fn migration_parity_manifest_and_runner_drift_is_rejected() {
    let root = fixture_root();
    let manifest_text =
        fs::read_to_string(root.join("manifest.yaml")).expect("manifest must be readable");
    let mut manifest_value: Value =
        serde_json::from_str(&manifest_text).expect("manifest must parse as JSON");
    manifest_value
        .as_object_mut()
        .expect("manifest must be an object")
        .insert("unexpected".into(), Value::Bool(true));
    assert!(
        serde_json::from_value::<MigrationManifest>(manifest_value).is_err(),
        "strict manifest loading must reject unknown fields"
    );

    let mut manifest = load_manifest(&root);
    assert!(manifest.required_operations.remove("getname"));
    assert!(
        std::panic::catch_unwind(|| {
            assert_manifest_operations_have_runner_arms(&manifest);
        })
        .is_err(),
        "manifest/registry drift must fail"
    );
}

#[test]
fn migration_manifest_accounts_for_every_deprecated_project_surface() {
    let root = fixture_root();
    let path = root.join("manifest.yaml");
    let text = fs::read_to_string(&path).expect("manifest must be readable");
    let manifest: MigrationManifest =
        serde_json::from_str(&text).expect("manifest must be strict JSON-compatible YAML");

    assert_project_surface_accounting(&path, &manifest);
}

#[test]
fn active_target_and_oracle_adapters_do_not_read_deprecated_evidence() {
    let target_runner = include_str!("support/font_runner.rs");
    let source_oracle = include_str!("../scripts/font_oracle.py");
    assert!(!target_runner.contains("tests/deprecated"));
    assert!(!source_oracle.contains("tests/deprecated"));
}

#[test]
fn every_input_matches_the_live_pillow_font_oracle_exactly() {
    let root = fixture_root();
    let manifest = load_manifest(&root);
    let input_dir = root.join(&manifest.input_dir);
    let asset_dir = root.join(&manifest.asset_dir);
    let cases = load_input_cases(&input_dir, &manifest);
    assert!(
        !cases.is_empty(),
        "font public-api input corpus must not be empty"
    );
    assert_case_ids_are_unique(&cases);
    assert_case_ids_match_operations(&cases);
    assert_referenced_assets_exist(&asset_dir, &cases);
    assert_manifest_covers_root_font_api(&manifest);
    assert_runner_exercises_root_font_api();
    assert_manifest_operations_have_runner_arms(&manifest);
    assert_python_imagefont_facade_delegates_variation_apis();

    let pillow_methods = pillow_imagefont_public_methods();
    assert_pillow_imagefont_public_names_are_classified();
    assert_manifest_operations_match_live_pillow_plus_repo_helpers(&manifest, &pillow_methods);
    let pillow_layout_members = pillow_imagefont_layout_members();
    let expected_layout_members = EXPECTED_IMAGEFONT_LAYOUT_MEMBERS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        pillow_layout_members, expected_layout_members,
        "live PIL.ImageFont.Layout members changed; update Font manifest coverage so only successful RAQM shaping remains out of scope"
    );
    let missing_pillow_methods = pillow_methods
        .iter()
        .filter(|operation| !manifest.required_operations.contains(operation.as_str()))
        .collect::<Vec<_>>();
    assert!(
        missing_pillow_methods.is_empty(),
        "font_manifest.yaml required_operations must include every live Pillow ImageFont public method: {missing_pillow_methods:?}"
    );
    let pillow_signatures = pillow_imagefont_public_signatures();
    assert_manifest_covers_pillow_public_signatures(&manifest, &cases, &pillow_signatures);
    assert_manifest_covers_required_public_parameter_values(&manifest, &cases);
    assert_documented_blocked_public_parameters();
    assert_gap_analysis_tracks_stroke_filled_status();
    assert_gap_analysis_live_corpus_matches_inputs(&input_dir, &manifest);
    assert_blocked_public_parameters_have_active_dependency_blockers();
    assert_imagingft_has_no_coverage_or_oracle_shortcuts();
    assert_libraqm_error_contract_is_hard_coded();
    assert_raqm_rows_use_dedicated_core_error(&cases, &asset_dir);
    assert_stroke_filled_rows_do_not_fake_branch_coverage(&cases);

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
        let actual: ResultEnvelope = serde_json::from_value(font_runner::run(case, &asset_dir))
            .unwrap_or_else(|error| panic!("{case_id}: target emitted malformed Result: {error}"));
        let operation = font_runner::operation(case).expect("case operation must be valid");
        let output_shape = *manifest
            .operation_shapes
            .get(operation)
            .unwrap_or_else(|| panic!("{case_id}: manifest output shape missing"));
        assert_exact_oracle_match(
            case_id,
            &manifest.source_name,
            &manifest.target_name,
            expected,
            &actual,
            output_shape,
        );
    }
}
