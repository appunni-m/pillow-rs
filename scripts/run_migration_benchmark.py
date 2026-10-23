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
import gzip
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
    from migration_execution_attestation import (
        AttestationCollector,
        merge_parity_attestations,
        report_fingerprint as execution_report_fingerprint,
        validate_attestation,
    )
    from migration_producer_attestation import merge_backend_attestations
    from migration_benchmark_samples import RawSampleCollector
    from simd_evidence import simd_vector_evidence
    from migration_benchmark_adapter import (
        benchmark_adapter_environment,
        benchmark_adapter_python,
    )
    from run_migration_parity import (
        ORACLE_ID,
        ORACLE_NAME,
        ORACLE_VERSION,
        TARGET_ID,
        TARGET_FEATURES,
        TIMED_SOURCE_FLAG,
        git_dirty,
        git_revision,
        process_group_options,
        reap_timed_out_process,
        receipt_terminal_complete,
    )
except ModuleNotFoundError:  # imported as ``scripts.run_migration_benchmark`` in tests
    from scripts.migration_execution_attestation import (
        AttestationCollector,
        merge_parity_attestations,
        report_fingerprint as execution_report_fingerprint,
        validate_attestation,
    )
    from scripts.migration_producer_attestation import merge_backend_attestations
    from scripts.migration_benchmark_samples import RawSampleCollector
    from scripts.simd_evidence import simd_vector_evidence
    from scripts.migration_benchmark_adapter import (
        benchmark_adapter_environment,
        benchmark_adapter_python,
    )
    from scripts.run_migration_parity import (
        ORACLE_ID,
        ORACLE_NAME,
        ORACLE_VERSION,
        TARGET_ID,
        TARGET_FEATURES,
        TIMED_SOURCE_FLAG,
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
BENCHMARK_SOURCE_NAME = os.environ.get("MIGRATION_BENCHMARK_SOURCE_NAME", ORACLE_NAME)
BENCHMARK_SOURCE_VERSION = os.environ.get(
    "MIGRATION_BENCHMARK_SOURCE_VERSION", ORACLE_VERSION
)
PARALLEL_CPU_PROFILE = (
    os.environ.get("MIGRATION_TARGET_PROFILE", "").strip().lower()
    == "parallel-cpu"
)
_requested_backends = os.environ.get("MIGRATION_BENCHMARK_BACKENDS", "").strip()
if PARALLEL_CPU_PROFILE and _requested_backends:
    raise ValueError("Parallel CPU benchmarks cannot override their CPU-only backend")
if _requested_backends:
    TARGET_BACKENDS = tuple(
        item.strip().lower() for item in _requested_backends.split(",") if item.strip()
    )
    if not TARGET_BACKENDS or len(set(TARGET_BACKENDS)) != len(TARGET_BACKENDS) or any(
        backend not in {"cpu", "simd", "gpu"} for backend in TARGET_BACKENDS
    ):
        raise ValueError("MIGRATION_BENCHMARK_BACKENDS must list unique cpu,simd,gpu backends")
elif PARALLEL_CPU_PROFILE:
    TARGET_BACKENDS = ("cpu",)
else:
    TARGET_BACKENDS = ("cpu", "simd", "gpu")
PILLOW_SIMD_COMPARISON = (
    os.environ.get("MIGRATION_BENCHMARK_COHORT", "").strip().lower()
    == "pillow-simd"
)
if PILLOW_SIMD_COMPARISON and TARGET_BACKENDS != ("cpu", "simd"):
    raise ValueError("the Pillow-SIMD comparison cohort requires exactly cpu,simd backends")
DEFAULT_GPU_BENCHMARK_TIMEOUT_SECONDS = 900
MAX_GPU_BENCHMARK_TIMEOUT_SECONDS = 1800
# A suite aggregate is a statistical claim, not merely a record that one
# workload happened to complete.  Keep singleton intersections visible, but
# mark them non-comparable so a ratio cannot be mistaken for a cohort result.
MIN_COMPARABLE_SUITE_MEMBERS = 2


def target_profile_for_backend(backend: str) -> str:
    if backend not in {"cpu", "simd", "gpu"}:
        raise ValueError(f"unsupported benchmark backend: {backend}")
    if PARALLEL_CPU_PROFILE:
        if backend != "cpu":
            raise ValueError("Parallel CPU benchmarks only run the CPU executor")
        return "python-parallel-cpu"
    return f"python-{backend}"


TARGET_PROFILES = tuple(target_profile_for_backend(backend) for backend in TARGET_BACKENDS)
FROZEN_TARGET_PROFILES = ("python-cpu", "python-simd", "python-gpu")


def runtime_backend_for_profile(profile: str) -> str:
    """Return the executor name recorded by pipeline telemetry."""

    return "cpu" if profile == "python-parallel-cpu" else profile.removeprefix("python-")


def benchmark_source_subject_id() -> str:
    """Identify the timed source independently of its comparison cohort."""

    # The x86 comparison cohort times both regular Pillow and Pillow-SIMD.
    # Its workload/backend selection does not identify which source is timed.
    if BENCHMARK_SOURCE_NAME == "Pillow":
        return "pillow"
    if BENCHMARK_SOURCE_NAME == "Pillow-SIMD":
        return "pillow-simd"
    raise ValueError(
        f"unsupported benchmark source identity: {BENCHMARK_SOURCE_NAME!r}"
    )


def benchmark_subjects() -> list[tuple[str, str]]:
    if PARALLEL_CPU_PROFILE:
        # The ordinary Pillow timing is already recorded by the default run.
        # Keep this run's timed cohort to the Rayon-enabled target only; the
        # parity preflight still compares it with Pillow before timing.
        return [("target_profile", "python-parallel-cpu")]
    baseline_id = benchmark_source_subject_id()
    return [
        ("oracle", baseline_id),
        *(
            ("target_profile", target_profile_for_backend(backend))
            for backend in TARGET_BACKENDS
        ),
    ]


def active_target_profiles() -> tuple[str, ...]:
    """Resolve requested profiles from the currently selected backends."""

    return tuple(target_profile_for_backend(backend) for backend in TARGET_BACKENDS)


def suite_subject_is_comparable(
    subject: dict[str, Any], subject_id: str, *, execution_scope: str = "compute_backend"
) -> bool:
    """Return whether a subject can contribute to a paired suite ratio.

    Timing completion and backend proof are intentionally separate states.  A
    target can have a full duration vector while its pipeline receipt is
    missing, partial, or host-controlled.  Such a row remains in the
    independent suite coverage summary, but it must not enter a speedup
    cohort.  The Pillow oracle has an explicit non-pipeline receipt and is
    the only non-terminal exception. Explicit host-boundary workloads may
    compare completed public-call timings without claiming an image-backend
    receipt; their target receipt must carry no backend attribution.
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

    if subject_id in {"pillow", "pillow-simd"}:
        expected_backend = subject_id
        return (
            execution.get("status") == "not_applicable"
            and execution.get("requested_backend") == expected_backend
            and execution.get("actual_backend") == expected_backend
        )

    expected_backend = runtime_backend_for_profile(subject_id)

    if execution_scope == "host_boundary":
        return (
            execution.get("status") == "not_applicable"
            and execution.get("terminal_complete") is False
            and execution.get("requested_backend") == expected_backend
            and execution.get("actual_backend") is None
            and execution.get("actual_backend_counts") == {}
            and execution.get("sample_count") == 0
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
    if subject_id == "python-simd" and simd_vector_evidence(
        {
            "subject": subject_id,
            "operation_telemetry": execution.get("operation_telemetry"),
            "selected_step_ids": execution.get("selected_step_ids"),
        }
    ) != "positive_vector_work":
        return False
    # A representative untimed receipt proves the route for the workload;
    # receipt collection is intentionally outside the latency sample cohort.
    return True


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


def current_oracle_runtime_hash() -> str:
    """Identify the currently configured Pillow oracle without running a case."""

    if ORACLE_NAME != "Pillow":
        raise ValueError("reusable parity currently requires the Pillow oracle")
    python = os.environ.get("MIGRATION_ORACLE_PYTHON", sys.executable)
    script = "\n".join(
        (
            "import json, sys",
            f"sys.path.insert(0, {str(ROOT / 'scripts')!r})",
            "from pathlib import Path",
            "import PIL",
            "from migration_execution_identity import capture_runtime_identity",
            "version = str(getattr(PIL, '__version__', ''))",
            "handshake = {'side': 'source', 'implementation': 'Pillow',",
            "             'version': version, 'path': str(Path(PIL.__file__).resolve()),",
            "             'public_module': 'PIL'}",
            "print(json.dumps(capture_runtime_identity('source', handshake, 'pillow', []),",
            "                  sort_keys=True, separators=(',', ':'))) ",
        )
    )
    environment = os.environ.copy()
    target_python = str((ROOT / "pillow-rs-py" / "python").resolve())
    retained_pythonpath = [
        item
        for item in environment.get("PYTHONPATH", "").split(os.pathsep)
        if item and str(Path(item).resolve()) != target_python
    ]
    if retained_pythonpath:
        environment["PYTHONPATH"] = os.pathsep.join(retained_pythonpath)
    else:
        environment.pop("PYTHONPATH", None)
    environment.pop(TIMED_SOURCE_FLAG, None)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    returncode, stdout, stderr = run_process(
        [python, "-c", script],
        env=environment,
        timeout=120,
        label="Pillow oracle identity preflight",
    )
    if returncode != 0:
        detail = stderr.strip().replace("\n", " ")[-800:]
        raise RuntimeError(f"Pillow oracle identity preflight failed: {detail}")
    try:
        identity = json.loads(stdout)
        stable = identity["stable"]
        stable_sha256 = identity["stable_sha256"]
    except (json.JSONDecodeError, KeyError, TypeError) as exc:
        raise RuntimeError("Pillow oracle identity preflight emitted invalid JSON") from exc
    if (
        stable.get("profile") != "pillow"
        or stable.get("implementation") != ORACLE_NAME
        or stable.get("version") != ORACLE_VERSION
        or not isinstance(stable_sha256, str)
        or len(stable_sha256) != 64
    ):
        raise RuntimeError("Pillow oracle identity differs from the pinned oracle")
    return stable_sha256


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def current_command_argv() -> list[str]:
    """Return the interpreter and arguments that actually invoked this runner."""

    return [sys.executable, *sys.argv]


def current_command_cwd() -> str:
    """Return the working directory of the process producing this report."""

    return os.getcwd()


def is_benchmark_runner_argv(argv: Any) -> bool:
    """Recognize current and legacy identities for benchmark-produced parity."""

    return argv == ["make", "migration-parity-benchmark"] or (
        isinstance(argv, list)
        and len(argv) >= 2
        and isinstance(argv[1], str)
        and Path(argv[1]).name == "run_migration_benchmark.py"
    )


def benchmark_environment() -> dict[str, Any]:
    environment: dict[str, Any] = {
        "machine_id": platform.node() or "unknown",
        "os": platform.platform(),
        "architecture": platform.machine(),
        "cpu": os.environ.get("MIGRATION_BENCHMARK_CPU_MODEL", "").strip()
        or platform.processor()
        or "unknown",
        "memory_bytes": 0,
        "power_mode": "unknown",
        "toolchain": platform.python_version(),
    }
    runner_id = os.environ.get("MIGRATION_BENCHMARK_RUNNER_ID", "").strip()
    if runner_id:
        environment["runner_id"] = runner_id
    return environment


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
        if payload.get("schema") != "migration-parity/parity-input@2":
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
            "argv": current_command_argv(),
            "cwd": current_command_cwd(),
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


def write_parity_artifact(output: Path, result: dict[str, Any]) -> None:
    """Stream a merged parity receipt; use gzip when the path ends in `.gz`."""
    output.parent.mkdir(parents=True, exist_ok=True)
    opener = gzip.open if output.suffix == ".gz" else open
    with opener(output, "wt", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")


def read_and_remove_backend_parity(path: Path) -> dict[str, Any]:
    """Load one completed backend receipt and free its temporary disk copy."""
    try:
        if path.name.endswith(".gz"):
            with gzip.open(path, "rt", encoding="utf-8") as stream:
                return json.load(stream)
        return json.loads(path.read_text(encoding="utf-8"))
    finally:
        path.unlink(missing_ok=True)


def read_parity_artifact(path: Path) -> dict[str, Any]:
    """Read a parity receipt without consuming its source artifact."""

    if path.name.endswith(".gz"):
        with gzip.open(path, "rt", encoding="utf-8") as stream:
            return json.load(stream)
    return json.loads(path.read_text(encoding="utf-8"))


def validate_reused_parity(
    parity: dict[str, Any],
    manifest: Path,
    cases: list[dict[str, Any]],
    *,
    selected_profiles_by_case: dict[str, set[str]],
    report_path: Path,
    attestation: dict[str, Any],
    expected_inputs: list[dict[str, str]],
) -> dict[tuple[str, str], str]:
    """Validate a report-bound parity superset and return its runtime hashes."""

    summary = parity.get("summary", {})
    comparisons = parity.get("comparisons", [])
    comparison_count = len(comparisons) if isinstance(comparisons, list) else -1
    if (
        parity.get("schema") != "migration-parity/parity-result@1"
        or parity.get("status") != "completed"
        or not isinstance(summary, dict)
        or any(
            type(summary.get(field)) is not int or summary[field] != expected
            for field, expected in (
                ("selected", comparison_count),
                ("executed", comparison_count),
                ("passed", comparison_count),
                ("failed", 0),
                ("not_run", 0),
                ("infrastructure_errors", 0),
            )
        )
        or parity.get("infrastructure_errors")
    ):
        raise ValueError("reused parity receipt is not a complete passing run")
    identity = parity.get("identity", {})
    if identity.get("manifest", {}).get("sha256") != sha256(manifest):
        raise ValueError("reused parity receipt has a different manifest")
    oracles = identity.get("oracles", [])
    if not oracles or (
        oracles[0].get("name"), oracles[0].get("version")
    ) != (ORACLE_NAME, ORACLE_VERSION):
        raise ValueError("reused parity receipt has a different oracle")

    input_records = identity.get("inputs")
    if not isinstance(input_records, list):
        raise ValueError("reused parity receipt has no input identities")
    inputs_by_path: dict[str, dict[str, Any]] = {}
    for record in input_records:
        if (
            not isinstance(record, dict)
            or set(record) != {"path", "schema", "sha256"}
            or not isinstance(record.get("path"), str)
            or record["path"] in inputs_by_path
        ):
            raise ValueError("reused parity receipt has invalid input identities")
        inputs_by_path[record["path"]] = record
    for expected_input in expected_inputs:
        if inputs_by_path.get(expected_input.get("path")) != expected_input:
            raise ValueError("reused parity receipt has different selected input bytes")

    try:
        validate_attestation(
            attestation,
            result=parity,
            allow_duplicate_parity_sources=(
                is_benchmark_runner_argv(
                    identity.get("command", {}).get("argv")
                )
            ),
        )
        current_report = execution_report_fingerprint(report_path)
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
        raise ValueError(f"reused parity execution identity is invalid: {exc}") from exc
    bound_report = attestation.get("report", {})
    if any(
        bound_report.get(field) != current_report[field]
        for field in (
            "canonical_sha256",
            "canonical_bytes",
            "container_sha256",
            "container_bytes",
        )
    ):
        raise ValueError("reused parity attestation does not bind the supplied report")

    expected: set[tuple[str, str]] = set()
    for case in cases:
        for profile in TARGET_PROFILES:
            if (
                parity_profile_for_subject(profile)
                in selected_profiles_by_case.get(case["case_id"], set())
            ):
                expected.add((case["case_id"], profile))
    expected_profiles = {profile for _case_id, profile in expected}
    targets = identity.get("targets", [])
    if {item.get("target_profile") for item in targets} != expected_profiles:
        raise ValueError("reused parity receipt has different target profiles")
    for target in targets:
        profile = target.get("target_profile")
        if (
            target.get("target_id") != TARGET_ID
            or target.get("revision") != git_revision()
            or target.get("dirty") is not git_dirty()
            or target.get("backend") != runtime_backend_for_profile(profile)
            or target.get("features") != TARGET_FEATURES
        ):
            raise ValueError("reused parity receipt has different target identity")
    actual_rows = parity["comparisons"]
    actual = {
        (item.get("case_id"), item.get("target_profile"))
        for item in actual_rows
    }
    if len(actual) != len(actual_rows):
        raise ValueError("reused parity receipt contains duplicate case/profile comparisons")
    if not expected <= actual:
        raise ValueError("reused parity receipt does not cover the selected cases/profiles")
    if any(item.get("outcome") != "pass" for item in actual_rows):
        raise ValueError("reused parity receipt contains a non-passing comparison")

    runtime_hashes: dict[tuple[str, str], str] = {}
    required_runtime_profiles = {("source", "pillow")} | {
        ("target", profile) for profile in expected_profiles
    }
    for invocation in attestation.get("invocations", []):
        key = (invocation.get("side"), invocation.get("profile"))
        if (
            key not in required_runtime_profiles
            or invocation.get("role") != "parity"
        ):
            continue
        receipt = invocation.get("receipt")
        runtime = receipt.get("runtime") if isinstance(receipt, dict) else None
        stable_sha256 = runtime.get("stable_sha256") if isinstance(runtime, dict) else None
        if not isinstance(stable_sha256, str) or len(stable_sha256) != 64:
            raise ValueError("reused parity attestation omits a selected runtime fingerprint")
        previous = runtime_hashes.setdefault(key, stable_sha256)
        if previous != stable_sha256:
            raise ValueError("reused parity attestation changes runtime identity within a profile")
    if set(runtime_hashes) != required_runtime_profiles:
        raise ValueError("reused parity attestation omits a selected source or target profile")
    return runtime_hashes


def run_parity(
    manifest: Path,
    output: Path,
    timeout: int,
    cases: list[dict[str, Any]],
    *,
    selected_profiles_by_case: dict[str, set[str]],
    attestation_output: Path | None = None,
    producer_attestation_output: Path | None = None,
) -> dict[str, Any]:
    if producer_attestation_output is not None and attestation_output is None:
        raise ValueError("parity producer attestations require execution attestations")
    script = ROOT / "scripts" / "run_migration_parity.py"
    output.parent.mkdir(parents=True, exist_ok=True)
    backend_results: list[dict[str, Any]] = []
    backend_attestations: list[
        tuple[dict[str, Any], dict[str, Any], dict[str, Any]]
    ] = []
    backend_producer_artifacts: list[tuple[Path, Path, Path]] = []
    with tempfile.TemporaryDirectory(prefix="migration-benchmark-parity-") as directory:
        temporary = Path(directory)
        for backend in TARGET_BACKENDS:
            target_profile = target_profile_for_backend(backend)
            backend_cases = [
                case
                for case in cases
                if parity_profile_for_subject(target_profile)
                in selected_profiles_by_case.get(case["case_id"], set())
            ]
            if not backend_cases:
                continue
            # Full parity receipts include image outputs and can exceed the
            # available disk space as plain JSON. The parity runner streams
            # gzip receipts, so keep its temporary backend artifact compressed
            # too; only the decoded result remains in memory for aggregation.
            backend_output = temporary / f"{backend}.json.gz"
            backend_attestation = temporary / f"{backend}.attestation.json"
            backend_producer_attestation = temporary / f"{backend}.producer.json"
            backend_timeout = (
                min(timeout, 300) if backend == "gpu" else timeout
            )
            parity_environment = os.environ.copy()
            # The parity oracle is always the configured Pillow source. The
            # timed benchmark may use a distinct source such as Pillow-SIMD;
            # do not let its source-selection flag leak into this preflight.
            parity_environment.pop(TIMED_SOURCE_FLAG, None)
            parity_environment.update(
                {
                    "MIGRATION_TARGET_BACKEND": backend,
                    "MIGRATION_STRICT_TARGET_BACKEND": "1",
                    "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
                    + os.pathsep
                    + os.environ.get("PYTHONPATH", ""),
                }
            )
            returncode, stdout, stderr = run_process(
                [
                    sys.executable,
                    str(script),
                    "--manifest",
                    str(manifest),
                    "--output",
                    str(backend_output),
                    "--timeout",
                    str(backend_timeout),
                    *(
                        ["--attestation-output", str(backend_attestation)]
                        if attestation_output is not None
                        else []
                    ),
                    *(
                        [
                            "--producer-attestation-output",
                            str(backend_producer_attestation),
                        ]
                        if producer_attestation_output is not None
                        else []
                    ),
                    *[
                        argument
                        for case in backend_cases
                        for argument in ("--case-id", case["case_id"])
                    ],
                ],
                env=parity_environment,
                timeout=backend_timeout,
                label=f"{backend} parity preflight",
            )
            if not backend_output.is_file():
                detail = stderr.strip().replace("\n", " ")[-800:]
                raise RuntimeError(
                    f"{backend} parity preflight did not emit a result: {detail}"
                )
            try:
                report_binding = (
                    execution_report_fingerprint(backend_output)
                    if attestation_output is not None
                    else None
                )
                result = (
                    read_parity_artifact(backend_output)
                    if attestation_output is not None
                    else read_and_remove_backend_parity(backend_output)
                )
            except (EOFError, OSError, json.JSONDecodeError) as exc:
                detail = " ".join(
                    part.strip().replace("\n", " ")[-800:]
                    for part in (stdout, stderr)
                    if part.strip()
                )
                raise RuntimeError(
                    f"{backend} parity preflight emitted an invalid receipt "
                    f"(exit {returncode}): {exc}; {detail}"
                ) from exc
            if result.get("status") != "completed":
                details = result.get("infrastructure_errors", [])
                detail = json.dumps(details, sort_keys=True)
                raise RuntimeError(
                    f"{backend} parity preflight failed infrastructure checks: {detail}"
                )
            if attestation_output is not None:
                if not backend_attestation.is_file():
                    raise RuntimeError(
                        f"{backend} parity preflight did not emit an execution attestation"
                    )
                try:
                    attestation_document = json.loads(
                        backend_attestation.read_text(encoding="utf-8")
                    )
                except (OSError, json.JSONDecodeError) as exc:
                    raise RuntimeError(
                        f"{backend} parity preflight emitted an invalid execution attestation: {exc}"
                    ) from exc
                assert report_binding is not None
                validate_attestation(
                    attestation_document,
                    result=result,
                    report_binding=report_binding,
                )
                backend_attestations.append(
                    (attestation_document, result, report_binding)
                )
                if producer_attestation_output is not None:
                    producer_bundle = backend_producer_attestation.with_name(
                        backend_producer_attestation.name + ".bundle.json"
                    )
                    if not backend_producer_attestation.is_file() or not producer_bundle.is_file():
                        raise RuntimeError(
                            f"{backend} parity preflight did not emit its producer closure"
                        )
                    backend_producer_artifacts.append(
                        (
                            backend_producer_attestation,
                            backend_attestation,
                            producer_bundle,
                        )
                    )
            if returncode not in {0, 1}:
                detail = stderr.strip().replace("\n", " ")[-800:]
                raise RuntimeError(
                    f"{backend} parity preflight exited {returncode}: {detail}"
                )
            if attestation_output is not None:
                # The decoded result and report fingerprint are enough for
                # aggregation; release the potentially large gzip receipt now.
                backend_output.unlink(missing_ok=True)
            backend_results.append(result)
        merged = merge_parity_results(backend_results, timeout)
        write_parity_artifact(output, merged)
        if attestation_output is not None:
            merge_parity_attestations(
                attestation_output,
                output,
                merged,
                backend_attestations,
            )
            if producer_attestation_output is not None:
                merge_backend_attestations(
                    producer_attestation_output,
                    output,
                    attestation_output,
                    merged,
                    backend_producer_artifacts,
                )
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
    attestation: AttestationCollector | None = None,
    raw_sample_capture: RawSampleCollector | None = None,
    raw_sample_workloads_by_case: dict[str, list[str]] | None = None,
) -> dict[str, Any]:
    effective_timeout = (
        gpu_benchmark_timeout(timeout)
        if side == "target" and backend == "gpu"
        else timeout
    )
    script = ROOT / "scripts" / "run_migration_parity.py"
    raw_sample_group_id = uuid.uuid4().hex if raw_sample_capture is not None else None
    raw_sample_profile = (
        (
            benchmark_source_subject_id()
            if side == "source"
            else target_profile_for_backend(backend)
        )
        if raw_sample_capture is not None
        else None
    )

    def command_for(child_repeat: int, *, include_timings: bool) -> list[str]:
        adapter_python = benchmark_adapter_python(side)
        command = [
            adapter_python,
            str(script),
            "--side",
            side,
            "--manifest",
            str(manifest),
            "--repeat",
            str(child_repeat),
            "--lifecycle",
            lifecycle,
        ]
        if include_timings:
            command.append("--timings")
        command.extend(("--timing-boundary", timing_boundary))
        for step_id in timing_steps:
            command.extend(("--timing-step", step_id))
        return command

    def execute(
        child_cases: list[dict[str, Any]],
        child_repeat: int,
        *,
        include_timings: bool,
        collect_execution: bool = False,
    ) -> dict[str, Any]:
        adapter_environment = benchmark_adapter_environment(side, backend)
        attestation_overrides: dict[str, str] | None = None
        if attestation is not None:
            attestation_overrides = attestation.child_environment(
                side=side,
                profile=(
                    benchmark_source_subject_id()
                    if side == "source"
                    else target_profile_for_backend(backend)
                ),
                role=(
                    "benchmark_execution_proof"
                    if collect_execution
                    else "benchmark_timed"
                ),
                cases=child_cases,
                context={
                    "lifecycle": lifecycle,
                    "boundary": (
                        timing_boundary if include_timings or collect_execution else None
                    ),
                    "step_ids": (
                        (sorted(set(timing_steps)) or ["call"])
                        if include_timings or collect_execution
                        else []
                    ),
                    "repeat": child_repeat,
                },
            )
            adapter_environment.update(attestation_overrides)
        execution_path: Path | None = None
        with tempfile.TemporaryDirectory(
            prefix="migration-benchmark-receipt-"
        ) as temporary:
            if side == "target" and collect_execution:
                execution_path = Path(temporary) / "execution.json"
                adapter_environment["MIGRATION_PARITY_EXECUTION_OUTPUT"] = str(
                    execution_path
                )
            else:
                adapter_environment.pop("MIGRATION_PARITY_EXECUTION_OUTPUT", None)
            try:
                returncode, stdout, stderr = run_process(
                    command_for(child_repeat, include_timings=include_timings),
                    input_text=json.dumps(child_cases, separators=(",", ":")),
                    timeout=effective_timeout,
                    label=(
                        f"{backend} benchmark adapter"
                        if side == "target"
                        else f"{BENCHMARK_SOURCE_NAME} benchmark adapter"
                    ),
                    env=adapter_environment,
                )
            finally:
                if attestation is not None and attestation_overrides is not None:
                    try:
                        attestation.collect(attestation_overrides)
                    except Exception:
                        # Receipt collection is a separate evidence channel;
                        # it must not replace a completed sample or the
                        # adapter's original failure.
                        pass
            if returncode != 0:
                detail = stderr.strip().replace("\n", " ")[-800:]
                raise RuntimeError(f"{side} benchmark adapter failed: {detail}")
            payload = json.loads(stdout)
            if execution_path is not None and not execution_path.is_file():
                raise RuntimeError(
                    f"{side} benchmark adapter did not write its execution receipt"
                )
        if set(payload) != {"identity", "results", "timings_ns", "telemetry", "execution"}:
            raise RuntimeError(f"{side} benchmark adapter emitted invalid timing envelope")
        identity = payload["identity"]
        if side == "source":
            if (
                identity.get("side") != "source"
                or identity.get("implementation") != BENCHMARK_SOURCE_NAME
                or identity.get("version") != BENCHMARK_SOURCE_VERSION
            ):
                raise RuntimeError(
                    f"{BENCHMARK_SOURCE_NAME} benchmark adapter emitted the wrong identity"
                )
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
        if raw_sample_capture is not None and include_timings:
            raw_sample_capture.record_timed_child(
                group_id=raw_sample_group_id or "",
                side=side,
                profile=raw_sample_profile,
                cases=child_cases,
                workload_ids_by_case=raw_sample_workloads_by_case or {},
                planned_repeat=repeat,
                child_repeat=child_repeat,
                lifecycle=lifecycle,
                boundary=timing_boundary,
                step_ids=list(timing_steps),
                timings_ns=payload["timings_ns"],
                telemetry=payload["telemetry"],
                child_invocation_id=(
                    attestation_overrides.get(
                        "MIGRATION_EXECUTION_ATTESTATION_INVOCATION_ID"
                    )
                    if attestation_overrides is not None
                    else None
                ),
            )
        return payload

    if lifecycle != "cold":
        timed = execute(cases, repeat, include_timings=True)
        if side == "target":
            # The representative receipt run proves which backend executes
            # this exact case under the same strict backend profile.  It is
            # deliberately separate from latency samples: receipt collection
            # performs extra work in both Rust and Python and must not inflate
            # only the target's timed calls.
            proof = execute(
                cases,
                1,
                include_timings=False,
                collect_execution=True,
            )
            timed["execution"] = proof["execution"]
        return timed

    # A cold sample is intentionally isolated in a fresh adapter process. This
    # includes GPU adapter/pipeline initialization in the first backend phase
    # and prevents a prior workload from warming the process-local caches.
    merged: dict[str, Any] | None = None
    for case in cases:
        for _ in range(repeat):
            payload = execute([case], 1, include_timings=True)
            if merged is None:
                merged = payload
                continue
            merged["results"].extend(payload["results"])
            for field in ("timings_ns", "telemetry", "execution"):
                for case_id, records in payload[field].items():
                    merged[field].setdefault(case_id, []).extend(records)
    if merged is None:
        raise RuntimeError("cold benchmark adapter received no cases")
    if side == "target":
        proof_execution: dict[str, Any] = {}
        for case in cases:
            proof = execute(
                [case],
                1,
                include_timings=False,
                collect_execution=True,
            )
            proof_execution.update(proof["execution"])
        merged["execution"] = proof_execution
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
        "host_allocation_count",
        "host_allocated_bytes",
    )
    return {
        "sample_count": len(resources),
        **{
            field: execution_numeric_summary(resources, field)
            for field in fields
        },
    }


def execution_operation_telemetry(records: list[dict[str, Any]]) -> list[Any]:
    """Preserve route and vector counts with the timed step that produced them."""
    telemetry: list[Any] = []
    for index, record in enumerate(records):
        if "operation_telemetry" not in record:
            continue
        record_telemetry = record["operation_telemetry"]
        if not isinstance(record_telemetry, list):
            raise ValueError(
                f"execution_records[{index}].operation_telemetry: expected array"
            )
        step_id = record.get("step_id")
        if record_telemetry and (not isinstance(step_id, str) or not step_id):
            raise ValueError(
                f"execution_records[{index}].step_id: expected non-empty string "
                "when operation telemetry is present"
            )
        for operation in record_telemetry:
            if not isinstance(operation, dict):
                raise ValueError(
                    f"execution_records[{index}].operation_telemetry: "
                    "expected objects"
                )
            telemetry.append({**operation, "step_id": step_id})
    return telemetry


def execution_result(
    subject_kind: str,
    subject_id: str,
    execution_records: list[dict[str, Any]],
    policy: dict[str, Any],
    errors: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    errors = list(errors or [])
    if subject_kind == "oracle":
        source_backend = subject_id if subject_id in {"pillow", "pillow-simd"} else "pillow"
        return {
            "status": "not_applicable",
            "terminal_complete": False,
            "requested_backend": source_backend,
            "actual_backend": source_backend,
            "actual_backend_counts": {source_backend: 1},
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
            "operation_telemetry": [],
            "errors": errors,
        }

    # Backend/resource receipts are gathered by one representative untimed
    # execution. They prove the route for this workload, but are not latency
    # samples and need not match the timing sample count. The proof child may
    # execute a full parity workflow even when this workload times selected
    # steps, so route and operation evidence must be scoped to those steps.
    observed_step_ids = (
        set(policy.get("step_ids", []))
        if policy.get("boundary") == "observed_steps"
        else None
    )
    measured = (
        [
            record
            for record in execution_records
            if record.get("step_id") in observed_step_ids
        ]
        if observed_step_ids is not None
        else execution_records
    )
    terminal_records = [
        record for record in execution_records if receipt_terminal_complete(record)
    ]
    completed_candidates = [
        record
        for record in measured
        if record.get("status") == "completed"
        and isinstance(record.get("actual_backend"), str)
        and isinstance(record.get("operation_count"), int)
        and record["operation_count"] > 0
    ]
    cached_candidates = [
        record
        for record in (measured if observed_step_ids is not None else terminal_records)
        if record.get("status") == "cached"
    ]
    if observed_step_ids is None:
        completed = [
            record
            for record in completed_candidates
            if receipt_terminal_complete(record)
        ]
        cached = [
            record for record in cached_candidates if receipt_terminal_complete(record)
        ]
    else:
        # A selected step can prove its operation even when a later, unrelated
        # step owns the full workflow's terminal marker.
        completed = completed_candidates
        cached = cached_candidates
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
    # The no-fallback predicate covers every receipt in the proof execution. A
    # host-controlled setup/prefix receipt can be followed by a terminal GPU
    # receipt, so counting only selected/completed receipts would falsely admit
    # that row to an actual-backend cohort.
    fallback_counts: dict[str, int] = {}
    for record in execution_records:
        reason = record.get("fallback_reason")
        if isinstance(reason, str) and reason:
            fallback_counts[reason] = fallback_counts.get(reason, 0) + 1
    actual_backends = sorted(actual_counts)
    requested_backend = runtime_backend_for_profile(subject_id)
    terminal_completed_candidates = [
        record
        for record in terminal_records
        if record.get("status") == "completed"
        and isinstance(record.get("actual_backend"), str)
        and isinstance(record.get("operation_count"), int)
        and record["operation_count"] > 0
    ]
    terminal_cached_candidates = [
        record for record in terminal_records if record.get("status") == "cached"
    ]
    complete = len(terminal_records) == 1
    terminal_complete = (
        complete
        and len(terminal_completed_candidates) + len(terminal_cached_candidates) == 1
        and partial_count == 0
        and not errors
    )
    terminal_gap = complete and not terminal_complete
    has_partial = partial_count > 0 or bool(errors) or terminal_gap
    all_not_applicable = (
        bool(measured)
        and not errors
        and not_applicable_count == len(measured)
    )
    host_boundary_without_backend_receipt = (
        policy.get("execution_scope") == "host_boundary"
        and not errors
        and not actual_counts
        and not fallback_counts
        and not completed_candidates
        and not cached_candidates
        and partial_count == 0
        and not has_partial
    )
    if all_not_applicable or host_boundary_without_backend_receipt:
        # Public lifecycle/draw/metadata calls can be valid benchmark timings
        # without entering the compute-pipeline telemetry boundary. Keep that
        # distinction explicit instead of calling the row ``not_proven`` (or
        # implying a backend capability failure); it remains excluded from
        # actual-backend performance cohorts by its null receipt.
        status = "not_applicable"
    elif (
        terminal_complete
        and not has_partial
        and (observed_step_ids is None or completed or cached)
    ):
        status = "completed"
    elif has_partial and (
        completed_candidates
        or cached_candidates
        or completed
        or cached
        or partial_count
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
    # The receipt belongs to this workload's selected operation(s). A later
    # unrelated workflow terminal cannot make a partial or unproven workload
    # complete; keep the explicit terminal bit aligned with the report schema.
    terminal_complete = terminal_complete and status == "completed"
    selected_step_ids: list[str] = []
    for record in completed:
        step_id = record.get("step_id")
        if isinstance(step_id, str) and step_id and step_id not in selected_step_ids:
            selected_step_ids.append(step_id)
    return {
        "status": status,
        "terminal_complete": terminal_complete,
        "selected_step_ids": selected_step_ids,
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
        "operation_telemetry": execution_operation_telemetry(completed),
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
    started_at: str,
    target_profiles: list[str] | tuple[str, ...] | None = None,
) -> dict[str, Any]:
    assets = [
        item
        for item in parity_identity.get("assets", [])
        if item["item_id"] in {case["case_id"] for case in cases}
    ]
    return {
        "run_id": f"migration-benchmark-{uuid.uuid4().hex}",
        "started_at": started_at,
        "finished_at": now(),
        "manifest": {
            "path": str(manifest.relative_to(ROOT)),
            "schema": "migration-parity/manifest@3",
            "sha256": sha256(manifest),
        },
        "inputs": [
            {
                "path": path,
                "schema": (
                    "migration-parity/benchmark-input@1"
                    if "/benchmark/" in path
                    else "migration-parity/parity-input@2"
                ),
                "sha256": sha256(FIXTURE_ROOT / path),
            }
            for path in input_paths
        ],
        "assets": assets,
        "oracles": parity_identity["oracles"],
        # The parity preflight may select only the profiles declared by the
        # public operation. Use the selected workloads' subject union so the
        # result identity names every timed profile without adding undeclared
        # backends.
        "targets": benchmark_target_identities(parity_identity, target_profiles),
        "command": {
            "command_id": "benchmark",
            "argv": current_command_argv(),
            "cwd": current_command_cwd(),
            "timeout_seconds": timeout,
        },
    }


def execution_identity(
    target_profiles: list[str] | tuple[str, ...] | None = None,
) -> dict[str, Any]:
    """Return identity headers for a benchmark with no parity-backed cases."""

    requested_profiles = TARGET_PROFILES if target_profiles is None else target_profiles
    backends_by_profile = {
        target_profile_for_backend(backend): backend for backend in TARGET_BACKENDS
    }
    return {
        "oracles": [
            {
                "oracle_id": ORACLE_ID,
                "name": ORACLE_NAME,
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
                "features": TARGET_FEATURES.copy(),
            }
            for profile in requested_profiles
            for backend in [backends_by_profile[profile]]
        ],
    }


def benchmark_target_identities(
    parity_identity: dict[str, Any],
    target_profiles: list[str] | tuple[str, ...] | None = None,
) -> list[dict[str, Any]]:
    """Return identities for every selected target profile timed by this run.

    Preserve matching preflight records (which carry the exact child identity)
    and fill missing profile records from the local execution identity.
    """

    records = {
        item["target_profile"]: item for item in execution_identity()["targets"]
    }
    records.update(
        {item["target_profile"]: item for item in parity_identity.get("targets", [])}
    )
    requested = TARGET_PROFILES if target_profiles is None else target_profiles
    return [records[profile] for profile in requested]


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
        "assets": input_spec["assets"],
        "steps": input_spec["steps"],
        "observations": input_spec["observations"],
        # Private adapter metadata; it is not emitted in a parity or
        # benchmark result artifact.
        "_benchmark_cache_state": workload["context"]["cache_state"],
    }


def select_workloads(
    workloads: dict[str, dict[str, Any]],
    *,
    pipeline: bool,
    workload_ids: list[str] | None,
    limit: int | None,
) -> list[dict[str, Any]]:
    """Select benchmark workloads while applying filters independently."""

    selected = list(workloads.values())
    if pipeline:
        selected = [
            workload
            for workload in selected
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

    if workload_ids:
        missing = [
            workload_id for workload_id in workload_ids if workload_id not in workloads
        ]
        if missing:
            raise ValueError(f"unknown benchmark workload: {missing[0]}")
        selected_ids = {workload["workload_id"] for workload in selected}
        outside_selection = [
            workload_id
            for workload_id in workload_ids
            if workload_id not in selected_ids
        ]
        if outside_selection:
            raise ValueError(
                f"benchmark workload is not in the selected pipeline set: "
                f"{outside_selection[0]}"
            )
        selected = [workloads[workload_id] for workload_id in workload_ids]

    if limit is not None:
        selected = selected[:limit]
    return selected


def workload_subjects_for_profile(
    subjects: Any,
) -> list[dict[str, Any]]:
    """Intersect one frozen subject contract with the active backend profile.

    The source subject stays distinct for the Pillow-SIMD cohort. Parallel CPU
    is derived only from an input that declares serial CPU, and is timed under
    its own feature-identified profile after the same Pillow parity preflight.
    """

    declared = validate_frozen_subject_contract(subjects)
    declared_targets = {
        subject["id"]
        for subject in declared
        if subject["kind"] == "target_profile"
    }
    if PARALLEL_CPU_PROFILE:
        if "python-cpu" not in declared_targets:
            raise ValueError(
                "Parallel CPU benchmark input must declare the serial CPU profile"
            )
        return [{"kind": "target_profile", "id": "python-parallel-cpu"}]

    selected_targets = [
        profile for profile in active_target_profiles() if profile in declared_targets
    ]
    if not selected_targets:
        raise ValueError(
            "benchmark subject contract declares no target profile requested by "
            "the active backend selection"
        )
    baseline_id = benchmark_source_subject_id()
    return [
        {"kind": "oracle", "id": baseline_id},
        *(
            {"kind": "target_profile", "id": profile}
            for profile in selected_targets
        ),
    ]


def validate_frozen_subject_contract(
    subjects: Any,
) -> list[dict[str, str]]:
    """Validate the portable subjects stored in a frozen benchmark input."""

    if not isinstance(subjects, list) or not subjects:
        raise ValueError("benchmark input subjects must be a non-empty array")
    validated: list[dict[str, str]] = []
    seen_ids: set[str] = set()
    oracle_count = 0
    target_count = 0
    for index, subject in enumerate(subjects):
        if not isinstance(subject, dict) or set(subject) != {"kind", "id"}:
            raise ValueError(
                f"benchmark input subjects[{index}] must contain exactly kind and id"
            )
        kind = subject["kind"]
        subject_id = subject["id"]
        if not isinstance(kind, str) or not isinstance(subject_id, str):
            raise ValueError(
                f"benchmark input subjects[{index}] kind and id must be strings"
            )
        if subject_id in seen_ids:
            raise ValueError(f"benchmark input declares duplicate subject {subject_id!r}")
        seen_ids.add(subject_id)
        if kind == "oracle":
            if subject_id != "pillow":
                raise ValueError(
                    f"benchmark input declares unknown oracle subject {subject_id!r}"
                )
            oracle_count += 1
        elif kind == "target_profile":
            if subject_id not in FROZEN_TARGET_PROFILES:
                raise ValueError(
                    f"benchmark input declares unknown target profile {subject_id!r}"
                )
            target_count += 1
        else:
            raise ValueError(
                f"benchmark input declares unknown subject kind {kind!r}"
            )
        validated.append({"kind": kind, "id": subject_id})
    if oracle_count != 1:
        raise ValueError("benchmark input must declare exactly one Pillow oracle subject")
    if target_count == 0:
        raise ValueError("benchmark input must declare at least one target profile")
    return validated


def frozen_workload_subjects(
    workloads: list[dict[str, Any]],
) -> dict[str, list[dict[str, str]]]:
    """Snapshot validated subject contracts before any active-profile rewrite."""

    snapshots: dict[str, list[dict[str, str]]] = {}
    for workload in workloads:
        workload_id = workload.get("workload_id")
        if not isinstance(workload_id, str) or not workload_id:
            raise ValueError("benchmark workload must have a non-empty workload_id")
        if workload_id in snapshots:
            raise ValueError(f"duplicate selected benchmark workload: {workload_id}")
        try:
            snapshots[workload_id] = validate_frozen_subject_contract(
                workload.get("subjects")
            )
        except ValueError as exc:
            raise ValueError(f"{workload_id}: {exc}") from exc
    return snapshots


def apply_profile_to_workloads(
    selected_workloads: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Build per-run subjects while preserving the frozen input documents."""

    profiled: list[dict[str, Any]] = []
    for workload in selected_workloads:
        selected = dict(workload)
        selected["subjects"] = workload_subjects_for_profile(
            workload.get("subjects")
        )
        profiled.append(selected)
    return profiled


def workload_target_profiles(workload: dict[str, Any]) -> list[str]:
    return [
        subject["id"]
        for subject in workload["subjects"]
        if subject["kind"] == "target_profile"
    ]


def parity_profile_for_subject(subject_profile: str) -> str:
    """Map the derived Parallel CPU timing subject to its serial parity lane."""

    return "python-cpu" if subject_profile == "python-parallel-cpu" else subject_profile


def applicable_parity_profiles(
    target_profiles: list[str],
    *,
    active_profiles: tuple[str, ...] | list[str] | None = None,
) -> list[str]:
    """Return this workload's selected profiles covered by its parity case."""

    requested = {parity_profile_for_subject(profile) for profile in target_profiles}
    active = TARGET_PROFILES if active_profiles is None else active_profiles
    return [
        profile
        for profile in active
        if parity_profile_for_subject(profile) in requested
    ]


def parity_gate_outcome(
    case_id: str,
    target_profiles: list[str],
    applicable_profiles: list[str],
    comparisons: dict[tuple[str, str], dict[str, Any]],
) -> str:
    """Require passing parity for every declared timed target subject.

    A failed executed comparison is a failure. A profile omitted from the
    case's applicability set, or a missing/non-passing receipt, leaves the
    workload unproven without turning absent evidence into a pass.
    """

    outcomes = {
        profile: comparisons.get((case_id, profile), {}).get("outcome")
        for profile in applicable_profiles
    }
    if any(outcome == "fail" for outcome in outcomes.values()):
        return "fail"
    if not target_profiles or set(target_profiles) != set(applicable_profiles):
        return "not_proven"
    if any(outcomes.get(profile) != "pass" for profile in target_profiles):
        return "not_proven"
    return "pass"


def parity_timing_allowed(outcome: str) -> bool:
    """Allow execution-only timing unless an executed parity lane failed."""

    return outcome in {"pass", "not_proven"}


def selected_benchmark_subjects(
    workloads: list[dict[str, Any]],
) -> list[tuple[str, str]]:
    """Return active result subjects actually declared by selected workloads."""

    declared_ids = {
        subject["id"]
        for workload in workloads
        for subject in workload["subjects"]
    }
    return [
        (kind, subject_id)
        for kind, subject_id in benchmark_subjects()
        if subject_id in declared_ids
    ]


def validate_selected_workloads(
    selected_workloads: list[dict[str, Any]],
    original_subjects: dict[str, list[dict[str, str]]],
) -> None:
    if not selected_workloads:
        raise ValueError("no benchmark workloads selected")
    selected_ids = [workload.get("workload_id") for workload in selected_workloads]
    if len(selected_ids) != len(set(selected_ids)) or set(selected_ids) != set(
        original_subjects
    ):
        raise ValueError(
            "selected benchmark workloads do not match their original frozen contracts"
        )
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        original = original_subjects.get(workload_id)
        if original is None:
            raise ValueError(
                f"{workload_id}: missing original frozen benchmark subject contract"
            )
        frozen = validate_frozen_subject_contract(original)
        expected_subjects = workload_subjects_for_profile(frozen)
        if workload.get("subjects") != expected_subjects:
            raise ValueError(
                f"{workload_id}: benchmark subjects do not preserve the declared "
                "subjects requested by the active target profile"
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
    benchmark_started_at = now()
    attestation_output = getattr(args, "attestation_output", None)
    producer_output = getattr(args, "producer_attestation_output", None)
    raw_samples_output = getattr(args, "raw_samples_output", None)
    parity_attestation_output = getattr(args, "parity_attestation_output", None)
    parity_producer_output = getattr(args, "parity_producer_attestation_output", None)
    reuse_parity_path = getattr(args, "reuse_parity", None)
    reuse_attestation_path = getattr(args, "reuse_parity_attestation", None)
    if reuse_parity_path is not None:
        if reuse_attestation_path is None:
            raise ValueError("--reuse-parity requires --reuse-parity-attestation")
        if attestation_output is None:
            raise ValueError(
                "--reuse-parity requires --attestation-output to compare the loaded target"
            )
    elif reuse_attestation_path is not None:
        raise ValueError("--reuse-parity-attestation requires --reuse-parity")
    if reuse_parity_path is not None and parity_attestation_output is not None:
        raise ValueError(
            "--parity-attestation-output is only available when parity runs in this process"
        )
    if reuse_parity_path is not None and parity_producer_output is not None:
        raise ValueError(
            "--parity-producer-attestation-output is only available when parity runs in this process"
        )
    if parity_producer_output is not None and parity_attestation_output is None:
        raise ValueError(
            "--parity-producer-attestation-output requires --parity-attestation-output"
        )
    if producer_output is not None and attestation_output is None:
        raise ValueError("--producer-attestation-output requires --attestation-output")
    output_paths = [Path(args.output).resolve(), Path(args.parity_output).resolve()]
    output_paths.extend(
        Path(path).resolve()
        for path in (
            attestation_output,
            producer_output,
            raw_samples_output,
            parity_attestation_output,
            parity_producer_output,
        )
        if path is not None
    )
    if len(output_paths) != len(set(output_paths)):
        raise ValueError("benchmark output artifacts must use distinct paths")
    input_paths = [
        Path(path).resolve()
        for path in (reuse_parity_path, reuse_attestation_path)
        if path is not None
    ]
    if len(input_paths) != len(set(input_paths)) or set(output_paths) & set(input_paths):
        raise ValueError("reused parity inputs must be distinct from output artifacts")
    if parity_attestation_output is not None and parity_attestation_output.exists():
        raise FileExistsError(
            f"parity attestation artifact already exists: {parity_attestation_output}"
        )
    raw_samples: RawSampleCollector | None = None
    if raw_samples_output is not None:
        raw_sample_path = Path(raw_samples_output).resolve()
        conflicting_paths = [args.output, args.parity_output]
        if attestation_output is not None:
            conflicting_paths.append(Path(attestation_output))
        if producer_output is not None:
            conflicting_paths.append(Path(producer_output))
        if any(Path(path).resolve() == raw_sample_path for path in conflicting_paths):
            raise ValueError("--raw-samples-output must not collide with another artifact")
        raw_samples = RawSampleCollector(raw_sample_path)
    attestation = (
        (AttestationCollector(attestation_output, producer_output=producer_output)
         if producer_output is not None else AttestationCollector(attestation_output))
        if attestation_output is not None
        else None
    )
    manifest = args.manifest.resolve()
    manifest_data = load_manifest(manifest)
    cases_by_id, case_inputs = load_parity_cases(manifest_data)
    workloads, suites_input, workload_inputs = load_benchmarks(manifest_data)
    selected_workloads = select_workloads(
        workloads,
        pipeline=args.pipeline,
        workload_ids=args.workload_id,
        limit=args.limit,
    )
    original_subjects = frozen_workload_subjects(selected_workloads)
    selected_workloads = apply_profile_to_workloads(selected_workloads)
    validate_selected_workloads(selected_workloads, original_subjects)

    workload_cases: dict[str, dict[str, Any]] = {}
    parity_case_ids: list[str] = []
    parity_profiles_by_workload: dict[str, list[str]] = {}
    requested_parity_profiles_by_case: dict[str, set[str]] = {}
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
            workload_cases[workload_id] = dict(cases_by_id[case_id])
            parity_case_ids.append(case_id)
            requested_profiles = {
                parity_profile_for_subject(profile)
                for profile in workload_target_profiles(workload)
            }
            requested_parity_profiles_by_case.setdefault(case_id, set()).update(
                requested_profiles
            )
        elif input_spec["kind"] == "workflow":
            workload_cases[workload_id] = benchmark_workflow_case(workload)
        else:
            raise ValueError(
                f"{workload_id}: benchmark input kind {input_spec['kind']!r} "
                "cannot be executed by the adapter benchmark"
            )

    parity_case_ids = list(dict.fromkeys(parity_case_ids))
    measured_cases: list[dict[str, Any]] = []
    for case_id in parity_case_ids:
        parity_case = dict(cases_by_id[case_id])
        if requested_parity_profiles_by_case[case_id]:
            measured_cases.append(parity_case)
    for workload in selected_workloads:
        if workload["input"]["kind"] != "parity_case":
            continue
        workload_id = workload["workload_id"]
        parity_profiles_by_workload[workload_id] = applicable_parity_profiles(
            workload_target_profiles(workload)
        )
    parity_output = args.parity_output.resolve()
    reuse_runtime_hashes: dict[tuple[str, str], str] = {}
    reuse_parity_proof: dict[str, str] | None = None
    if reuse_parity_path is not None:
        if not measured_cases:
            raise ValueError("--reuse-parity is unused because no parity cases were selected")
        resolved_reuse_path = reuse_parity_path.resolve()
        resolved_reuse_attestation = reuse_attestation_path.resolve()
        parity = read_parity_artifact(resolved_reuse_path)
        parity_attestation = read_parity_artifact(resolved_reuse_attestation)
        expected_inputs = [
            {
                "path": input_path,
                "schema": "migration-parity/parity-input@2",
                "sha256": sha256(FIXTURE_ROOT / input_path),
            }
            for input_path in sorted(
                {case_inputs[case["case_id"]] for case in measured_cases}
            )
        ]
        reuse_runtime_hashes = validate_reused_parity(
            parity,
            manifest,
            measured_cases,
            selected_profiles_by_case=requested_parity_profiles_by_case,
            report_path=resolved_reuse_path,
            attestation=parity_attestation,
            expected_inputs=expected_inputs,
        )
        if reuse_runtime_hashes[("source", "pillow")] != current_oracle_runtime_hash():
            raise ValueError("current Pillow oracle identity differs from reused parity")
        reuse_parity_proof = {
            "report_sha256": parity_attestation["report"]["canonical_sha256"],
            "run_id": parity["identity"]["run_id"],
            "attestation_sha256": sha256(resolved_reuse_attestation),
        }
        write_parity_artifact(parity_output, parity)
    elif measured_cases:
        parity = run_parity(
            manifest,
            parity_output,
            args.timeout,
            measured_cases,
            selected_profiles_by_case=requested_parity_profiles_by_case,
            attestation_output=(
                parity_attestation_output.resolve()
                if parity_attestation_output is not None
                else None
            ),
            producer_attestation_output=(
                parity_producer_output.resolve()
                if parity_producer_output is not None
                else None
            ),
        )
    else:
        started = now()
        parity_identity = {
            "run_id": f"migration-benchmark-no-parity-{uuid.uuid4().hex}",
            "started_at": started,
            "finished_at": now(),
            "manifest": {
                "path": str(manifest.relative_to(ROOT)),
                "schema": "migration-parity/manifest@3",
                "sha256": sha256(manifest),
            },
            "inputs": [],
            "assets": [],
            **execution_identity(
                [
                    profile
                    for profile in TARGET_PROFILES
                    if any(
                        profile in workload_target_profiles(workload)
                        for workload in selected_workloads
                    )
                ]
            ),
            "command": {
                "command_id": "benchmark",
                "argv": current_command_argv(),
                "cwd": current_command_cwd(),
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
        write_parity_artifact(parity_output, parity)
    parity_by_case = {
        (item["case_id"], item["target_profile"]): item
        for item in parity["comparisons"]
    }

    parity_outcome_by_workload = {
        workload_id: parity_gate_outcome(
            workload["input"]["case_id"],
            workload_target_profiles(workload),
            parity_profiles_by_workload.get(workload_id, []),
            parity_by_case,
        )
        for workload_id, workload in (
            (item["workload_id"], item)
            for item in selected_workloads
            if item["input"]["kind"] == "parity_case"
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
        tuple[str, tuple[str, ...], str, int, int, int], dict[str, dict[str, Any]]
    ] = {}
    timing_group_profiles: dict[
        tuple[str, tuple[str, ...], str, int, int, int], dict[str, set[str]]
    ] = {}
    timing_workloads: dict[
        tuple[str, tuple[str, ...], str, int, int, int], list[dict[str, Any]]
    ] = {}
    workload_repeats: dict[str, int] = {}
    for workload in selected_workloads:
        workload_id = workload["workload_id"]
        case_id = workload_cases[workload_id]["case_id"]
        gate = workload["measurement"]["correctness_gate"]
        if workload["input"]["kind"] == "parity_case":
            # Keep measurements as execution-only evidence when parity did
            # not cover every requested profile. Any executed parity failure
            # still blocks timing for this correctness-gated workload.
            eligible = parity_timing_allowed(
                parity_outcome_by_workload.get(workload_id, "not_proven")
            )
        else:
            eligible = gate in {"successful_execution", "not_applicable"}
        if not eligible:
            continue
        policy = workload["measurement"]
        repeat = (
            int(policy["warmup_iterations"])
            + int(policy["measurement_iterations"]) * int(policy["samples"])
        )
        workload_repeats[workload_id] = repeat
        key = (
            policy["boundary"],
            tuple(policy["step_ids"]),
            policy["cache_state"],
            int(policy["warmup_iterations"]),
            int(policy["measurement_iterations"]),
            int(policy["samples"]),
        )
        group_cases = timing_groups.setdefault(key, {})
        group_profiles = timing_group_profiles.setdefault(key, {})
        timed_case = workload_cases[workload_id]
        group_cases.setdefault(case_id, dict(timed_case))
        group_profiles.setdefault(case_id, set()).update(
            workload_target_profiles(workload)
        )
        timing_workloads.setdefault(key, []).append(workload)
    for group_key, group_cases in timing_groups.items():
        (
            timing_boundary,
            timing_step_ids,
            lifecycle,
            warmup_iterations,
            measurement_iterations,
            samples,
        ) = group_key
        timing_cases = list(group_cases.values())
        repeat = warmup_iterations + measurement_iterations * samples
        attestation_kwargs = {"attestation": attestation} if attestation is not None else {}
        sample_workloads = timing_workloads.get(group_key, [])
        def values_by_workload(values: dict[str, Any]) -> dict[str, Any]:
            # A parity case may be timed under several boundaries or sampling
            # policies. Keep each group's values under its workload identity;
            # case IDs alone would let later groups overwrite earlier rows.
            return {
                workload["workload_id"]: values[case_id]
                for workload in sample_workloads
                if (case_id := workload_cases[workload["workload_id"]]["case_id"])
                in values
            }

        if not PARALLEL_CPU_PROFILE:
            source_sample_kwargs: dict[str, Any] = {}
            if raw_samples is not None:
                source_workloads_by_case: dict[str, list[str]] = {}
                for workload in sample_workloads:
                    if not any(
                        subject["kind"] == "oracle"
                        for subject in workload["subjects"]
                    ):
                        continue
                    workload_case_id = workload_cases[workload["workload_id"]][
                        "case_id"
                    ]
                    source_workloads_by_case.setdefault(
                        workload_case_id, []
                    ).append(workload["workload_id"])
                source_sample_kwargs = {
                    "raw_sample_capture": raw_samples,
                    "raw_sample_workloads_by_case": source_workloads_by_case,
                }
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
                **attestation_kwargs,
                **source_sample_kwargs,
            )
            source_timing.update(values_by_workload(source_receipt["timings_ns"]))
            source_phase_telemetry.update(values_by_workload(source_receipt["telemetry"]))
            source_execution.update(values_by_workload(source_receipt["execution"]))
            source_status.update(
                values_by_workload(
                    {item["case_id"]: item["status"] for item in source_receipt["results"]}
                )
            )
            source_execution_errors.update(
                values_by_workload(
                    {
                        item["case_id"]: item.get("execution_errors", [])
                        for item in source_receipt["results"]
                    }
                )
            )
        for backend in TARGET_BACKENDS:
            profile = target_profile_for_backend(backend)
            backend_cases = [
                case
                for case in timing_cases
                if profile
                in timing_group_profiles[group_key].get(case["case_id"], set())
            ]
            if not backend_cases:
                continue
            target_sample_kwargs: dict[str, Any] = {}
            if raw_samples is not None:
                target_workloads_by_case: dict[str, list[str]] = {}
                for workload in sample_workloads:
                    if profile not in workload_target_profiles(workload):
                        continue
                    workload_case_id = workload_cases[workload["workload_id"]][
                        "case_id"
                    ]
                    target_workloads_by_case.setdefault(
                        workload_case_id, []
                    ).append(workload["workload_id"])
                target_sample_kwargs = {
                    "raw_sample_capture": raw_samples,
                    "raw_sample_workloads_by_case": target_workloads_by_case,
                }
            target_receipt = run_timed_side(
                "target",
                manifest,
                backend_cases,
                repeat,
                args.timeout,
                backend=backend,
                timing_boundary=timing_boundary,
                timing_steps=list(timing_step_ids),
                lifecycle=lifecycle,
                **attestation_kwargs,
                **target_sample_kwargs,
            )
            target_timings[profile].update(
                values_by_workload(target_receipt["timings_ns"])
            )
            target_phase_telemetry[profile].update(
                values_by_workload(target_receipt["telemetry"])
            )
            target_execution[profile].update(
                values_by_workload(target_receipt["execution"])
            )
            target_status[profile].update(
                values_by_workload(
                    {item["case_id"]: item["status"] for item in target_receipt["results"]}
                )
            )
            target_execution_errors[profile].update(
                values_by_workload(
                    {
                        item["case_id"]: item.get("execution_errors", [])
                        for item in target_receipt["results"]
                    }
                )
            )

    def timed_success(
        workload: dict[str, Any], workload_id: str, expected_repeat: int
    ) -> bool:
        subjects = workload["subjects"]
        source_required = any(subject["kind"] == "oracle" for subject in subjects)
        target_profiles = workload_target_profiles(workload)
        source_complete = not source_required or (
            source_status.get(workload_id) == "completed"
            and len(source_timing.get(workload_id, [])) == expected_repeat
        )
        return bool(target_profiles) and source_complete and all(
            target_status[profile].get(workload_id) == "completed"
            and len(target_timings[profile].get(workload_id, [])) == expected_repeat
            for profile in target_profiles
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
        expected_repeat = workload_repeats.get(workload_id, 0)
        if not timed_success(workload, workload_id, expected_repeat):
            details = {
                "source_status": (
                    source_status.get(workload_id)
                    if any(subject["kind"] == "oracle" for subject in workload["subjects"])
                    else "not_requested"
                ),
                "source_errors": (
                    source_execution_errors.get(workload_id, [])
                    if any(subject["kind"] == "oracle" for subject in workload["subjects"])
                    else []
                ),
                "targets": {
                    profile: {
                        "status": target_status[profile].get(workload_id),
                        "errors": target_execution_errors[profile].get(workload_id, []),
                    }
                    for profile in workload_target_profiles(workload)
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
        parity_outcome = (
            parity_outcome_by_workload.get(workload_id, "not_proven")
            if workload["input"]["kind"] == "parity_case"
            else "not_proven"
        )
        parity_pass = (
            workload["input"]["kind"] == "parity_case"
            and parity_outcome == "pass"
        )
        parity_fail = (
            workload["input"]["kind"] == "parity_case"
            and parity_outcome == "fail"
        )
        execution_pass = timed_success(
            workload, workload_id, workload_repeats.get(workload_id, 0)
        )
        measured_pass = parity_pass and execution_pass if workload["input"]["kind"] == "parity_case" else execution_pass
        correctness = {
            "gate": workload["measurement"]["correctness_gate"],
            "outcome": (
                "fail"
                if parity_fail
                else "pass"
                if measured_pass
                else "not_proven"
            ),
            "evidence_id": (
                parity["identity"]["run_id"]
                if parity_pass or parity_fail
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
                return source_execution_errors.get(workload_id, [])
            return target_execution_errors.get(subject_id, {}).get(workload_id, [])

        if (workload["input"]["kind"] == "parity_case" and not parity_fail) or benchmark_execution:
            def durations_for(kind: str, subject_id: str) -> list[int]:
                if kind == "oracle":
                    return (
                        source_timing.get(workload_id, [])
                        if source_status.get(workload_id) == "completed"
                        else []
                    )
                return (
                    target_timings.get(subject_id, {}).get(workload_id, [])
                    if target_status.get(subject_id, {}).get(workload_id)
                    == "completed"
                    else []
                )

            subjects = [
                subject_result(
                    kind,
                    subject_id,
                    durations_for(kind, subject_id),
                    (
                        source_phase_telemetry.get(workload_id, [])
                        if kind == "oracle"
                        else target_phase_telemetry.get(subject_id, {}).get(workload_id, [])
                    ),
                    (
                        source_execution.get(workload_id, [])
                        if kind == "oracle"
                        else target_execution.get(subject_id, {}).get(workload_id, [])
                    ),
                    workload["measurement"],
                    errors_for(kind, subject_id),
                )
                for kind, subject_id in (
                    (subject["kind"], subject["id"])
                    for subject in workload["subjects"]
                )
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
                for kind, subject_id in (
                    (subject["kind"], subject["id"])
                    for subject in workload["subjects"]
                )
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
    result_subject_order = selected_benchmark_subjects(selected_workloads)
    suites: list[dict[str, Any]] = []
    for suite in suites_input:
        suite_subjects: list[dict[str, Any]] = []
        comparisons: list[dict[str, Any]] = []
        for subject_kind, subject_id in result_subject_order:
            metric_values: dict[str, list[tuple[float, int]]] = {}
            for member in suite["members"]:
                workload = workload_by_id.get(member["workload_id"])
                if workload is None:
                    continue
                subject = next(
                    (
                        item
                        for item in workload["subjects"]
                        if item["id"] == subject_id
                    ),
                    None,
                )
                if subject is None:
                    continue
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
        if PARALLEL_CPU_PROFILE:
            # The parallel artifact is a target-only timing cohort. Its public
            # Pillow comparison is joined from the default run at export time.
            suites.append({
                "suite_id": suite["suite_id"],
                "members": suite["members"],
                "subjects": suite_subjects,
                "comparisons": [],
            })
            continue
        baseline_subject_id = benchmark_source_subject_id()
        oracle_subject = next(
            item for item in suite_subjects if item["id"] == baseline_subject_id
        )
        oracle_values = {
            item["metric"]: item["weighted_mean"]
            for item in oracle_subject["measurements"]
        }
        declared_members = list(suite["members"])
        declared_member_count = len(declared_members)
        host_boundary_suite = bool(declared_members) and all(
            workload_by_id.get(member["workload_id"], {})
            .get("measurement_policy", {})
            .get("execution_scope")
            == "host_boundary"
            for member in declared_members
        )
        minimum_common_members = (
            1 if host_boundary_suite else MIN_COMPARABLE_SUITE_MEMBERS
        )
        suite_target_profiles = [
            subject_id
            for subject_kind, subject_id in result_subject_order
            if subject_kind == "target_profile"
        ]
        for target_profile in suite_target_profiles:
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
                baseline_subject = subjects_by_id.get(baseline_subject_id, {})
                target_member_subject = subjects_by_id.get(target_profile, {})
                # Keep the timing summary independent from the performance
                # cohort.  Only a value-complete Pillow row and a target row
                # with a terminal requested=actual, no-fallback receipt may
                # contribute to a paired suite ratio.
                if suite_subject_is_comparable(
                    baseline_subject, baseline_subject_id
                ) and suite_subject_is_comparable(
                    target_member_subject,
                    target_profile,
                    execution_scope=workload["measurement_policy"].get(
                        "execution_scope", "compute_backend"
                    ),
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
                baseline_subject_id: {},
                target_profile: {},
            }
            for member in common_members:
                workload = workload_by_id[member["workload_id"]]
                weight = int(member["weight"])
                for subject_id in (baseline_subject_id, target_profile):
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
                set(pair_values[baseline_subject_id])
                & set(pair_values[target_profile])
            )
            for metric in pair_metrics:
                baseline_values = pair_values[baseline_subject_id][metric]
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
                    "baseline_subject": baseline_subject_id,
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
                        if len(common_ids) >= minimum_common_members
                        else "not_comparable"
                    ),
                })
            if not pair_metrics:
                # Retain an explicit non-comparable record when no pair has a
                # measurable metric.  Otherwise a suite with no common IDs
                # would silently disappear from the comparison denominator.
                for metric in sorted(set(oracle_values) | set(target_values) | {"latency"}):
                    comparisons.append({
                        "baseline_subject": baseline_subject_id,
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
        started_at=benchmark_started_at,
        target_profiles=[
            subject_id
            for subject_kind, subject_id in result_subject_order
            if subject_kind == "target_profile"
        ],
    )
    result_identity["finished_at"] = now()
    measured = sum(
        1
        for item in workload_results
        if any(subject["status"] == "completed" for subject in item["subjects"])
    )
    not_run = len(workload_results) - measured
    environment = benchmark_environment()
    result = {
        "schema": "migration-parity/benchmark-result@1",
        "identity": result_identity,
        "status": "completed",
        "environment": environment,
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
    if reuse_runtime_hashes:
        assert attestation is not None
        attestation.require_runtime_identities(
            {
                key: fingerprint
                for key, fingerprint in reuse_runtime_hashes.items()
                if key[0] == "target"
            },
            role="benchmark_execution_proof",
        )
    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    result_path = args.output.resolve()
    if raw_samples is not None:
        raw_samples.publish(
            report=result,
            report_path=result_path,
            workload_case_ids={
                workload["workload_id"]: workload_cases[workload["workload_id"]][
                    "case_id"
                ]
                for workload in selected_workloads
            },
        )
    result_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    if attestation is not None:
        try:
            attestation.finalize(
                result_path,
                result=result,
                selection={
                    "case_profiles": [],
                    "workload_profiles": [
                        {
                            "workload_id": workload["workload_id"],
                            "case_id": workload_cases[workload["workload_id"]][
                                "case_id"
                            ],
                            "subject_id": subject["id"],
                        }
                        for workload in selected_workloads
                        for subject in workload["subjects"]
                    ],
                },
                parity_proof=reuse_parity_proof,
            )
        except Exception:
            # The raw benchmark report is authoritative. A sidecar write
            # failure must not alter its contents or successful exit status.
            pass
    print(json.dumps(result["summary"], sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--attestation-output", type=Path)
    parser.add_argument("--producer-attestation-output", type=Path)
    parser.add_argument(
        "--raw-samples-output",
        type=Path,
        help="retain ordered timing and phase samples in a separate sidecar",
    )
    parser.add_argument("--parity-output", type=Path, default=DEFAULT_PARITY_RESULT)
    parser.add_argument(
        "--reuse-parity",
        type=Path,
        help="reuse a passing Pillow parity report whose selected cases and runtime identities match",
    )
    parser.add_argument(
        "--reuse-parity-attestation",
        type=Path,
        help="report-bound execution attestation for --reuse-parity",
    )
    parser.add_argument(
        "--parity-attestation-output",
        type=Path,
        help="write a report-bound execution attestation for a fresh parity run",
    )
    parser.add_argument(
        "--parity-producer-attestation-output",
        type=Path,
        help="write a producer receipt and bundle for a fresh parity run",
    )
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
