#!/usr/bin/env python3
"""Run benchmark workloads from the fixed input contract.

Parity-backed workloads are still correctness-gated before timing.  The
contract also permits benchmark-only workflow inputs: those are executed on
Pillow and each target backend and require successful execution, but do not
add cases to the parity corpus or claim byte parity.  Source and target
timings are collected in separate adapter processes from the same workflow
definitions, then emitted as a strict ``migration-parity/benchmark-result@1``
artifact.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
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

try:
    from run_migration_parity import (
        ORACLE_ID,
        ORACLE_VERSION,
        TARGET_ID,
        git_dirty,
        git_revision,
        process_group_options,
        reap_timed_out_process,
        receipt_terminal_complete,
    )
except ModuleNotFoundError:  # imported as ``scripts.run_migration_benchmark`` in tests
    from scripts.run_migration_parity import (
        ORACLE_ID,
        ORACLE_VERSION,
        TARGET_ID,
        git_dirty,
        git_revision,
        process_group_options,
        reap_timed_out_process,
        receipt_terminal_complete,
    )

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "benchmark-result.json"
DEFAULT_PARITY_RESULT = ROOT / "build" / "migration-parity" / "parity-result.json"
TARGET_BACKENDS = ("cpu", "simd", "gpu")
TARGET_PROFILES = tuple(f"python-{backend}" for backend in TARGET_BACKENDS)
DEFAULT_GPU_BENCHMARK_TIMEOUT_SECONDS = 900
MAX_GPU_BENCHMARK_TIMEOUT_SECONDS = 1800
# A suite aggregate is a statistical claim, not merely a record that one
# workload happened to complete.  Keep singleton intersections visible, but
# mark them non-comparable so a ratio cannot be mistaken for a cohort result.
MIN_COMPARABLE_SUITE_MEMBERS = 2


def target_profile_for_backend(backend: str) -> str:
    if backend not in TARGET_BACKENDS:
        raise ValueError(f"unsupported benchmark backend: {backend}")
    return f"python-{backend}"


def benchmark_subjects() -> list[tuple[str, str]]:
    return [
        ("oracle", "pillow"),
        *(
            ("target_profile", target_profile_for_backend(backend))
            for backend in TARGET_BACKENDS
        ),
    ]


def suite_subject_is_comparable(
    subject: dict[str, Any], subject_id: str
) -> bool:
    """Return whether a subject can contribute to a paired suite ratio.

    Timing completion and backend proof are intentionally separate states.  A
    target can have a full duration vector while its pipeline receipt is
    missing, partial, or host-controlled.  Such a row remains in the
    independent suite coverage summary, but it must not enter a speedup
    cohort.  The Pillow oracle has an explicit non-pipeline receipt and is
    the only non-terminal exception.
    """

    if subject.get("status") != "completed":
        return False
    execution = subject.get("execution")
    if not isinstance(execution, dict):
        return False
    if execution.get("errors"):
        return False
    if execution.get("fallback_reason_counts") != {}:
        return False
    measurements = subject.get("measurements")
    if not isinstance(measurements, list) or not any(
        isinstance(measurement, dict)
        and isinstance(measurement.get("sample_count"), int)
        and measurement["sample_count"] > 0
        and isinstance(measurement.get("statistics"), dict)
        and isinstance(measurement["statistics"].get("mean"), (int, float))
        for measurement in measurements
    ):
        return False

    expected_backend = (
        "pillow" if subject_id == "pillow" else subject_id.removeprefix("python-")
    )
    if subject_id == "pillow":
        return (
            execution.get("status") == "not_applicable"
            and execution.get("requested_backend") == "pillow"
            and execution.get("actual_backend") == "pillow"
        )

    if execution.get("status") != "completed":
        return False
    if execution.get("terminal_complete") is not True:
        return False
    if execution.get("requested_backend") != expected_backend:
        return False
    if execution.get("actual_backend") != expected_backend:
        return False
    actual_counts = execution.get("actual_backend_counts")
    if not isinstance(actual_counts, dict) or set(actual_counts) != {
        expected_backend
    }:
        return False
    count = actual_counts.get(expected_backend)
    if not isinstance(count, int) or count <= 0:
        return False
    execution_samples = execution.get("sample_count")
    if not isinstance(execution_samples, int) or execution_samples <= 0:
        return False
    return any(
        measurement.get("metric") == "latency"
        and measurement.get("sample_count") == execution_samples
        for measurement in measurements
        if isinstance(measurement, dict)
    )


def gpu_benchmark_timeout(requested_seconds: int) -> int:
    raw_limit = os.environ.get(
        "MIGRATION_GPU_BENCHMARK_TIMEOUT_SECONDS",
        str(DEFAULT_GPU_BENCHMARK_TIMEOUT_SECONDS),
    )
    try:
        configured_limit = int(raw_limit)
    except ValueError as exc:
        raise ValueError(
            "MIGRATION_GPU_BENCHMARK_TIMEOUT_SECONDS must be an integer"
        ) from exc
    if configured_limit <= 0:
        raise ValueError(
            "MIGRATION_GPU_BENCHMARK_TIMEOUT_SECONDS must be positive"
        )
    return min(requested_seconds, configured_limit, MAX_GPU_BENCHMARK_TIMEOUT_SECONDS)


def run_process(
    command: list[str],
    *,
    env: dict[str, str],
    timeout: int,
    label: str,
    input_text: str | None = None,
) -> tuple[int, str, str]:
    """Run one isolated benchmark adapter with a bounded process-group watchdog."""

    process = subprocess.Popen(
        command,
        cwd=ROOT,
        text=True,
        stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        **process_group_options(),
    )
    try:
        stdout, stderr = process.communicate(input=input_text, timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        stdout, stderr = reap_timed_out_process(process)
        detail = stderr.strip().replace("\n", " ")[-800:]
        suffix = f": {detail}" if detail else ""
        raise RuntimeError(f"{label} timed out after {timeout}s{suffix}") from exc
    return process.returncode, stdout, stderr


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def sha256(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


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


def merge_parity_results(results: list[dict[str, Any]], timeout: int) -> dict[str, Any]:
    """Join backend-specific parity evidence without collapsing target identity."""

    first_identity = results[0]["identity"]

    def unique_records(
        records: list[dict[str, Any]], keys: tuple[str, ...]
    ) -> list[dict[str, Any]]:
        seen: set[tuple[Any, ...]] = set()
        unique: list[dict[str, Any]] = []
        for record in records:
            key = tuple(record[item] for item in keys)
            if key not in seen:
                seen.add(key)
                unique.append(record)
        return unique

    comparisons = [
        comparison
        for result in results
        for comparison in result["comparisons"]
    ]
    infrastructure = [
        error
        for result in results
        for error in result["infrastructure_errors"]
    ]
    identities = [result["identity"] for result in results]
    identity = {
        **first_identity,
        "run_id": f"migration-parity-benchmark-gate-{uuid.uuid4().hex}",
        "started_at": min(item["started_at"] for item in identities),
        "finished_at": max(item["finished_at"] for item in identities),
        "inputs": unique_records(
            [item for identity in identities for item in identity["inputs"]],
            ("path",),
        ),
        "assets": unique_records(
            [item for identity in identities for item in identity["assets"]],
            ("input_path", "item_id", "asset_id"),
        ),
        "targets": unique_records(
            [item for identity in identities for item in identity["targets"]],
            ("target_profile", "target_id"),
        ),
        "command": {
            "command_id": "parity",
            "argv": ["make", "migration-parity-benchmark"],
            "cwd": ".",
            "timeout_seconds": timeout,
        },
    }
    return {
        "schema": "migration-parity/parity-result@1",
        "identity": identity,
        "status": "completed",
        "summary": {
            "selected": sum(result["summary"]["selected"] for result in results),
            "executed": len(comparisons),
            "passed": sum(result["summary"]["passed"] for result in results),
            "failed": sum(result["summary"]["failed"] for result in results),
            "not_run": sum(result["summary"]["not_run"] for result in results),
            "infrastructure_errors": len(infrastructure),
        },
        "comparisons": comparisons,
        "infrastructure_errors": infrastructure,
    }


def run_parity(
    manifest: Path,
    output: Path,
    timeout: int,
    cases: list[dict[str, Any]],
) -> dict[str, Any]:
    script = ROOT / "scripts" / "run_migration_parity.py"
    output.parent.mkdir(parents=True, exist_ok=True)
    backend_results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="migration-benchmark-parity-") as directory:
        temporary = Path(directory)
        for backend in TARGET_BACKENDS:
            target_profile = target_profile_for_backend(backend)
            # A benchmark may reference a parity case whose manifest target
            # profile is CPU-only.  The parity preflight verifies that case on
            # its declared target; the timed benchmark below still executes
            # every requested backend and retains its own unsupported receipt.
            backend_cases = [
                case
                for case in cases
                if target_profile in case.get("target_profiles", [])
            ]
            if not backend_cases:
                continue
            backend_output = temporary / f"{backend}.json"
            backend_timeout = (
                min(timeout, 300) if backend == "gpu" else timeout
            )
            returncode, _stdout, stderr = run_process(
                [
                    sys.executable,
                    str(script),
                    "--manifest",
                    str(manifest),
                    "--output",
                    str(backend_output),
                    "--timeout",
                    str(backend_timeout),
                    *[
                        argument
                        for case in backend_cases
                        for argument in ("--case-id", case["case_id"])
                    ],
                ],
                env={
                    **os.environ,
                    "MIGRATION_TARGET_BACKEND": backend,
                    "MIGRATION_STRICT_TARGET_BACKEND": "1",
                    "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
                    + os.pathsep
                    + os.environ.get("PYTHONPATH", ""),
                },
                timeout=backend_timeout,
                label=f"{backend} parity preflight",
            )
            if not backend_output.is_file():
                detail = stderr.strip().replace("\n", " ")[-800:]
                raise RuntimeError(
                    f"{backend} parity preflight did not emit a result: {detail}"
                )
            result = json.loads(backend_output.read_text(encoding="utf-8"))
            if result.get("status") != "completed":
                details = result.get("infrastructure_errors", [])
                detail = json.dumps(details, sort_keys=True)
                raise RuntimeError(
                    f"{backend} parity preflight failed infrastructure checks: {detail}"
                )
            if returncode not in {0, 1}:
                detail = stderr.strip().replace("\n", " ")[-800:]
                raise RuntimeError(
                    f"{backend} parity preflight exited {returncode}: {detail}"
                )
            backend_results.append(result)
    merged = merge_parity_results(backend_results, timeout)
    output.write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")
    return merged


def run_timed_side(
    side: str,
    manifest: Path,
    cases: list[dict[str, Any]],
    repeat: int,
    timeout: int,
    *,
    backend: str,
    timing_boundary: str,
    timing_steps: list[str],
    lifecycle: str = "warm",
) -> dict[str, Any]:
    effective_timeout = (
        gpu_benchmark_timeout(timeout)
        if side == "target" and backend == "gpu"
        else timeout
    )
    script = ROOT / "scripts" / "run_migration_parity.py"

    def command_for(child_repeat: int) -> list[str]:
        command = [
            sys.executable,
            str(script),
            "--side",
            side,
            "--manifest",
            str(manifest),
            "--repeat",
            str(child_repeat),
            "--timings",
            "--timing-boundary",
            timing_boundary,
            "--lifecycle",
            lifecycle,
        ]
        for step_id in timing_steps:
            command.extend(("--timing-step", step_id))
        return command

    def execute(child_cases: list[dict[str, Any]], child_repeat: int) -> dict[str, Any]:
        returncode, stdout, stderr = run_process(
            command_for(child_repeat),
            input_text=json.dumps(child_cases, separators=(",", ":")),
            timeout=effective_timeout,
            label=f"{backend if side == 'target' else 'Pillow'} benchmark adapter",
            env={
                **os.environ,
                "MIGRATION_TARGET_BACKEND": backend,
                "MIGRATION_STRICT_TARGET_BACKEND": "1",
                "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
                + os.pathsep
                + os.environ.get("PYTHONPATH", ""),
            },
        )
        if returncode != 0:
            detail = stderr.strip().replace("\n", " ")[-800:]
            raise RuntimeError(f"{side} benchmark adapter failed: {detail}")
        payload = json.loads(stdout)
        if set(payload) != {"identity", "results", "timings_ns", "telemetry", "execution"}:
            raise RuntimeError(f"{side} benchmark adapter emitted invalid timing envelope")
        identity = payload["identity"]
        if side == "source":
            if identity.get("side") != "source" or identity.get("implementation") != "Pillow":
                raise RuntimeError("Pillow benchmark adapter emitted the wrong identity")
        else:
            backend_state = identity.get("backend_state", {})
            if (
                identity.get("side") != "target"
                or identity.get("implementation") != "pillow-rs"
                or identity.get("backend") != backend
                or backend_state.get("requested") != backend
                or backend_state.get("active") != [backend]
            ):
                raise RuntimeError(
                    f"target benchmark adapter did not isolate the {backend!r} backend"
                )
        return payload

    if lifecycle != "cold":
        return execute(cases, repeat)

    # A cold sample is intentionally isolated in a fresh adapter process. This
    # includes GPU adapter/pipeline initialization in the first backend phase
    # and prevents a prior workload from warming the process-local caches.
    merged: dict[str, Any] | None = None
    for case in cases:
        for _ in range(repeat):
            payload = execute([case], 1)
            if merged is None:
                merged = payload
                continue
            merged["results"].extend(payload["results"])
            for field in ("timings_ns", "telemetry", "execution"):
                for case_id, records in payload[field].items():
                    merged[field].setdefault(case_id, []).extend(records)
    if merged is None:
        raise RuntimeError("cold benchmark adapter received no cases")
    return merged


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


def execution_numeric_summary(
    records: list[dict[str, Any]], field: str
) -> dict[str, Any]:
    values = [
        int(record[field])
        for record in records
        if isinstance(record.get(field), int) and not isinstance(record.get(field), bool)
    ]
    if not values:
        return {
            "sample_count": 0,
            "min": None,
            "median": None,
            "mean": None,
            "max": None,
            "total": None,
        }
    return {
        "sample_count": len(values),
        "min": min(values),
        "median": statistics.median(values),
        "mean": statistics.mean(values),
        "max": max(values),
        "total": sum(values),
    }


def execution_resource_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    resources = [
        record["resource"]
        for record in records
        if isinstance(record.get("resource"), dict)
    ]
    fields = (
        "upload_bytes",
        "readback_bytes",
        "auxiliary_bytes",
        "parameter_bytes",
        "retained_cache_bytes",
        "full_frame_copy_count",
        "mode_conversion_count",
        "host_buffer_count",
        "host_buffer_bytes",
        "peak_live_host_bytes",
        "fused_operation_count",
    )
    return {
        "sample_count": len(resources),
        **{
            field: execution_numeric_summary(resources, field)
            for field in fields
        },
    }


def execution_result(
    subject_kind: str,
    subject_id: str,
    execution_records: list[dict[str, Any]],
    policy: dict[str, Any],
    errors: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    errors = list(errors or [])
    if subject_kind == "oracle":
        return {
            "status": "not_applicable",
            "terminal_complete": False,
            "requested_backend": "pillow",
            "actual_backend": "pillow",
            "actual_backend_counts": {"pillow": 1},
            "fallback_reason_counts": {},
            "operation_count": execution_numeric_summary([], "operation_count"),
            "dispatch_count": execution_numeric_summary([], "dispatch_count"),
            "resize_coeff_cache_hits": execution_numeric_summary(
                [], "resize_coeff_cache_hits"
            ),
            "resize_coeff_cache_misses": execution_numeric_summary(
                [], "resize_coeff_cache_misses"
            ),
            "phase_timings_ns": {
                field: execution_numeric_summary([], field)
                for field in ("route_ns", "validation_ns", "backend_ns")
            },
            "resource": execution_resource_summary([]),
            "sample_count": 0,
            "cached_sample_count": 0,
            "errors": errors,
        }

    warmup = int(policy["warmup_iterations"])
    measured_count = int(policy["measurement_iterations"]) * int(policy["samples"])
    expected = warmup + measured_count
    measured = execution_records[warmup:expected]
    completed_candidates = [
        record
        for record in measured
        if record.get("status") == "completed"
        and isinstance(record.get("actual_backend"), str)
        and isinstance(record.get("operation_count"), int)
        and record["operation_count"] > 0
    ]
    cached_candidates = [
        record for record in measured if record.get("status") == "cached"
    ]
    completed = [
        record
        for record in completed_candidates
        if receipt_terminal_complete(record)
    ]
    cached = [
        record for record in cached_candidates if receipt_terminal_complete(record)
    ]
    cached_sample_count = len(cached)
    partial_count = sum(
        1 for record in measured if record.get("status") == "partial"
    )
    not_applicable_count = sum(
        1 for record in measured if record.get("status") == "not_applicable"
    )
    actual_counts: dict[str, int] = {}
    for record in completed:
        if isinstance(record.get("actual_backend"), str):
            backend = str(record["actual_backend"])
            actual_counts[backend] = actual_counts.get(backend, 0) + 1
    # The no-fallback predicate covers the complete measured workflow.  A
    # host-controlled setup/prefix receipt can be followed by a terminal GPU
    # receipt, so counting only ``completed`` would falsely admit that row to
    # an actual-backend cohort.
    fallback_counts: dict[str, int] = {}
    for record in measured:
        reason = record.get("fallback_reason")
        if isinstance(reason, str) and reason:
            fallback_counts[reason] = fallback_counts.get(reason, 0) + 1
    actual_backends = sorted(actual_counts)
    requested_backend = subject_id.removeprefix("python-")
    complete = (
        len(completed_candidates) + len(cached_candidates)
        == len(measured)
        == measured_count
    )
    terminal_complete = (
        complete
        and len(completed) + len(cached) == measured_count
        and not errors
    )
    terminal_gap = complete and not terminal_complete
    has_partial = partial_count > 0 or bool(errors) or terminal_gap
    all_not_applicable = (
        bool(measured)
        and not errors
        and not_applicable_count == len(measured)
    )
    if all_not_applicable:
        # Public lifecycle/draw/metadata calls can be valid benchmark timings
        # without entering the compute-pipeline telemetry boundary. Keep that
        # distinction explicit instead of calling the row ``not_proven`` (or
        # implying a backend capability failure); it remains excluded from
        # actual-backend performance cohorts by its null receipt.
        status = "not_applicable"
    elif terminal_complete and not has_partial:
        status = "completed"
    elif has_partial and (
        completed_candidates or cached_candidates or completed or cached
    ):
        status = "partial"
    elif errors and all(
        isinstance(item, dict)
        and isinstance(item.get("error"), dict)
        and item["error"].get("kind") == "unsupported"
        for item in errors
    ):
        status = "unsupported"
    else:
        status = "not_proven"
    return {
        "status": status,
        "terminal_complete": terminal_complete,
        "requested_backend": requested_backend,
        "actual_backend": (
            actual_backends[0]
            if len(actual_backends) == 1
            else "mixed"
            if actual_backends
            else None
        ),
        "actual_backend_counts": actual_counts,
        "fallback_reason_counts": fallback_counts,
        "operation_count": execution_numeric_summary(completed, "operation_count"),
        "dispatch_count": execution_numeric_summary(completed, "dispatch_count"),
        "resize_coeff_cache_hits": execution_numeric_summary(
            completed, "resize_coeff_cache_hits"
        ),
        "resize_coeff_cache_misses": execution_numeric_summary(
            completed, "resize_coeff_cache_misses"
        ),
        "phase_timings_ns": {
            field: execution_numeric_summary(completed, field)
            for field in ("route_ns", "validation_ns", "backend_ns")
        },
        "resource": execution_resource_summary(completed),
        "sample_count": len(completed),
        "cached_sample_count": cached_sample_count,
        "errors": errors,
    }


def subject_result(
    subject_kind: str,
    subject_id: str,
    durations_ns: list[int],
    phase_records: list[dict[str, int]],
    execution_records: list[dict[str, Any]],
    policy: dict[str, Any],
    errors: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    warmup = int(policy["warmup_iterations"])
    iterations = int(policy["measurement_iterations"])
    samples = int(policy["samples"])
    expected = warmup + iterations * samples
    measured = durations_ns[warmup:expected]
    measured_phases = phase_records[warmup:expected]
    phase_results = {
        phase_name: {
            "sample_count": len(measured_phases),
            "statistics": statistics_for(
                [
                    int(record.get(f"{phase_name}_ns", 0)) / 1_000_000
                    for record in measured_phases
                ],
                "latency",
            )["statistics"],
        }
        for phase_name in ("setup", "pipeline", "terminal", "total")
    }
    execution = execution_result(
        subject_kind,
        subject_id,
        execution_records,
        policy,
        errors,
    )
    if len(measured) != iterations * samples or len(measured_phases) != iterations * samples:
        return {
            "kind": subject_kind,
            "id": subject_id,
            "status": "failed",
            "measurements": [],
            "phases": phase_results,
            "execution": execution,
        }
    values_ms = [value / 1_000_000 for value in measured]
    return {
        "kind": subject_kind,
        "id": subject_id,
        "status": "completed",
        "measurements": [
            statistics_for(values_ms, metric) for metric in policy["metrics"]
        ],
        "phases": phase_results,
        "execution": execution,
    }


def identity(
    manifest: Path,
    input_paths: list[str],
    cases: list[dict[str, Any]],
    parity_identity: dict[str, Any],
    timeout: int,
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
        "oracles": parity_identity["oracles"],
        "targets": parity_identity["targets"],
        "command": {
            "command_id": "benchmark",
            "argv": ["make", "migration-parity-benchmark"],
            "cwd": ".",
            "timeout_seconds": timeout,
        },
    }


def execution_identity() -> dict[str, Any]:
    """Return identity headers for a benchmark with no parity-backed cases."""

    return {
        "oracles": [
            {
                "oracle_id": ORACLE_ID,
                "name": "Pillow",
                "version": ORACLE_VERSION,
                "runtime": "CPython 3.12",
            }
        ],
        "targets": [
            {
                "target_profile": target_profile_for_backend(backend),
                "target_id": TARGET_ID,
                "revision": git_revision(),
                "dirty": git_dirty(),
                "runtime": platform.python_version(),
                "backend": backend,
                "features": ["all-features"],
            }
            for backend in TARGET_BACKENDS
        ],
    }


def benchmark_workflow_case(
    workload: dict[str, Any],
) -> dict[str, Any]:
    """Adapt a benchmark-only workflow to the parity adapter's case shape."""

    input_spec = workload["input"]
    if input_spec.get("kind") != "workflow":
        raise ValueError(
            f"{workload['workload_id']}: expected a workflow benchmark input"
        )
    return {
        "case_id": f"benchmark-workflow.{workload['workload_id']}",
        "covers": workload["covers"],
        "target_profiles": [target_profile_for_backend(backend) for backend in TARGET_BACKENDS],
        "assets": input_spec["assets"],
        "steps": input_spec["steps"],
        "observations": input_spec["observations"],
        # Private adapter metadata; it is not emitted in a parity or
        # benchmark result artifact.
        "_benchmark_cache_state": workload["context"]["cache_state"],
    }


