#!/usr/bin/env python3
"""Check public API manifest/input coverage and thin FFI ABI surfaces."""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests" / "manifest.yaml"
FIXTURE_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = ROOT / "tests" / "fixtures" / "inputs" / "public-api"
DEFAULT_AUDIT_JSON = ROOT / "target" / "api-abi-audit" / "api_abi_audit.json"
DEFAULT_ROUTE_AUDIT_JSON = ROOT / "target" / "api-abi-audit" / "route_audit.json"
DEFAULT_ROUTE_AUDIT_MD = ROOT / "target" / "api-abi-audit" / "route_audit.md"

API_SURFACE_EXCLUSIONS = {
    # Public header setup hooks, not user-callable API subjects.
    "fterrors.FT_NOERRORDEF_",
    "ftmac.FT_DEPRECATED_ATTRIBUTE",
    "ftmoderr.FT_MODERRDEF",
}

C_ABI_SOURCE = ROOT / "ffi-c" / "src" / "lib.rs"
C_ABI_HEADER = ROOT / "ffi-c" / "include" / "fontdone_ffi.h"
WASM_ABI_SOURCE = ROOT / "ffi-wasm" / "src" / "lib.rs"

WASM_EXPORTS = {
    "fontdone_wasm_malloc",
    "fontdone_wasm_free",
    "fontdone_wasm_open_face",
    "fontdone_wasm_done_face",
    "fontdone_wasm_new_size",
    "fontdone_wasm_new_size_out",
    "fontdone_wasm_activate_size",
    "fontdone_wasm_done_size",
    "fontdone_wasm_active_size",
    "fontdone_wasm_done_freetype",
    "fontdone_wasm_bitmap_blend",
    "fontdone_wasm_bitmap_convert",
    "fontdone_wasm_bitmap_copy",
    "fontdone_wasm_bitmap_done",
    "fontdone_wasm_bitmap_init",
    "fontdone_wasm_bitmap_new",
    "fontdone_wasm_glyphslot_own_bitmap",
    "fontdone_wasm_face_check_truetype_patents",
    "fontdone_wasm_face_set_unpatented_hinting",
    "fontdone_wasm_outline_get_cbox",
    "fontdone_wasm_outline_get_bitmap",
    "fontdone_wasm_outline_render",
    "fontdone_wasm_outline_get_orientation",
    "fontdone_wasm_outline_reverse",
    "fontdone_wasm_outline_transform",
    "fontdone_wasm_get_truetype_engine_type",
    "fontdone_wasm_library_set_lcd_filter",
    "fontdone_wasm_library_set_lcd_filter_weights",
    "fontdone_wasm_library_set_lcd_geometry",
    "fontdone_wasm_mul_div",
    "fontdone_wasm_mul_fix",
    "fontdone_wasm_div_fix",
    "fontdone_wasm_error_string",
    "fontdone_wasm_open_type_validate",
    "fontdone_wasm_open_type_free",
    "fontdone_wasm_round_fix",
    "fontdone_wasm_ceil_fix",
    "fontdone_wasm_floor_fix",
    "fontdone_wasm_sin",
    "fontdone_wasm_cos",
    "fontdone_wasm_tan",
    "fontdone_wasm_atan2",
    "fontdone_wasm_angle_diff",
    "fontdone_wasm_vector_unit",
    "fontdone_wasm_vector_rotate",
    "fontdone_wasm_vector_length",
    "fontdone_wasm_vector_polarize",
    "fontdone_wasm_vector_from_polar",
    "fontdone_wasm_vector_transform",
    "fontdone_wasm_matrix_multiply",
    "fontdone_wasm_matrix_invert",
    "fontdone_wasm_set_pixel_sizes",
    "fontdone_wasm_set_char_size",
    "fontdone_wasm_request_size",
    "fontdone_wasm_select_size",
    "fontdone_wasm_get_char_index",
    "fontdone_wasm_get_char_variant_index",
    "fontdone_wasm_get_char_variant_is_default",
    "fontdone_wasm_get_variant_selectors",
    "fontdone_wasm_get_variants_of_char",
    "fontdone_wasm_get_chars_of_variant",
    "fontdone_wasm_get_kerning",
    "fontdone_wasm_select_charmap",
    "fontdone_wasm_get_charmap_count",
    "fontdone_wasm_get_active_charmap_index",
    "fontdone_wasm_get_charmap",
    "fontdone_wasm_get_cmap_format",
    "fontdone_wasm_get_cmap_language_id",
    "fontdone_wasm_set_charmap",
    "fontdone_wasm_set_charmap_from_face",
    "fontdone_wasm_get_fstype_flags",
    "fontdone_wasm_get_gasp",
    "fontdone_wasm_get_glyph_name",
    "fontdone_wasm_get_name_index",
    "fontdone_wasm_get_postscript_name",
    "fontdone_wasm_get_font_format",
    "fontdone_wasm_get_x11_font_format",
    "fontdone_wasm_set_named_instance",
    "fontdone_wasm_get_default_named_instance",
    "fontdone_wasm_get_sfnt_name_count",
    "fontdone_wasm_get_sfnt_name",
    "fontdone_wasm_get_sfnt_os2",
    "fontdone_wasm_load_sfnt_table",
    "fontdone_wasm_sfnt_table_info",
    "fontdone_wasm_get_first_char",
    "fontdone_wasm_get_next_char",
    "fontdone_wasm_library_version",
    "fontdone_wasm_load_char",
    "fontdone_wasm_load_glyph",
    "fontdone_wasm_get_advance",
    "fontdone_wasm_get_advances",
    "fontdone_wasm_get_subglyph_info",
    "fontdone_wasm_render_glyph",
    "fontdone_wasm_bitmap_embolden",
    "fontdone_wasm_glyphslot_adjust_weight",
    "fontdone_wasm_glyphslot_embolden",
    "fontdone_wasm_glyphslot_oblique",
    "fontdone_wasm_glyphslot_slant",
    "fontdone_wasm_get_slot",
    "fontdone_wasm_size_metrics",
}

DISALLOWED_FFI_SNIPPETS = (
    "std::fs",
    "std::path",
    "std::process",
    "serde",
    "serde_json",
    "Command::",
    "File::",
    "read_to_",
    "read(",
    "write(",
    "fontdone_test",
)

COMPILE_CONTRACT_OPERATIONS = {
    "constant",
    "constant_map",
    "record_layout",
    "abi_type_probe",
    "abi_type_map_probe",
    "abi_function_probe",
    "abi.compile_alias_probe",
    "abi.value_echo",
    "macro_eval",
    "macro_compile_probe",
    "face_macro_flags",
    "freetype.vector_transform",
    "ftglyph.matrix_multiply",
    "ftglyph.matrix_invert",
}

