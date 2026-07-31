#!/usr/bin/env python3
"""Collect target coverage for the fixed migration-parity coverage plans.

This lane is intentionally separate from parity comparison.  It executes the
selected public workflows only through the target facade under coverage.py,
then writes a strict ``coverage-result@1`` plus the coverage.py report used by
the managed coverage collector.  Rust files are reported as not instrumented
by this Python collector; they are never silently counted as covered.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import platform
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid
from typing import Any

import coverage

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "coverage-result.json"
DEFAULT_REPORT = ROOT / "target" / "coverage" / "migration-parity-python.json"
TARGET_PROFILE = "python-cpu"
TARGET_ID = "pillow-rs-python"
ORACLE_VERSION = "12.2.0"

sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_parity import (  # noqa: E402
    AssetStore,
    ENCODED_INPUTS,
    build_operation_index,
    git_dirty,
    git_revision,
    load_cases,
    load_manifest,
    run_case,
    sha256_bytes,
    sha256_file,
)


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def load_coverage_plans(manifest: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, str]]:
    from validate_migration_parity_contract import validate_inputs

    validate_inputs(manifest, FIXTURE_ROOT)
    plans: list[dict[str, Any]] = []
    paths: dict[str, str] = {}
    for relative in manifest["input_index"]["coverage"]:
        path = FIXTURE_ROOT / relative
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("schema") != "migration-parity/coverage-input@1":
            raise ValueError(f"{relative}: invalid coverage input schema")
        for plan in payload["plans"]:
            if plan["target_profile"] != TARGET_PROFILE:
                raise ValueError(f"{plan['plan_id']}: unexpected target profile")
            plans.append(plan)
            paths[plan["plan_id"]] = relative
    return plans, paths


def coverage_identity(
    manifest_path: Path,
    input_paths: list[str],
    command: dict[str, Any],
    selected_cases: list[dict[str, Any]],
    case_inputs: dict[str, str],
) -> dict[str, Any]:
    manifest = load_manifest(manifest_path)
    assets: list[dict[str, Any]] = []
    for case in selected_cases:
        for asset in case.get("assets", []):
            kind = asset["kind"]
            if kind == "ref":
                locator = asset["path"]
                digest = asset.get("sha256")
            elif kind == "inline":
                locator = None
                digest = asset.get("sha256")
            elif kind == "builtin":
                locator = asset.get("name")
                digest = (
                    sha256_bytes(ENCODED_INPUTS[asset["name"]])
                    if asset.get("name") in ENCODED_INPUTS
                    else None
                )
            else:
                locator = asset.get("path")
                digest = None
            assets.append(
                {
                    "input_path": case_inputs[case["case_id"]],
                    "item_id": case["case_id"],
                    "asset_id": asset["id"],
                    "kind": kind,
                    "locator": locator,
                    "sha256": digest,
                }
            )
    return {
        "run_id": f"migration-coverage-{uuid.uuid4().hex}",
        "started_at": now(),
        "finished_at": now(),
        "manifest": {
            "path": str(manifest_path.relative_to(ROOT)),
            "schema": manifest["schema"],
            "sha256": sha256_file(manifest_path),
        },
        "inputs": [
            {
                "path": path,
                "schema": (
                    "migration-parity/coverage-input@1"
                    if "/coverage/" in path
                    else "migration-parity/parity-input@1"
                ),
                "sha256": sha256_file(FIXTURE_ROOT / path),
            }
            for path in input_paths
        ],
        "assets": assets,
        "oracles": [
            {
                "oracle_id": "pillow",
                "name": "Pillow",
                "version": ORACLE_VERSION,
                "runtime": "CPython 3.12",
            }
        ],
        "targets": [
            {
                "target_profile": TARGET_PROFILE,
                "target_id": TARGET_ID,
                "revision": git_revision(),
                "dirty": git_dirty(),
                "runtime": platform.python_version(),
                "backend": "cpu",
                "features": ["all-features"],
            }
        ],
        "command": command,
    }


def report_files(report: dict[str, Any]) -> dict[Path, dict[str, Any]]:
    result: dict[Path, dict[str, Any]] = {}
    for raw_path, data in report.get("files", {}).items():
        path = Path(raw_path)
        if not path.is_absolute():
            path = (ROOT / path).resolve()
        result[path] = data
    return result


def file_dimensions(path: Path, data: dict[str, Any] | None) -> list[dict[str, Any]]:
    if data is None:
        return [
            {
                "dimension": dimension,
                "covered": 0,
                "total": 0,
                "uncovered": [],
            }
            for dimension in ("function", "line", "branch", "region")
        ]
    summary = data.get("summary", {})

    def uncovered(values: list[Any]) -> list[int]:
        """Normalize coverage.py/llvm-cov gap locations to line numbers.

        coverage.py emits branch gaps as ``[from, to]`` pairs; llvm-cov emits
        plain line numbers.  The strict result contract only stores
        repository-native locations, so pairs are reduced to their start line.
        """

        result: set[int] = set()
        for value in values:
            if isinstance(value, (list, tuple)):
                value = value[0] if value else 0
            result.add(int(value))
        return sorted(result)

    return [
        {
            "dimension": "function",
            "covered": int(summary.get("covered_functions", 0)),
            "total": int(summary.get("num_functions", 0)),
            "uncovered": [],
        },
        {
            "dimension": "line",
            "covered": int(summary.get("covered_lines", 0)),
            "total": int(summary.get("num_statements", 0)),
            "uncovered": uncovered(data.get("missing_lines", [])),
        },
        {
            "dimension": "branch",
            "covered": int(summary.get("covered_branches", 0)),
            "total": int(summary.get("num_branches", 0)),
            "uncovered": uncovered(data.get("missing_branches", [])),
        },
        # coverage.py has no region metric; the Rust LLVM lane supplies it.
        # A zero-total dimension is marked not_proven below rather than being
        # treated as a passing 100% rate.
        {
            "dimension": "region",
            "covered": int(summary.get("covered_regions", 0)),
            "total": int(summary.get("num_regions", 0)),
            "uncovered": uncovered(data.get("missing_regions", [])),
        },
    ]


def threshold_outcome(
    dimension: dict[str, Any], minimum_percent: int
) -> str:
    total = dimension["total"]
    if total == 0:
        return "not_proven"
    return (
        "pass"
        if dimension["covered"] * 100 >= total * minimum_percent
        else "fail"
    )


def coverage_case_failed(observations: list[dict[str, Any]]) -> bool:
    """Return whether a workflow failed to execute its coverage contract.

    Public errors are valid observations in the migration spec.  When a
    workflow intentionally exercises an error, the runner blocks dependent
    steps and records them as ``not_run``; those blocked steps do not make the
    case a coverage execution failure.  A not-run observation with no earlier
    public error still indicates an incomplete workflow and remains failed.
    """
    if not observations:
        return True
    saw_public_error = False
    for observation in observations:
        status = observation["status"]
        if status == "error":
            saw_public_error = True
        elif status == "not_run" and not saw_public_error:
            return True
    return False


def build_components(
    plan: dict[str, Any],
    component_index: dict[str, dict[str, Any]],
    files: dict[Path, dict[str, Any]],
) -> list[dict[str, Any]]:
    components: list[dict[str, Any]] = []
    for component_id in plan["component_ids"]:
        component = component_index[component_id]
        file_results: list[dict[str, Any]] = []
        for relative in component["paths"]:
            path = (ROOT / relative).resolve()
            data = files.get(path)
            file_results.append(
                {"path": relative, "dimensions": file_dimensions(path, data)}
            )
        thresholds: list[dict[str, Any]] = []
        for threshold in component["thresholds"]:
            dimensions = [
                dimension
                for item in file_results
                for dimension in item["dimensions"]
                if dimension["dimension"] == threshold["dimension"]
            ]
            covered = sum(item["covered"] for item in dimensions)
            total = sum(item["total"] for item in dimensions)
            aggregate = {
                "dimension": threshold["dimension"],
                "minimum_percent": int(threshold["minimum_percent"]),
                "covered": covered,
                "total": total,
                "outcome": threshold_outcome(
                    {"covered": covered, "total": total},
                    int(threshold["minimum_percent"]),
                ),
            }
            thresholds.append(aggregate)
        components.append(
            {
                "component_id": component_id,
                "files": file_results,
                "thresholds": thresholds,
            }
        )
    return components


def build_plan_result(
    plan: dict[str, Any],
    plan_input_path: str,
    case_results: dict[str, dict[str, Any]],
    component_index: dict[str, dict[str, Any]],
    files: dict[Path, dict[str, Any]],
    command_totals: dict[str, tuple[int, int]] | None = None,
) -> dict[str, Any]:
    selected_ids = list(plan["selectors"]["parity_case_ids"])
    tests_failed = sum(
        1
        for case_id in selected_ids
        if coverage_case_failed(case_results[case_id]["observations"])
    )
    command_totals = command_totals or {}
    tests_passed = len(selected_ids) - tests_failed
    for command_id in plan["selectors"]["command_ids"]:
        command_passed, command_failed = command_totals.get(command_id, (0, 0))
        tests_passed += command_passed
        tests_failed += command_failed
    return {
        "plan_id": plan["plan_id"],
        "target_profile": plan["target_profile"],
        "requirements": plan["covers"],
        "selected": {
            "parity_case_ids": selected_ids,
            "command_ids": plan["selectors"]["command_ids"],
        },
        "execution": {
            "status": "completed",
            "tests_passed": tests_passed,
            "tests_failed": tests_failed,
        },
        "components": build_components(plan, component_index, files),
    }


def run(args: argparse.Namespace) -> int:
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    plans, plan_paths = load_coverage_plans(manifest)
    cases, case_inputs = load_cases(manifest, case_ids=None, surface=None)
    cases_by_id = {case["case_id"]: case for case in cases}
    selected_ids = {
        case_id
        for plan in plans
        for case_id in plan["selectors"]["parity_case_ids"]
    }
    missing = selected_ids - cases_by_id.keys()
    if missing:
        raise ValueError(f"coverage selects missing parity cases: {sorted(missing)[:5]}")
    operation_index = build_operation_index(manifest)
    args.coverage_report.resolve().parent.mkdir(parents=True, exist_ok=True)
    cov = coverage.Coverage(
        data_file=str(args.coverage_data.resolve()),
        branch=True,
        source=[str((ROOT / "pillow-rs-py" / "python" / "pillow_rs").resolve())],
    )
    started = now()
    case_results: dict[str, dict[str, Any]] = {}
    with tempfile.TemporaryDirectory(prefix="migration-coverage-target-") as temporary:
        tempdir = Path(temporary)
        cov.start()
        try:
            for case_id in sorted(selected_ids):
                case_results[case_id] = run_case(
                    "target", cases_by_id[case_id], operation_index, tempdir
                )
            command_totals: dict[str, tuple[int, int]] = {}
            for plan in plans:
                for command_id in plan["selectors"]["command_ids"]:
                    if command_id in command_totals:
                        continue
                    if command_id == "coverage-font-native":
                        from run_migration_font_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imageops-native":
                        from run_migration_imageops_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imagesequence-native":
                        from run_migration_imagesequence_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imagecore-native":
                        from run_migration_imagecore_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imagedraw-native":
                        from run_migration_imagedraw_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imagecolor-native":
                        from run_migration_imagecolor_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    elif command_id == "coverage-imagepalette-native":
                        from run_migration_imagepalette_native_cases import run_native_cases

                        passed, _skipped, failed = run_native_cases()
                    else:
                        raise ValueError(f"unknown coverage command: {command_id}")
                    command_totals[command_id] = (passed, failed)
        finally:
            cov.stop()
            cov.save()
    cov.json_report(outfile=str(args.coverage_report.resolve()), pretty_print=True)
    report = json.loads(args.coverage_report.read_text(encoding="utf-8"))
    files = report_files(report)
    component_index = {
        component["id"]: component
        for component in manifest["coverage_components"]
    }
    input_paths = sorted(
        set(plan_paths.values())
        | {
            case_inputs[case_id]
            for case_id in selected_ids
        }
    )
    command = {
        "command_id": "coverage",
        "argv": ["make", "migration-parity-coverage"],
        "cwd": ".",
        "timeout_seconds": 3600,
    }
    identity = coverage_identity(
        manifest_path,
        input_paths,
        command,
        [cases_by_id[case_id] for case_id in sorted(selected_ids)],
        case_inputs,
    )
    identity["started_at"] = started
    identity["finished_at"] = now()
    plan_results = [
        build_plan_result(
            plan,
            plan_paths[plan["plan_id"]],
            case_results,
            component_index,
            files,
            command_totals,
        )
        for plan in plans
    ]
    tests_passed = sum(item["execution"]["tests_passed"] for item in plan_results)
    tests_failed = sum(item["execution"]["tests_failed"] for item in plan_results)
    result = {
        "schema": "migration-parity/coverage-result@1",
        "identity": identity,
        "status": "completed",
        "collector": {
            "name": "coverage.py",
            "version": coverage.__version__,
            "snapshot_id": None,
            "artifact_ingested": False,
        },
        "summary": {
            "plans_selected": len(plans),
            "plans_executed": len(plans),
            "plans_not_run": 0,
            "tests_passed": tests_passed,
            "tests_failed": tests_failed,
        },
        "plans": plan_results,
        "infrastructure_errors": [],
    }
    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.output.resolve().write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result["summary"], sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--coverage-report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument(
        "--coverage-data",
        type=Path,
        default=ROOT / "target" / "coverage" / ".migration-parity-python",
    )
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
