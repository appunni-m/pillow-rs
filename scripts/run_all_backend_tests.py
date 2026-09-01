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
TARGET_PARITY_LANES = (
    ("parity-cpu", "cpu"),
    ("parity-simd", "simd"),
    ("parity-gpu", "gpu"),
)
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


def receipt_terminal_complete(receipt: dict[str, Any]) -> bool:
    """Interpret the explicit terminal bit while reading old sidecars."""

    value = receipt.get("terminal_complete")
    if value is None:
        return receipt.get("status") in {"completed", "cached"}
    return type(value) is bool and value


def receipt_is_meaningful(receipt: dict[str, Any]) -> bool:
    """Return whether a receipt participates in pipeline classification."""

    return (
        receipt.get("pipeline_relevant") is not False
        and receipt.get("status") not in {"not_recorded", "not_applicable"}
    )


def validate_execution_receipts(
    cases: Any, *, label: str
) -> str | None:
    """Reject impossible receipt states before exposing lane evidence."""

    if not isinstance(cases, dict):
        return f"{label} cases are not an object"
    for case_id, receipts in cases.items():
        if not isinstance(case_id, str) or not isinstance(receipts, list):
            return f"{label} contains malformed case receipts"
        for receipt in receipts:
            if not isinstance(receipt, dict):
                return f"{label} contains a malformed receipt"
            if "terminal_complete" in receipt and type(
                receipt["terminal_complete"]
            ) is not bool:
                return f"{label} contains a non-boolean terminal_complete bit"
            if "pipeline_relevant" in receipt and type(
                receipt["pipeline_relevant"]
            ) is not bool:
                return f"{label} contains a non-boolean pipeline_relevant bit"
            if receipt_terminal_complete(receipt) and receipt.get("status") not in {
                "completed",
                "cached",
            }:
                return (
                    f"{label} contains terminal_complete=true for a non-terminal "
                    f"status ({receipt.get('status')!r})"
                )
            if receipt_terminal_complete(receipt) and receipt.get("errors"):
                return f"{label} contains terminal_complete=true with errors"
    return None


PIPELINE_CASE_STATUSES = {
    "not_applicable",
    "complete",
    "missing_receipt",
    "partial_receipt",
    "indeterminate",
}
PIPELINE_SUMMARY_FIELDS = {
    "pipeline_applicable_cases",
    "pipeline_complete_cases",
    "pipeline_missing_receipt_cases",
    "pipeline_partial_receipt_cases",
    "pipeline_not_applicable_cases",
    "pipeline_indeterminate_cases",
}