REAL_PARITY_OPERATIONS = {
    "new_memory_face",
    "set_pixel_sizes",
    "set_char_size",
    "freetype.request_size",
    "freetype.select_size",
    "size_metrics",
    "get_char_index",
    "charmap.get_char_index",
    "freetype.select_charmap",
    "freetype.set_charmap",
    "freetype.inspect_charmaps",
    "freetype.charmap_ownership",
    "freetype.get_charmap_index",
    "freetype.face_flags",
    "freetype.get_fstype_flags",
    "freetype.get_kerning",
    "freetype.ceil_fix",
    "freetype.floor_fix",
    "freetype.round_fix",
    "freetype.mul_div",
    "freetype.mul_fix",
    "freetype.div_fix",
    "freetype.get_postscript_name",
    "freetype.face_get_char_variant_index",
    "freetype.face_get_char_variant_is_default",
    "freetype.face_get_variant_selectors",
    "freetype.face_get_variants_of_char",
    "freetype.face_get_chars_of_variant",
    "ftmm.set_named_instance",
    "ftmm.get_default_named_instance",
    "freetype.get_glyph_name",
    "freetype.get_name_index",
    "freetype.new_face",
    "freetype.set_transform",
    "freetype.get_transform",
    "freetype.reference_face",
    "ftsnames.get_sfnt_name_count",
    "ftsnames.get_sfnt_name",
    "ftsnames.get_sfnt_name_by_record",
    "ftsnames.get_sfnt_name_group",
    "ftsnames.get_sfnt_lang_tag",
    "sfnt.get_name",
    "sfnt.get_sfnt_name",
    "sfnt.get_os2_unicode_ranges",
    "sfnt.get_sfnt_table",
    "sfnt.get_sfnt_table.record",
    "sfnt.get_sfnt_table.head",
    "sfnt.get_sfnt_table.maxp",
    "sfnt.get_sfnt_table.hhea",
    "sfnt.get_sfnt_table.hhea.after_variation",
    "sfnt.load_sfnt_table",
    "sfnt.table_info",
    "sfnt.mac_encoding_record",
    "fterrors.error_string",
    "freetype.get_first_char",
    "freetype.get_next_char",
    "freetype.library_version",
    "ftmodapi.get_truetype_engine_type",
    "freetype.done_freetype",
    "freetype.done_face",
    "freetype.face_check_truetype_patents",
    "freetype.face_set_unpatented_hinting",
    "ftlcdfil.set_lcd_filter",
    "ftlcdfil.set_lcd_filter_weights",
    "ftlcdfil.set_lcd_geometry",
    "load_char",
    "load_glyph",
    "freetype.inspect_glyph_metrics",
    "freetype.inspect_glyph_slot",
    "freetype.get_subglyph_info",
    "freetype.load_glyph_outline",
    "ftbbox.outline_get_bbox",
    "ftimage.outline_decompose",
    "ftoutln.outline_decompose",
    "ftoutln.outline_get_cbox",
    "ftoutln.get_orientation",
    "ftoutln.outline_reverse",
    "ftoutln.outline_transform",
    "ftglyph.glyph_get_cbox",
    "ftglyph.glyph_to_bitmap",
    "ftglyph.get_glyph",
    "ftglyph.glyph_copy",
    "ftglyph.record_inspect",
    "ftbitmap.bitmap_blend",
    "ftbitmap.bitmap_convert",
    "ftbitmap.bitmap_copy",
    "ftbitmap.bitmap_done",
    "ftbitmap.bitmap_embolden",
    "ftbitmap.glyphslot_own_bitmap",
    "ftbitmap.bitmap_init",
    "ftbitmap.bitmap_new",
    "ftcache.sbit_cache_lookup",
    "ftcache.manager_reset",
    "ftoutln.outline_render",
    "ftoutln.outline_render_direct",
    "ftadvanc.get_advance",
    "ftadvanc.get_advances",
    "render_glyph",
    "ftsynth.glyphslot_slant_after_load",
    "ftsynth.glyphslot_oblique_after_load",
    "ftsynth.glyphslot_adjust_weight_after_load",
    "ftsynth.glyphslot_embolden_after_load",
    "ftgasp.get_gasp",
    "tttables.get_cmap_format",
    "tttables.get_cmap_language_id",
    "ftsizes.new_size_sequence",
    "ftsizes.done_size_sequence",
    "ftsizes.activate_size_sequence",
    "ftsizes.activate_select_size_sequence",
}

EXPLICIT_UNSUPPORTED_OPERATIONS = {
    "freetype.face_properties",
}

AUDIT_ONLY_PENDING_CORE_CASES = {
    "tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation",
}

SHARED_RUNTIME_FALLBACK_CASES = {
    # The unified runtime currently compares one shared Rust direct-span
    # fallback for all three actual lanes, not exported C/WASM public routes.
    "ftimage.FT_Raster_Span_Func.direct_render_emits_spans",
}

PLACEHOLDER_STYLE_CATEGORIES = {
    "generic-fallback",
    "generic-error-fallback",
    "null-error-fallback",
    "explicit-unsupported",
    "raw-slot-null-validation",
    "void-fallback",
    "wrapper-null-validation",
}

SUPPLEMENTARY_SAFE_API_FLAGS = {
    "assert_font_render_mode_agrees": "Font render helpers",
    "assert_font_truetype_constructor_agrees": "Font constructors",
    "assert_font_face_count_agrees": "Font face-count helper",
    "assert_font_empty_text_render": "Font render helpers",
    "assert_font_getmetrics_agrees": "Font convenience metrics",
    "assert_font_getlength_agrees": "Font convenience metrics",
    "assert_font_glyph_metrics_agrees": "Font convenience metrics",
    "assert_font_getmask_agrees": "Font convenience masks",
    "assert_font_empty_text_mask": "Font convenience masks",
    "assert_font_getkerning_agrees": "Font convenience kerning",
    "assert_font_hori_advance_agrees": "Font convenience advances",
    "assert_font_charmap_accessors_agree": "Font charmap convenience accessors",
}

NON_FREETYPE_GOAL_SURFACES = (
    "Pillow adapter methods in imagingft.rs model Pillow _imagingft.c rather "
    "than a public C FreeType API.",
    "High-level fontdone::Font convenience methods are supplementary unless "
    "routed through a public FreeType manifest row.",
    "Former local fontdone::ffi::handles inspection helpers were removed from "
    "the Rust facade; face metrics, face flags, active size, and charmaps now "
    "use public FT_Face-shaped fields, and SFNT/CMAP metadata routes through "
    "public FreeType-shaped functions.",
)

COMPILE_CONTRACT_PREFIXES = (
    "abi.",
    "c_compile.",
    "c_preprocessor.",
    "cxx_preprocessor.",
    "fttypes.",
)

REAL_PARITY_PREFIXES = (
    "freetype.face_macro",
    "fttrigon.",
)


@dataclass(frozen=True)
class ManifestSubject:
    subject_id: str
    kind: str
    symbol: str
    header: str
    cases: set[str]


@dataclass(frozen=True)
class ConcreteInput:
    subject: str
    case_id: str
    case: str
    operation: str
    variant_id: str | None
    expect_error: bool
    compare_error_output: bool
    allow_oracle_errors: bool
    expectation_status: str
    assets: dict[str, object]
    params: dict[str, object]


