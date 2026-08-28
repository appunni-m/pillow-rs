#!/usr/bin/env python3
"""Build an ordered reverse-coverage manifest for the Pillow oracle.

The input is coverage.py's report from the same public parity corpus used by
the target lanes.  The output is evidence, not a new test denominator: it
keeps every measured Pillow Python module, maps active manifest operations to
coverage.py functions/classes when possible, and leaves codec/support modules
visible when they are not represented by the current public surface.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_REPORT = ROOT / "target" / "coverage" / "migration-parity-pillow.json"
DEFAULT_RECEIPT = ROOT / "build" / "migration-parity" / "pillow-oracle-coverage-result.json"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "docs" / "coverage-pillow-missing-feature-manifest.json"
DEFAULT_MARKDOWN = ROOT / "docs" / "coverage-pillow-missing-feature-manifest.md"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def normalized_pillow_path(report_key: str) -> str:
    parts = Path(report_key).parts
    try:
        index = parts.index("PIL")
    except ValueError:
        return Path(report_key).as_posix()
    return "/".join(parts[index:])


def module_file_from_source_path(source_path: str) -> str | None:
    parts = source_path.split(".")
    if len(parts) < 2 or parts[0] != "PIL":
        return None
    return f"PIL/{parts[1].replace('.', '/')}.py"


def source_symbol_from_source_path(source_path: str) -> str | None:
    parts = source_path.split(".")
    if len(parts) <= 2 or parts[0] != "PIL":
        return None
    return ".".join(parts[2:])


def compact_summary(value: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    fields = (
        "covered_lines",
        "num_statements",
        "missing_lines",
        "percent_statements_covered",
        "covered_branches",
        "num_branches",
        "missing_branches",
        "percent_branches_covered",
        "num_partial_branches",
    )
    return {field: value[field] for field in fields if field in value}


def summary_priority(summary: dict[str, Any]) -> int:
    return int(summary.get("missing_lines", 0)) * 100 + int(
        summary.get("missing_branches", 0)
    ) * 10


def operation_priority(operation: dict[str, Any]) -> int:
    summary = operation.get("coverage_match", {}).get("summary", {})
    return summary_priority(summary) if isinstance(summary, dict) else 0


def load_public_operations(manifest: dict[str, Any]) -> tuple[dict[str, list[dict[str, Any]]], dict[tuple[str, str], list[str]], dict[str, str]]:
    operations_by_file: dict[str, list[dict[str, Any]]] = {}
    case_ids_by_operation: dict[tuple[str, str], list[str]] = {}
    case_input_by_id: dict[str, str] = {}

    for surface in manifest.get("surfaces", []):
        surface_id = surface["id"]
        for operation in surface.get("operations", []):
            source_path = operation.get("source", {}).get("path")
            if not isinstance(source_path, str):
                continue
            source_file = module_file_from_source_path(source_path)
            if source_file is None:
                continue
            operation_record = {
                "surface": surface_id,
                "operation": operation["id"],
                "operation_id": source_path,
                "source_path": source_path,
                "source_symbol": source_symbol_from_source_path(source_path),
            }
            operations_by_file.setdefault(source_file, []).append(operation_record)

    for relative in manifest.get("input_index", {}).get("parity", []):
        input_path = FIXTURE_ROOT / relative
        document = json.loads(input_path.read_text(encoding="utf-8"))
        for case in document.get("cases", []):
            case_id = case["case_id"]
            case_input_by_id[case_id] = relative
            key = (case.get("surface"), case.get("operation"))
            case_ids_by_operation.setdefault(key, []).append(case_id)

    for operations in operations_by_file.values():
        operations.sort(key=lambda item: item["operation_id"])
    for key in case_ids_by_operation:
        case_ids_by_operation[key] = sorted(set(case_ids_by_operation[key]))
    return operations_by_file, case_ids_by_operation, case_input_by_id


def coverage_symbol(
    source_symbol: str | None,
    functions: dict[str, Any],
    classes: dict[str, Any],
) -> tuple[str | None, str | None, dict[str, Any] | None]:
    """Match one public manifest symbol to a coverage.py function/class."""

    if not source_symbol:
        return None, None, None
    if source_symbol in functions:
        return "function", source_symbol, functions[source_symbol]
    if source_symbol in classes:
        return "class", source_symbol, classes[source_symbol]
    constructor = f"{source_symbol}.__init__"
    if constructor in functions:
        return "constructor", constructor, functions[constructor]
    return None, None, None


def missing_function_records(
    functions: dict[str, Any], classes: dict[str, Any]
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for kind, collection in (("function", functions), ("class", classes)):
        for name, value in collection.items():
            if not isinstance(value, dict):
                continue
            summary = value.get("summary", {})
            if not isinstance(summary, dict):
                continue
            missing_lines = value.get("missing_lines", [])
            missing_branches = value.get("missing_branches", [])
            if not missing_lines and not missing_branches and not summary.get(
                "missing_lines", 0
            ) and not summary.get("missing_branches", 0):
                continue
            record = {
                "kind": kind,
                "name": name,
                "start_line": value.get("start_line"),
                "summary": compact_summary(summary),
                "missing_lines": missing_lines,
                "missing_branches": missing_branches,
            }
            records.append(record)
    records.sort(
        key=lambda item: (
            -summary_priority(item.get("summary") or {}),
            item.get("start_line") or 0,
            item["kind"],
            item["name"],
        )
    )
    return records


def source_classification(source_file: str, public_operations: list[dict[str, Any]]) -> str:
    if public_operations:
        return "active_public_module"
    name = Path(source_file).stem
    if name.endswith(("ImagePlugin", "ImageFile")) or name in {
        "ContainerIO",
        "FontFile",
        "ImageFile",
        "features",
    }:
        return "codec_or_support_module_outside_active_surface"
    return "pillow_support_module_outside_active_surface"


def operation_gap_record(
    operation: dict[str, Any],
    *,
    functions: dict[str, Any],
    classes: dict[str, Any],
    case_ids_by_operation: dict[tuple[str, str], list[str]],
    case_input_by_id: dict[str, str],
) -> dict[str, Any]:
    kind, coverage_symbol_name, coverage_value = coverage_symbol(
        operation.get("source_symbol"), functions, classes
    )
    summary = coverage_value.get("summary", {}) if isinstance(coverage_value, dict) else {}
    if not isinstance(summary, dict):
        summary = {}
    case_ids = case_ids_by_operation.get(
        (operation["surface"], operation["operation"]), []
    )
    missing = int(summary.get("missing_lines", 0)) or int(
        summary.get("missing_branches", 0)
    )
    missing_lines = int(summary.get("missing_lines", 0))
    missing_branches = int(summary.get("missing_branches", 0))
    if missing_lines or missing_branches:
        gap_status = "missing"
    elif kind is None:
        gap_status = "not_separately_attributed"
    else:
        gap_status = "covered"
    result: dict[str, Any] = {
        **operation,
        "coverage_match": {
            "kind": kind,
            "symbol": coverage_symbol_name,
            "summary": compact_summary(summary),
        },
        "case_count": len(case_ids),
        "input_paths": sorted({case_input_by_id[case_id] for case_id in case_ids}),
        "gap_status": gap_status,
        "missing_line_count": missing_lines,
        "missing_branch_count": missing_branches,
        "priority": missing_lines * 100 + missing_branches * 10,
    }
    if missing:
        result["case_ids"] = case_ids
        result["recommendation"] = (
            "Inspect the listed Pillow line/branch arcs, then add or extend "
            "input-only parity cases for this public operation if the branch is "
            "reachable through the public contract."
        )
    elif kind is None:
        result["recommendation"] = (
            "The operation is in the active manifest but coverage.py has no "
            "separate function/class record; use the module-level missing lines "
            "and the live public workflow before adding a case."
        )
    return result


def build_manifest(
    report: dict[str, Any],
    receipt: dict[str, Any] | None,
    active_manifest: dict[str, Any],
    manifest_path: Path,
    report_path: Path,
    receipt_path: Path,
) -> dict[str, Any]:
    operations_by_file, case_ids_by_operation, case_input_by_id = load_public_operations(
        active_manifest
    )
    report_files = report.get("files", {})
    entries: list[dict[str, Any]] = []
    for report_key, file_data in report_files.items():
        if not isinstance(file_data, dict):
            continue
        source_file = normalized_pillow_path(report_key)
        summary = file_data.get("summary", {})
        if not isinstance(summary, dict):
            summary = {}
        public_operations = operations_by_file.get(source_file, [])
        functions = file_data.get("functions", {})
        classes = file_data.get("classes", {})
        if not isinstance(functions, dict):
            functions = {}
        if not isinstance(classes, dict):
            classes = {}
        operation_records = [
            operation_gap_record(
                operation,
                functions=functions,
                classes=classes,
                case_ids_by_operation=case_ids_by_operation,
                case_input_by_id=case_input_by_id,
            )
            for operation in public_operations
        ]
        public_gaps = [
            operation
            for operation in operation_records
            if operation["gap_status"] == "missing"
        ]
        entry = {
            "source_file": source_file,
            "report_file": report_key,
            "classification": source_classification(source_file, public_operations),
            "gap_status": "missing"
            if summary.get("missing_lines", 0) or summary.get("missing_branches", 0)
            else "covered",
            "priority": summary_priority(summary),
            "summary": compact_summary(summary),
            "missing_lines": file_data.get("missing_lines", []),
            "missing_branches": file_data.get("missing_branches", []),
            "missing_symbols": missing_function_records(functions, classes),
            "public_operations": operation_records,
            "public_operations_with_gaps": [
                operation["operation_id"] for operation in public_gaps
            ],
        }
        if public_operations:
            entry["next_action"] = (
                "Prioritize public operation gaps whose mapped function/class has "
                "missing lines or branches; classify remaining module-internal "
                "arcs before adding inputs."
            )
        elif entry["gap_status"] == "missing":
            entry["next_action"] = (
                "This module is outside the active public Pillow-rs surface. "
                "Add a public format/workflow only when that surface is intended, "
                "otherwise keep it visible as an unmapped oracle-support gap."
            )
        else:
            entry["next_action"] = "No missing source lines or branches in this snapshot."
        entries.append(entry)

    entries.sort(
        key=lambda item: (-int(item["priority"]), item["source_file"])
    )
    for rank, entry in enumerate(entries, start=1):
        entry["rank"] = rank

    public_operations = [
        operation
        for entry in entries
        for operation in entry["public_operations"]
    ]
    public_operations.sort(
        key=lambda operation: (
            0 if operation["gap_status"] == "missing" else 1,
            -operation_priority(operation),
            operation["operation_id"],
        )
    )
    feature_gaps = [
        operation
        for operation in public_operations
        if operation["gap_status"] == "missing"
    ]
    feature_manifest: list[dict[str, Any]] = []
    for rank, operation in enumerate(public_operations, start=1):
        feature_manifest.append({**operation, "rank": rank})
    ranked_feature_gaps: list[dict[str, Any]] = []
    for rank, operation in enumerate(feature_gaps, start=1):
        ranked_feature_gaps.append({**operation, "rank": rank})

    totals = report.get("totals", {})
    execution = receipt.get("execution", {}) if isinstance(receipt, dict) else {}
    coverage = receipt.get("coverage", {}) if isinstance(receipt, dict) else {}
    return {
        "schema": "pillow-rs/reverse-coverage-gap-manifest@1",
        "purpose": (
            "Ordered evidence of Pillow Python source that the shared public "
            "parity corpus did not execute; it is not an active input denominator."
        ),
        "provenance": {
            "coverage_report": relative_path(report_path),
            "coverage_report_sha256": sha256_file(report_path),
            "coverage_report_format": report.get("meta", {}).get("format"),
            "coverage_report_version": report.get("meta", {}).get("version"),
            "receipt": relative_path(receipt_path) if receipt_path.exists() else None,
            "receipt_execution": execution,
            "receipt_coverage": coverage,
            "active_manifest": relative_path(manifest_path),
            "active_manifest_sha256": sha256_file(manifest_path),
        },
        "corpus": {
            "kind": "public-parity-corpus",
            "cases_selected": len(case_input_by_id),
            "operation_count": sum(len(value) for value in operations_by_file.values()),
            "input_files": sorted(set(case_input_by_id.values())),
        },
        "totals": totals,
        "summary": {
            "source_files": len(entries),
            "source_files_with_gaps": sum(entry["gap_status"] == "missing" for entry in entries),
            "active_public_modules": sum(
                entry["classification"] == "active_public_module" for entry in entries
            ),
            "unmapped_gap_modules": sum(
                entry["gap_status"] == "missing"
                and entry["classification"] != "active_public_module"
                for entry in entries
            ),
            "public_operations_with_gaps": sum(
                len(entry["public_operations_with_gaps"]) for entry in entries
            ),
            "public_operations": len(public_operations),
        },
        "feature_gaps": ranked_feature_gaps,
        "feature_manifest": feature_manifest,
        "entries": entries,
    }


def markdown_report(document: dict[str, Any]) -> str:
    totals = document.get("totals", {})
    summary = document.get("summary", {})

    def percentage_text(field: str) -> str:
        value = totals.get(field)
        return f"{float(value):.2f}" if isinstance(value, (int, float)) else "n/a"

    lines = [
        "# Pillow reverse-coverage gap manifest",
        "",
        "This is generated evidence from the same public parity corpus. It is not a new test denominator.",
        "",
        f"- Python lines: {totals.get('covered_lines', 0)}/{totals.get('num_statements', 0)} ({percentage_text('percent_statements_covered')}%)",
        f"- Python branches: {totals.get('covered_branches', 0)}/{totals.get('num_branches', 0)} ({percentage_text('percent_branches_covered')}%)",
        f"- Source files with gaps: {summary.get('source_files_with_gaps', 0)}/{summary.get('source_files', 0)}",
        f"- Active public operations with mapped gaps: {summary.get('public_operations_with_gaps', 0)}",
        "",
        "## Ordered public feature gaps",
        "",
        "The JSON contains all active public operations in `feature_manifest`. The table below shows only operations whose mapped Pillow function/class still has missing lines or branches in this snapshot; inspect the listed case IDs before adding inputs.",
        "",
        "| Rank | Public operation | Cases | Missing lines | Missing branches | Priority |",
        "| ---: | --- | ---: | ---: | ---: | ---: |",
    ]
    for operation in document.get("feature_gaps", []):
        lines.append(
            "| {rank} | `{operation_id}` | {case_count} | {missing_lines} | {missing_branches} | {priority} |".format(
                rank=operation["rank"],
                operation_id=operation["operation_id"],
                case_count=operation["case_count"],
                missing_lines=operation["missing_line_count"],
                missing_branches=operation["missing_branch_count"],
                priority=operation["priority"],
            )
        )
    lines.extend(
        [
            "",
        "## Ordered source gaps",
        "",
        "| Rank | Pillow source | Missing lines | Missing branches | Priority | Classification |",
        "| ---: | --- | ---: | ---: | ---: | --- |",
        ]
    )
    for entry in document.get("entries", []):
        summary = entry.get("summary") or {}
        lines.append(
            "| {rank} | `{source}` | {lines} | {branches} | {priority} | {classification} |".format(
                rank=entry["rank"],
                source=entry["source_file"],
                lines=summary.get("missing_lines", 0),
                branches=summary.get("missing_branches", 0),
                priority=entry["priority"],
                classification=entry["classification"],
            )
        )
    lines.extend(
        [
            "",
            "## Reading the manifest",
            "",
            "For active public modules, inspect `public_operations_with_gaps`, each operation's `case_ids`, and `missing_symbols` before adding an input. Codec/support modules without active public operations are intentionally listed as unmapped rather than silently treated as missing parity APIs.",
            "",
            "WGSL shader execution is reported by the all-backends artifact separately. This Pillow manifest measures only coverage.py's Python source files; Pillow's native extension is not part of these totals.",
            "",
        ]
    )
    return "\n".join(lines)


def run(args: argparse.Namespace) -> int:
    report_path = args.report.resolve()
    receipt_path = args.receipt.resolve()
    manifest_path = args.manifest.resolve()
    report = json.loads(report_path.read_text(encoding="utf-8"))
    receipt = (
        json.loads(receipt_path.read_text(encoding="utf-8"))
        if receipt_path.exists()
        else None
    )
    active_manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    document = build_manifest(
        report, receipt, active_manifest, manifest_path, report_path, receipt_path
    )
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.markdown:
        markdown = args.markdown.resolve()
        markdown.parent.mkdir(parents=True, exist_ok=True)
        markdown.write_text(markdown_report(document), encoding="utf-8")
    print(json.dumps(document["summary"], sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--markdown", type=Path, default=DEFAULT_MARKDOWN)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