def validate_pipeline_case_status(
    statuses: Any,
    cases: dict[str, Any],
    summary: dict[str, Any],
    *,
    label: str,
) -> str | None:
    """Validate the per-case receipt/applicability partition.

    The raw receipt list remains authoritative for complete/partial states.
    A producer may choose among the three no-receipt states, but it cannot
    turn a recorded receipt into a non-pipeline case or omit a selected ID.
    """

    if not isinstance(statuses, dict):
        return f"{label} is not an object"
    if set(statuses) != set(cases):
        return f"{label} case IDs do not match receipt cases"
    counts = {status: 0 for status in PIPELINE_CASE_STATUSES}
    for case_id, value in statuses.items():
        if not isinstance(value, dict) or set(value) != {"status", "reason"}:
            return f"{label} contains malformed classification for {case_id!r}"
        status = value.get("status")
        reason = value.get("reason")
        if status not in PIPELINE_CASE_STATUSES:
            return f"{label} contains an unsupported status for {case_id!r}"
        if not isinstance(reason, str) or not reason:
            return f"{label} contains an invalid reason for {case_id!r}"
        receipts = cases[case_id]
        meaningful = any(
            receipt_is_meaningful(receipt)
            for receipt in receipts
        )
        terminal = any(receipt_terminal_complete(receipt) for receipt in receipts)
        if terminal and status != "complete":
            return f"{label} hides a terminal receipt for {case_id!r}"
        if meaningful and not terminal and status != "partial_receipt":
            return f"{label} hides a partial receipt for {case_id!r}"
        if not meaningful and status in {"complete", "partial_receipt"}:
            return f"{label} claims a receipt without a recorded receipt for {case_id!r}"
        counts[status] += 1

    expected = {
        "pipeline_applicable_cases": counts["complete"]
        + counts["missing_receipt"]
        + counts["partial_receipt"],
        "pipeline_complete_cases": counts["complete"],
        "pipeline_missing_receipt_cases": counts["missing_receipt"],
        "pipeline_partial_receipt_cases": counts["partial_receipt"],
        "pipeline_not_applicable_cases": counts["not_applicable"],
        "pipeline_indeterminate_cases": counts["indeterminate"],
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            return f"{label} disagrees with summary.{field}"
    return None


def validate_execution_summary(
    summary: Any, *, expected_selected: int, label: str
) -> str | None:
    """Validate legacy/current receipt denominators without hiding prefixes."""

    if not isinstance(summary, dict):
        return f"{label} summary is not an object"
    legacy = {
        "selected",
        "receipt_cases",
        "not_recorded_cases",
        "completed_receipts",
        "actual_backend_counts",
        "fallback_reason_counts",
    }
    current = legacy | {
        "terminal_complete_receipts",
        "terminal_incomplete_cases",
    }
    versioned = current | PIPELINE_SUMMARY_FIELDS
    if set(summary) not in (legacy, current, versioned):
        return f"{label} summary has an unsupported key set"
    for field in (
        "selected",
        "receipt_cases",
        "not_recorded_cases",
        "completed_receipts",
    ):
        if type(summary[field]) is not int or summary[field] < 0:
            return f"{label} summary has an invalid {field}"
    for field in ("actual_backend_counts", "fallback_reason_counts"):
        counts = summary[field]
        if not isinstance(counts, dict):
            return f"{label} summary has an invalid {field}"
        if any(
            not isinstance(key, str)
            or type(count) is not int
            or count < 0
            for key, count in counts.items()
        ):
            return f"{label} summary has invalid {field} entries"
    if summary["selected"] != expected_selected:
        return f"{label} summary selected count does not match the lane"
    if summary["receipt_cases"] + summary["not_recorded_cases"] != summary[
        "selected"
    ]:
        return f"{label} summary case counts are inconsistent"
    if set(summary) in (current, versioned):
        for field in ("terminal_complete_receipts", "terminal_incomplete_cases"):
            if type(summary[field]) is not int or summary[field] < 0:
                return f"{label} summary has an invalid {field}"
        if summary["terminal_complete_receipts"] > summary["completed_receipts"]:
            return f"{label} terminal receipts exceed completed receipts"
        if summary["terminal_incomplete_cases"] > summary["receipt_cases"]:
            return f"{label} incomplete cases exceed receipt cases"
        denominator = summary["terminal_complete_receipts"]
    else:
        denominator = summary["completed_receipts"]
    if set(summary) == versioned:
        for field in PIPELINE_SUMMARY_FIELDS:
            if type(summary[field]) is not int or summary[field] < 0:
                return f"{label} summary has an invalid {field}"
        if (
            summary["pipeline_applicable_cases"]
            != summary["pipeline_complete_cases"]
            + summary["pipeline_missing_receipt_cases"]
            + summary["pipeline_partial_receipt_cases"]
        ):
            return f"{label} pipeline-applicable case counts are inconsistent"
        if (
            summary["pipeline_applicable_cases"]
            + summary["pipeline_not_applicable_cases"]
            + summary["pipeline_indeterminate_cases"]
            != summary["selected"]
        ):
            return f"{label} pipeline case partition does not match selected"
        if summary["pipeline_partial_receipt_cases"] != summary[
            "terminal_incomplete_cases"
        ]:
            return f"{label} partial receipt count is inconsistent"
        if summary["pipeline_complete_cases"] > summary[
            "terminal_complete_receipts"
        ]:
            return f"{label} complete case count exceeds terminal receipts"
    if sum(summary["actual_backend_counts"].values()) != denominator:
        return f"{label} backend counts are inconsistent"
    return None


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
    if raw.get("schema") != "migration-parity/pipeline-execution-evidence@2":
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
    if "cases" not in raw or "pipeline_case_status" not in raw:
        return {
            "status": "not_measured",
            "reason": "normal-parity pipeline execution sidecar has no complete case classification",
            "artifact": relative_path,
        }
    receipt_error = validate_execution_receipts(
        raw.get("cases"), label="normal-parity pipeline execution sidecar"
    )
    if receipt_error is not None:
        return {
            "status": "not_measured",
            "reason": receipt_error,
            "artifact": relative_path,
        }
    summary_error = validate_execution_summary(
        summary,
        expected_selected=expected_scope.get("selected", -1),
        label="normal-parity pipeline execution sidecar",
    )
    if summary_error is not None:
        return {
            "status": "not_measured",
            "reason": summary_error,
            "artifact": relative_path,
        }
    classification_error = validate_pipeline_case_status(
        raw.get("pipeline_case_status"),
        raw["cases"],
        summary,
        label="normal-parity pipeline execution sidecar.pipeline_case_status",
    )
    if classification_error is not None:
        return {
            "status": "not_measured",
            "reason": classification_error,
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


def backend_coverage_report(lanes: list[dict[str, Any]]) -> dict[str, Any]:
    """Classify target backend proof independently from public parity status.

    A public parity pass only proves that serialized Pillow observations agree;
    it does not prove that a requested target backend produced those values.
    Keep this verdict explicit so a normal fallback lane cannot be presented as
    complete backend coverage.  The report requires the current terminal-bit
    summary shape; legacy summaries remain readable in diagnostic validators but
    cannot establish a current backend-coverage claim.
    """

    by_lane = {
        lane.get("lane_id"): lane
        for lane in lanes
        if isinstance(lane, dict) and isinstance(lane.get("lane_id"), str)
    }
    target_lanes: list[dict[str, Any]] = []
    for lane_id, expected_backend in TARGET_PARITY_LANES:
        lane = by_lane.get(lane_id)
        scope = lane.get("scope", {}) if isinstance(lane, dict) else {}
        selected = scope.get("selected", 0) if isinstance(scope, dict) else 0
        if type(selected) is not int or selected < 0:
            selected = 0
        evidence = lane.get("execution_evidence") if isinstance(lane, dict) else None
        summary = evidence.get("summary") if isinstance(evidence, dict) else None
        reasons: list[str] = []
        terminal_complete_receipts = 0
        terminal_incomplete_cases = 0
        not_recorded_cases = 0
        pipeline_applicable_cases = 0
        pipeline_complete_cases = 0
        pipeline_missing_receipt_cases = 0
        pipeline_partial_receipt_cases = 0
        pipeline_not_applicable_cases = 0
        pipeline_indeterminate_cases = 0
        actual_backend_counts: dict[str, int] = {}
        fallback_reason_counts: dict[str, int] = {}

        if not isinstance(lane, dict) or lane.get("status") != "passed":
            reasons.append("public parity lane did not pass")
        if (
            not isinstance(evidence, dict)
            or evidence.get("status") != "measured"
            or not isinstance(summary, dict)
        ):
            reasons.append("terminal execution evidence is missing")
        else:
            required_summary = {
                "selected",
                "receipt_cases",
                "not_recorded_cases",
                "completed_receipts",
                "terminal_complete_receipts",
                "terminal_incomplete_cases",
                "actual_backend_counts",
                "fallback_reason_counts",
            }
            if not required_summary.issubset(summary):
                reasons.append("terminal-complete summary is missing")
            else:
                terminal_complete_receipts = summary["terminal_complete_receipts"]
                terminal_incomplete_cases = summary["terminal_incomplete_cases"]
                not_recorded_cases = summary["not_recorded_cases"]
                if PIPELINE_SUMMARY_FIELDS.issubset(summary):
                    pipeline_applicable_cases = summary[
                        "pipeline_applicable_cases"
                    ]
                    pipeline_complete_cases = summary["pipeline_complete_cases"]
                    pipeline_missing_receipt_cases = summary[
                        "pipeline_missing_receipt_cases"
                    ]
                    pipeline_partial_receipt_cases = summary[
                        "pipeline_partial_receipt_cases"
                    ]
                    pipeline_not_applicable_cases = summary[
                        "pipeline_not_applicable_cases"
                    ]
                    pipeline_indeterminate_cases = summary[
                        "pipeline_indeterminate_cases"
                    ]
                else:
                    # Keep old normalized artifacts readable for diagnostics.
                    # Their single no-receipt bucket cannot establish that a
                    # case was outside the pipeline, so retain the old strict
                    # proof interpretation until a schema-2 sidecar is run.
                    pipeline_applicable_cases = selected
                    pipeline_complete_cases = terminal_complete_receipts
                    pipeline_missing_receipt_cases = not_recorded_cases
                    pipeline_partial_receipt_cases = terminal_incomplete_cases
                actual_backend_counts = dict(summary["actual_backend_counts"])
                fallback_reason_counts = dict(summary["fallback_reason_counts"])
                if terminal_complete_receipts == 0 and pipeline_applicable_cases:
                    reasons.append("no terminal-complete receipt")
                if terminal_incomplete_cases:
                    reasons.append(
                        f"{terminal_incomplete_cases} cases lack terminal-complete receipts"
                    )
                if pipeline_missing_receipt_cases:
                    reasons.append(
                        f"{pipeline_missing_receipt_cases} pipeline-applicable cases have no receipt"
                    )
                if pipeline_partial_receipt_cases:
                    reasons.append(
                        f"{pipeline_partial_receipt_cases} pipeline-applicable cases have partial receipts"
                    )
                if pipeline_indeterminate_cases:
                    reasons.append(
                        f"{pipeline_indeterminate_cases} cases have indeterminate pipeline applicability"
                    )
                if pipeline_applicable_cases == 0 and selected:
                    reasons.append("no pipeline-applicable cases were selected")
                if actual_backend_counts != {
                    expected_backend: terminal_complete_receipts
                }:
                    reasons.append(
                        "actual backend counts do not equal the requested backend "
                        "for every terminal-complete receipt"
                    )
                if fallback_reason_counts:
                    reasons.append("fallback reasons are present")

        target_lanes.append(
            {
                "lane_id": lane_id,
                "requested_backend": expected_backend,
                "status": "proven" if not reasons else "not_proven",
                "selected": selected,
                "terminal_complete_receipts": terminal_complete_receipts,
                "terminal_incomplete_cases": terminal_incomplete_cases,
                "not_recorded_cases": not_recorded_cases,
                "pipeline_applicable_cases": pipeline_applicable_cases,
                "pipeline_complete_cases": pipeline_complete_cases,
                "pipeline_missing_receipt_cases": pipeline_missing_receipt_cases,
                "pipeline_partial_receipt_cases": pipeline_partial_receipt_cases,
                "pipeline_not_applicable_cases": pipeline_not_applicable_cases,
                "pipeline_indeterminate_cases": pipeline_indeterminate_cases,
                "actual_backend_counts": dict(sorted(actual_backend_counts.items())),
                "fallback_reason_counts": dict(sorted(fallback_reason_counts.items())),
                "reasons": reasons,
            }
        )
    return {
        "status": (
            "proven"
            if all(item["status"] == "proven" for item in target_lanes)
            else "not_proven"
        ),
        "target_lanes": target_lanes,
    }


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
            # The orchestrator bounds GPU lanes to the smoke/full deadlines
            # below, but the parity adapter has its own shorter default.  Pass
            # the same bounded deadline through so a valid full lane cannot
            # self-time out at 120s before the parent reaches its 300s guard.
            **(
                {"MIGRATION_GPU_TIMEOUT_SECONDS": str(lane_timeout)}
                if backend == "gpu"
                else {}
            ),
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
                != "migration-parity/pipeline-execution-evidence@2"
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
            elif "cases" not in raw_execution or "pipeline_case_status" not in raw_execution:
                execution_evidence = {
                    "status": "not_measured",
                    "reason": "WASM pipeline execution evidence has no complete case classification",
                    "artifact": relative_artifact,
                }
            elif (
                receipt_error := validate_execution_receipts(
                    raw_execution.get("cases"),
                    label="WASM pipeline execution evidence",
                )
            ) is not None:
                execution_evidence = {
                    "status": "not_measured",
                    "reason": receipt_error,
                    "artifact": relative_artifact,
                }
            elif (
                classification_error := validate_pipeline_case_status(
                    raw_execution.get("pipeline_case_status"),
                    raw_execution["cases"],
                    execution_summary,
                    label="WASM pipeline execution evidence.pipeline_case_status",
                )
            ) is not None:
                execution_evidence = {
                    "status": "not_measured",
                    "reason": classification_error,
                    "artifact": relative_artifact,
                }
            elif (
                summary_error := validate_execution_summary(
                    execution_summary,
                    expected_selected=expected_scope.get("selected", -1),
                    label="WASM pipeline execution evidence",
                )
            ) is not None:
                execution_evidence = {
                    "status": "not_measured",
                    "reason": summary_error,
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
    backend_coverage = backend_coverage_report(lanes)
    status = "passed" if not required_failures else "failed"
    if status == "passed" and (smoke["status"] != "passed" or not args.gpu_full):
        status = "passed_with_gpu_skipped"
    elif status == "passed" and backend_coverage["status"] != "proven":
        # Public observations can agree even when a target silently routes to
        # CPU or never reaches a terminal receipt.  Keep that parity result
        # usable, but make the missing backend proof impossible to mistake for
        # a complete all-backends claim.
        status = "passed_with_backend_gaps"
    result = {
        "schema": "migration-parity/all-backends-test-result@3",
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
        "backend_coverage": backend_coverage,
        "lanes": lanes,
    }
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if status in {
        "passed",
        "passed_with_gpu_skipped",
        "passed_with_backend_gaps",
    } else 1


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