CASE_COLLISION_CARRIERS = {
    # FreeType still exposes these deprecated mixed-case aliases alongside the
    # modern macros.  Store each alias with its modern spelling so a checkout
    # remains representable on case-insensitive filesystems.
    "ftimage.FT_Curve_Tag_Conic": "ftimage.FT_CURVE_TAG_CONIC",
    "ftimage.FT_Curve_Tag_Cubic": "ftimage.FT_CURVE_TAG_CUBIC",
    "ftimage.FT_Curve_Tag_On": "ftimage.FT_CURVE_TAG_ON",
    "ftimage.FT_Curve_Tag_Touch_X": "ftimage.FT_CURVE_TAG_TOUCH_X",
    "ftimage.FT_Curve_Tag_Touch_Y": "ftimage.FT_CURVE_TAG_TOUCH_Y",
    # FT_Bool is the public typedef; FT_BOOL is the distinct conversion macro.
    "fttypes.FT_BOOL": "fttypes.FT_Bool",
}


def filename_for_subject(subject: str) -> str:
    carrier = CASE_COLLISION_CARRIERS.get(subject, subject)
    return re.sub(r"[^A-Za-z0-9_.-]", "_", carrier) + ".json"


def read_manifest() -> dict[str, ManifestSubject]:
    subjects: dict[str, dict[str, object]] = {}
    current: str | None = None
    in_cases = False
    for line in MANIFEST.read_text().splitlines():
        if line.startswith("  - id: "):
            current = line.split(": ", 1)[1].strip()
            if current in subjects:
                raise RuntimeError(f"duplicate manifest subject id {current}")
            subjects[current] = {
                "kind": "",
                "symbol": "",
                "header": "",
                "cases": set(),
            }
            in_cases = False
        elif line.startswith("    kind: "):
            require_current(current)
            subjects[current]["kind"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    symbol: "):
            require_current(current)
            subjects[current]["symbol"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    header: "):
            require_current(current)
            subjects[current]["header"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    cases:"):
            require_current(current)
            in_cases = True
        elif line.startswith("      - id: "):
            require_current(current)
            if not in_cases:
                raise RuntimeError(f"case outside cases block for {current}")
            case_id = line.split(": ", 1)[1].strip()
            cases = subjects[current]["cases"]
            assert isinstance(cases, set)
            if case_id in cases:
                raise RuntimeError(f"{current}: duplicate manifest case {case_id}")
            cases.add(case_id)

    result: dict[str, ManifestSubject] = {}
    seen_symbols: dict[tuple[str, str], str] = {}
    for subject_id, raw in subjects.items():
        cases = raw["cases"]
        assert isinstance(cases, set)
        subject = ManifestSubject(
            subject_id=subject_id,
            kind=str(raw["kind"]),
            symbol=str(raw["symbol"]),
            header=str(raw["header"]),
            cases=cases,
        )
        if not subject.kind:
            raise RuntimeError(f"{subject_id}: missing kind")
        if not subject.symbol:
            raise RuntimeError(f"{subject_id}: missing symbol")
        if not subject.header:
            raise RuntimeError(f"{subject_id}: missing header")
        if not subject.cases:
            raise RuntimeError(f"{subject_id}: missing cases")
        key = (subject.header, subject.symbol)
        if key in seen_symbols:
            raise RuntimeError(
                f"{subject_id}: duplicates symbol {subject.symbol} in {seen_symbols[key]}"
            )
        seen_symbols[key] = subject_id
        result[subject_id] = subject
    return result


def require_current(current: str | None) -> None:
    if current is None:
        raise RuntimeError("manifest field before subject")


def check_explicit_inputs(subject_id: str, index: int, inputs: object) -> list[str]:
    prefix = f"{subject_id}: cases[{index}]"
    if not isinstance(inputs, dict):
        return [f"{prefix} inputs must be object"]
    errors: list[str] = []
    if "variability" in inputs:
        errors.append(f"{prefix} uses forbidden implicit variability")
    assets = inputs.get("assets", {})
    if isinstance(assets, dict) and "font_folder" in assets:
        errors.append(f"{prefix} uses forbidden runtime font-folder discovery")
    variants = inputs.get("variants", [])
    if not variants:
        return errors
    if not isinstance(variants, list):
        errors.append(f"{prefix} variants must be list")
        return errors
    if assets or inputs.get("params") not in (None, {}):
        errors.append(f"{prefix} mixes variants with direct assets or params")
    seen: set[str] = set()
    for variant_index, variant in enumerate(variants):
        variant_prefix = f"{prefix} variants[{variant_index}]"
        if not isinstance(variant, dict):
            errors.append(f"{variant_prefix} must be object")
            continue
        variant_id = variant.get("id")
        if not isinstance(variant_id, str) or not variant_id:
            errors.append(f"{variant_prefix} needs a non-empty id")
        elif variant_id in seen:
            errors.append(f"{variant_prefix} duplicates id {variant_id}")
        else:
            seen.add(variant_id)
        coverage = variant.get("coverage")
        if not isinstance(coverage, list) or not coverage or not all(
            isinstance(item, str) and item for item in coverage
        ):
            errors.append(f"{variant_prefix} needs non-empty coverage intent strings")
        variant_assets = variant.get("assets", {})
        if isinstance(variant_assets, dict) and "font_folder" in variant_assets:
            errors.append(f"{variant_prefix} uses forbidden runtime font-folder discovery")
    return errors


def check_file(subject: ManifestSubject) -> list[str]:
    errors: list[str] = []
    path = INPUT_DIR / filename_for_subject(subject.subject_id)
    if not path.exists():
        return [f"{subject.subject_id}: missing {path.relative_to(ROOT)}"]

    try:
        data = json.loads(path.read_text())
    except Exception as exc:  # noqa: BLE001 - report parse failure with path.
        return [f"{subject.subject_id}: invalid json: {exc}"]

    if data.get("version") != 1:
        errors.append(f"{subject.subject_id}: version must be 1")
    declared_subjects = data.get("subjects")
    if declared_subjects is None:
        declared_subjects = [data.get("subject")]
    if not isinstance(declared_subjects, list) or not all(
        isinstance(item, str) for item in declared_subjects
    ):
        errors.append(f"{subject.subject_id}: top-level subjects must be a string list")
        declared_subjects = []
    elif subject.subject_id not in declared_subjects:
        errors.append(f"{subject.subject_id}: top-level subject mismatch")
    manifest_cases_field = data.get("manifest_cases")
    if manifest_cases_field is not None:
        if not isinstance(manifest_cases_field, list):
            errors.append(f"{subject.subject_id}: manifest_cases must be list")
        else:
            unknown = sorted(set(manifest_cases_field) - subject.cases)
            if unknown:
                errors.append(f"{subject.subject_id}: manifest_cases contains unknown cases {unknown}")

    all_cases = data.get("cases")
    if not isinstance(all_cases, list) or not all_cases:
        errors.append(f"{subject.subject_id}: cases must be a non-empty list")
        return errors
    for index, case in enumerate(all_cases):
        if not isinstance(case, dict):
            errors.append(f"{subject.subject_id}: cases[{index}] must be object")
        elif case.get("subject") not in declared_subjects:
            errors.append(f"{subject.subject_id}: cases[{index}] has undeclared subject")
    cases = [
        case
        for case in all_cases
        if isinstance(case, dict) and case.get("subject") == subject.subject_id
    ]
    if not cases:
        errors.append(f"{subject.subject_id}: cases must cover the subject")
        return errors

    covered: set[str] = set()
    for index, case in enumerate(cases):
        manifest_case = case.get("case")
        covers_manifest_cases = case.get("covers_manifest_cases", [])
        if not isinstance(covers_manifest_cases, list):
            errors.append(f"{subject.subject_id}: cases[{index}] covers_manifest_cases must be list")
            covers_manifest_cases = []

        if not isinstance(manifest_case, str):
            errors.append(f"{subject.subject_id}: cases[{index}] missing case")
        elif manifest_case not in subject.cases:
            if not covers_manifest_cases:
                errors.append(
                    f"{subject.subject_id}: cases[{index}] unknown manifest case {manifest_case} "
                    "without covers_manifest_cases"
                )
        else:
            covered.add(manifest_case)
        for covered_case in covers_manifest_cases:
            if not isinstance(covered_case, str) or covered_case not in subject.cases:
                errors.append(
                    f"{subject.subject_id}: cases[{index}] invalid covered manifest case {covered_case}"
                )
            else:
                covered.add(covered_case)
        for key in ("case_id", "operation", "schema", "inputs", "expectation"):
            if key not in case:
                errors.append(f"{subject.subject_id}: cases[{index}] missing {key}")
        errors.extend(check_explicit_inputs(subject.subject_id, index, case.get("inputs")))
        if case.get("schema") == "scalar":
            errors.append(f"{subject.subject_id}: cases[{index}] uses legacy scalar schema")
        expectation = case.get("expectation")
        if isinstance(expectation, dict):
            if "output_shape" not in expectation:
                errors.append(f"{subject.subject_id}: cases[{index}] expectation missing output_shape")
            if "compare" not in expectation:
                errors.append(f"{subject.subject_id}: cases[{index}] expectation missing compare")
        else:
            errors.append(f"{subject.subject_id}: cases[{index}] expectation must be object")

    missing = sorted(subject.cases - covered)
    if missing:
        errors.append(f"{subject.subject_id}: missing manifest cases {missing}")
    return errors


def public_surface_subjects(audit_json: Path) -> dict[str, tuple[str, str]]:
    data = json.loads(audit_json.read_text())
    expected: dict[str, tuple[str, str]] = {}
    for bucket, key in (
        ("functions", "symbol"),
        ("types", "type"),
        ("constants", "constant"),
        ("enum_variants", "constant"),
        ("error_codes", "constant"),
    ):
        for row in data[bucket]:
            subject = subject_for_c_symbol(row["c_file"], row[key])
            if subject not in API_SURFACE_EXCLUSIONS:
                expected[subject] = (row["c_file"], row[key])
    return expected


def subject_for_c_symbol(c_file: str, symbol: str) -> str:
    return f"{Path(c_file).stem}.{symbol}"


def check_manifest_public_surface(
    subjects: dict[str, ManifestSubject],
    audit_json: Path,
) -> list[str]:
    errors: list[str] = []
    expected = public_surface_subjects(audit_json)
    manifest_ids = set(subjects)
    expected_ids = set(expected)
    for subject in sorted(expected_ids - manifest_ids):
        c_file, symbol = expected[subject]
        errors.append(f"manifest missing public C subject {subject} ({symbol} in {c_file})")
    for subject in sorted(manifest_ids - expected_ids):
        errors.append(f"manifest has non-public or duplicate-derived subject {subject}")
    for subject_id in sorted(manifest_ids & expected_ids):
        c_file, symbol = expected[subject_id]
        subject = subjects[subject_id]
        if subject.header != c_file:
            errors.append(f"{subject_id}: header mismatch manifest={subject.header} audit={c_file}")
        if subject.symbol != symbol:
            errors.append(f"{subject_id}: symbol mismatch manifest={subject.symbol} audit={symbol}")
    return errors


def check_input_directory(subjects: dict[str, ManifestSubject]) -> list[str]:
    errors: list[str] = []
    expected_paths = {
        INPUT_DIR / filename_for_subject(subject_id)
        for subject_id in subjects
    }
    actual_paths = set(INPUT_DIR.rglob("*.json"))
    for path in sorted(expected_paths - actual_paths):
        errors.append(f"missing public API input file {path.relative_to(ROOT)}")
    for path in sorted(actual_paths - expected_paths):
        errors.append(f"extra public API input file {path.relative_to(ROOT)}")
    return errors


def check_ffi_abi_surface(audit_json: Path) -> list[str]:
    errors: list[str] = []
    audit = json.loads(audit_json.read_text())
    public_functions = {row["symbol"] for row in audit["functions"]}

    c_exports = exported_functions(C_ABI_SOURCE)
    for export in sorted(c_exports):
        if export not in public_functions:
            errors.append(f"ffi-c exports non-FreeType C symbol {export}")
    header_text = C_ABI_HEADER.read_text()
    if "fontdone_" in header_text:
        errors.append("ffi-c public header exposes fontdone_* helper symbols")
    for export in sorted(c_exports):
        if not re.search(r"\b" + re.escape(export) + r"\s*\(", header_text):
            errors.append(f"ffi-c header missing exported symbol {export}")

    wasm_exports = exported_functions(WASM_ABI_SOURCE)
    extra_wasm = wasm_exports - WASM_EXPORTS
    missing_wasm = WASM_EXPORTS - wasm_exports
    for export in sorted(extra_wasm):
        errors.append(f"ffi-wasm exports unexpected symbol {export}")
    for export in sorted(missing_wasm):
        errors.append(f"ffi-wasm missing expected symbol {export}")

    for path in (C_ABI_SOURCE, WASM_ABI_SOURCE):
        text = path.read_text()
        for snippet in DISALLOWED_FFI_SNIPPETS:
            if snippet in text:
                errors.append(f"{path.relative_to(ROOT)} contains disallowed wrapper logic {snippet}")
    return errors


def exported_functions(path: Path) -> set[str]:
    text = path.read_text()
    pattern = r"#\[unsafe\(no_mangle\)\]\s*pub\s+extern\s+\"C\"\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)"
    return set(re.findall(pattern, text))


def concrete_inputs(items: dict[str, ManifestSubject]) -> list[ConcreteInput]:
    rows: list[ConcreteInput] = []
    for subject_id in sorted(items):
        path = INPUT_DIR / filename_for_subject(subject_id)
        data = json.loads(path.read_text())
        for case in data.get("cases", []):
            if case.get("subject") != subject_id:
                continue
            expectation = object_dict(case.get("expectation", {}))
            compare = object_dict(expectation.get("compare", {}))
            expectation_status = str(expectation.get("status", ""))
            inputs = case.get("inputs", {})
            if not isinstance(inputs, dict):
                continue
            variants = inputs.get("variants")
            if isinstance(variants, list) and variants:
                for variant in variants:
                    if not isinstance(variant, dict):
                        continue
                    variant_expectation = object_dict(variant.get("expectation", {}))
                    variant_compare = object_dict(variant_expectation.get("compare", {}))
                    rows.append(
                        ConcreteInput(
                            subject=subject_id,
                            case_id=str(case.get("case_id", "")),
                            case=str(case.get("case", "")),
                            operation=str(case.get("operation", "")),
                            variant_id=str(variant.get("id", "")) or None,
                            expect_error=bool(variant.get("expect_error", case.get("expect_error", False))),
                            compare_error_output=bool(
                                variant_compare.get(
                                    "compare_error_output",
                                    compare.get("compare_error_output", False),
                                )
                            )
                            or exact_error_public_family(
                                str(case.get("operation", "")),
                                bool(variant.get("expect_error", case.get("expect_error", False))),
                            ),
                            allow_oracle_errors=bool(
                                variant_compare.get(
                                    "allow_oracle_errors",
                                    compare.get("allow_oracle_errors", False),
                                )
                            ),
                            expectation_status=expectation_status,
                            assets=object_dict(variant.get("assets", {})),
                            params=object_dict(variant.get("params", {})),
                        )
                    )
            else:
                rows.append(
                    ConcreteInput(
                        subject=subject_id,
                        case_id=str(case.get("case_id", "")),
                        case=str(case.get("case", "")),
                        operation=str(case.get("operation", "")),
                        variant_id=None,
                        expect_error=bool(case.get("expect_error", False)),
                        compare_error_output=bool(compare.get("compare_error_output", False))
                        or exact_error_public_family(
                            str(case.get("operation", "")),
                            bool(case.get("expect_error", False)),
                        ),
                        allow_oracle_errors=bool(compare.get("allow_oracle_errors", False)),
                        expectation_status=expectation_status,
                        assets=object_dict(inputs.get("assets", {})),
                        params=object_dict(inputs.get("params", {})),
                    )
                )
    return rows


def object_dict(value: object) -> dict[str, object]:
    return value if isinstance(value, dict) else {}


def exact_error_public_family(operation: str, expect_error: bool) -> bool:
    """Require exact status and output for public error routes with dedicated runners."""
    return expect_error and (
        operation.startswith("ftsizes.")
        or operation in {"freetype.get_kerning", "freetype.get_subglyph_info"}
    )


def operation_is_compile_contract(operation: str) -> bool:
    return operation in COMPILE_CONTRACT_OPERATIONS or operation.startswith(
        COMPILE_CONTRACT_PREFIXES
    )


def operation_is_real_parity(operation: str) -> bool:
    return operation in REAL_PARITY_OPERATIONS or operation.startswith(REAL_PARITY_PREFIXES)


def has_runtime_asset(row: ConcreteInput) -> bool:
    return any(key in row.assets for key in ("font", "fixture", "foreign_font"))


def unresolved_asset_reason(value: object, label: str) -> str | None:
    if isinstance(value, list):
        for index, item in enumerate(value):
            reason = unresolved_asset_reason(item, f"{label}[{index}]")
            if reason:
                return reason
        return None
    if not isinstance(value, dict):
        return None
    if value.get("status") == "required_future_asset":
        return f"{label} is marked required_future_asset"
    for key, item in value.items():
        if key not in {"id", "path"}:
            reason = unresolved_asset_reason(item, f"{label}.{key}")
            if reason:
                return reason
    if value.get("kind") not in {"ref", "file"}:
        return None
    if value.get("role") == "missing_path":
        return None
    reference = value.get("id") or value.get("path")
    if not isinstance(reference, str) or "/" not in reference:
        return None
    if not (FIXTURE_DIR / reference).is_file():
        return f"{label} references missing fixture {reference}"
    return None


def pending_route_reason(row: ConcreteInput) -> str | None:
    if not operation_is_real_parity(row.operation):
        return None
    for name, asset in sorted(row.assets.items()):
        reason = unresolved_asset_reason(asset, name)
        if reason:
            return reason
    return None


def lifecycle_handle(row: ConcreteInput, name: str) -> str | None:
    value = row.params.get(name)
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        raw = value.get("handle") or value.get("value")
        return raw if isinstance(raw, str) else None
    return None


def param_is_null(row: ConcreteInput, name: str) -> bool:
    return name in row.params and row.params[name] is None or lifecycle_handle(row, name) == "null"


def lifecycle_handle_is_null(row: ConcreteInput, name: str) -> bool:
    handle = lifecycle_handle(row, name)
    return isinstance(handle, str) and handle.lower() == "null"


def has_null_lifecycle_handle(row: ConcreteInput) -> bool:
    return any(
        lifecycle_handle(row, name) == "null"
        for name in ("face", "library", "slot", "glyph_slot", "pathname", "aface")
    )


def pending_core_reason(row: ConcreteInput) -> str | None:
    if row.case_id == "ftimage.FT_GLYPH_FORMAT_NONE.reset_slot_uses_none":
        return (
            "unloaded glyph slot lifecycle is not exposed by the Rust FFI, "
            "C ABI, or WASM ABI wrappers"
        )
    if (
        row.operation == "ftbitmap.glyphslot_own_bitmap"
        and row.case == "error_copy_allocation_failure"
    ):
        return (
            "FT_GlyphSlot_Own_Bitmap allocation-failure parity needs maintained "
            "allocator fault injection for the pinned C oracle, Rust FFI, C ABI, "
            "and WASM ABI"
        )
    if (
        row.case_id
        == "freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format.unrouted_slot_states"
    ):
        return "unloaded and unsupported synthetic glyph-slot states need explicit public runner support"
    if (
        row.operation
        in {
            "ftsynth.glyphslot_adjust_weight_after_load",
            "ftsynth.glyphslot_embolden_after_load",
        }
        and "bitmap_strike_font" in row.assets
    ):
        return (
            "bitmap glyph-slot synthesis requires FT_GlyphSlot_AdjustWeight "
            "bitmap-slot behavior plus an executable embedded-bitmap strike route"
        )
    if (
        row.case_id == "tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation"
        and row.operation == "sfnt.get_sfnt_table.record"
    ):
        return "MVAR variation table behavior must be implemented before this SFNT table row can run"
    if (
        row.operation == "ftsnames.get_sfnt_name"
        and row.expectation_status == "build_dependent"
        and lifecycle_handle(row, "face") == "non_sfnt"
    ):
        return "non-SFNT face fixture must open before FT_Get_Sfnt_Name"
    if row.operation != "ftmm.set_named_instance":
        return None
    if any(
        isinstance(item, dict) and item.get("operation") == "FT_Set_MM_Design_Coordinates"
        for item in list_value(row.params.get("prior_calls"))
    ):
        return "Adobe MM named-instance reset requires real Adobe MM support"
    if "compare_namedstyle_index" in row.params:
        return "namedstyle coordinate parity requires FT_MM_Var support"
    if "glyph_index" in row.params:
        return "named-instance glyph-output parity requires gvar/HVAR support"
    return None


def size_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "new_memory_face" and lifecycle_handle(row, "file_base") == "null":
        return "FT_New_Memory_Face null file_base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.new_size":
        if lifecycle_handle(row, "face") == "null":
            return "FT_New_Size null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if lifecycle_handle(row, "output") == "null":
            return "FT_New_Size null output pointer validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.done_size" and lifecycle_handle(row, "size") == "null":
        return "FT_Done_Size null size validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.activate_size" and lifecycle_handle(row, "size") == "null":
        return "FT_Activate_Size null size validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def otvalid_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftotval.open_type_validate":
        if param_is_null(row, "face"):
            return "FT_OpenType_Validate null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if "null_output_indices" in row.params:
            return "FT_OpenType_Validate null output pointers validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftotval.open_type_free":
        if param_is_null(row, "face"):
            return "FT_OpenType_Free null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if param_is_null(row, "table") and has_runtime_asset(row):
            return "FT_OpenType_Free null table validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def otvalid_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftotval.open_type_validate"
        and row.params.get("face") == "valid_without_OPENTYPE_VALIDATE_service"
        and has_runtime_asset(row)
    ):
        return "FT_OpenType_Validate missing-service face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def font_format_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation in {"ftfntfmt.get_font_format", "ftfntfmt.get_x11_font_format"}:
        if has_runtime_asset(row) or row.params.get("face") is None:
            return "font-format service string/nullness validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def outline_get_bitmap_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.subject == "ftoutln.FT_Outline_Get_Bitmap"
        and row.operation == "ftoutln.outline_get_bitmap"
    ):
        return "FT_Outline_Get_Bitmap validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.subject == "ftimage.FT_PIXEL_MODE_NONE"
        and row.case == "invalid_render_target_errors"
        and row.operation == "ftoutln.outline_get_bitmap"
    ):
        return "FT_PIXEL_MODE_NONE invalid FT_Outline_Get_Bitmap target validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def set_debug_hook_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftmodapi.set_debug_hook" and row.subject == "ftmodapi.FT_Set_Debug_Hook":
        return "FT_Set_Debug_Hook slot mutation/no-op behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def add_default_modules_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftmodapi.add_default_modules"
        and row.case_id
        == "ftmodapi.FT_Add_Default_Modules.null_library_no_return_error"
    ):
        return "FT_Add_Default_Modules null-library void/no-crash behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def lifecycle_null_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "freetype.done_freetype"
        and row.case_id == "freetype.FT_Done_FreeType.error_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Done_FreeType null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.done_face"
        and row.case_id == "freetype.FT_Done_Face.error_null_face"
        and lifecycle_handle(row, "face") == "null"
    ):
        return "FT_Done_Face null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_char"
        and row.case_id
        == "freetype.FT_Load_Char.error_null_face_or_invalid_flags.null_face"
        and lifecycle_handle(row, "face") == "null"
    ):
        return "FT_Load_Char null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_char"
        and row.case_id == "freetype.FT_Load_Char.error_null_face_or_invalid_flags"
        and row.case == "error_null_face_or_invalid_flags"
    ):
        return "FT_Load_Char reserved-load-flag error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id == "freetype.FT_Set_Char_Size.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Char_Size null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id == "freetype.FT_Set_Char_Size.error_probe_face_invalid_size_handle"
    ):
        return "FT_Set_Char_Size probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_pixel_sizes"
        and row.case_id == "freetype.FT_Set_Pixel_Sizes.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Pixel_Sizes null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_pixel_sizes"
        and row.case_id
        == "freetype.FT_Set_Pixel_Sizes.error_probe_face_invalid_size_handle"
    ):
        return "FT_Set_Pixel_Sizes probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_charmap"
        and row.case_id == "freetype.FT_Select_Charmap.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Select_Charmap null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.set_charmap"
        and row.case_id == "freetype.FT_Set_Charmap.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Charmap null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_size"
        and row.case_id == "freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Select_Size null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_null_face_or_request"
        and any(
            isinstance(variant, dict)
            and (variant.get("face") == "null" or variant.get("request") == "null")
            for variant in list_value(row.params.get("variants"))
        )
    ):
        return "FT_Request_Size null-face/null-request errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_ppem_overflow"
    ):
        return "FT_Request_Size ppem-overflow error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id
        == "freetype.FT_SIZE_REQUEST_TYPE_MAX.request_size_rejects_sentinel"
    ):
        return "FT_Request_Size sentinel request-type error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id
        == "fterrdef.FT_Err_Divide_By_Zero.invalid_size_transform_division_returns_error"
    ):
        return "FT_Request_Size BBOX divide-by-zero error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_invalid_request_or_unavailable_strike"
    ):
        return "FT_Request_Size invalid-request matrix validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_probe_face_invalid_size_handle"
    ):
        return "FT_Request_Size probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_Load_Glyph.error_null_face_or_invalid_flags.null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Load_Glyph null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "fterrdef.FT_Err_Invalid_Face_Handle.face_api_rejects_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Err_Invalid_Face_Handle load-glyph null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def wrapper_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "freetype.get_subglyph_info" and "null_output_indices" in row.params:
        return (
            "FT_Get_SubGlyph_Info native C dereferences valid-slot output pointers; "
            "the row validates Rust FFI, C ABI, and WASM ABI null-output policy "
            "after proving the subglyph is native-C-callable with non-null outputs"
        )
    return None