def validate_selected_workloads(
    selected_workloads: list[dict[str, Any]],
) -> None:
    if not selected_workloads:
        raise ValueError("no benchmark workloads selected")
    expected_subjects = [
        {"kind": kind, "id": subject_id}
        for kind, subject_id in benchmark_subjects()
    ]
    for workload in selected_workloads:
        if workload["subjects"] != expected_subjects:
            raise ValueError(
                f"{workload['workload_id']}: benchmark subjects do not match "
                "the Pillow/CPU/SIMD/GPU contract"
            )
    for workload in selected_workloads:
        policy = workload["measurement"]
        repeat = (
            int(policy["warmup_iterations"])
            + int(policy["measurement_iterations"]) * int(policy["samples"])
        )
        if repeat <= 0:
            raise ValueError(
                f"{workload['workload_id']}: benchmark repeat policy is empty"
            )


def run(args: argparse.Namespace) -> int:
    manifest = args.manifest.resolve()
    manifest_data = load_manifest(manifest)
    cases_by_id, case_inputs = load_parity_cases(manifest_data)
    workloads, suites_input, workload_inputs = load_benchmarks(manifest_data)
    selected_workloads = list(workloads.values())
    if args.pipeline:
        selected_workloads = [
            workload
            for workload in selected_workloads
            if workload["workload_id"].startswith(
                (
                    "pipeline-op.",
                    "pipeline-chain.",
                    "pipeline-lifecycle.",
                    "pipeline-matrix.",
                    "pipeline.quick.",
                )
            )
        ]
    elif args.workload_id:
        selected_workloads = [
            workloads[item] for item in args.workload_id if item in workloads
        ]
        missing_workloads = [item for item in args.workload_id if item not in workloads]
        if missing_workloads:
            raise ValueError(f"unknown benchmark workload: {missing_workloads[0]}")
    if args.limit is not None:
        selected_workloads = selected_workloads[: args.limit]
    validate_selected_workloads(selected_workloads)

    workload_cases: dict[str, dict[str, Any]] = {}
    parity_case_ids: list[str] = []
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        input_spec = workload["input"]
        if input_spec["kind"] == "parity_case":
            case_id = input_spec["case_id"]
            if case_id not in cases_by_id:
                raise ValueError(
                    f"{workload_id}: benchmark references missing parity case: "
                    f"{case_id}"
                )
            workload_cases[workload_id] = cases_by_id[case_id]
            parity_case_ids.append(case_id)
        elif input_spec["kind"] == "workflow":
            workload_cases[workload_id] = benchmark_workflow_case(workload)
        else:
            raise ValueError(
                f"{workload_id}: benchmark input kind {input_spec['kind']!r} "
                "cannot be executed by the adapter benchmark"
            )

    parity_case_ids = list(dict.fromkeys(parity_case_ids))
    measured_cases = [cases_by_id[case_id] for case_id in parity_case_ids]
    parity_output = args.parity_output.resolve()
    if measured_cases:
        parity = run_parity(manifest, parity_output, args.timeout, measured_cases)
    else:
        started = now()
        parity_identity = {
            "run_id": f"migration-benchmark-no-parity-{uuid.uuid4().hex}",
            "started_at": started,
            "finished_at": now(),
            "manifest": {
                "path": str(manifest.relative_to(ROOT)),
                "schema": "migration-parity/manifest@2",
                "sha256": sha256(manifest),
            },
            "inputs": [],
            "assets": [],
            **execution_identity(),
            "command": {
                "command_id": "benchmark",
                "argv": ["make", "migration-parity-benchmark"],
                "cwd": ".",
                "timeout_seconds": args.timeout,
            },
        }
        parity = {
            "schema": "migration-parity/parity-result@1",
            "identity": parity_identity,
            "status": "completed",
            "summary": {
                "selected": 0,
                "executed": 0,
                "passed": 0,
                "failed": 0,
                "not_run": 0,
                "infrastructure_errors": 0,
            },
            "comparisons": [],
            "infrastructure_errors": [],
        }
        parity_output.parent.mkdir(parents=True, exist_ok=True)
        parity_output.write_text(json.dumps(parity, indent=2) + "\n", encoding="utf-8")
    parity_by_case = {
        (item["case_id"], item["target_profile"]): item
        for item in parity["comparisons"]
    }

    def parity_profiles(case_id: str) -> list[str]:
        case = next(item for item in measured_cases if item["case_id"] == case_id)
        declared = set(case.get("target_profiles", []))
        return [
            profile
            for profile in TARGET_PROFILES
            if not declared or profile in declared
        ]

    pass_cases = {
        case_id
        for case_id in parity_case_ids
        if all(
            parity_by_case.get(
                (case_id, target_profile), {}
            ).get("outcome")
            == "pass"
            for target_profile in parity_profiles(case_id)
        )
    }
    # Measuring failed correctness cases would make a performance number look
    # authoritative while the workload is not behaviorally equivalent.  A
    # benchmark-only workflow uses the explicit successful_execution gate and
    # is checked after both adapters return timing receipts.
    source_timing: dict[str, list[int]] = {}
    source_phase_telemetry: dict[str, list[dict[str, int]]] = {}
    source_execution: dict[str, list[dict[str, Any]]] = {}
    target_timings: dict[str, dict[str, list[int]]] = {
        profile: {} for profile in TARGET_PROFILES
    }
    target_phase_telemetry: dict[str, dict[str, list[dict[str, int]]]] = {
        profile: {} for profile in TARGET_PROFILES
    }
    target_execution: dict[str, dict[str, list[dict[str, Any]]]] = {
        profile: {} for profile in TARGET_PROFILES
    }
    source_status: dict[str, str] = {}
    source_execution_errors: dict[str, list[dict[str, Any]]] = {}
    target_status: dict[str, dict[str, str]] = {
        profile: {} for profile in TARGET_PROFILES
    }
    target_execution_errors: dict[str, dict[str, list[dict[str, Any]]]] = {
        profile: {} for profile in TARGET_PROFILES
    }
    timing_groups: dict[
        tuple[str, tuple[str, ...], str, int, int, int], list[dict[str, Any]]
    ] = {}
    case_repeats: dict[str, int] = {}
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        case_id = workload_cases[workload_id]["case_id"]
        gate = workload["measurement"]["correctness_gate"]
        eligible = (
            case_id in pass_cases
            if workload["input"]["kind"] == "parity_case"
            else gate in {"successful_execution", "not_applicable"}
        )
        if not eligible:
            continue
        policy = workload["measurement"]
        repeat = (
            int(policy["warmup_iterations"])
            + int(policy["measurement_iterations"]) * int(policy["samples"])
        )
        case_repeats[workload_cases[workload_id]["case_id"]] = repeat
        key = (
            policy["boundary"],
            tuple(policy["step_ids"]),
            policy["cache_state"],
            int(policy["warmup_iterations"]),
            int(policy["measurement_iterations"]),
            int(policy["samples"]),
        )
        timing_groups.setdefault(key, []).append(workload_cases[workload_id])
    for (
        timing_boundary,
        timing_step_ids,
        lifecycle,
        warmup_iterations,
        measurement_iterations,
        samples,
    ), timing_cases in timing_groups.items():
        repeat = warmup_iterations + measurement_iterations * samples
        source_receipt = run_timed_side(
            "source",
            manifest,
            timing_cases,
            repeat,
            args.timeout,
            backend="cpu",
            timing_boundary=timing_boundary,
            timing_steps=list(timing_step_ids),
            lifecycle=lifecycle,
        )
        source_timing.update(source_receipt["timings_ns"])
        source_phase_telemetry.update(source_receipt["telemetry"])
        source_execution.update(source_receipt["execution"])
        source_status.update(
            {item["case_id"]: item["status"] for item in source_receipt["results"]}
        )
        source_execution_errors.update(
            {
                item["case_id"]: item.get("execution_errors", [])
                for item in source_receipt["results"]
            }
        )
        for backend in TARGET_BACKENDS:
            profile = target_profile_for_backend(backend)
            target_receipt = run_timed_side(
                "target",
                manifest,
                timing_cases,
                repeat,
                args.timeout,
                backend=backend,
                timing_boundary=timing_boundary,
                timing_steps=list(timing_step_ids),
                lifecycle=lifecycle,
            )
            target_timings[profile].update(target_receipt["timings_ns"])
            target_phase_telemetry[profile].update(target_receipt["telemetry"])
            target_execution[profile].update(target_receipt["execution"])
            target_status[profile].update(
                {
                    item["case_id"]: item["status"]
                    for item in target_receipt["results"]
                }
            )
            target_execution_errors[profile].update(
                {
                    item["case_id"]: item.get("execution_errors", [])
                    for item in target_receipt["results"]
                }
            )

    def timed_success(case_id: str, expected_repeat: int) -> bool:
        return (
            source_status.get(case_id) == "completed"
            and len(source_timing.get(case_id, [])) == expected_repeat
            and all(
                target_status[profile].get(case_id) == "completed"
                and len(target_timings[profile].get(case_id, [])) == expected_repeat
                for profile in TARGET_PROFILES
            )
        )

    def empty_phases() -> dict[str, Any]:
        return {
            phase: {
                "sample_count": 0,
                "statistics": statistics_for([], "latency")["statistics"],
            }
            for phase in ("setup", "pipeline", "terminal", "total")
        }

    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        case_id = workload_cases[workload_id]["case_id"]
        expected_repeat = case_repeats.get(case_id, 0)
        if not timed_success(case_id, expected_repeat):
            details = {
                "source_status": source_status.get(case_id),
                "source_errors": source_execution_errors.get(case_id, []),
                "targets": {
                    profile: {
                        "status": target_status[profile].get(case_id),
                        "errors": target_execution_errors[profile].get(case_id, []),
                    }
                    for profile in TARGET_PROFILES
                },
            }
            print(
                f"benchmark execution gate not met for {workload_id}: "
                f"{json.dumps(details, sort_keys=True)}",
                file=sys.stderr,
            )

    workload_results: list[dict[str, Any]] = []
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        case_id = workload_cases[workload_id]["case_id"]
        parity_pass = (
            workload["input"]["kind"] == "parity_case"
            and case_id in pass_cases
        )
        execution_pass = timed_success(case_id, case_repeats.get(case_id, 0))
        measured_pass = parity_pass and execution_pass if workload["input"]["kind"] == "parity_case" else execution_pass
        correctness = {
            "gate": workload["measurement"]["correctness_gate"],
            "outcome": "pass" if measured_pass else "not_proven",
            "evidence_id": (
                parity["identity"]["run_id"]
                if parity_pass
                else None
            ),
        }
        subjects: list[dict[str, Any]] = []
        benchmark_execution = workload["input"]["kind"] == "workflow"
        # Preserve each backend's valid timing independently.  A missing GPU
        # adapter or an unsupported GPU cell must not erase CPU/SIMD samples
        # from the same declared parity case; the workload correctness gate
        # remains not_proven until every required subject completes.
        def errors_for(kind: str, subject_id: str) -> list[dict[str, Any]]:
            if kind == "oracle":
                return source_execution_errors.get(case_id, [])
            return target_execution_errors.get(subject_id, {}).get(case_id, [])

        if parity_pass or benchmark_execution:
            def durations_for(kind: str, subject_id: str) -> list[int]:
                if kind == "oracle":
                    return (
                        source_timing.get(case_id, [])
                        if source_status.get(case_id) == "completed"
                        else []
                    )
                return (
                    target_timings.get(subject_id, {}).get(case_id, [])
                    if target_status.get(subject_id, {}).get(case_id)
                    == "completed"
                    else []
                )

            subjects = [
                subject_result(
                    kind,
                    subject_id,
                    durations_for(kind, subject_id),
                    (
                        source_phase_telemetry.get(case_id, [])
                        if kind == "oracle"
                        else target_phase_telemetry.get(subject_id, {}).get(case_id, [])
                    ),
                    (
                        source_execution.get(case_id, [])
                        if kind == "oracle"
                        else target_execution.get(subject_id, {}).get(case_id, [])
                    ),
                    workload["measurement"],
                    errors_for(kind, subject_id),
                )
                for kind, subject_id in benchmark_subjects()
            ]
        else:
            subjects = [
                {
                    "kind": kind,
                    "id": subject_id,
                    "status": "not_run",
                    "measurements": [],
                    "phases": empty_phases(),
                    "execution": execution_result(
                        kind,
                        subject_id,
                        [],
                        workload["measurement"],
                        errors_for(kind, subject_id),
                    ),
                }
                for kind, subject_id in benchmark_subjects()
            ]
        workload_results.append(
            {
                "workload_id": workload_id,
                "requirements": workload["covers"],
                "measurement_policy": workload["measurement"],
                "context": workload["context"],
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
        for subject_kind, subject_id in benchmark_subjects():
            metric_values: dict[str, list[tuple[float, int]]] = {}
            for member in suite["members"]:
                workload = workload_by_id.get(member["workload_id"])
                if workload is None:
                    continue
                subject = next(item for item in workload["subjects"] if item["id"] == subject_id)
                if subject["status"] != "completed":
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
            suite_subjects.append({
                "kind": subject_kind,
                "id": subject_id,
                "status": "completed" if measurements else "not_run",
                "measurements": measurements,
            })
        oracle_subject = next(item for item in suite_subjects if item["id"] == "pillow")
        oracle_values = {
            item["metric"]: item["weighted_mean"]
            for item in oracle_subject["measurements"]
        }
        declared_members = list(suite["members"])
        declared_member_count = len(declared_members)
        for target_profile in TARGET_PROFILES:
            target_subject = next(
                item for item in suite_subjects if item["id"] == target_profile
            )
            target_values = {
                item["metric"]: item["weighted_mean"]
                for item in target_subject["measurements"]
            }
            common_members: list[dict[str, Any]] = []
            excluded_members: list[dict[str, Any]] = []
            for member in declared_members:
                workload = workload_by_id.get(member["workload_id"])
                if workload is None:
                    excluded_members.append(
                        {
                            "workload_id": member["workload_id"],
                            "baseline_status": "missing",
                            "subject_status": "missing",
                        }
                    )
                    continue
                subjects_by_id = {
                    item["id"]: item for item in workload["subjects"]
                }
                baseline_subject = subjects_by_id.get("pillow", {})
                target_member_subject = subjects_by_id.get(target_profile, {})
                # Keep the timing summary independent from the performance
                # cohort.  Only a value-complete Pillow row and a target row
                # with a terminal requested=actual, no-fallback receipt may
                # contribute to a paired suite ratio.
                if suite_subject_is_comparable(
                    baseline_subject, "pillow"
                ) and suite_subject_is_comparable(
                    target_member_subject, target_profile
                ):
                    common_members.append(member)
                else:
                    excluded_members.append(
                        {
                            "workload_id": member["workload_id"],
                            "baseline_status": baseline_subject.get("status", "missing"),
                            "subject_status": target_member_subject.get("status", "missing"),
                        }
                    )
            common_ids = sorted(member["workload_id"] for member in common_members)
            common_digest = hashlib.sha256(
                ("\n".join(common_ids) + "\n").encode()
            ).hexdigest()

            # Recompute both sides from exactly the same receipt-proven
            # workload IDs.  The independent suite subject summaries above
            # remain useful for coverage, but may not be used as a speed
            # comparison when backend execution was not proven.
            pair_values: dict[str, dict[str, list[tuple[float, int]]]] = {
                "pillow": {},
                target_profile: {},
            }
            for member in common_members:
                workload = workload_by_id[member["workload_id"]]
                weight = int(member["weight"])
                for subject_id in ("pillow", target_profile):
                    subject = next(
                        item
                        for item in workload["subjects"]
                        if item["id"] == subject_id
                    )
                    for measurement in subject["measurements"]:
                        mean = measurement["statistics"].get("mean")
                        if mean is not None:
                            pair_values[subject_id].setdefault(
                                measurement["metric"], []
                            ).append((float(mean), weight))

            pair_metrics = sorted(
                set(pair_values["pillow"]) & set(pair_values[target_profile])
            )
            for metric in pair_metrics:
                baseline_values = pair_values["pillow"][metric]
                target_metric_values = pair_values[target_profile][metric]
                baseline_weight = sum(weight for _, weight in baseline_values)
                target_weight = sum(weight for _, weight in target_metric_values)
                baseline = (
                    sum(value * weight for value, weight in baseline_values)
                    / baseline_weight
                    if baseline_weight
                    else None
                )
                target_value = (
                    sum(value * weight for value, weight in target_metric_values)
                    / target_weight
                    if target_weight
                    else None
                )
                comparisons.append({
                    "baseline_subject": "pillow",
                    "subject_id": target_profile,
                    "metric": metric,
                    "baseline_value": baseline,
                    "subject_value": target_value,
                    "unit": "millisecond" if metric == "latency" else "operations_per_second",
                    "ratio": target_value / baseline if baseline else None,
                    "declared_member_count": declared_member_count,
                    "common_member_count": len(common_ids),
                    "common_member_ids_sha256": common_digest,
                    "excluded_members": excluded_members,
                    "status": (
                        "comparable"
                        if len(common_ids) >= MIN_COMPARABLE_SUITE_MEMBERS
                        else "not_comparable"
                    ),
                })
            if not pair_metrics:
                # Retain an explicit non-comparable record when no pair has a
                # measurable metric.  Otherwise a suite with no common IDs
                # would silently disappear from the comparison denominator.
                for metric in sorted(set(oracle_values) | set(target_values) | {"latency"}):
                    comparisons.append({
                        "baseline_subject": "pillow",
                        "subject_id": target_profile,
                        "metric": metric,
                        "baseline_value": None,
                        "subject_value": None,
                        "unit": "millisecond" if metric == "latency" else "operations_per_second",
                        "ratio": None,
                        "declared_member_count": declared_member_count,
                        "common_member_count": len(common_ids),
                        "common_member_ids_sha256": common_digest,
                        "excluded_members": excluded_members,
                        "status": "not_comparable",
                    })
        suites.append({"suite_id": suite["suite_id"], "members": suite["members"], "subjects": suite_subjects, "comparisons": comparisons})
    input_paths = sorted(
        {
            workload_inputs[workload["workload_id"]]
            for workload in selected_workloads
        }
        | {case_inputs[case_id] for case_id in parity_case_ids}
    )
    selected_cases = list(
        {
            case["case_id"]: case
            for case in workload_cases.values()
        }.values()
    )
    result_identity = identity(
        manifest,
        input_paths,
        selected_cases,
        parity["identity"],
        args.timeout,
    )
    result_identity["started_at"] = parity["identity"]["started_at"]
    result_identity["finished_at"] = now()
    measured = sum(
        1
        for item in workload_results
        if any(subject["status"] == "completed" for subject in item["subjects"])
    )
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
    parser.add_argument(
        "--pipeline",
        action="store_true",
        help="select the complete PipelineOp and composition benchmark suites",
    )
    parser.add_argument("--timeout", type=int, default=7200)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
