#!/usr/bin/env python3
"""Report region coverage per public operation from the maintained coverage lane.

The strict coverage artifact records per-file function/line/branch/region
dimensions for every manifest-declared component path.  Those component
aggregates are useful for backlog ordering, but they are not operation-level
coverage because several public operations share each component.  When the
scoped getbbox artifact is present, this report also records exact operation
evidence from only getbbox parity inputs: the Python facade statement and the
Rust getbbox function regions.

Output is a generated markdown report listing every operation with region
coverage below 95% in ascending order, plus per-file detail for the involved
components so follow-up case work can target the exact files.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_COVERAGE = ROOT / "build" / "migration-parity" / "coverage-result-rust.json"
DEFAULT_OUTPUT = ROOT / "docs" / "migration-parity-region-coverage.md"
DEFAULT_OPERATION_COVERAGE = (
    ROOT / "build" / "migration-parity" / "coverage-operation-rust.json"
)
DEFAULT_OPERATION_PYTHON_REPORT = (
    ROOT / "target" / "coverage" / "migration-parity-operation-python.json"
)
DEFAULT_OPERATION_LLVM_REPORT = (
    ROOT / "target" / "coverage" / "migration-parity-operation-rust.json"
)
GETBBOX_OPERATION = "PIL.Image.Image.getbbox"
THRESHOLD_PERCENT = 95

sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_parity import (  # noqa: E402
    load_manifest,
    sha256_file,
)


def component_file_regions(
    coverage: dict[str, Any],
) -> dict[str, dict[str, dict[str, int]]]:
    """Map component_id -> {relative path -> {covered, total}} for regions."""

    components: dict[str, dict[str, dict[str, int]]] = {}
    for plan in coverage.get("plans", []):
        for component in plan.get("components", []):
            component_id = component["component_id"]
            files = components.setdefault(component_id, {})
            for file_result in component.get("files", []):
                relative = file_result["path"]
                region = next(
                    (
                        item
                        for item in file_result.get("dimensions", [])
                        if item["dimension"] == "region"
                    ),
                    {"covered": 0, "total": 0},
                )
                files[relative] = {
                    "covered": int(region.get("covered", 0)),
                    "total": int(region.get("total", 0)),
                }
    return components


def operation_rows(
    manifest: dict[str, Any],
    components: dict[str, dict[str, dict[str, int]]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            coverage = operation.get("coverage", {})
            if coverage.get("applicability") != "required":
                continue
            component_ids = coverage.get("component_ids", [])
            file_totals: dict[str, dict[str, int]] = {}
            for component_id in component_ids:
                for relative, dims in components.get(component_id, {}).items():
                    file_totals[relative] = dims
            covered = sum(item["covered"] for item in file_totals.values())
            total = sum(item["total"] for item in file_totals.values())
            rows.append(
                {
                    "operation": f"{surface['id']}.{operation['id']}",
                    "component_ids": component_ids,
                    "files": sorted(file_totals),
                    "covered": covered,
                    "total": total,
                    "percent": (100.0 * covered / total) if total else None,
                }
            )
    return rows


def operation_surface_coverage(
    operation_artifact: dict[str, Any],
    python_report: dict[str, Any],
    llvm_report: dict[str, Any],
) -> dict[str, Any]:
    """Extract exact getbbox facade/function evidence from a scoped run."""

    selected_case_ids = [
        case_id
        for plan in operation_artifact.get("plans", [])
        for case_id in plan.get("selected", {}).get("parity_case_ids", [])
    ]
    if not selected_case_ids or not all(
        case_id.startswith(f"{GETBBOX_OPERATION}.") for case_id in selected_case_ids
    ):
        raise ValueError("operation coverage artifact is not scoped to getbbox")

    python_file = python_report.get("files", {}).get(
        "pillow-rs-py/python/pillow_rs/image.py"
    )
    if python_file is None:
        raise ValueError("getbbox Python facade file is missing from coverage report")
    python_function = python_file.get("functions", {}).get("Image.getbbox")
    if python_function is None:
        raise ValueError("getbbox Python facade function is missing from coverage report")
    python_summary = python_function.get("summary", {})
    python_covered = int(python_summary.get("covered_lines", 0))
    python_total = int(python_summary.get("num_statements", 0))

    rust_functions: list[dict[str, Any]] = []
    for data in llvm_report.get("data", []):
        for function in data.get("functions", []):
            filenames = [str(path) for path in function.get("filenames", [])]
            if (
                any(path.endswith("pillow-rs/src/ops/analysis.rs") for path in filenames)
                and "getbbox" in function.get("name", "").lower()
            ):
                rust_functions.append(function)
    if len(rust_functions) != 1:
        raise ValueError(
            f"expected one Rust getbbox function, found {len(rust_functions)}"
        )
    rust_regions = rust_functions[0].get("regions", [])
    rust_total = len(rust_regions)
    rust_covered = sum(1 for region in rust_regions if len(region) > 4 and int(region[4]) > 0)
    return {
        "operation": GETBBOX_OPERATION,
        "run_id": operation_artifact.get("identity", {}).get("run_id"),
        "case_count": len(selected_case_ids),
        "rust": {"covered": rust_covered, "total": rust_total},
        "python": {"covered": python_covered, "total": python_total},
    }


def render(
    manifest: dict[str, Any],
    coverage: dict[str, Any],
    rows: list[dict[str, Any]],
    components: dict[str, dict[str, dict[str, int]]],
    manifest_path: Path,
    coverage_path: Path,
    surface: dict[str, Any] | None = None,
) -> str:
    manifest_digest = sha256_file(manifest_path)
    run_id = coverage["identity"]["run_id"]
    below = [row for row in rows if row["percent"] is not None and row["percent"] < THRESHOLD_PERCENT]
    below.sort(key=lambda row: (row["percent"] or 0, row["operation"]))
    lines = [
        "# Migration parity region coverage",
        "",
        "This is a generated coverage view. The metric is **region coverage**",
        "(covered regions / total regions) from the maintained merged lane; it is",
        "not parity proof and does not change the manifest or lane inputs.",
        "",
        "```yaml",
        f"generator: scripts/report_migration_parity_region_coverage.py@3",
        f"manifest_path: {manifest_path.relative_to(ROOT)}",
        f"manifest_schema: {manifest['schema']}",
        f"manifest_sha256: {manifest_digest}",
        f"coverage_run_id: {run_id}",
        "coverage_target_profile: python-cpu",
        "metric: region",
        f"threshold: below {THRESHOLD_PERCENT}%",
        "```",
        "",
        "The operation table is a component aggregate used only to order the",
        "backlog. Several public operations share a component, so these rows are",
        "not operation-level coverage.",
        "",
        f"## PIL.Image.Image.getbbox",
        "",
    ]
    getbbox = next(row for row in rows if row["operation"] == GETBBOX_OPERATION)
    if surface is None:
        lines += [
            "Exact operation-level evidence is not available in this report run.",
            f"The component aggregate is `{getbbox['covered']}/{getbbox['total']}` "
            f"({getbbox['percent']:.1f}%), and must not be read as getbbox surface coverage.",
        ]
    else:
        rust = surface["rust"]
        python = surface["python"]
        rust_percent = 100.0 * rust["covered"] / rust["total"] if rust["total"] else None
        python_percent = (
            100.0 * python["covered"] / python["total"] if python["total"] else None
        )
        lines += [
            f"Scoped input-only evidence covers `{surface['case_count']}` getbbox cases "
            f"(run `{surface['run_id']}`).",
            f"Rust implementation regions: `{rust['covered']}/{rust['total']}` "
            f"({rust_percent:.1f}%).",
            f"Python facade statements: `{python['covered']}/{python['total']}` "
            f"({python_percent:.1f}%).",
            f"Component aggregate for backlog ordering: `{getbbox['covered']}/{getbbox['total']}` "
            f"({getbbox['percent']:.1f}%).",
        ]
    lines += [
        "",
        f"## Operations below {THRESHOLD_PERCENT}% region coverage",
        "",
        f"{len(below)} of {len(rows)} coverage-required operations are below {THRESHOLD_PERCENT}%.",
        "",
        "| Operation | Component(s) | Region coverage | Percent |",
        "| --- | --- | ---: | ---: |",
    ]
    for row in below:
        components_label = ", ".join(row["component_ids"])
        lines.append(
            f"| `{row['operation']}` | `{components_label}` | {row['covered']}/{row['total']} | {row['percent']:.1f}% |"
        )
    lines += [
        "",
        "## Per-file region coverage for involved components",
        "",
        "| Component | File | Region coverage | Percent |",
        "| --- | --- | ---: | ---: |",
    ]
    file_rows: list[tuple[str, str, int, int, float | None]] = []
    involved_components = sorted(
        {component_id for row in below for component_id in row["component_ids"]}
    )
    for component_id in involved_components:
        for relative, dims in components.get(component_id, {}).items():
            percent = (100.0 * dims["covered"] / dims["total"]) if dims["total"] else None
            file_rows.append(
                (component_id, relative, dims["covered"], dims["total"], percent)
            )
    file_rows.sort(key=lambda item: (item[0], item[4] if item[4] is not None else 100.0))
    for component_id, relative, covered, total, percent in file_rows:
        percent_text = f"{percent:.1f}%" if percent is not None else "n/a"
        lines.append(
            f"| `{component_id}` | `{relative}` | {covered}/{total} | {percent_text} |"
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--coverage", type=Path, default=DEFAULT_COVERAGE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--operation-coverage", type=Path, default=DEFAULT_OPERATION_COVERAGE)
    parser.add_argument("--operation-python-report", type=Path, default=DEFAULT_OPERATION_PYTHON_REPORT)
    parser.add_argument("--operation-llvm-report", type=Path, default=DEFAULT_OPERATION_LLVM_REPORT)
    args = parser.parse_args()
    if not args.coverage.is_file():
        raise SystemExit(
            f"coverage artifact missing: {args.coverage}\n"
            "run `make migration-parity-coverage-rust` first"
        )
    manifest = load_manifest(args.manifest.resolve())
    coverage = json.loads(args.coverage.read_text(encoding="utf-8"))
    if coverage.get("schema") != "migration-parity/coverage-result@1":
        raise SystemExit(f"{args.coverage}: not a coverage-result@1 artifact")
    components = component_file_regions(coverage)
    rows = operation_rows(manifest, components)
    surface = None
    if (
        args.operation_coverage.is_file()
        and args.operation_python_report.is_file()
        and args.operation_llvm_report.is_file()
    ):
        operation_coverage = json.loads(args.operation_coverage.read_text(encoding="utf-8"))
        selected_case_ids = [
            case_id
            for plan in operation_coverage.get("plans", [])
            for case_id in plan.get("selected", {}).get("parity_case_ids", [])
        ]
        if selected_case_ids and all(
            case_id.startswith(f"{GETBBOX_OPERATION}.") for case_id in selected_case_ids
        ):
            python_report = json.loads(args.operation_python_report.read_text(encoding="utf-8"))
            llvm_report = json.loads(args.operation_llvm_report.read_text(encoding="utf-8"))
            surface = operation_surface_coverage(operation_coverage, python_report, llvm_report)
    report = render(
        manifest,
        coverage,
        rows,
        components,
        args.manifest.resolve(),
        args.coverage.resolve(),
        surface,
    )
    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(report + "\n", encoding="utf-8")
    getbbox = next(row for row in rows if row["operation"] == GETBBOX_OPERATION)
    if surface is None:
        print(
            f"{GETBBOX_OPERATION} component aggregate -> "
            f"{getbbox['covered']}/{getbbox['total']} ({getbbox['percent']:.1f}%); "
            "surface evidence not present"
        )
    else:
        rust = surface["rust"]
        python = surface["python"]
        print(
            f"{GETBBOX_OPERATION} surface -> "
            f"Rust regions {rust['covered']}/{rust['total']}, "
            f"Python facade statements {python['covered']}/{python['total']}"
        )
    below = [row for row in rows if row["percent"] is not None and row["percent"] < THRESHOLD_PERCENT]
    below.sort(key=lambda row: (row["percent"] or 0, row["operation"]))
    print(f"operations below {THRESHOLD_PERCENT}%: {len(below)}/{len(rows)}")
    for row in below:
        print(f"  {row['percent']:5.1f}%  {row['operation']}")
    print(f"report written to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
