"""Optional producer receipts, captured outside all public-call timers.

The raw result and execution receipt retain their existing schemas. These
companions bind their closed bytes to a frozen producer inventory and every
planned child. Capture failures stay diagnostic and never alter image results.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import threading
from typing import Any

try:
    from migration_producer_identity import (
        capture_producer_bundle,
        capture_producer_identity,
        validate_producer_identity,
    )
except ModuleNotFoundError:
    from scripts.migration_producer_identity import (
        capture_producer_bundle,
        capture_producer_identity,
        validate_producer_identity,
    )

PREFIX = "MIGRATION_PRODUCER_ATTESTATION_"
SCHEMA = "migration-parity/producer-attestation@2"
CHILD_SCHEMA = "migration-parity/child-producer-attestation@1"
CAPTURE_ERRORS = (OSError, RuntimeError, ValueError, TypeError, KeyError)


def observed_invocation_context() -> dict[str, Any]:
    """Capture runner argv and the effective top-level Make context, if any.

    GNU Make exposes its executable and goals to recipes, but does not expose
    the original parent process argv portably. Keep that context separate from
    the actual Python runner argv instead of labeling a reconstructed command
    as literal process argv.
    """

    executable = os.environ.get("PILLOW_RS_OUTER_MAKE_EXECUTABLE")
    raw_goals = os.environ.get("PILLOW_RS_OUTER_MAKE_GOALS")
    if executable is None and raw_goals is None:
        make_context = None
    elif not executable or raw_goals is None:
        raise ValueError("outer Make context is incomplete")
    else:
        goals = raw_goals.split()
        if not goals:
            raise ValueError("outer Make context has no top-level goal")
        make_context = {"executable": executable, "goals": goals}
    return {
        "runner_argv": [sys.executable, *sys.argv],
        "effective_make_context": make_context,
    }


def validate_invocation_context(value: Any) -> None:
    if not isinstance(value, dict) or set(value) != {
        "runner_argv",
        "effective_make_context",
    }:
        raise ValueError("producer invocation context has invalid fields")
    argv = value["runner_argv"]
    if not isinstance(argv, list) or not argv or not all(
        isinstance(argument, str) and argument for argument in argv
    ):
        raise ValueError("producer runner argv must be a non-empty string array")
    make_context = value["effective_make_context"]
    if make_context is None:
        return
    if not isinstance(make_context, dict) or set(make_context) != {
        "executable",
        "goals",
    }:
        raise ValueError("producer effective Make context has invalid fields")
    if not isinstance(make_context["executable"], str) or not make_context["executable"]:
        raise ValueError("producer Make executable must be non-empty")
    goals = make_context["goals"]
    if not isinstance(goals, list) or not goals or not all(
        isinstance(goal, str) and goal for goal in goals
    ):
        raise ValueError("producer Make goals must be a non-empty string array")


def _base():
    # The execution collector imports this module only for the opt-in route.
    try:
        import migration_execution_attestation as base
    except ModuleNotFoundError:
        from scripts import migration_execution_attestation as base
    return base


def _read_bundle(path: Path) -> tuple[dict[str, Any], str]:
    raw = path.read_bytes()
    return json.loads(raw), hashlib.sha256(raw).hexdigest()


def _child_valid(receipt: Any, bundle: dict[str, Any], bundle_sha: str, invocation: str) -> None:
    if not isinstance(receipt, dict) or set(receipt) != {
        "schema", "status", "invocation_id", "producer_bundle_sha256", "identity", "error"
    }:
        raise ValueError("invalid child producer receipt fields")
    if receipt["schema"] != CHILD_SCHEMA or receipt["invocation_id"] != invocation:
        raise ValueError("child producer invocation/schema differs")
    if receipt["producer_bundle_sha256"] != bundle_sha:
        raise ValueError("child producer bundle differs")
    if receipt["status"] != "complete" or receipt["error"] is not None:
        raise ValueError("child producer identity is not proven")
    validate_producer_identity(receipt["identity"], bundle)
    if receipt["identity"]["role"] != "child":
        raise ValueError("child producer identity has the wrong role")


def write_child_producer_from_environment() -> None:
    """Called after the public workflow and execution identity capture."""
    destination = os.environ.get(PREFIX + "OUTPUT")
    if not destination:
        return
    receipt = {
        "schema": CHILD_SCHEMA, "status": "not_proven",
        "invocation_id": os.environ.get(PREFIX + "INVOCATION_ID", ""),
        "producer_bundle_sha256": None, "identity": None, "error": None,
    }
    try:
        bundle, digest = _read_bundle(Path(os.environ[PREFIX + "BUNDLE"]))
        receipt["producer_bundle_sha256"] = digest
        receipt["identity"] = capture_producer_identity(bundle, role="child")
        receipt["status"] = "complete"
        _child_valid(receipt, bundle, digest, receipt["invocation_id"])
    except CAPTURE_ERRORS as exc:
        receipt.update(status="not_proven", identity=None, error=str(exc)[:1000])
    try:
        _base()._publish(Path(destination), receipt)
    except CAPTURE_ERRORS:
        # The parent's exact invocation membership gate rejects a missing file.
        pass


def merge_backend_attestations(
    output: Path,
    report_path: Path,
    execution_path: Path,
    result: dict[str, Any],
    backend_artifacts: list[tuple[Path, Path, Path]],
) -> dict[str, Any]:
    """Merge selected backend producer closures beside their merged report.

    Each tuple contains one backend's producer attestation, execution
    attestation, and producer bundle. The source attestations remain private
    temporary artifacts; the merged closure binds the selected public report
    and its merged execution attestation.
    """

    if not backend_artifacts:
        raise ValueError("no backend producer attestations to merge")
    if result.get("schema") not in {
        "migration-parity/parity-result@1",
        "migration-parity/parity-result@2",
    }:
        raise ValueError("merged producer attestation requires a parity report")

    base = _base()
    execution = json.loads(execution_path.read_text(encoding="utf-8"))
    if execution.get("schema") != base.SCHEMA or execution.get("status") != "complete":
        raise ValueError("merged parity execution attestation is not complete")
    if execution.get("errors"):
        raise ValueError("merged parity execution attestation contains errors")
    merged_invocations = execution.get("invocations")
    if not isinstance(merged_invocations, list):
        raise ValueError("merged parity execution attestation has no invocation list")
    expected_invocations = [item.get("invocation_id") for item in merged_invocations]
    if any(not isinstance(item, str) or not item for item in expected_invocations):
        raise ValueError("merged parity execution attestation has an invalid invocation")
    if len(expected_invocations) != len(set(expected_invocations)):
        raise ValueError("merged parity execution attestation duplicates an invocation")

    merged_producer_records: list[dict[str, Any]] = []
    seen_invocations: set[str] = set()
    common_bundle_bytes: bytes | None = None
    common_bundle_sha: str | None = None
    common_parent_start: dict[str, Any] | None = None
    common_parent_end: dict[str, Any] | None = None
    merged_producer_invocations: list[str] = []

    for producer_path, backend_execution_path, bundle_path in backend_artifacts:
        producer = json.loads(producer_path.read_text(encoding="utf-8"))
        backend_execution = json.loads(backend_execution_path.read_text(encoding="utf-8"))
        bundle_bytes = bundle_path.read_bytes()
        bundle = json.loads(bundle_bytes)
        bundle_sha = hashlib.sha256(bundle_bytes).hexdigest()
        if (
            set(producer)
            != {
                "schema",
                "status",
                "report",
                "execution_attestation_sha256",
                "producer_bundle_sha256",
                "parent_start",
                "parent_end",
                "invocations",
                "observed_invocation",
                "errors",
            }
            or producer.get("schema") != SCHEMA
            or producer.get("status") != "complete"
        ):
            raise ValueError("backend producer attestation is not complete")
        if producer.get("errors") != []:
            raise ValueError("backend producer attestation contains errors")
        if backend_execution.get("schema") != base.SCHEMA or backend_execution.get("status") != "complete":
            raise ValueError("backend parity execution attestation is not complete")
        if producer.get("producer_bundle_sha256") != bundle_sha:
            raise ValueError("backend producer bundle hash differs from its bytes")
        execution_sha = hashlib.sha256(backend_execution_path.read_bytes()).hexdigest()
        if producer.get("execution_attestation_sha256") != execution_sha:
            raise ValueError("backend producer attestation execution hash differs")
        if producer.get("report") != {
            key: backend_execution.get("report", {}).get(key)
            for key in ("canonical_sha256", "schema", "run_id")
        }:
            raise ValueError("backend producer attestation report binding differs")

        parent_start = producer.get("parent_start")
        parent_end = producer.get("parent_end")
        validate_producer_identity(parent_start, bundle)
        validate_producer_identity(parent_end, bundle)
        if parent_start.get("role") != "parent" or parent_end.get("role") != "parent":
            raise ValueError("backend producer parent identity has the wrong role")
        start_modules = {record["module"]: record for record in parent_start["loaded_modules"]}
        end_modules = {record["module"]: record for record in parent_end["loaded_modules"]}
        if any(end_modules.get(name) != record for name, record in start_modules.items()):
            raise ValueError("backend producer parent modules changed during capture")

        backend_execution_invocations = backend_execution.get("invocations")
        records = producer.get("invocations")
        if not isinstance(backend_execution_invocations, list) or not isinstance(records, list):
            raise ValueError("backend producer closure has an invalid invocation list")
        backend_ids = [item.get("invocation_id") for item in backend_execution_invocations]
        producer_ids = [item.get("invocation_id") for item in records]
        if (
            any(not isinstance(item, str) or not item for item in backend_ids)
            or len(backend_ids) != len(set(backend_ids))
            or producer_ids != backend_ids
        ):
            raise ValueError("backend producer invocation membership differs")
        for record in records:
            if (
                set(record) != {"invocation_id", "receipt", "errors"}
                or record["errors"] != []
            ):
                raise ValueError("backend producer invocation is not proven")
            invocation_id = record["invocation_id"]
            if invocation_id in seen_invocations:
                raise ValueError("backend producer attestations duplicate an invocation")
            seen_invocations.add(invocation_id)
            merged_producer_invocations.append(invocation_id)
            _child_valid(record["receipt"], bundle, bundle_sha, invocation_id)
            merged_producer_records.append(record)

        if common_bundle_bytes is None:
            common_bundle_bytes = bundle_bytes
            common_bundle_sha = bundle_sha
            common_parent_start = parent_start
            common_parent_end = parent_end
        elif bundle_bytes != common_bundle_bytes:
            raise ValueError("backend producer bundles differ")

    if (
        seen_invocations != set(expected_invocations)
        or merged_producer_invocations != expected_invocations
    ):
        raise ValueError("merged producer invocations differ from merged execution")
    assert common_bundle_bytes is not None
    assert common_bundle_sha is not None
    assert common_parent_start is not None
    assert common_parent_end is not None

    report_binding = base.report_fingerprint(report_path)
    report_reference = {
        "canonical_sha256": report_binding["canonical_sha256"],
        "schema": result["schema"],
        "run_id": result["identity"]["run_id"],
    }
    if execution.get("report") != {
        **report_binding,
        "schema": result["schema"],
        "run_id": result["identity"]["run_id"],
    }:
        raise ValueError("merged parity execution attestation does not bind the report")

    merged = {
        "schema": SCHEMA,
        "status": "complete",
        "report": report_reference,
        "execution_attestation_sha256": hashlib.sha256(execution_path.read_bytes()).hexdigest(),
        "producer_bundle_sha256": common_bundle_sha,
        "parent_start": common_parent_start,
        "parent_end": common_parent_end,
        "invocations": merged_producer_records,
        "observed_invocation": observed_invocation_context(),
        "errors": [],
    }

    bundle_output = output.with_name(output.name + ".bundle.json")
    if output.exists() or bundle_output.exists():
        raise FileExistsError("merged parity producer output already exists")
    bundle_output.parent.mkdir(parents=True, exist_ok=True)
    with bundle_output.open("xb") as stream:
        stream.write(common_bundle_bytes)
        stream.flush()
        os.fsync(stream.fileno())
    base._publish(output.resolve(), merged)
    return merged


class ProducerCollector:
    """One optional producer bundle and companion for an execution collector."""

    def __init__(self, output: Path):
        self.output = output.resolve()
        self.bundle_path = self.output.with_name(self.output.name + ".bundle.json")
        self.children = self.output.with_name(self.output.name + ".children")
        for path in (self.output, self.bundle_path):
            if path.exists():
                raise FileExistsError(f"producer artifact already exists: {path}")
        self._planned: dict[str, Path] = {}
        self._collected: dict[str, dict[str, Any]] = {}
        self._lock = threading.Lock()
        self.bundle = None
        self.bundle_sha = None
        self.parent_start = None
        self.observed_invocation = None
        self.errors: list[str] = []
        try:
            self.observed_invocation = observed_invocation_context()
            main = sys.modules.get("__main__")
            entrypoint = Path(getattr(main, "__file__", ""))
            bundle = capture_producer_bundle(Path(__file__).resolve().parent, entrypoint)
            self.parent_start = capture_producer_identity(bundle, role="parent")
            _base()._publish(self.bundle_path, bundle)
            self.bundle, self.bundle_sha = _read_bundle(self.bundle_path)
        except CAPTURE_ERRORS as exc:
            self.errors.append(str(exc)[:1000])

    def child_environment(self, invocation: str) -> dict[str, str]:
        with self._lock:
            if invocation in self._planned:
                raise ValueError("duplicate producer invocation")
            path = self.children / f"{invocation}.json"
            self._planned[invocation] = path
        return {
            PREFIX + "OUTPUT": str(path), PREFIX + "BUNDLE": str(self.bundle_path),
            PREFIX + "INVOCATION_ID": invocation,
        }

    def collect(self, invocation: str) -> None:
        with self._lock:
            path = self._planned[invocation]
            if invocation in self._collected:
                raise ValueError("producer receipt collected more than once")
        receipt = None
        errors = []
        try:
            receipt = json.loads(path.read_text(encoding="utf-8"))
            if self.bundle is None or self.bundle_sha is None:
                raise ValueError("parent producer bundle is not proven")
            _child_valid(receipt, self.bundle, self.bundle_sha, invocation)
        except CAPTURE_ERRORS as exc:
            errors.append(str(exc)[:1000])
        with self._lock:
            self._collected[invocation] = {
                "invocation_id": invocation, "receipt": receipt, "errors": errors,
            }

    def finalize(self, execution_path: Path, execution: dict[str, Any]) -> dict[str, Any]:
        with self._lock:
            records = [
                self._collected.get(invocation, {
                    "invocation_id": invocation, "receipt": None,
                    "errors": ["child producer receipt was not collected"],
                })
                for invocation in self._planned
            ]
        document = {
            "schema": SCHEMA, "status": "not_proven",
            "report": {key: execution["report"][key] for key in ("canonical_sha256", "schema", "run_id")},
            "execution_attestation_sha256": hashlib.sha256(execution_path.read_bytes()).hexdigest(),
            "producer_bundle_sha256": self.bundle_sha,
            "parent_start": self.parent_start, "parent_end": None,
            "invocations": records,
            "observed_invocation": self.observed_invocation,
            "errors": list(self.errors),
        }
        try:
            if document["errors"] or self.bundle is None:
                raise ValueError("parent producer inventory is not proven")
            bundle, digest = _read_bundle(self.bundle_path)
            if digest != self.bundle_sha or bundle != self.bundle:
                raise ValueError("producer bundle bytes changed")
            document["parent_end"] = capture_producer_identity(bundle, role="parent")
            end_modules = {record["module"]: record for record in document["parent_end"]["loaded_modules"]}
            if any(end_modules.get(record["module"]) != record for record in self.parent_start["loaded_modules"]):
                raise ValueError("parent loaded producer modules disappeared or changed")
            expected = {record["invocation_id"] for record in execution["invocations"]}
            observed = [record["invocation_id"] for record in records]
            if not expected or len(observed) != len(set(observed)) or set(observed) != expected:
                raise ValueError("producer invocation membership differs from execution receipt")
            for record in records:
                if record["errors"]:
                    raise ValueError("one or more child producer receipts are not proven")
                _child_valid(record["receipt"], bundle, digest, record["invocation_id"])
            document["status"] = "complete"
        except CAPTURE_ERRORS as exc:
            document["errors"].append(str(exc)[:1000])
        _base()._publish(self.output, document)
        return document