def raw_slot_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftsynth.glyphslot_null_noop":
        return (
            "raw FT_GlyphSlot null no-op validates pinned C, Rust FFI, and C ABI; "
            "the WASM handle ABI has no raw glyph-slot pointer surface"
        )
    return None


def list_value(value: object) -> list[object]:
    return value if isinstance(value, list) else []


def shape_fallback_reason(row: ConcreteInput) -> str | None:
    params = row.params
    operation = row.operation
    if operation == "load_char" and "char_code" not in params:
        return "load_char lacks char_code"
    if operation == "load_glyph" and "glyph_index" not in params and "glyph_selector" not in params:
        return "load_glyph lacks glyph selector"
    if operation == "render_glyph" and not any(
        key in params for key in ("char_code", "glyph_index", "glyph_selector")
    ):
        return "render_glyph lacks glyph selector"
    if operation == "set_char_size" and "variants" in params:
        return "set_char_size variants are not routed through the real oracle"
    if (
        operation == "freetype.request_size"
        and "requests" not in params
        and "request" not in params
        and "variants" not in params
    ):
        return "request_size lacks request rows"
    if operation == "freetype.set_charmap" and not any(
        key in params for key in ("charmap_indices", "variants")
    ):
        return "set_charmap lacks charmap selector rows"
    if operation == "ftoutln.outline_get_cbox" and not any(
        key in params for key in ("glyph_index", "glyph_indices", "scenarios")
    ):
        return "outline_get_cbox lacks glyph selector"
    if operation == "ftsnames.get_sfnt_name" and "indexes" not in params:
        return "get_sfnt_name lacks name indexes"
    if operation.startswith("sfnt.get_sfnt_table") and any(
        key in params for key in ("variation_sequence", "variation_calls")
    ):
        return "sfnt table variation sequence is not routed"
    if operation == "sfnt.load_sfnt_table" and not any(
        key in params for key in ("offset", "reads", "tags")
    ):
        return "load_sfnt_table lacks offset/read/tag selector"
    if operation == "sfnt.table_info" and not any(
        key in params
        for key in ("table_index", "invalid_index", "table_indices", "table_index_ignored")
    ):
        return "table_info lacks table index selector"
    if operation == "freetype.new_face" and any(
        lifecycle_handle(row, name) == "null" for name in ("pathname", "library", "aface")
    ):
        return "new_face null-handle row uses generic oracle fallback"
    if (
        operation == "new_memory_face"
        and not has_runtime_asset(row)
        and lifecycle_handle(row, "file_base") != "null"
    ):
        return "new_memory_face lacks runtime font asset"
    if operation == "set_pixel_sizes" and not has_runtime_asset(row):
        if lifecycle_handle(row, "face") != "null":
            return f"{operation} lacks runtime font asset"
    if operation in {"freetype.done_freetype", "freetype.done_face"} and not has_runtime_asset(row):
        handle_name = "library" if operation == "freetype.done_freetype" else "face"
        if lifecycle_handle(row, handle_name) != "null":
            return f"{operation} lacks runtime font asset"
    if operation == "freetype.face_set_unpatented_hinting" and not any(
        key in params for key in ("bool_values", "values", "value", "toggle_sequence")
    ):
        return "face_set_unpatented_hinting lacks bool values"
    if operation in {
        "ftsynth.glyphslot_slant_noop",
        "ftsynth.glyphslot_oblique_noop",
        "ftsynth.glyphslot_adjust_weight_noop",
        "ftsynth.glyphslot_embolden_noop",
    }:
        return "synthetic unsupported glyph-slot fixture is required_future_asset"
    if (
        operation
        in {
            "ftsynth.glyphslot_adjust_weight_after_load",
            "ftsynth.glyphslot_embolden_after_load",
        }
        and "bitmap_strike_font" in row.assets
    ):
        return "embedded bitmap strike fixture is required_future_asset"
    return None


