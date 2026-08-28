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
from contextlib import contextmanager
import fcntl
import hashlib
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
    ROOT / "target" / "llvm-cov-target" / "pillow-rs-%p-%m.raw"
)
DEFAULT_COVERAGE_DATA = ROOT / "target" / "coverage" / ".migration-parity-python-rust"
EXTENSION = (
    ROOT / "pillow-rs-py" / "python" / "pillow_rs" / "_core.abi3.so"
)
LLVM_COV_TARGET = ROOT / "target" / "llvm-cov-target"
COVERAGE_BUILD_STAMP = LLVM_COV_TARGET / ".pillow-rs-coverage-build"
INSTRUMENTED_EXTENSION_NAMES = (
    "lib_core.dylib",
    "lib_core.so",
    "_core.dll",
    "lib_core.dll",
)
TARGET_BACKEND = os.environ.get("MIGRATION_TARGET_BACKEND", "cpu").strip().lower()
if TARGET_BACKEND == "all":
    COVERAGE_BACKENDS = ("cpu", "simd")
elif TARGET_BACKEND == "all-gpu":
    COVERAGE_BACKENDS = ("cpu", "simd", "gpu")
else:
    COVERAGE_BACKENDS = (TARGET_BACKEND,)
COMMAND = {
    "command_id": "coverage-rust",
    "argv": ([f"MIGRATION_TARGET_BACKEND={TARGET_BACKEND}"] if TARGET_BACKEND != "cpu" else [])
    + ["make", "migration-parity-coverage-rust"],
    "cwd": ".",
    "timeout_seconds": 7200,
}


@contextmanager
def coverage_run_lock():
    """Serialize LLVM coverage runs that share the instrumented target tree.

    Coverage MCP may schedule otherwise independent filtered runs at the same
    time.  The instrumented extension, Cargo target, raw profiles, and
    temporary Python extension are intentionally shared by this collector, so
    concurrent runs would delete one another's objects or profiles.  A small
    advisory lock keeps the reusable build cache while making those runs
    deterministic.
    """

    lock_path = ROOT / "target" / ".migration-parity-rust-coverage.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("w", encoding="utf-8") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def coverage_build_inputs() -> list[Path]:
    """Return source/config files that can change the instrumented extension."""

    roots = (
        ROOT / "Cargo.toml",
        ROOT / "Cargo.lock",
        ROOT / "rust-toolchain.toml",
        ROOT / "rust-toolchain",
        ROOT / ".cargo",
        ROOT / "pillow-rs" / "Cargo.toml",
        ROOT / "pillow-rs" / "src",
        ROOT / "pillow-rs-py" / "Cargo.toml",
        ROOT / "pillow-rs-py" / "pyproject.toml",
        ROOT / "pillow-rs-py" / "src",
    )
    files: list[Path] = []
    for root in roots:
        if root.is_file():
            files.append(root)
        elif root.is_dir():
            files.extend(
                path
                for path in root.rglob("*")
                if path.is_file()
                and ".git" not in path.parts
                and "target" not in path.parts
            )
    return sorted(set(files))


