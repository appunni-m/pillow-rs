#!/usr/bin/env python3
"""Run correctness-gated benchmark workloads from the fixed input contract.

The benchmark lane never stores timings in active inputs.  It first runs the
live parity command and measures only workloads whose exact parity case passed.
Source and target timings are collected in separate adapter processes from the
same workflow definitions, then emitted as a strict
``migration-parity/benchmark-result@1`` artifact.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import uuid
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "benchmark-result.json"
DEFAULT_PARITY_RESULT = ROOT / "build" / "migration-parity" / "parity-result.json"
TARGET_PROFILE = "python-cpu"
TARGET_ID = "pillow-rs-python"
ORACLE_VERSION = "12.2.0"


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def sha256(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_revision() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_dirty() -> bool:
    try:
        return bool(
            subprocess.check_output(
                ["git", "status", "--porcelain"], cwd=ROOT, text=True
            ).strip()
        )
    except (OSError, subprocess.CalledProcessError):
        return True


def load_manifest(path: Path) -> dict[str, Any]:
    import yaml

    from validate_migration_parity_contract import validate_manifest

    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    return validate_manifest(manifest, manifest_path=path)


def load_parity_cases(manifest: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, str]]:
    from validate_migration_parity_contract import validate_inputs

    validate_inputs(manifest, FIXTURE_ROOT)
    cases: dict[str, dict[str, Any]] = {}
    inputs: dict[str, str] = {}
    for relative in manifest["input_index"]["parity"]:
        payload = json.loads((FIXTURE_ROOT / relative).read_text(encoding="utf-8"))
        if payload.get("schema") != "migration-parity/parity-input@1":
            raise ValueError(f"{relative}: invalid parity input schema")
        for case in payload["cases"]:
            if case["case_id"] in cases:
                raise ValueError(f"duplicate parity case: {case['case_id']}")
            cases[case["case_id"]] = case
            inputs[case["case_id"]] = relative
    return cases, inputs


def load_benchmarks(manifest: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]], dict[str, str]]:
    workloads: dict[str, dict[str, Any]] = {}
    suites: list[dict[str, Any]] = []
    inputs: dict[str, str] = {}
    for relative in manifest["input_index"]["benchmark"]:
        payload = json.loads((FIXTURE_ROOT / relative).read_text(encoding="utf-8"))
        if payload.get("schema") != "migration-parity/benchmark-input@1":
            raise ValueError(f"{relative}: invalid benchmark input schema")
        for workload in payload["workloads"]:
            if workload["workload_id"] in workloads:
                raise ValueError(f"duplicate benchmark workload: {workload['workload_id']}")
            workloads[workload["workload_id"]] = workload
            inputs[workload["workload_id"]] = relative
        suites.extend(payload.get("suites", []))
    return workloads, suites, inputs


def run_parity(
    manifest: Path, output: Path, timeout: int
) -> dict[str, Any]:
    script = ROOT / "scripts" / "run_migration_parity.py"
    output.parent.mkdir(parents=True, exist_ok=True)
    process = subprocess.run(
        [sys.executable, str(script), "--manifest", str(manifest), "--output", str(output)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
        env={
            **os.environ,
            "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
            + os.pathsep
            + os.environ.get("PYTHONPATH", ""),
        },
    )
    if not output.is_file():
        detail = process.stderr.strip().replace("\n", " ")[-800:]
        raise RuntimeError(f"parity preflight did not emit a result: {detail}")
    result = json.loads(output.read_text(encoding="utf-8"))
    if result.get("status") == "infrastructure_failed":
        raise RuntimeError("parity preflight failed infrastructure checks")
    return result


def run_timed_side(
    side: str,
    manifest: Path,
    cases: list[dict[str, Any]],
    repeat: int,
    timeout: int,
) -> dict[str, Any]:
    script = ROOT / "scripts" / "run_migration_parity.py"
    process = subprocess.run(
        [
            sys.executable,
            str(script),
            "--side",
            side,
            "--manifest",
            str(manifest),
            "--repeat",
            str(repeat),
            "--timings",
            "--timing-step",
            "call",
        ],
        input=json.dumps(cases, separators=(",", ":")),
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
        env={
            **os.environ,
            "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
            + os.pathsep
            + os.environ.get("PYTHONPATH", ""),
        },
    )
    if process.returncode != 0:
        detail = process.stderr.strip().replace("\n", " ")[-800:]
        raise RuntimeError(f"{side} benchmark adapter failed: {detail}")
    payload = json.loads(process.stdout)
    if set(payload) != {"identity", "results", "timings_ns"}:
        raise RuntimeError(f"{side} benchmark adapter emitted invalid timing envelope")
    return payload


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    index = min(len(ordered) - 1, max(0, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def statistics_for(values_ms: list[float], metric: str) -> dict[str, Any]:
    if metric == "throughput":
        values = [1000.0 / value for value in values_ms if value > 0]
        unit = "operations_per_second"
    else:
        values = values_ms
        unit = "millisecond"
    if not values:
        return {
            "metric": metric,
            "unit": unit,
            "sample_count": 0,
            "statistics": {
                "min": None,
                "median": None,
                "mean": None,
                "p95": None,
                "p99": None,
                "max": None,
                "total": None,
                "weighted_mean": None,
                "standard_deviation": None,
            },
            "raw_samples_ref": None,
        }
    return {
        "metric": metric,
        "unit": unit,
        "sample_count": len(values),
        "statistics": {
            "min": min(values),
            "median": statistics.median(values),
            "mean": statistics.mean(values),
            "p95": percentile(values, 0.95),
            "p99": percentile(values, 0.99),
            "max": max(values),
            "total": sum(values),
            "weighted_mean": None,
            "standard_deviation": statistics.pstdev(values) if len(values) > 1 else 0.0,
        },
        "raw_samples_ref": None,
    }


def subject_result(
    subject_kind: str,
    subject_id: str,
    durations_ns: list[int],
    policy: dict[str, Any],
) -> dict[str, Any]:
    warmup = int(policy["warmup_iterations"])
    iterations = int(policy["measurement_iterations"])
    samples = int(policy["samples"])
    expected = warmup + iterations * samples
    measured = durations_ns[warmup:expected]
    if len(measured) != iterations * samples:
        return {
            "kind": subject_kind,
            "id": subject_id,
            "status": "failed",
            "measurements": [],
        }
    values_ms = [value / 1_000_000 for value in measured]
    return {
        "kind": subject_kind,
        "id": subject_id,
        "status": "completed",
        "measurements": [
            statistics_for(values_ms, metric) for metric in policy["metrics"]
        ],
    }


def identity(
    manifest: Path,
    input_paths: list[str],
    cases: list[dict[str, Any]],
    case_inputs: dict[str, str],
    parity_identity: dict[str, Any],
) -> dict[str, Any]:
    assets = [
        item
        for item in parity_identity.get("assets", [])
        if item["item_id"] in {case["case_id"] for case in cases}
    ]
    return {
        "run_id": f"migration-benchmark-{uuid.uuid4().hex}",
        "started_at": now(),
        "finished_at": now(),
        "manifest": {
            "path": str(manifest.relative_to(ROOT)),
            "schema": "migration-parity/manifest@2",
            "sha256": sha256(manifest),
        },
        "inputs": [
            {
                "path": path,
                "schema": (
                    "migration-parity/benchmark-input@1"
                    if "/benchmark/" in path
                    else "migration-parity/parity-input@1"
                ),
                "sha256": sha256(FIXTURE_ROOT / path),
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
        "command": {
            "command_id": "benchmark",
            "argv": ["make", "migration-parity-benchmark"],
            "cwd": ".",
            "timeout_seconds": 7200,
        },
    }


def run(args: argparse.Namespace) -> int:
    manifest = args.manifest.resolve()
    manifest_data = load_manifest(manifest)
    cases_by_id, case_inputs = load_parity_cases(manifest_data)
    workloads, suites_input, workload_inputs = load_benchmarks(manifest_data)
    parity_output = args.parity_output.resolve()
    parity = run_parity(manifest, parity_output, args.timeout)
    parity_by_case = {item["case_id"]: item for item in parity["comparisons"]}
    selected_workloads = list(workloads.values())
    if args.workload_id:
        selected_workloads = [
            workloads[item] for item in args.workload_id if item in workloads
        ]
        missing_workloads = [item for item in args.workload_id if item not in workloads]
        if missing_workloads:
            raise ValueError(f"unknown benchmark workload: {missing_workloads[0]}")
    if args.limit is not None:
        selected_workloads = selected_workloads[: args.limit]
    case_ids = [workload["input"]["case_id"] for workload in selected_workloads]
    missing_cases = [case_id for case_id in case_ids if case_id not in cases_by_id]
    if missing_cases:
        raise ValueError(f"benchmark references missing parity case: {missing_cases[0]}")
    policies = {
        json.dumps(workload["measurement"], sort_keys=True): workload["measurement"]
        for workload in selected_workloads
    }
    repeat_values = {
        int(policy["warmup_iterations"])
        + int(policy["measurement_iterations"]) * int(policy["samples"])
        for policy in policies.values()
    }
    if len(repeat_values) != 1:
        raise ValueError("benchmark workloads must use one repeat policy per batch")
    repeat = next(iter(repeat_values))
    measured_cases = [cases_by_id[case_id] for case_id in case_ids]
    pass_cases = {
        case_id
        for case_id in case_ids
        if parity_by_case.get(case_id, {}).get("outcome") == "pass"
    }
    # Measuring failed correctness cases would make a performance number look
    # authoritative while the workload is not behaviorally equivalent.
    timing_cases = [case for case in measured_cases if case["case_id"] in pass_cases]
    source_timing: dict[str, list[int]] = {}
    target_timing: dict[str, list[int]] = {}
    if timing_cases:
        source_timing = run_timed_side(
            "source", manifest, timing_cases, repeat, args.timeout
        )["timings_ns"]
        target_timing = run_timed_side(
            "target", manifest, timing_cases, repeat, args.timeout
        )["timings_ns"]
    workload_results: list[dict[str, Any]] = []
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        case_id = workload["input"]["case_id"]
        parity_comparison = parity_by_case.get(case_id)
        parity_pass = parity_comparison is not None and parity_comparison["outcome"] == "pass"
        correctness = {
            "gate": workload["measurement"]["correctness_gate"],
            "outcome": "pass" if parity_pass else "not_proven",
            "evidence_id": parity["identity"]["run_id"] if parity_pass else None,
        }
        subjects: list[dict[str, Any]] = []
        if parity_pass:
            subjects.append(
                subject_result(
                    "oracle", "pillow", source_timing.get(case_id, []), workload["measurement"]
                )
            )
            subjects.append(
                subject_result(
                    "target_profile",
                    TARGET_PROFILE,
                    target_timing.get(case_id, []),
                    workload["measurement"],
                )
            )
        else:
            subjects = [
                {"kind": "oracle", "id": "pillow", "status": "not_run", "measurements": []},
                {"kind": "target_profile", "id": TARGET_PROFILE, "status": "not_run", "measurements": []},
            ]
        workload_results.append(
            {
                "workload_id": workload_id,
                "requirements": workload["covers"],
                "measurement_policy": workload["measurement"],
                "correctness": correctness,
                "subjects": subjects,
                "budgets": [],
            }
        )
    workload_by_id = {item["workload_id"]: item for item in workload_results}
    suites: list[dict[str, Any]] = []
    for suite in suites_input:
        suite_subjects: list[dict[str, Any]] = []
        comparisons: list[dict[str, Any]] = []
        for subject_kind, subject_id in (("oracle", "pillow"), ("target_profile", TARGET_PROFILE)):
            metric_values: dict[str, list[tuple[float, int]]] = {}
            status = "completed"
            for member in suite["members"]:
                workload = workload_by_id.get(member["workload_id"])
                if workload is None:
                    status = "not_run"
                    continue
                subject = next(item for item in workload["subjects"] if item["id"] == subject_id)
                if subject["status"] != "completed":
                    status = "not_run"
                    continue
                for measurement in subject["measurements"]:
                    weighted = measurement["statistics"]["mean"]
                    if weighted is not None:
                        metric_values.setdefault(measurement["metric"], []).append((weighted, int(member["weight"])))
            measurements = []
            for metric, values in sorted(metric_values.items()):
                total_weight = sum(weight for _, weight in values)
                weighted_mean = sum(value * weight for value, weight in values) / total_weight if total_weight else None
                measurements.append({"metric": metric, "unit": "millisecond" if metric == "latency" else "operations_per_second", "weighted_mean": weighted_mean})
            suite_subjects.append({"kind": subject_kind, "id": subject_id, "status": status, "measurements": measurements})
        oracle_subject = next(item for item in suite_subjects if item["id"] == "pillow")
        target_subject = next(item for item in suite_subjects if item["id"] == TARGET_PROFILE)
        oracle_values = {item["metric"]: item["weighted_mean"] for item in oracle_subject["measurements"]}
        target_values = {item["metric"]: item["weighted_mean"] for item in target_subject["measurements"]}
        for metric in sorted(set(oracle_values) & set(target_values)):
            baseline = oracle_values[metric]
            target_value = target_values[metric]
            comparisons.append({
                "baseline_subject": "pillow",
                "subject_id": TARGET_PROFILE,
                "metric": metric,
                "baseline_value": baseline,
                "subject_value": target_value,
                "unit": "millisecond" if metric == "latency" else "operations_per_second",
                "ratio": target_value / baseline if baseline else None,
            })
        suites.append({"suite_id": suite["suite_id"], "members": suite["members"], "subjects": suite_subjects, "comparisons": comparisons})
    input_paths = sorted(set(workload_inputs.values()) | {case_inputs[case_id] for case_id in case_ids})
    selected_cases = [cases_by_id[case_id] for case_id in case_ids]
    result_identity = identity(manifest, input_paths, selected_cases, case_inputs, parity["identity"])
    result_identity["started_at"] = parity["identity"]["started_at"]
    result_identity["finished_at"] = now()
    measured = sum(1 for item in workload_results if item["correctness"]["outcome"] == "pass" and all(subject["status"] == "completed" for subject in item["subjects"]))
    not_run = len(workload_results) - measured
    result = {
        "schema": "migration-parity/benchmark-result@1",
        "identity": result_identity,
        "status": "completed",
        "environment": {
            "machine_id": platform.node() or "unknown",
            "os": platform.platform(),
            "architecture": platform.machine(),
            "cpu": platform.processor() or "unknown",
            "memory_bytes": 0,
            "power_mode": "unknown",
            "toolchain": platform.python_version(),
        },
        "summary": {
            "workloads_selected": len(workload_results),
            "workloads_measured": measured,
            "workloads_not_run": not_run,
            "budgets_passed": 0,
            "budgets_failed": 0,
            "budgets_not_proven": 0,
        },
        "workloads": workload_results,
        "suites": suites,
        "infrastructure_errors": [],
    }
    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.output.resolve().write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result["summary"], sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--parity-output", type=Path, default=DEFAULT_PARITY_RESULT)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--workload-id", action="append")
    parser.add_argument("--timeout", type=int, default=7200)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
