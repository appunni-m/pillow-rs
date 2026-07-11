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
INPUT_DIR = ROOT / "tests" / "fixtures" / "inputs" / "public-api"
DEFAULT_AUDIT_JSON = ROOT / "target" / "api-abi-audit" / "api_abi_audit.json"

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
    "fontdone_wasm_done_freetype",
    "fontdone_wasm_face_check_truetype_patents",
    "fontdone_wasm_face_set_unpatented_hinting",
    "fontdone_wasm_get_truetype_engine_type",
    "fontdone_wasm_library_set_lcd_filter",
    "fontdone_wasm_library_set_lcd_filter_weights",
    "fontdone_wasm_library_set_lcd_geometry",
    "fontdone_wasm_mul_div",
    "fontdone_wasm_mul_fix",
    "fontdone_wasm_div_fix",
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
    "fontdone_wasm_get_char_index",
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


@dataclass(frozen=True)
class ManifestSubject:
    subject_id: str
    kind: str
    symbol: str
    header: str
    cases: set[str]


def filename_for_subject(subject: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]", "_", subject) + ".json"


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
    if data.get("subject") != subject.subject_id:
        errors.append(f"{subject.subject_id}: top-level subject mismatch")
    manifest_cases_field = data.get("manifest_cases")
    if manifest_cases_field is not None:
        if not isinstance(manifest_cases_field, list):
            errors.append(f"{subject.subject_id}: manifest_cases must be list")
        else:
            unknown = sorted(set(manifest_cases_field) - subject.cases)
            if unknown:
                errors.append(f"{subject.subject_id}: manifest_cases contains unknown cases {unknown}")

    cases = data.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append(f"{subject.subject_id}: cases must be a non-empty list")
        return errors

    covered: set[str] = set()
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            errors.append(f"{subject.subject_id}: cases[{index}] must be object")
            continue
        if case.get("subject") != subject.subject_id:
            errors.append(f"{subject.subject_id}: cases[{index}] subject mismatch")
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("subjects", nargs="*")
    parser.add_argument("--audit-json", type=Path, default=DEFAULT_AUDIT_JSON)
    args = parser.parse_args()

    subjects = read_manifest()
    selected = args.subjects
    if selected:
        items = {subject: subjects[subject] for subject in selected}
    else:
        items = subjects

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