def coverage_build_fingerprint() -> str:
    """Hash the instrumented Rust inputs, including coverage build settings."""

    digest = hashlib.sha256()
    digest.update(b"toolchain=nightly\n")
    digest.update(b"rustflags=-Cinstrument-coverage -Zcoverage-options=branch\n")
    digest.update(f"python={sys.executable}\n".encode("utf-8"))
    digest.update(f"python-version={sys.version}\n".encode("utf-8"))
    for path in coverage_build_inputs():
        digest.update(str(path).encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def prepare_llvm_target() -> tuple[str, bool]:
    """Reuse an unchanged instrumented target while clearing only old profiles."""

    fingerprint = coverage_build_fingerprint()
    cached = False
    if LLVM_COV_TARGET.is_dir() and COVERAGE_BUILD_STAMP.is_file():
        cached = COVERAGE_BUILD_STAMP.read_text(encoding="utf-8").strip() == fingerprint
    if not cached:
        if LLVM_COV_TARGET.exists():
            shutil.rmtree(LLVM_COV_TARGET)
        LLVM_COV_TARGET.mkdir(parents=True, exist_ok=True)
    else:
        LLVM_COV_TARGET.mkdir(parents=True, exist_ok=True)

    # Profiles are run-specific evidence. Never let a cached build make a
    # later run accumulate execution counts from an older input corpus.
    for pattern in ("*.raw", "*.profraw", "*.profdata", "*-profraw-list"):
        for stale in LLVM_COV_TARGET.rglob(pattern):
            stale.unlink()
    return fingerprint, cached


def install_instrumented_extension() -> Path:
    """Make the cached LLVM build the active Python extension.

    ``maturin develop --skip-install`` can leave the normal extension in the
    source tree when Cargo reuses a fully fresh cached build.  That makes the
    parity cases pass while silently producing an all-zero Rust profile.  The
    cdylib in the coverage target is the exact module Maturin would place in
    ``pillow_rs``; copy it explicitly on every run so cache hits remain valid.
    """

    candidates = [
        LLVM_COV_TARGET / "debug" / name
        for name in INSTRUMENTED_EXTENSION_NAMES
    ]
    artifact = next((path for path in candidates if path.is_file()), None)
    if artifact is None:
        names = ", ".join(INSTRUMENTED_EXTENSION_NAMES)
        raise RuntimeError(
            f"instrumented extension artifact not found under {LLVM_COV_TARGET / 'debug'} "
            f"(expected one of: {names})"
        )
    shutil.copy2(artifact, EXTENSION)
    return artifact


sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_coverage import (  # noqa: E402
    build_components,
    coverage_identity,
    coverage_not_applicable_operations,
    load_coverage_plans,
    now,
    scoped_coverage_command,
    scope_coverage_plans,
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
    line_counts: dict[int, int] = {}
    for segment in file_entry.get("segments", []):
        if len(segment) <= 3 or not segment[3]:
            continue
        line = int(segment[0])
        line_counts[line] = max(line_counts.get(line, 0), int(segment[2]))
    missing_lines = sorted(line for line, count in line_counts.items() if count == 0)
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


def run_locked(args: argparse.Namespace) -> int:
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    plans, plan_paths = load_coverage_plans(manifest)
    cases, case_inputs = load_cases(manifest, case_ids=None, surface=None)
    cases_by_id = {case["case_id"]: case for case in cases}
    operation_paths = {
        operation["source"]["path"]
        for surface in manifest["surfaces"]
        for operation in surface["operations"]
    }
    if args.operation is not None and args.operation not in operation_paths:
        raise ValueError(f"unknown public operation: {args.operation}")
    plans, selected_ids = scope_coverage_plans(
        plans,
        cases_by_id,
        case_ids=set(args.case_id) if args.case_id else None,
        operation=args.operation,
        exclude_case_ids=set(args.exclude_case_id) if args.exclude_case_id else None,
        excluded_operations=coverage_not_applicable_operations(manifest),
    )
    plan_paths = {plan_id: plan_paths[plan_id] for plan_id in (plan["plan_id"] for plan in plans)}

    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.python_report.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.llvm_report.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.coverage_data.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.profile.resolve().parent.mkdir(parents=True, exist_ok=True)

    # cargo-llvm-cov can discover stale instrumented libraries left in its
    # target directory, even when their old profile data was removed. The
    # fingerprinted cache removes the directory when Rust inputs change, while
    # allowing repeated coverage runs for unchanged code to reuse Cargo's
    # instrumented build.
    build_fingerprint, build_cache_hit = prepare_llvm_target()
    profile_temp_dir: Path | None = None
    if args.profile == DEFAULT_LLVM_PROFILE:
        profile_temp_dir = Path(tempfile.mkdtemp(prefix="pillow-rs-llvm-", dir="/private/tmp"))
        args.profile = profile_temp_dir / DEFAULT_LLVM_PROFILE.name
    if args.profile.exists():
        args.profile.unlink()

    had_extension = EXTENSION.is_file()
    restore_path: Path | None = None
    if had_extension:
        # Keep the backup outside the coverage artifact directory. The managed
        # runner may replace or clean that directory while producing the LLVM
        # report, which previously deleted the backup before this finally
        # block could restore the normal extension.
        with tempfile.NamedTemporaryFile(
            prefix="pillow-rs-core-",
            suffix=".migration-backup",
            delete=False,
        ) as handle:
            restore_path = Path(handle.name)
        shutil.copy2(EXTENSION, restore_path)

    started = now()
    llvm_version = "unknown"

    def remove_build_profiles() -> None:
        # Instrumented build scripts and proc macros with no explicit profile
        # path write default_*.profraw next to their working directory.
        for stale in ROOT.glob("default_*.profraw"):
            stale.unlink()

    def materialize_profiles() -> None:
        """Expose freshly written raw profiles to cargo-llvm-cov.

        Some managed runners remove ``*.profraw`` files between subprocesses
        as disposable instrumentation artifacts. LLVM's runtime does not
        require that suffix, so keep the run files under ``.raw`` while the
        parity subprocesses execute and rename them only at the report
        boundary, where cargo-llvm-cov expects ``*.profraw``.
        """

        if args.profile.suffix != ".raw":
            return
        for raw_profile in args.profile.parent.glob("*.raw"):
            if raw_profile.is_file():
                destination = LLVM_COV_TARGET / raw_profile.with_suffix(".profraw").name
                shutil.move(str(raw_profile), destination)

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
        instrumented_artifact = install_instrumented_extension()
        COVERAGE_BUILD_STAMP.write_text(build_fingerprint + "\n", encoding="utf-8")
        print(
            f"coverage build cache: {'hit' if build_cache_hit else 'miss'} "
            f"({instrumented_artifact})"
        )
        remove_build_profiles()

        for stale in LLVM_COV_TARGET.glob("*.profraw"):
            stale.unlink()
        run_env = os.environ.copy()
        target_python = str(ROOT / "pillow-rs-py" / "python")
        run_env["PYTHONPATH"] = target_python + os.pathsep + run_env.get("PYTHONPATH", "")
        run_env["LLVM_PROFILE_FILE"] = str(args.profile)
        # Exercise the legacy FreeTypeFont core variants that the ordinary
        # parity facade does not select (getlength, getmask2_with_start,
        # native_getvaraxes, native_getvarnames, native_setvaraxes,
        # native_setvarname, ...) through the maintained input-only corpus.
        # Keep this in the public Python surface: non-parity harnesses must not inflate
        # migration coverage.
        selected_command_ids = {
            command_id
            for plan in plans
            for command_id in plan["selectors"]["command_ids"]
        }
        # The canonical lane historically includes the maintained font-native
        # corpus even though the generated input plans intentionally keep
        # command_ids empty. Scoped operation lanes must not inherit that
        # component exercise, or their operation evidence would be inflated.
        run_font_native = (
            (args.operation is None and not args.case_id)
            or "coverage-font-native" in selected_command_ids
        )
        # The canonical full lane also includes the maintained image-core
        # native corpus. These are supported public `pillow_rs` inputs that
        # have no Pillow parity endpoint, so they must be measured through the
        # instrumented extension rather than silently omitted from Rust
        # coverage. Scoped operation/case runs intentionally remain
        # operation-attributable and do not inherit this supplement.
        canonical_full_lane = args.operation is None and not args.case_id
        native_supplement_scripts: list[str] = []
        native_supplements = {
            "coverage-imagecore-native": "run_migration_imagecore_native_cases.py",
            "coverage-imageops-native": "run_migration_imageops_native_cases.py",
            "coverage-imagesequence-native": "run_migration_imagesequence_native_cases.py",
            "coverage-imagedraw-native": "run_migration_imagedraw_native_cases.py",
            "coverage-imagecolor-native": "run_migration_imagecolor_native_cases.py",
            "coverage-imagepalette-native": "run_migration_imagepalette_native_cases.py",
        }
        for command_id, script_name in native_supplements.items():
            if canonical_full_lane or command_id in selected_command_ids:
                native_supplement_scripts.append(script_name)
        child_results: list[dict[str, Any]] = []
        python_data_paths: list[Path] = []
        for backend in COVERAGE_BACKENDS:
            child_output = (
                ROOT
                / "target"
                / "coverage"
                / f"child-coverage-result-{backend}.json"
            )
            child_output.unlink(missing_ok=True)
            backend_env = {**run_env, "MIGRATION_TARGET_BACKEND": backend}
            backend_coverage_data = args.coverage_data.with_name(
                f"{args.coverage_data.name}-{backend}"
            )
            backend_coverage_data.unlink(missing_ok=True)
            python_data_paths.append(backend_coverage_data)
            subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts" / "run_migration_coverage.py"),
                    "--output",
                    str(child_output),
                    "--coverage-report",
                    str(args.python_report),
                    "--coverage-data",
                    str(backend_coverage_data),
                ]
                + (["--operation", args.operation] if args.operation else [])
                + sum((["--case-id", case_id] for case_id in (args.case_id or [])), [])
                + sum(
                    (["--exclude-case-id", case_id] for case_id in (args.exclude_case_id or [])),
                    [],
                ),
                env=backend_env,
                cwd=ROOT,
                check=True,
            )
            child_results.append(json.loads(child_output.read_text(encoding="utf-8")))
            child_output.unlink()

            if run_font_native:
                subprocess.run(
                    [
                        sys.executable,
                        str(ROOT / "scripts" / "run_migration_font_native_cases.py"),
                    ],
                    env={
                        **backend_env,
                        "RUSTFLAGS": "-Cinstrument-coverage -Zcoverage-options=branch",
                        "LLVM_PROFILE_FILE": str(args.profile),
                    },
                    cwd=ROOT,
                    check=True,
                )

            for script_name in native_supplement_scripts:
                subprocess.run(
                    [
                        sys.executable,
                        str(ROOT / "scripts" / script_name),
                    ],
                    env={
                        **backend_env,
                        "RUSTFLAGS": "-Cinstrument-coverage -Zcoverage-options=branch",
                        "LLVM_PROFILE_FILE": str(args.profile),
                    },
                    cwd=ROOT,
                    check=True,
                )

            materialize_profiles()

        child = child_results[-1]
        if any(
            item["summary"]["plans_selected"] != child["summary"]["plans_selected"]
            or item["summary"]["plans_executed"] != child["summary"]["plans_executed"]
            for item in child_results
        ):
            raise RuntimeError("combined coverage backends selected different coverage plans")

        combined_python = coverage.Coverage(
            data_file=str(args.coverage_data.resolve()),
            branch=True,
            source=[str((ROOT / "pillow-rs-py" / "python" / "pillow_rs").resolve())],
        )
        combined_python.erase()
        combined_python.combine(
            data_paths=[str(path.resolve()) for path in python_data_paths],
            strict=True,
        )
        combined_python.save()
        combined_python.json_report(
            outfile=str(args.python_report.resolve()), pretty_print=True
        )

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
            # cargo-llvm-cov's default target root is ``target/llvm-cov-target``;
            # setting CARGO_TARGET_DIR here would make it append that directory
            # a second time while locating the freshly emitted profiles.
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
        command = scoped_coverage_command(
            COMMAND,
            operation=args.operation,
            case_ids=args.case_id,
            exclude_case_ids=args.exclude_case_id,
        )
        identity = coverage_identity(
            manifest_path,
            input_paths,
            command,
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
        if had_extension and restore_path is not None:
            if not restore_path.is_file():
                raise RuntimeError(
                    f"missing extension backup during coverage cleanup: {restore_path}"
                )
            shutil.copy2(restore_path, EXTENSION)
            restore_path.unlink()
            print(f"restored extension: {EXTENSION}")
        remove_build_profiles()
        if profile_temp_dir is not None:
            shutil.rmtree(profile_temp_dir, ignore_errors=True)
    return 0


def run(args: argparse.Namespace) -> int:
    with coverage_run_lock():
        return run_locked(args)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--operation")
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--exclude-case-id", action="append")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--python-report", type=Path, default=DEFAULT_PYTHON_REPORT)
    parser.add_argument("--llvm-report", type=Path, default=DEFAULT_LLVM_REPORT)
    parser.add_argument("--profile", type=Path, default=DEFAULT_LLVM_PROFILE)
    parser.add_argument("--coverage-data", type=Path, default=DEFAULT_COVERAGE_DATA)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