def route_category(row: ConcreteInput) -> tuple[str, str]:
    pending = pending_core_reason(row)
    if pending:
        return ("pending-core", pending)
    shape_reason = shape_fallback_reason(row)
    if shape_reason:
        if row.expect_error and not has_runtime_asset(row):
            return ("null-error-fallback", shape_reason)
        return ("shape-incomplete-fallback", shape_reason)
    if row.operation in EXPLICIT_UNSUPPORTED_OPERATIONS:
        return ("explicit-unsupported", "explicit Rust stub returns Unimplemented_Feature")
    if operation_is_compile_contract(row.operation):
        return ("compile-contract", "header, layout, macro, or scalar contract")
    route_pending = pending_route_reason(row)
    if route_pending:
        return ("pending-route", route_pending)
    lifecycle_null_reason = lifecycle_null_real_parity_reason(row)
    if lifecycle_null_reason:
        return ("real-parity", lifecycle_null_reason)
    if row.expect_error and not row.compare_error_output:
        return (
            "generic-error-fallback",
            "expected error is accepted without exact C status/output comparison",
        )
    if row.allow_oracle_errors:
        return (
            "generic-fallback",
            "oracle errors are explicitly accepted instead of requiring the declared route",
        )
    if row.case_id in SHARED_RUNTIME_FALLBACK_CASES:
        return (
            "generic-fallback",
            "shared Rust fallback across Rust, C ABI, and WASM actual lanes; not public-route parity",
        )
    size_null_reason = size_null_validation_reason(row)
    if size_null_reason:
        return ("real-null-validation", size_null_reason)
    otvalid_null_reason = otvalid_null_validation_reason(row)
    if otvalid_null_reason:
        return ("real-null-validation", otvalid_null_reason)
    otvalid_real_reason = otvalid_real_parity_reason(row)
    if otvalid_real_reason:
        return ("real-parity", otvalid_real_reason)
    font_format_real_reason = font_format_real_parity_reason(row)
    if font_format_real_reason:
        return ("real-parity", font_format_real_reason)
    outline_get_bitmap_real_reason = outline_get_bitmap_real_parity_reason(row)
    if outline_get_bitmap_real_reason:
        return ("real-parity", outline_get_bitmap_real_reason)
    set_debug_hook_real_reason = set_debug_hook_real_parity_reason(row)
    if set_debug_hook_real_reason:
        return ("real-parity", set_debug_hook_real_reason)
    add_default_modules_real_reason = add_default_modules_real_parity_reason(row)
    if add_default_modules_real_reason:
        return ("real-parity", add_default_modules_real_reason)
    wrapper_null_reason = wrapper_null_validation_reason(row)
    if wrapper_null_reason:
        return ("wrapper-null-validation", wrapper_null_reason)
    raw_slot_null_reason = raw_slot_null_validation_reason(row)
    if raw_slot_null_reason:
        return ("raw-slot-null-validation", raw_slot_null_reason)
    if operation_is_real_parity(row.operation):
        return ("real-parity", "explicit C oracle, Rust FFI, C ABI, and WASM route")
    if row.expect_error and not has_runtime_asset(row):
        return ("generic-error-fallback", "no-asset expected-error row")
    if not row.expect_error and not has_runtime_asset(row) and has_null_lifecycle_handle(row):
        return ("void-fallback", "no-asset null-handle void row")
    return ("generic-fallback", "no explicit maintained route classification")


