#!/usr/bin/env python3
"""Run the complete safe test campaign across every supported target lane.

The Python parity adapter selects one target backend per process, so a single
parity process cannot honestly exercise CPU, SIMD, and GPU at once.  This
orchestrator keeps the campaign as one maintained command while running the
active corpus once per backend.  It also sends the same public corpus through
Node WASM and a real browser WASM page, recording both lanes and their
capability probes in one generated evidence file.

GPU is gated by one bounded public parity case, then the full GPU corpus is
requested by default with a separate hard deadline.  Adapter absence is
recorded as explicitly skipped/not proven.  A timeout, crash, parity mismatch,
or other real GPU failure remains fatal.  Every child lane starts in its own
process group so a native driver wedge cannot outlive the lane deadline.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "all-backends-test-result.json"
DEFAULT_MANIFEST = ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"
DEFAULT_TIMEOUT_SECONDS = 7200
GPU_SMOKE_TIMEOUT_SECONDS = 180
GPU_FULL_TIMEOUT_SECONDS = 300
PROCESS_REAP_TIMEOUT_SECONDS = 10
GIT_COMMAND_TIMEOUT_SECONDS = 10
GPU_SMOKE_CASE = "PIL.ImageFilter.UnsharpMask.behavior.default"
GPU_FULL_DISABLED_REASON = (
    "full GPU parity was disabled explicitly; the bounded smoke gate passed, "
    "but the full GPU corpus remains not proven"
)
BACKENDS = ("cpu", "simd")
WGSL_ROOT = ROOT / "pillow-rs" / "src" / "compute" / "pool_gpu" / "shaders"


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def git_revision() -> str:
    returncode, stdout, _stderr, timed_out = run_command(
        ["git", "rev-parse", "HEAD"],
        timeout_seconds=GIT_COMMAND_TIMEOUT_SECONDS,
    )
    if returncode == 0 and not timed_out:
        return stdout.strip()
    return "unknown"


def process_group_options() -> dict[str, Any]:
    """Return platform options that isolate one complete child lane."""

    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {"start_new_session": True}


def kill_process_group(process: subprocess.Popen[str]) -> None:
    """Hard-stop the child and all descendants in its isolated group."""

    if os.name == "nt":
        try:
            killer = subprocess.Popen(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                cwd=ROOT,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                **process_group_options(),
            )
        except OSError:
            process.kill()
            return
        try:
            killer.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            killer.kill()
            killer.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        return
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def reap_timed_out_process(
    process: subprocess.Popen[str],
) -> tuple[str, str]:
    """Kill the isolated group and reap its direct child without waiting forever."""

    kill_process_group(process)
    try:
        return process.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        process.kill()
        try:
            return process.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError(
                "timed-out process group did not exit after hard termination"
            ) from exc


def run_command(
    command: list[str],
    *,
    env: dict[str, str] | None = None,
    timeout_seconds: int,
) -> tuple[int, str, str, bool]:
    """Run one maintained make/npm lane with a bounded process lifetime."""

    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    if timeout_seconds <= 0:
        raise ValueError("command timeout must be positive")
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=merged_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **process_group_options(),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stdout, stderr = reap_timed_out_process(process)
        return 124, stdout, stderr, True
    return process.returncode, stdout, stderr, False


def parity_lane_timeout(
    backend: str, *, requested_seconds: int, smoke: bool
) -> int:
    """Return the outer deadline for one parity lane."""

    if requested_seconds <= 0:
        raise ValueError("parity lane timeout must be positive")
    if backend != "gpu":
        return requested_seconds
    hard_limit = GPU_SMOKE_TIMEOUT_SECONDS if smoke else GPU_FULL_TIMEOUT_SECONDS
    return min(requested_seconds, hard_limit)


def result_summary(path: Path) -> dict[str, Any] | None:
    value = result_document(path)
    if value is None:
        return None
    summary = value.get("summary")
    return summary if isinstance(summary, dict) else None


def result_document(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def wgsl_inventory() -> list[dict[str, Any]]:
    """Describe every checked-in WGSL asset without calling it coverage."""

    if not WGSL_ROOT.is_dir():
        return []
    inventory: list[dict[str, Any]] = []
    for path in sorted(WGSL_ROOT.glob("*.wgsl")):
        source = path.read_text(encoding="utf-8")
        inventory.append(
            {
                "shader_file": path.name,
                "source_path": path.relative_to(ROOT).as_posix(),
                "source_lines": len(source.splitlines()),
                "compute_entrypoints": source.count("@compute"),
                "conditional_sites": len(re.findall(r"\bif\s*\(", source)),
                "loop_sites": len(re.findall(r"\b(?:for|loop|while)\b", source)),
            }
        )
    return inventory


def merge_gpu_shader_coverage(
    path: Path,
    *,
    scope: dict[str, Any],
    reason: str,
) -> dict[str, Any]:
    """Attach static WGSL inventory to target-side dispatch telemetry."""

    raw = result_document(path)
    records = raw.get("records", []) if isinstance(raw, dict) else []
    if not isinstance(records, list):
        records = []
    records = [record for record in records if isinstance(record, dict)]
    by_file: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        shader_file = record.get("shader_file")
        if isinstance(shader_file, str) and shader_file:
            by_file.setdefault(shader_file, []).append(record)

    inventory = wgsl_inventory()
    inventory_by_file = {item["shader_file"]: item for item in inventory}
    files: list[dict[str, Any]] = []
    for item in inventory:
        file_records = sorted(
            by_file.get(item["shader_file"], []),
            key=lambda record: str(record.get("variant_name", "")),
        )
        dispatches = sum(int(record.get("dispatches", 0)) for record in file_records)
        workgroups = sum(int(record.get("workgroups", 0)) for record in file_records)
        files.append(
            {
                **item,
                "status": "executed" if dispatches else "not_executed",
                "dispatches": dispatches,
                "workgroups": workgroups,
                "variants": file_records,
            }
        )
    for shader_file in sorted(set(by_file) - set(inventory_by_file)):
        files.append(
            {
                "shader_file": shader_file,
                "source_path": None,
                "source_lines": None,
                "compute_entrypoints": None,
                "conditional_sites": None,
                "loop_sites": None,
                "status": "executed",
                "dispatches": sum(
                    int(record.get("dispatches", 0))
                    for record in by_file[shader_file]
                ),
                "workgroups": sum(
                    int(record.get("workgroups", 0))
                    for record in by_file[shader_file]
                ),
                "variants": sorted(
                    by_file[shader_file],
                    key=lambda record: str(record.get("variant_name", "")),
                ),
                "inventory_status": "missing_from_checkout_inventory",
            }
        )

    executed_files = sum(item["status"] == "executed" for item in files)
    dispatches = sum(item["dispatches"] for item in files)
    workgroups = sum(item["workgroups"] for item in files)
    measured = bool(records)
    result = {
        "schema": "migration-parity/gpu-wgsl-coverage@1",
        "status": "measured" if measured else "not_measured",
        "reason": (
            str(raw.get("reason"))
            if isinstance(raw, dict) and raw.get("reason")
            else reason
        ),
        "backend": "gpu",
        "scope": scope,
        "execution": {
            "shader_variants_executed": len(records),
            "shader_files_executed": executed_files,
            "shader_files_declared": len(inventory),
            "dispatches": dispatches,
            "workgroups": workgroups,
        },
        "inventory": {
            "root": WGSL_ROOT.relative_to(ROOT).as_posix(),
            "files": files,
        },
        "source_coverage": (
            raw.get("source_coverage")
            if isinstance(raw, dict) and isinstance(raw.get("source_coverage"), dict)
            else {
                "status": "not_measured",
                "reason": (
                    "WGSL source line and branch instrumentation is not enabled; "
                    "this artifact proves runtime shader dispatch only."
                ),
            }
        ),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return result


def public_case_ids(manifest_path: Path = DEFAULT_MANIFEST) -> list[str]:
    """Load the canonical public parity IDs used by every backend lane."""

    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    ids: list[str] = []
    fixture_root = manifest_path.parent
    for relative in manifest["input_index"]["parity"]:
        document = json.loads((fixture_root / relative).read_text(encoding="utf-8"))
        ids.extend(case["case_id"] for case in document["cases"])
    if len(ids) != len(set(ids)):
        raise ValueError("public parity input IDs are not unique")
    return sorted(ids)


def public_case_scope(requested: list[str] | None) -> dict[str, Any]:
    """Return one deterministic scope receipt shared by all child lanes."""

    all_ids = public_case_ids()
    all_id_set = set(all_ids)
    requested_ids = sorted(set(requested or []))
    unknown = sorted(set(requested_ids) - all_id_set)
    if unknown:
        raise ValueError(f"unknown public parity case IDs: {unknown}")
    selected = requested_ids if requested_ids else all_ids
    digest = hashlib.sha256(("\n".join(selected) + "\n").encode()).hexdigest()
    return {
        "kind": "public-parity-corpus",
        "selection": "explicit-case-filter" if requested_ids else "all-public-cases",
        "selected": len(selected),
        "case_ids_sha256": digest,
        "filter": requested_ids or None,
    }


def scope_with_execution(scope: dict[str, Any], *, executed: int, pending: int = 0) -> dict[str, Any]:
    result = dict(scope)
    result["executed"] = executed
    result["pending"] = pending
    return result


def pipeline_execution_evidence(
    path: Path,
    *,
    expected_scope: dict[str, Any],
    expected_backend: str,
) -> dict[str, Any]:
    """Validate the normal-parity backend receipt sidecar for one lane."""

    relative_path = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.as_posix()
    raw = result_document(path)
    if not isinstance(raw, dict):
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar was not produced",
            "artifact": relative_path,
        }
    if raw.get("schema") != "migration-parity/pipeline-execution-evidence@1":
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar has an unsupported schema",
            "artifact": relative_path,
        }
    identity = raw.get("identity")
    scope = raw.get("scope")
    summary = raw.get("summary")
    if (
        not isinstance(identity, dict)
        or identity.get("backend") != expected_backend
        or not isinstance(scope, dict)
        or scope.get("kind") != expected_scope.get("kind")
        or scope.get("selected") != expected_scope.get("selected")
        or scope.get("case_ids_sha256") != expected_scope.get("case_ids_sha256")
        or not isinstance(summary, dict)
    ):
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar identity or scope did not match the lane",
            "artifact": relative_path,
        }
    required_summary = {
        "selected",
        "receipt_cases",
        "not_recorded_cases",
        "completed_receipts",
        "actual_backend_counts",
        "fallback_reason_counts",
    }
    if set(summary) != required_summary:
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar summary is incomplete",
            "artifact": relative_path,
        }
    if (
        summary["selected"] != expected_scope.get("selected")
        or summary["receipt_cases"] + summary["not_recorded_cases"]
        != summary["selected"]
        or sum(summary["actual_backend_counts"].values())
        != summary["completed_receipts"]
    ):
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar summary is inconsistent",
            "artifact": relative_path,
        }
    return {
        "status": "measured",
        "reason": str(raw.get("reason", "")),
        "artifact": relative_path,
        "summary": summary,
    }


def gpu_adapter_unavailable(result: dict[str, Any] | None) -> bool:
    """Return whether a GPU smoke result stopped before kernel execution."""

    if result is None:
        return False
    marker = "GPU adapter not available"
    for error in result.get("infrastructure_errors", []):
        if isinstance(error, dict) and marker in str(error.get("message", "")):
            return True
    for comparison in result.get("comparisons", []):
        if not isinstance(comparison, dict):
            continue
        target = comparison.get("target", {})
        if not isinstance(target, dict):
            continue
        for observation in target.get("observations", []):
            if not isinstance(observation, dict):
                continue
            error = observation.get("error", {})
            if isinstance(error, dict) and marker in str(error.get("message", "")):
                return True
    return False


def lane_record(
    *,
    lane_id: str,
    kind: str,
    backend: str | None,
    command: list[str],
    status: str,
    returncode: int | None,
    artifact: Path | None = None,
    summary: dict[str, Any] | None = None,
    scope: dict[str, Any] | None = None,
    shader_coverage: dict[str, Any] | None = None,
    shader_coverage_artifact: Path | None = None,
    execution_evidence: dict[str, Any] | None = None,
    capabilities: dict[str, Any] | None = None,
    timed_out: bool = False,
    reason: str | None = None,
) -> dict[str, Any]:
    record: dict[str, Any] = {
        "lane_id": lane_id,
        "kind": kind,
        "backend": backend,
        "command": command,
        "status": status,
        "returncode": returncode,
        "timed_out": timed_out,
    }
    if artifact is not None:
        try:
            record["artifact"] = artifact.relative_to(ROOT).as_posix()
        except ValueError:
            record["artifact"] = artifact.as_posix()
    if summary is not None:
        record["summary"] = summary
    if scope is not None:
        record["scope"] = scope
    if shader_coverage is not None:
        record["shader_coverage"] = shader_coverage
    if shader_coverage_artifact is not None:
        try:
            record["shader_coverage_artifact"] = shader_coverage_artifact.relative_to(
                ROOT
            ).as_posix()
        except ValueError:
            record["shader_coverage_artifact"] = shader_coverage_artifact.as_posix()
    if execution_evidence is not None:
        record["execution_evidence"] = execution_evidence
    if capabilities is not None:
        record["capabilities"] = capabilities
    if reason is not None:
        record["reason"] = reason
    return record


def run_parity_lane(
    backend: str,
    *,
    output_dir: Path,
    timeout_seconds: int,
    case_ids: list[str],
    scope: dict[str, Any],
    smoke_case: str | None = None,
    smoke: bool = False,
) -> dict[str, Any]:
    artifact = output_dir / (
        f"parity-{backend}-smoke.json" if smoke else f"parity-{backend}.json"
    )
    shader_artifact = (
        output_dir / ("gpu-wgsl-coverage-smoke.json" if smoke else "gpu-wgsl-coverage.json")
        if backend == "gpu"
        else None
    )
    execution_artifact = output_dir / (
        f"parity-{backend}-smoke-execution.json"
        if smoke
        else f"parity-{backend}-execution.json"
    )
    if smoke:
        selected_smoke_case = smoke_case or GPU_SMOKE_CASE
        command = [
            "make",
            "migration-parity-case",
            f"MIGRATION_PARITY_CASE={selected_smoke_case}",
            f"MIGRATION_PARITY_CASE_OUTPUT={artifact}",
        ]
    else:
        command = [
            "make",
            "migration-parity-test",
            f"MIGRATION_PARITY_OUTPUT={artifact}",
        ]
    artifact.unlink(missing_ok=True)
    if shader_artifact is not None:
        shader_artifact.unlink(missing_ok=True)
    execution_artifact.unlink(missing_ok=True)
    lane_timeout = parity_lane_timeout(
        backend, requested_seconds=timeout_seconds, smoke=smoke
    )
    returncode, stdout, stderr, timed_out = run_command(
        command,
        env={
            "MIGRATION_TARGET_BACKEND": backend,
            **(
                {
                    # Keep the normal GPU lane on the target's documented
                    # fallback behavior.  The sidecar records only actual
                    # WGSL dispatches, so fallback execution cannot be
                    # mistaken for shader coverage.  Strict GPU-only
                    # capability auditing belongs in its own lane.
                    "MIGRATION_GPU_WGSL_COVERAGE_OUTPUT": str(shader_artifact),
                }
                if shader_artifact is not None
                else {}
            ),
            "MIGRATION_PARITY_EXECUTION_OUTPUT": str(execution_artifact),
            **({"MIGRATION_PARITY_CASE_IDS": ",".join(case_ids)} if case_ids and not smoke else {}),
        },
        timeout_seconds=lane_timeout,
    )
    result = result_document(artifact)
    summary = result.get("summary") if isinstance(result, dict) else None
    if not isinstance(summary, dict):
        summary = None
    adapter_unavailable = smoke and gpu_adapter_unavailable(result)
    if adapter_unavailable and not timed_out:
        status = "skipped"
    else:
        status = (
            "passed"
            if returncode == 0 and summary is not None and not timed_out
            else "failed"
        )
    if timed_out:
        stage = "GPU smoke" if smoke else f"{backend.upper()} parity"
        reason = f"bounded {stage} timeout after {lane_timeout}s"
    elif adapter_unavailable:
        reason = "GPU adapter unavailable; full GPU lane was not executed"
    elif returncode != 0:
        reason = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    elif summary is None:
        reason = "validated parity artifact was not produced"
    else:
        reason = None
    shader_summary = None
    if shader_artifact is not None:
        shader_document = merge_gpu_shader_coverage(
            shader_artifact,
            scope=scope_with_execution(
                scope if not smoke else public_case_scope([smoke_case or GPU_SMOKE_CASE]),
                executed=(summary or {}).get("executed", 0),
            ),
            reason=reason or "GPU parity lane completed without shader telemetry.",
        )
        shader_summary = {
            "status": shader_document["status"],
            "reason": shader_document["reason"],
        }
    expected_execution_scope = (
        scope
        if not smoke
        else public_case_scope([smoke_case or GPU_SMOKE_CASE])
    )
    execution_summary = pipeline_execution_evidence(
        execution_artifact,
        expected_scope=expected_execution_scope,
        expected_backend=backend,
    )
    record = lane_record(
        lane_id=f"parity-{backend}{'-smoke' if smoke else ''}",
        kind="python-py3-parity",
        backend=backend,
        command=command,
        status=status,
        returncode=returncode,
        artifact=artifact,
        summary=summary,
        scope=scope_with_execution(
            scope if not smoke else public_case_scope([smoke_case or GPU_SMOKE_CASE]),
            executed=(summary or {}).get("executed", 0),
        ),
        shader_coverage=shader_summary,
        shader_coverage_artifact=shader_artifact,
        execution_evidence=execution_summary,
        timed_out=timed_out,
        reason=reason,
    )
    if status != "passed" and not smoke:
        record["output_tail"] = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    return record


def wasm_lane_record(
    *,
    lane_id: str,
    kind: str,
    artifact: Path,
    result: dict[str, Any] | None,
    command: list[str],
    returncode: int,
    stdout: str,
    stderr: str,
    timed_out: bool,
    expected_scope: dict[str, Any],
) -> dict[str, Any]:
    summary = result.get("summary") if isinstance(result, dict) else None
    if not isinstance(summary, dict):
        summary = None
    actual_scope = result.get("scope") if isinstance(result, dict) else None
    if not isinstance(actual_scope, dict):
        actual_scope = expected_scope
    artifact_passed = (
        isinstance(result, dict)
        and result.get("status") == "completed"
        and summary is not None
        and summary.get("failed") == 0
        and summary.get("infrastructure_errors") == 0
    )
    status = "passed" if artifact_passed and not timed_out else "failed"
    reason = None
    if timed_out:
        reason = "bounded command timeout"
    elif not artifact_passed:
        reason = (stderr or stdout).strip().replace("\n", " ")[-1000:]
        if not reason:
            reason = "validated WASM parity artifact was not produced or contains failures"
    elif actual_scope.get("pending", 0):
        reason = (
            f"{actual_scope['pending']} selected public workflows are pending in the "
            "WASM facade; see the lane artifact for per-reason accounting"
        )
    shader_coverage = result.get("shader_coverage") if isinstance(result, dict) else None
    capabilities = result.get("capabilities") if isinstance(result, dict) else None
    execution_evidence: dict[str, Any] | None = None
    if isinstance(result, dict):
        raw_execution = result.get("execution_evidence")
        relative_artifact = (
            artifact.relative_to(ROOT).as_posix()
            if artifact.is_relative_to(ROOT)
            else artifact.as_posix()
        )
        if not isinstance(raw_execution, dict):
            execution_evidence = {
                "status": "not_measured",
                "reason": "WASM adapter did not emit pipeline execution evidence",
                "artifact": relative_artifact,
            }
        else:
            execution_scope = raw_execution.get("scope")
            execution_summary = raw_execution.get("summary")
            if (
                raw_execution.get("schema")
                != "migration-parity/pipeline-execution-evidence@1"
                or raw_execution.get("status") != "measured"
                or not isinstance(execution_scope, dict)
                or execution_scope.get("kind") != expected_scope.get("kind")
                or execution_scope.get("selected") != expected_scope.get("selected")
                or execution_scope.get("case_ids_sha256")
                != expected_scope.get("case_ids_sha256")
                or not isinstance(execution_summary, dict)
            ):
                execution_evidence = {
                    "status": "not_measured",
                    "reason": "WASM pipeline execution evidence identity or scope did not match the lane",
                    "artifact": relative_artifact,
                }
            elif (
                execution_summary.get("selected") != expected_scope.get("selected")
                or execution_summary.get("receipt_cases", 0)
                + execution_summary.get("not_recorded_cases", 0)
                != expected_scope.get("selected")
                or sum(execution_summary.get("actual_backend_counts", {}).values())
                != execution_summary.get("completed_receipts")
            ):
                execution_evidence = {
                    "status": "not_measured",
                    "reason": "WASM pipeline execution evidence summary is inconsistent",
                    "artifact": relative_artifact,
                }
            else:
                execution_evidence = {
                    "status": "measured",
                    "reason": str(raw_execution.get("reason", "")),
                    "artifact": relative_artifact,
                    "summary": execution_summary,
                }
    lane_returncode = 0 if status == "passed" else (returncode or 1)
    record = lane_record(
        lane_id=lane_id,
        kind=kind,
        backend=None,
        command=command,
        status=status,
        returncode=lane_returncode,
        artifact=artifact,
        summary=summary,
        scope=actual_scope,
        shader_coverage=shader_coverage if isinstance(shader_coverage, dict) else None,
        capabilities=capabilities if isinstance(capabilities, dict) else None,
        execution_evidence=execution_evidence,
        timed_out=timed_out,
        reason=reason,
    )
    if status != "passed":
        record["output_tail"] = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    return record


def run_js_lane(
    *, output_dir: Path, timeout_seconds: int, case_ids: list[str], scope: dict[str, Any]
) -> list[dict[str, Any]]:
    node_artifact = output_dir / "parity-js-wasm.json"
    browser_artifact = output_dir / "parity-browser-wasm.json"
    node_artifact.unlink(missing_ok=True)
    browser_artifact.unlink(missing_ok=True)
    command = ["make", "test-wasm"]
    returncode, stdout, stderr, timed_out = run_command(
        command,
        env={
            "MIGRATION_WASM_NO_OPT": "1",
            "NPM_CONFIG_CACHE": "/tmp/pillow-rs-npm-cache",
            "MIGRATION_JS_PARITY_OUTPUT": str(node_artifact),
            "MIGRATION_BROWSER_PARITY_OUTPUT": str(browser_artifact),
            **({"MIGRATION_PARITY_CASE_IDS": ",".join(case_ids)} if case_ids else {}),
        },
        timeout_seconds=timeout_seconds,
    )
    node_result = result_document(node_artifact)
    browser_result = result_document(browser_artifact)
    node_lane = wasm_lane_record(
        lane_id="js-wasm-parity",
        kind="javascript-wasm-parity",
        artifact=node_artifact,
        result=node_result,
        command=command,
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
        timed_out=timed_out,
        expected_scope=scope,
    )
    browser_lane = wasm_lane_record(
        lane_id="browser-wasm-parity",
        kind="browser-wasm-parity",
        artifact=browser_artifact,
        result=browser_result,
        command=command,
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
        timed_out=timed_out,
        expected_scope=scope,
    )
    node_scope = node_lane.get("scope", {})
    browser_scope = browser_lane.get("scope", {})
    node_digest = node_scope.get("case_ids_sha256")
    browser_digest = browser_scope.get("case_ids_sha256")
    if node_digest and browser_digest and node_digest != browser_digest:
        message = (
            "Node/browser WASM scope digest mismatch: "
            f"node={node_digest}, browser={browser_digest}"
        )
        for lane in (node_lane, browser_lane):
            lane["status"] = "failed"
            lane["returncode"] = returncode or 1
            lane["reason"] = message
    return [node_lane, browser_lane]


def run_campaign(args: argparse.Namespace) -> int:
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output_dir = output.parent / "all-backends"
    output_dir.mkdir(parents=True, exist_ok=True)
    requested_case_ids = sorted(set(args.case_id or []))
    scope = public_case_scope(requested_case_ids)
    smoke_case = requested_case_ids[0] if requested_case_ids else GPU_SMOKE_CASE

    lanes: list[dict[str, Any]] = []
    started_at = now()
    for backend in BACKENDS:
        lanes.append(
            run_parity_lane(
                backend,
                output_dir=output_dir,
                timeout_seconds=args.timeout,
                case_ids=requested_case_ids,
                scope=scope,
            )
        )

    smoke = run_parity_lane(
        "gpu",
        output_dir=output_dir,
        timeout_seconds=args.timeout,
        case_ids=requested_case_ids,
        scope=scope,
        smoke_case=smoke_case,
        smoke=True,
    )
    lanes.append(smoke)
    if smoke["status"] == "passed" and args.gpu_full:
        lanes.append(
            run_parity_lane(
                "gpu",
                output_dir=output_dir,
                timeout_seconds=args.timeout,
                case_ids=requested_case_ids,
                scope=scope,
            )
        )
    else:
        lanes.append(
            lane_record(
                lane_id="parity-gpu",
                kind="python-py3-parity",
                backend="gpu",
                command=["make", "migration-parity-test", "MIGRATION_TARGET_BACKEND=gpu"],
                status="skipped",
                returncode=None,
                scope=scope_with_execution(scope, executed=0, pending=scope["selected"]),
                reason=(
                    "GPU smoke gate did not pass; full GPU lane was not executed"
                    if smoke["status"] != "passed"
                    else GPU_FULL_DISABLED_REASON
                ),
            )
        )

    lanes.extend(
        run_js_lane(
            output_dir=output_dir,
            timeout_seconds=args.timeout,
            case_ids=requested_case_ids,
            scope=scope,
        )
    )

    required_failures = [lane for lane in lanes if lane["status"] == "failed"]
    status = "passed" if not required_failures else "failed"
    if (smoke["status"] != "passed" or not args.gpu_full) and status == "passed":
        status = "passed_with_gpu_skipped"
    result = {
        "schema": "migration-parity/all-backends-test-result@2",
        "status": status,
        "started_at": started_at,
        "finished_at": now(),
        "revision": git_revision(),
        "input_scope": scope,
        "gpu_gate": {
            "case_id": smoke_case,
            "status": smoke["status"],
            "timeout_seconds": GPU_SMOKE_TIMEOUT_SECONDS,
        },
        "gpu_full_requested": args.gpu_full,
        "lanes": lanes,
    }
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if status in {"passed", "passed_with_gpu_skipped"} else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument(
        "--case-id",
        action="append",
        help="run the same explicit public parity case filter in every full lane",
    )
    gpu_group = parser.add_mutually_exclusive_group()
    gpu_group.add_argument(
        "--gpu-full",
        dest="gpu_full",
        action="store_true",
        help="run bounded full GPU parity after the smoke gate (default)",
    )
    gpu_group.add_argument(
        "--no-gpu-full",
        dest="gpu_full",
        action="store_false",
        help="run only the GPU smoke gate and mark full GPU parity not proven",
    )
    parser.set_defaults(gpu_full=True)
    return parser.parse_args(argv)


if __name__ == "__main__":
    raise SystemExit(run_campaign(parse_args()))
