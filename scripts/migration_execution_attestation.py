"""Separate execution-identity receipts; never part of an image timer or result."""

from __future__ import annotations

import gzip
import copy
import hashlib
import json
import os
from pathlib import Path
import threading
from typing import Any
import uuid

try:
    from migration_execution_identity import (
        capture_runtime_identity,
        validate_runtime_identity,
    )
except ModuleNotFoundError:
    from scripts.migration_execution_identity import (
        capture_runtime_identity,
        validate_runtime_identity,
    )

PREFIX = "MIGRATION_EXECUTION_ATTESTATION_"
CHILD_SCHEMA = "migration-parity/child-execution-attestation@1"
SCHEMA = "migration-parity/execution-attestation@1"
ROLES = {"parity", "benchmark_timed", "benchmark_execution_proof"}
CHILD_KEYS = {
    "schema", "status", "invocation_id", "role", "side", "profile",
    "case_ids", "context", "runtime", "error",
}
CONTEXT_KEYS = {"lifecycle", "boundary", "step_ids", "repeat"}


def _exact(value: Any, keys: set[str], name: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError(f"invalid {name} fields")


def _ids(value: Any, name: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
    ):
        raise ValueError(f"invalid {name} IDs")
    return value


def _context(value: Any) -> None:
    _exact(value, CONTEXT_KEYS, "invocation context")
    if value["lifecycle"] not in {"warm", "cold", "resident"}:
        raise ValueError("invalid invocation lifecycle")
    if value["boundary"] not in {None, "observed_steps", "whole_workflow"}:
        raise ValueError("invalid invocation boundary")
    if type(value["repeat"]) is not int or value["repeat"] <= 0:
        raise ValueError("invalid invocation repeat")
    steps = value["step_ids"]
    if not isinstance(steps, list) or any(not isinstance(s, str) or not s for s in steps):
        raise ValueError("invalid invocation timing steps")
    if steps != sorted(set(steps)) or (value["boundary"] is None and steps):
        raise ValueError("invalid invocation timing step selection")


def _strings(value: Any, name: str) -> None:
    if not isinstance(value, list) or any(not isinstance(s, str) or not s for s in value):
        raise ValueError(f"invalid {name}")


def _digest(value: Any) -> None:
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ValueError("invalid SHA-256 digest")


def _publish(path: Path, document: dict[str, Any]) -> None:
    """Atomically replace our exclusive empty reservation, preserving old files.

    A failed write may leave its new reservation/staging file for diagnosis.
    There is deliberately no deletion or overwrite of a pre-existing artifact.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(descriptor)
    staging = path.with_name(f".{path.name}.{uuid.uuid4().hex}.partial")
    staging_descriptor = os.open(staging, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(staging_descriptor, "w", encoding="utf-8") as stream:
        json.dump(document, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write("\n")
    os.replace(staging, path)


def validate_child_receipt(document: Any) -> None:
    _exact(document, CHILD_KEYS, "child receipt")
    if document["schema"] != CHILD_SCHEMA:
        raise ValueError("unsupported child receipt schema")
    if document["status"] not in {"complete", "not_proven"}:
        raise ValueError("invalid child receipt status")
    if document["role"] not in ROLES or document["side"] not in {"source", "target"}:
        raise ValueError("invalid child receipt role/side")
    if any(not isinstance(document[k], str) or not document[k] for k in ["invocation_id", "profile"]):
        raise ValueError("missing child invocation/profile")
    _ids(document["case_ids"], "child case")
    _context(document["context"])
    if document["status"] == "complete":
        if document["error"] is not None:
            raise ValueError("complete child receipt has an error")
        validate_runtime_identity(document["runtime"])
        stable = document["runtime"]["stable"]
        if stable.get("side") != document["side"] or stable.get("profile") != document["profile"]:
            raise ValueError("child runtime side/profile mismatch")
    elif document["runtime"] is not None or not isinstance(document["error"], str) or not document["error"]:
        raise ValueError("invalid unproven child receipt")


def write_child_attestation_from_environment(
    side: str,
    handshake: dict[str, Any],
    cases: list[dict[str, Any]],
    *,
    context: dict[str, Any],
    declared_features: list[str],
) -> None:
    """Capture only after calls finish; no optional image import or timer work."""
    destination = os.environ.get(PREFIX + "OUTPUT")
    if not destination:
        return
    document = {
        "schema": CHILD_SCHEMA,
        "status": "not_proven",
        "invocation_id": os.environ.get(PREFIX + "INVOCATION_ID", ""),
        "role": os.environ.get(PREFIX + "ROLE", ""),
        "side": side,
        "profile": os.environ.get(PREFIX + "PROFILE", ""),
        "case_ids": sorted(case["case_id"] for case in cases),
        "context": context,
        "runtime": None,
        "error": None,
    }
    try:
        document["runtime"] = capture_runtime_identity(
            side, handshake, document["profile"], declared_features
        )
        document["status"] = "complete"
        validate_child_receipt(document)
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
        document.update(status="not_proven", runtime=None, error=str(exc)[:1000])
    try:
        _publish(Path(destination), document)
    except (OSError, ValueError, TypeError):
        # A missing/incomplete child artifact becomes not_proven in the parent;
        # it must not change the already executed public workflow or samples.
        pass
    if os.environ.get("MIGRATION_PRODUCER_ATTESTATION_OUTPUT"):
        try:
            try:
                from migration_producer_attestation import write_child_producer_from_environment
            except ModuleNotFoundError:
                from scripts.migration_producer_attestation import write_child_producer_from_environment
            write_child_producer_from_environment()
        except (OSError, RuntimeError, ValueError, TypeError, KeyError):
            pass


def report_fingerprint(path: Path) -> dict[str, Any]:
    """Hash complete original JSON independently of its stored gzip container."""
    def digest(stream: Any) -> tuple[str, int]:
        sha = hashlib.sha256()
        count = 0
        while chunk := stream.read(1024 * 1024):
            sha.update(chunk)
            count += len(chunk)
        return sha.hexdigest(), count

    with path.open("rb") as stream:
        container_sha, container_bytes = digest(stream)
    if path.name.lower().endswith(".gz"):
        with gzip.open(path, "rb") as stream:
            canonical_sha, canonical_bytes = digest(stream)
    else:
        canonical_sha, canonical_bytes = container_sha, container_bytes
    return {
        "path": str(path.resolve()),
        "canonical_sha256": canonical_sha,
        "canonical_bytes": canonical_bytes,
        "container_sha256": container_sha,
        "container_bytes": container_bytes,
    }


class AttestationCollector:
    """Retain every opted-in child and keep identity failures out of raw results."""

    def __init__(self, output: Path, *, producer_output: Path | None = None):
        self.output = output.resolve()
        if self.output.exists():
            raise FileExistsError(f"attestation artifact already exists: {self.output}")
        self.children = self.output.parent / (self.output.name + ".children")
        self._planned: dict[str, dict[str, Any]] = {}
        self._collected: dict[str, dict[str, Any]] = {}
        self._lock = threading.Lock()
        self.producer = None
        if producer_output is not None:
            try:
                from migration_producer_attestation import ProducerCollector
            except ModuleNotFoundError:
                from scripts.migration_producer_attestation import ProducerCollector
            self.producer = ProducerCollector(producer_output)

    def child_environment(
        self, *, side: str, profile: str, role: str,
        cases: list[dict[str, Any]], context: dict[str, Any],
    ) -> dict[str, str]:
        case_ids = sorted(case["case_id"] for case in cases)
        _ids(case_ids, "planned case")
        _context(context)
        if side not in {"source", "target"} or role not in ROLES or not isinstance(profile, str) or not profile:
            raise ValueError("invalid planned child identity")
        invocation = uuid.uuid4().hex
        path = self.children / f"{invocation}.json"
        expected = {
            "invocation_id": invocation, "role": role, "side": side,
            "profile": profile, "case_ids": case_ids, "context": copy.deepcopy(context),
            "receipt_path": str(path),
        }
        with self._lock:
            self._planned[invocation] = expected
        environment = {
            PREFIX + "OUTPUT": str(path), PREFIX + "INVOCATION_ID": invocation,
            PREFIX + "ROLE": role, PREFIX + "PROFILE": profile,
        }
        if self.producer is not None:
            environment.update(self.producer.child_environment(invocation))
        return environment

    def collect(self, overrides: dict[str, str]) -> None:
        invocation = overrides[PREFIX + "INVOCATION_ID"]
        with self._lock:
            expected = self._planned[invocation]
            if invocation in self._collected:
                raise ValueError("child receipt collected more than once")
        receipt = None
        errors = []
        try:
            receipt = json.loads(Path(expected["receipt_path"]).read_text(encoding="utf-8"))
            validate_child_receipt(receipt)
            for field in ["invocation_id", "role", "side", "profile", "case_ids", "context"]:
                if receipt[field] != expected[field]:
                    raise ValueError(f"child {field} differs from actual invocation")
            if receipt["status"] != "complete":
                errors.append(receipt["error"])
        except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
            receipt = None
            errors.append(str(exc)[:1000])
        with self._lock:
            self._collected[invocation] = {**expected, "receipt": receipt, "errors": errors}
        if self.producer is not None:
            self.producer.collect(invocation)

    def require_runtime_identities(
        self,
        expected: dict[tuple[str, str], str],
        *,
        role: str,
    ) -> None:
        """Require selected current-run children to match prior proof hashes."""

        with self._lock:
            records = list(self._collected.values())
        for (side, profile), expected_sha256 in expected.items():
            matches = [
                record
                for record in records
                if record["side"] == side
                and record["profile"] == profile
                and record["role"] == role
            ]
            if not matches:
                raise ValueError(
                    f"current run has no {role} identity for {side}/{profile}"
                )
            for record in matches:
                receipt = record.get("receipt")
                runtime = receipt.get("runtime") if isinstance(receipt, dict) else None
                actual_sha256 = (
                    runtime.get("stable_sha256") if isinstance(runtime, dict) else None
                )
                if record.get("errors") or actual_sha256 != expected_sha256:
                    raise ValueError(
                        f"current {side}/{profile} runtime identity differs from reused parity"
                    )

    def finalize(
        self, report_path: Path, *, result: dict[str, Any],
        selection: dict[str, Any], parity_proof: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        with self._lock:
            records = [
                self._collected.get(key, {**planned, "receipt": None, "errors": ["child receipt was not collected"]})
                for key, planned in self._planned.items()
            ]
        identity = result["identity"]
        document = {
            "schema": SCHEMA, "status": "not_proven",
            "report": {**report_fingerprint(report_path), "schema": result["schema"], "run_id": identity["run_id"]},
            "manifest": identity["manifest"], "inputs": identity["inputs"],
            "selection": selection, "invocations": records, "errors": [],
            "parity_proof": parity_proof,
        }
        try:
            validate_attestation(document, result=result, allow_unproven=True)
            document["status"] = "complete"
        except (RuntimeError, ValueError, TypeError, KeyError) as exc:
            document["errors"] = [str(exc)[:1000]]
        _publish(self.output, document)
        if self.producer is not None:
            self.producer.finalize(self.output, document)
        return document


def merge_parity_attestations(
    output: Path,
    report_path: Path,
    result: dict[str, Any],
    backend_attestations: list[
        tuple[dict[str, Any], dict[str, Any], dict[str, Any]]
    ],
) -> dict[str, Any]:
    """Bind per-backend parity receipts to their merged report."""

    if not backend_attestations:
        raise ValueError("no backend parity attestations to merge")
    if result.get("schema") not in {
        "migration-parity/parity-result@1",
        "migration-parity/parity-result@2",
    }:
        raise ValueError("merged execution attestation requires a parity report")

    manifest = result["identity"]["manifest"]
    merged_inputs = {
        item["path"]: item for item in result["identity"].get("inputs", [])
    }
    invocations: list[dict[str, Any]] = []
    invocation_ids: set[str] = set()
    for document, backend_result, report_binding in backend_attestations:
        validate_attestation(
            document,
            result=backend_result,
            report_binding=report_binding,
        )
        if document["manifest"] != manifest:
            raise ValueError("backend parity attestations have different manifests")
        for item in document["inputs"]:
            if merged_inputs.get(item.get("path")) != item:
                raise ValueError("backend parity attestation has a different input")
        for invocation in document["invocations"]:
            invocation_id = invocation["invocation_id"]
            if invocation_id in invocation_ids:
                raise ValueError("backend parity attestations duplicate an invocation")
            invocation_ids.add(invocation_id)
            invocations.append(invocation)

    merged = {
        "schema": SCHEMA,
        "status": "complete",
        "report": {
            **report_fingerprint(report_path),
            "schema": result["schema"],
            "run_id": result["identity"]["run_id"],
        },
        "manifest": manifest,
        "inputs": result["identity"].get("inputs", []),
        "selection": {
            "case_profiles": [
                {
                    "case_id": row["case_id"],
                    "target_profile": row["target_profile"],
                }
                for row in result["comparisons"]
            ],
            "workload_profiles": [],
        },
        "invocations": invocations,
        "errors": [],
        "parity_proof": None,
    }
    validate_attestation(
        merged,
        result=result,
        allow_duplicate_parity_sources=True,
    )
    _publish(output.resolve(), merged)
    return merged


def validate_attestation(
    document: Any,
    *,
    result: dict[str, Any],
    allow_unproven: bool = False,
    allow_duplicate_parity_sources: bool = False,
    report_binding: dict[str, Any] | None = None,
) -> None:
    """Check collected identity against the writer's bound report projection.

    ``result`` is the producer's projection of the report it just closed, or a
    projection a consumer parsed from that same file. This does not validate
    public observations, parity proof links, backend execution, or source-tree
    identity; strict admission must independently validate those contracts.
    """
    _exact(document, {"schema", "status", "report", "manifest", "inputs", "selection", "invocations", "errors", "parity_proof"}, "attestation")
    if document["schema"] != SCHEMA or document["status"] not in {"complete", "not_proven"}:
        raise ValueError("unsupported attestation schema/status")
    if not allow_unproven and document["status"] != "complete":
        raise ValueError("execution identity is not proven")
    _strings(document["errors"], "attestation errors")
    if document["errors"] or result["status"] != "completed":
        raise ValueError("report or attestation is incomplete")
    report = document["report"]
    _exact(report, {"path", "canonical_sha256", "canonical_bytes", "container_sha256", "container_bytes", "schema", "run_id"}, "report binding")
    if not isinstance(report["path"], str) or not Path(report["path"]).is_absolute():
        raise ValueError("invalid report path")
    for key in ["canonical_sha256", "container_sha256"]:
        _digest(report[key])
    for key in ["canonical_bytes", "container_bytes"]:
        if type(report[key]) is not int or report[key] <= 0:
            raise ValueError("invalid report byte length")
    if not isinstance(report["run_id"], str) or not report["run_id"]:
        raise ValueError("invalid report run ID")
    actual = (
        report_fingerprint(Path(report["path"]))
        if report_binding is None
        else report_binding
    )
    if report_binding is not None and report_binding.get("path") != report["path"]:
        raise ValueError("report binding path mismatch")
    if any(report[key] != value for key, value in actual.items()):
        raise ValueError("report bytes changed")
    if report["schema"] != result["schema"] or report["run_id"] != result["identity"]["run_id"]:
        raise ValueError("report schema/run ID mismatch")
    if document["manifest"] != result["identity"]["manifest"] or document["inputs"] != result["identity"]["inputs"]:
        raise ValueError("report manifest/input mismatch")
    _exact(document["selection"], {"case_profiles", "workload_profiles"}, "selection")
    selection = document["selection"]
    if any(not isinstance(selection[key], list) for key in selection):
        raise ValueError("invalid selection list")
    if result["schema"] in {
        "migration-parity/parity-result@1",
        "migration-parity/parity-result@2",
    }:
        comparisons = result["comparisons"]
        expected = {(row["case_id"], row["target_profile"]) for row in comparisons}
        if len(expected) != len(result["comparisons"]) or selection["workload_profiles"]:
            raise ValueError("invalid parity selection")
        selected = []
        for pair in selection["case_profiles"]:
            _exact(pair, {"case_id", "target_profile"}, "case/profile")
            if any(not isinstance(v, str) or not v for v in pair.values()):
                raise ValueError("invalid case/profile identity")
            selected.append((pair["case_id"], pair["target_profile"]))
        if len(selected) != len(set(selected)) or set(selected) != expected or not expected:
            raise ValueError("parity case/profile membership differs from report")
        required = {(case, "target", profile, "parity") for case, profile in expected}
        source_cases = {
            row["case_id"]
            for row in comparisons
            if result["schema"].endswith("@1")
            or row.get("verification") == "parity"
        }
        if result["schema"].endswith("@2") and any(
            row.get("verification") not in {"parity", "fault_contract"}
            for row in comparisons
        ):
            raise ValueError("invalid parity verification kind")
    elif result["schema"] == "migration-parity/benchmark-result@1":
        if selection["case_profiles"]:
            raise ValueError("invalid benchmark selection")
        expected = {(work["workload_id"], subject["id"]) for work in result["workloads"] for subject in work["subjects"]}
        selected = []
        required = set()
        source_cases = set()
        for pair in selection["workload_profiles"]:
            _exact(pair, {"workload_id", "case_id", "subject_id"}, "workload/profile")
            if any(not isinstance(v, str) or not v for v in pair.values()):
                raise ValueError("invalid workload/profile identity")
            if pair["subject_id"] not in {"pillow", "pillow-simd", "python-cpu", "python-simd", "python-gpu", "python-parallel-cpu"}:
                raise ValueError("unsupported workload subject")
            selected.append((pair["workload_id"], pair["subject_id"]))
            side = "target" if pair["subject_id"].startswith("python-") else "source"
            required.add((pair["case_id"], side, pair["subject_id"], "benchmark_timed"))
            if side == "target":
                required.add((pair["case_id"], side, pair["subject_id"], "benchmark_execution_proof"))
        if len(selected) != len(set(selected)) or set(selected) != expected or not expected:
            raise ValueError("benchmark workload/profile membership differs from report")
    else:
        raise ValueError("unsupported bound report schema")
    observed = set()
    source_observed: list[str] = []
    fingerprints = {}
    invocation_ids = set()
    if not isinstance(document["invocations"], list):
        raise ValueError("invalid invocation list")
    for record in document["invocations"]:
        _exact(record, {"invocation_id", "role", "side", "profile", "case_ids", "context", "receipt_path", "receipt", "errors"}, "invocation")
        _strings(record["errors"], "invocation errors")
        if not isinstance(record["receipt_path"], str) or not Path(record["receipt_path"]).is_absolute():
            raise ValueError("invalid child receipt path")
        if record["invocation_id"] in invocation_ids or record["errors"] or record["receipt"] is None:
            raise ValueError("missing, duplicate or invalid invocation receipt")
        invocation_ids.add(record["invocation_id"])
        receipt = record["receipt"]
        validate_child_receipt(receipt)
        if receipt["status"] != "complete":
            raise ValueError("child identity is not proven")
        for field in ["invocation_id", "role", "side", "profile", "case_ids", "context"]:
            if receipt[field] != record[field]:
                raise ValueError("invocation and receipt disagree")
        key = (record["side"], record["profile"])
        fingerprint = receipt["runtime"]["stable_sha256"]
        if key in fingerprints and fingerprints[key] != fingerprint:
            raise ValueError("execution identity changed between children")
        fingerprints[key] = fingerprint
        for case in record["case_ids"]:
            membership = (case, record["side"], record["profile"], record["role"])
            if record["role"] == "parity" and record["side"] == "source":
                source_observed.append(case)
            else:
                if membership in observed and record["role"] == "parity":
                    raise ValueError("duplicate parity case invocation")
                observed.add(membership)
    if observed != required:
        raise ValueError("missing or extra case/profile child membership")
    if result["schema"] in {
        "migration-parity/parity-result@1",
        "migration-parity/parity-result@2",
    } and (
        set(source_observed) != source_cases
        or (
            not allow_duplicate_parity_sources
            and len(source_observed) != len(set(source_observed))
        )
    ):
        raise ValueError("missing, duplicate, or extra parity source invocation coverage")
    if not document["invocations"]:
        raise ValueError("no child identity captured")
    if document["parity_proof"] is not None:
        _exact(document["parity_proof"], {"report_sha256", "run_id", "attestation_sha256"}, "parity proof reference")
        if result["schema"] != "migration-parity/benchmark-result@1":
            raise ValueError("parity report cannot reference another parity proof")
        _digest(document["parity_proof"]["report_sha256"])
        _digest(document["parity_proof"]["attestation_sha256"])
        if not isinstance(document["parity_proof"]["run_id"], str) or not document["parity_proof"]["run_id"]:
            raise ValueError("invalid parity proof run ID")
