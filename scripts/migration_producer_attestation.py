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
SCHEMA = "migration-parity/producer-attestation@1"
CHILD_SCHEMA = "migration-parity/child-producer-attestation@1"
CAPTURE_ERRORS = (OSError, RuntimeError, ValueError, TypeError, KeyError)


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
        self.errors: list[str] = []
        try:
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
            "invocations": records, "errors": list(self.errors),
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