def supplementary_safe_api_flags(row: ConcreteInput) -> list[str]:
    return [
        flag
        for flag in sorted(SUPPLEMENTARY_SAFE_API_FLAGS)
        if row.params.get(flag) is True
    ]


def runtime_id(row: ConcreteInput) -> str:
    if row.variant_id:
        return f"{row.case_id}@{row.variant_id}"
    return row.case_id


def build_route_audit(items: dict[str, ManifestSubject]) -> dict[str, object]:
    rows = []
    for row in concrete_inputs(items):
        category, reason = route_category(row)
        supplementary_flags = supplementary_safe_api_flags(row)
        rows.append(
            {
                "subject": row.subject,
                "case_id": row.case_id,
                "runtime_id": runtime_id(row),
                "case": row.case,
                "operation": row.operation,
                "category": category,
                "reason": reason,
                "expect_error": row.expect_error,
                "compare_error_output": row.compare_error_output,
                "allow_oracle_errors": row.allow_oracle_errors,
                "expectation_status": row.expectation_status,
                "supplementary_safe_api_flags": supplementary_flags,
            }
        )

    category_counts: dict[str, int] = {}
    operation_counts: dict[str, dict[str, int]] = {}
    examples: dict[tuple[str, str], str] = {}
    pending_core_rows: list[str] = []
    pending_route_rows: list[str] = []
    runtime_pending_rows: list[str] = []
    placeholder_style_rows: list[str] = []
    supplementary_counts: dict[str, int] = {}
    supplementary_examples: dict[str, str] = {}
    for row in rows:
        category_counts[row["category"]] = category_counts.get(row["category"], 0) + 1
        by_category = operation_counts.setdefault(row["operation"], {})
        by_category[row["category"]] = by_category.get(row["category"], 0) + 1
        examples.setdefault((row["operation"], row["category"]), row["runtime_id"])
        if row["category"] == "pending-core":
            pending_core_rows.append(row["runtime_id"])
            if row["runtime_id"] not in AUDIT_ONLY_PENDING_CORE_CASES:
                runtime_pending_rows.append(row["runtime_id"])
        if row["category"] == "pending-route":
            pending_route_rows.append(row["runtime_id"])
            runtime_pending_rows.append(row["runtime_id"])
        if row["category"] in PLACEHOLDER_STYLE_CATEGORIES:
            placeholder_style_rows.append(row["runtime_id"])
        flags = row["supplementary_safe_api_flags"]
        assert isinstance(flags, list)
        for flag in flags:
            label = SUPPLEMENTARY_SAFE_API_FLAGS[flag]
            supplementary_counts[label] = supplementary_counts.get(label, 0) + 1
            supplementary_examples.setdefault(label, row["runtime_id"])

    return {
        "total_concrete_cases": len(rows),
        "category_counts": dict(sorted(category_counts.items())),
        "goal_ledger": {
            "runtime_pending": len(runtime_pending_rows),
            "route_pending": len(pending_route_rows),
            "route_core_pending": len(pending_core_rows),
            "route_or_core_pending": len(pending_route_rows) + len(pending_core_rows),
            "green_placeholder_style_rows": len(placeholder_style_rows),
        },
        "runtime_pending_rows": sorted(runtime_pending_rows),
        "route_pending_rows": sorted(pending_route_rows),
        "route_core_pending_rows": sorted(pending_core_rows),
        "placeholder_style_category_counts": {
            category: category_counts.get(category, 0)
            for category in sorted(PLACEHOLDER_STYLE_CATEGORIES)
            if category_counts.get(category, 0)
        },
        "supplementary_safe_api_assertions": {
            "total_assertions": sum(supplementary_counts.values()),
            "counts": dict(sorted(supplementary_counts.items())),
            "examples": dict(sorted(supplementary_examples.items())),
            "note": (
                "These assertions prove high-level Rust convenience behavior on "
                "existing public FreeType rows, but they are not independent C "
                "FreeType public API routes."
            ),
        },
        "operation_counts": dict(sorted(operation_counts.items())),
        "examples": {
            f"{operation}|{category}": example
            for (operation, category), example in sorted(examples.items())
        },
        "rows": rows,
    }


