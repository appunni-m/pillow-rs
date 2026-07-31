#!/usr/bin/env python3
"""Collect merged Python + Rust coverage for the fixed migration-parity plans.

This lane is the maintained counterpart of ``run_migration_coverage.py`` for
the manifest-declared Rust component paths.  It builds a temporary nightly
LLVM-instrumented copy of the PyO3 extension, executes the indexed coverage
workflows through the same public target facade (also under coverage.py for
the Python wrapper files), and emits a strict ``coverage-result@1`` artifact
with per-file function/line/branch/region dimensions for every
manifest-declared component path.  The original extension is restored
afterwards, so the workspace is not left in an instrumented state.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from typing import Any

import coverage

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "coverage-result-rust.json"
DEFAULT_PYTHON_REPORT = ROOT / "target" / "coverage" / "migration-parity-python.json"
DEFAULT_LLVM_REPORT = ROOT / "target" / "coverage" / "migration-parity-rust.json"
DEFAULT_LLVM_PROFILE = (
    ROOT / "target" / "llvm-cov-target" / "pillow-rs-%p-%m.profraw"
)
DEFAULT_COVERAGE_DATA = ROOT / "target" / "coverage" / ".migration-parity-python-rust"
EXTENSION = (
    ROOT / "pillow-rs-py" / "python" / "pillow_rs" / "_core.abi3.so"
)
LLVM_COV_TARGET = ROOT / "target" / "llvm-cov-target"
COMMAND = {
    "command_id": "coverage-rust",
    "argv": ["make", "migration-parity-coverage-rust"],
    "cwd": ".",
    "timeout_seconds": 7200,
}

sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_coverage import (  # noqa: E402
    build_components,
    coverage_identity,
    load_coverage_plans,
    now,
)
from run_migration_parity import (  # noqa: E402
    load_cases,
    load_manifest,
)


def llvm_shape(file_entry: dict[str, Any]) -> dict[str, Any]:
    """Map one llvm-cov file entry into the shared coverage.py-shaped record.

    The shared ``file_dimensions`` in ``run_migration_coverage.py`` reads
    ``covered_*``/``num_*`` summary keys plus ``missing_*`` location lists, so
    this shape lets one result builder serve both collectors.
    """

    summary = file_entry.get("summary", {})

    def counts(key: str) -> tuple[int, int]:
        value = summary.get(key) or {}
        return int(value.get("covered", 0)), int(value.get("count", 0))

    covered_functions, num_functions = counts("functions")
    covered_lines, num_lines = counts("lines")
    covered_branches, num_branches = counts("branches")
    covered_regions, num_regions = counts("regions")

    # llvm-cov export uses positional arrays: segments are
    # [line, col, count, has_count, is_region_entry, is_gap_region] and
    # branches are [line, col, src_line, src_col, true_count, false_count, ...].
    # A zero true/false count means that branch arm was not executed.
    missing_lines = sorted(
        {
            int(segment[0])
            for segment in file_entry.get("segments", [])
            if len(segment) > 3 and segment[3] and int(segment[2]) == 0
        }
    )
    missing_branches = sorted(
        {
            int(branch[0])
            for branch in file_entry.get("branches", [])
            if len(branch) > 4
            and (int(branch[4]) == 0 or (len(branch) > 5 and int(branch[5]) == 0))
        }
    )
    missing_regions = missing_lines
    return {
        "summary": {
            "covered_functions": covered_functions,
            "num_functions": num_functions,
            "covered_lines": covered_lines,
            "num_statements": num_lines,
            "covered_branches": covered_branches,
            "num_branches": num_branches,
            "covered_regions": covered_regions,
            "num_regions": num_regions,
        },
        "missing_lines": missing_lines,
        "missing_branches": missing_branches,
        "missing_regions": missing_regions,
    }


def merged_file_data(
    python_files: dict[Path, dict[str, Any]],
    llvm_files: dict[Path, dict[str, Any]],
    path: Path,
) -> dict[str, Any]:
    """Pick the authoritative collector for one declared component path."""

    if path.suffix == ".py":
        return python_files.get(path) or {
            "summary": {},
            "missing_lines": [],
            "missing_branches": [],
            "missing_regions": [],
        }
    llvm_entry = llvm_files.get(path)
    if llvm_entry is not None:
        return llvm_shape(llvm_entry)
    return {
        "summary": {},
        "missing_lines": [],
        "missing_branches": [],
        "missing_regions": [],
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

    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.python_report.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.llvm_report.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.profile.resolve().parent.mkdir(parents=True, exist_ok=True)
    if args.profile.exists():
        args.profile.unlink()

    had_extension = EXTENSION.is_file()
    restore_path = ROOT / "target" / "coverage" / "_core.abi3.so.migration-backup"
    if had_extension:
        shutil.copy2(EXTENSION, restore_path)

    started = now()
    llvm_version = "unknown"

    def remove_build_profiles() -> None:
        # Instrumented build scripts and proc macros with no explicit profile
        # path write default_*.profraw next to their working directory.
        for stale in ROOT.glob("default_*.profraw"):
            stale.unlink()

    try:
        build_env = os.environ.copy()
        build_env["RUSTUP_TOOLCHAIN"] = "nightly"
        build_env["RUSTFLAGS"] = "-Cinstrument-coverage -Zcoverage-options=branch"
        build_env["LLVM_PROFILE_FILE"] = str(args.profile)
        args.profile.resolve().parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                sys.executable,
                "-m",
                "maturin",
                "develop",
                "--skip-install",
                "--target-dir",
                str(LLVM_COV_TARGET),
                "--manifest-path",
                str(ROOT / "pillow-rs-py" / "Cargo.toml"),
            ],
            env=build_env,
            cwd=ROOT,
            check=True,
        )
        remove_build_profiles()

        for stale in LLVM_COV_TARGET.glob("*.profraw"):
            stale.unlink()
        run_env = os.environ.copy()
        run_env["LLVM_PROFILE_FILE"] = str(args.profile)
        child_output = ROOT / "target" / "coverage" / "child-coverage-result.json"
        subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "run_migration_coverage.py"),
                "--output",
                str(child_output),
                "--coverage-report",
                str(args.python_report),
                "--coverage-data",
                str(args.coverage_data),
            ],
            env=run_env,
            cwd=ROOT,
            check=True,
        )
        child = json.loads(child_output.read_text(encoding="utf-8"))
        child_output.unlink()

        llvm_version = subprocess.run(
            ["cargo", "+nightly", "llvm-cov", "--version"],
            capture_output=True,
            text=True,
            check=True,
            cwd=ROOT,
        ).stdout.strip().splitlines()[0]
        subprocess.run(
            [
                "cargo",
                "+nightly",
                "llvm-cov",
                "report",
                "--branch",
                "--json",
                "--output-path",
                str(args.llvm_report),
            ],
            env={**os.environ, "RUSTUP_TOOLCHAIN": "nightly"},
            cwd=ROOT,
            check=True,
        )

        python_report = json.loads(args.python_report.read_text(encoding="utf-8"))
        llvm_report = json.loads(args.llvm_report.read_text(encoding="utf-8"))
        python_files: dict[Path, dict[str, Any]] = {}
        for raw_path, data in python_report.get("files", {}).items():
            path = Path(raw_path)
            if not path.is_absolute():
                path = (ROOT / path).resolve()
            python_files[path] = data
        llvm_files: dict[Path, dict[str, Any]] = {}
        for data in llvm_report.get("data", []):
            for file_entry in data.get("files", []):
                llvm_files[Path(file_entry["filename"]).resolve()] = file_entry

        component_index = {
            component["id"]: component
            for component in manifest["coverage_components"]
        }
        files = {
            (ROOT / path).resolve(): merged_file_data(
                python_files, llvm_files, (ROOT / path).resolve()
            )
            for component in manifest["coverage_components"]
            for path in component["paths"]
        }
        input_paths = sorted(
            set(plan_paths.values())
            | {case_inputs[case_id] for case_id in selected_ids}
        )
        identity = coverage_identity(
            manifest_path,
            input_paths,
            COMMAND,
            [cases_by_id[case_id] for case_id in sorted(selected_ids)],
            case_inputs,
        )
        identity["started_at"] = started
        identity["finished_at"] = now()
        plan_results = []
        for plan in plans:
            child_plan = next(
                item
                for item in child["plans"]
                if item["plan_id"] == plan["plan_id"]
            )
            plan_results.append(
                {
                    "plan_id": plan["plan_id"],
                    "target_profile": plan["target_profile"],
                    "requirements": plan["covers"],
                    "selected": child_plan["selected"],
                    "execution": child_plan["execution"],
                    "components": build_components(plan, component_index, files),
                }
            )
        result = {
            "schema": "migration-parity/coverage-result@1",
            "identity": identity,
            "status": "completed",
            "collector": {
                "name": "coverage.py + cargo-llvm-cov",
                "version": f"{coverage.__version__} + {llvm_version}",
                "snapshot_id": None,
                "artifact_ingested": False,
            },
            "summary": {
                "plans_selected": child["summary"]["plans_selected"],
                "plans_executed": child["summary"]["plans_executed"],
                "plans_not_run": child["summary"]["plans_not_run"],
                "tests_passed": child["summary"]["tests_passed"],
                "tests_failed": child["summary"]["tests_failed"],
            },
            "plans": plan_results,
            "infrastructure_errors": [],
        }
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result["summary"], sort_keys=True))
    finally:
        if had_extension:
            shutil.copy2(restore_path, EXTENSION)
            restore_path.unlink()
            print(f"restored extension: {EXTENSION}")
        remove_build_profiles()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--python-report", type=Path, default=DEFAULT_PYTHON_REPORT)
    parser.add_argument("--llvm-report", type=Path, default=DEFAULT_LLVM_REPORT)
    parser.add_argument("--profile", type=Path, default=DEFAULT_LLVM_PROFILE)
    parser.add_argument("--coverage-data", type=Path, default=DEFAULT_COVERAGE_DATA)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