def write_route_audit(report: dict[str, object], json_path: Path, md_path: Path) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

    operation_counts = report["operation_counts"]
    assert isinstance(operation_counts, dict)
    examples = report["examples"]
    assert isinstance(examples, dict)

    lines = [
        "# Unified Public API Route Audit",
        "",
        f"Concrete cases: {report['total_concrete_cases']}",
        "",
        "## Category Counts",
        "",
        "| Category | Cases |",
        "|---|---:|",
    ]
    category_counts = report["category_counts"]
    assert isinstance(category_counts, dict)
    for category, count in category_counts.items():
        lines.append(f"| {category} | {count} |")

    goal_ledger = report["goal_ledger"]
    assert isinstance(goal_ledger, dict)
    lines.extend(
        [
            "",
            "## Goal Ledger",
            "",
            "| Ledger | Cases |",
            "|---|---:|",
            f"| runtime pending | {goal_ledger['runtime_pending']} |",
            f"| route pending | {goal_ledger['route_pending']} |",
            f"| full route/core pending | {goal_ledger['route_core_pending']} |",
            f"| route or core pending | {goal_ledger['route_or_core_pending']} |",
            f"| green placeholder-style rows | {goal_ledger['green_placeholder_style_rows']} |",
            "",
            "Runtime pending combines `pending-route` rows with the subset of `pending-core` rows that the unified runtime parity suite reports as pending. "
            "The route-or-core ledger also includes audit-visible core rows that are not current runnable runtime parity cases.",
        ]
    )

    route_pending_rows = report["route_pending_rows"]
    assert isinstance(route_pending_rows, list)
    if route_pending_rows:
        lines.extend(["", "### Route-Pending Rows", ""])
        for runtime_id in route_pending_rows:
            lines.append(f"- `{runtime_id}`")

    pending_rows = report["route_core_pending_rows"]
    assert isinstance(pending_rows, list)
    if pending_rows:
        lines.extend(["", "### Pending Rows", ""])
        for runtime_id in pending_rows:
            lines.append(f"- `{runtime_id}`")

    placeholder_counts = report["placeholder_style_category_counts"]
    assert isinstance(placeholder_counts, dict)
    if placeholder_counts:
        lines.extend(
            [
                "",
                "### Placeholder-Style Categories",
                "",
                "| Category | Cases |",
                "|---|---:|",
            ]
        )
        for category, count in placeholder_counts.items():
            lines.append(f"| {category} | {count} |")

    supplementary = report["supplementary_safe_api_assertions"]
    assert isinstance(supplementary, dict)
    supplementary_counts = supplementary["counts"]
    assert isinstance(supplementary_counts, dict)
    if supplementary_counts:
        lines.extend(
            [
                "",
                "## Supplementary Non-FreeType Goal Assertions",
                "",
                str(supplementary["note"]),
                "",
                "| Surface | Rows | Example |",
                "|---|---:|---|",
            ]
        )
        supplementary_examples = supplementary["examples"]
        assert isinstance(supplementary_examples, dict)
        for surface, count in supplementary_counts.items():
            example = supplementary_examples.get(surface, "")
            lines.append(f"| {surface} | {count} | `{example}` |")
        lines.extend(
            [
                "",
                "The following surfaces should not drive the public FreeType manifest or coverage goal:",
            ]
        )
        for surface in NON_FREETYPE_GOAL_SURFACES:
            lines.append(f"- {surface}")

    lines.extend(
        [
            "",
            "## Operations Needing R0 Disposition",
            "",
            "| Operation | Category | Cases | Example |",
            "|---|---|---:|---|",
        ]
    )
    flagged = {
        "generic-fallback",
        "generic-error-fallback",
        "null-error-fallback",
        "shape-incomplete-fallback",
        "void-fallback",
        "explicit-unsupported",
        "pending-route",
        "pending-core",
    }
    for operation, counts in operation_counts.items():
        assert isinstance(counts, dict)
        for category, count in sorted(counts.items()):
            if category not in flagged:
                continue
            example = examples.get(f"{operation}|{category}", "")
            lines.append(f"| `{operation}` | {category} | {count} | `{example}` |")
    md_path.write_text("\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("subjects", nargs="*")
    parser.add_argument("--audit-json", type=Path, default=DEFAULT_AUDIT_JSON)
    parser.add_argument("--route-audit", action="store_true")
    parser.add_argument("--route-audit-json", type=Path, default=DEFAULT_ROUTE_AUDIT_JSON)
    parser.add_argument("--route-audit-md", type=Path, default=DEFAULT_ROUTE_AUDIT_MD)
    args = parser.parse_args()

    subjects = read_manifest()
    selected = args.subjects
    if selected:
        items = {subject: subjects[subject] for subject in selected}
    else:
        items = subjects

    if args.route_audit:
        report = build_route_audit(items)
        write_route_audit(report, args.route_audit_json, args.route_audit_md)
        print(f"wrote route audit json {args.route_audit_json}")
        print(f"wrote route audit markdown {args.route_audit_md}")
        print(
            "route audit concrete_cases={} category_counts={}".format(
                report["total_concrete_cases"],
                report["category_counts"],
            )
        )
        return 0

    errors: list[str] = []
    if not selected:
        errors.extend(check_manifest_public_surface(subjects, args.audit_json))
        errors.extend(check_input_directory(subjects))
        errors.extend(check_ffi_abi_surface(args.audit_json))
    for subject in items.values():
        errors.extend(check_file(subject))

    if errors:
        for error in errors:
            print(error)
        return 1
    print(f"checked {len(items)} public API input files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
