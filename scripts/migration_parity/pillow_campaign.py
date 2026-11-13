"""Pillow execution and comparison adapters for the generic parity store."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
import hashlib
from pathlib import Path
import platform
from typing import Any

from scripts import run_migration_parity as parity
from scripts.migration_parity.pillow_fixture_adapter import (
    PROJECT_ID,
    _from_store_tree,
    _to_store_tree,
    record_to_runner_case,
)
from scripts.parity_store import (
    CaseRecord,
    ComparisonDecision,
    DifferenceRecord,
    ExecutionRecord,
    ImplementationIdentity,
    ObservationRecord,
    canonical_value_digest,
)

DEFAULT_TIMEOUT_SECONDS = 3600


class PillowBatchExecutor:
    """Run one bounded typed case batch through a Pillow public interface."""

    def __init__(
        self,
        *,
        side: str,
        manifest_path: Path,
        identity: ImplementationIdentity,
        handshake: Mapping[str, Any],
        timeout_seconds: int,
    ) -> None:
        if side not in {"source", "target"}:
            raise ValueError(f"invalid Pillow parity side: {side!r}")
        if timeout_seconds <= 0:
            raise ValueError("Pillow adapter timeout must be positive")
        self.side = side
        self.manifest_path = manifest_path
        self.identity = identity
        self.handshake = dict(handshake)
        self.timeout_seconds = timeout_seconds

    def execute_batch(self, cases: Sequence[CaseRecord]) -> Mapping[str, ExecutionRecord]:
        raw_cases = [record_to_runner_case(case) for case in cases]
        started_at = datetime.now(timezone.utc)
        handshake, results = parity.run_side_subprocess(
            self.side,
            self.manifest_path,
            raw_cases,
            self.timeout_seconds,
        )
        finished_at = datetime.now(timezone.utc)
        if handshake != self.handshake:
            raise RuntimeError(f"{self.side} identity changed during the parity campaign")
        return {
            case.case_id: runner_result_to_execution(
                case,
                results[case.case_id],
                identity=self.identity,
                started_at=started_at,
                finished_at=finished_at,
            )
            for case in cases
        }


class PillowComparator:
    """Keep the existing Pillow comparator behind the generic adapter API."""

    def __init__(
        self,
        identity: ImplementationIdentity,
        manifest: dict[str, Any],
    ) -> None:
        self.identity = identity
        self.operation_index = parity.build_operation_index(manifest)

    def compare(
        self,
        case: CaseRecord,
        source: ExecutionRecord | None,
        target: ExecutionRecord,
    ) -> ComparisonDecision:
        raw_case = record_to_runner_case(case)
        source_result = (
            parity.not_applicable_source_result(raw_case)
            if source is None
            else execution_to_runner_result(case, source)
        )
        target_result = execution_to_runner_result(case, target)
        outcome, differences = parity.compare_case(
            raw_case,
            source_result,
            target_result,
            self.operation_index,
        )
        return ComparisonDecision(
            outcome=outcome,
            differences=tuple(
                DifferenceRecord(
                    step_id=item["step_id"],
                    value_path=item["path"],
                    kind=item["kind"],
                    message=item["message"],
                    source_value=_to_store_tree(item.get("source")),
                    target_value=_to_store_tree(item.get("target")),
                    source_value_present="source" in item,
                    target_value_present="target" in item,
                )
                for item in differences
            ),
        )


def build_pillow_adapters(
    manifest: dict[str, Any],
    manifest_path: Path,
    profile_id: str,
) -> tuple[PillowBatchExecutor | None, PillowBatchExecutor, PillowComparator]:
    """Handshake with both sides and freeze their identities for one profile."""

    target_profile_id = parity.target_profile_for_backend(parity.TARGET_BACKEND)
    if profile_id != target_profile_id:
        raise ValueError(
            f"selected profile {profile_id!r} does not match process profile "
            f"{target_profile_id!r}"
        )
    timeout_seconds = parity.effective_adapter_timeout(DEFAULT_TIMEOUT_SECONDS)
    source_handshake, _ = parity.run_side_subprocess(
        "source", manifest_path, [], DEFAULT_TIMEOUT_SECONDS
    )
    if source_handshake.get("version") != parity.ORACLE_VERSION:
        raise RuntimeError(
            f"oracle identity handshake did not pin {parity.ORACLE_NAME} "
            f"{parity.ORACLE_VERSION}"
        )
    target_handshake, _ = parity.run_side_subprocess(
        "target", manifest_path, [], timeout_seconds
    )

    adapter_digest = _adapter_digest()
    source_identity = _make_identity(
        kind="oracle",
        implementation_id=str(source_handshake["implementation"]),
        profile_id=parity.parity_source_profile(
            str(source_handshake["implementation"])
        ),
        revision=str(source_handshake["version"]),
        dirty=False,
        configuration={
            "handshake": source_handshake,
            "runtime": platform.python_version(),
            "adapter_sha256": adapter_digest,
        },
        attributes={
            "side": "source",
            "implementation": str(source_handshake["implementation"]),
            "version": str(source_handshake["version"]),
        },
    )
    target_identity = _make_identity(
        kind="target",
        implementation_id=str(target_handshake["implementation"]),
        profile_id=profile_id,
        revision=parity.git_revision(),
        dirty=parity.git_dirty(),
        configuration={
            "handshake": target_handshake,
            "backend": parity.TARGET_BACKEND,
            "architecture": (
                parity.TARGET_ARCHITECTURE
                if parity.TARGET_BACKEND == "cpu"
                else None
            ),
            "features": sorted(parity.TARGET_FEATURES),
            "runtime": platform.python_version(),
            "adapter_sha256": adapter_digest,
        },
        attributes={
            "side": "target",
            "version": str(target_handshake["version"]),
            "backend": parity.TARGET_BACKEND,
            "architecture": (
                parity.TARGET_ARCHITECTURE
                if parity.TARGET_BACKEND == "cpu"
                else "not-applicable"
            ),
            "features": ";".join(sorted(parity.TARGET_FEATURES)),
        },
    )
    comparator_identity = _make_identity(
        kind="comparator",
        implementation_id="pillow-rs.compare_case",
        profile_id=None,
        revision=parity.git_revision(),
        dirty=parity.git_dirty(),
        configuration={
            "comparison_module_sha256": hashlib.sha256(
                Path(parity.__file__).read_bytes()
            ).digest(),
            "adapter_sha256": adapter_digest,
            "specification_schema": manifest["schema"],
        },
        attributes={"comparator": "compare_case"},
    )
    return (
        PillowBatchExecutor(
            side="source",
            manifest_path=manifest_path,
            identity=source_identity,
            handshake=source_handshake,
            timeout_seconds=DEFAULT_TIMEOUT_SECONDS,
        ),
        PillowBatchExecutor(
            side="target",
            manifest_path=manifest_path,
            identity=target_identity,
            handshake=target_handshake,
            timeout_seconds=timeout_seconds,
        ),
        PillowComparator(comparator_identity, manifest),
    )


def runner_result_to_execution(
    case: CaseRecord,
    result: dict[str, Any],
    *,
    identity: ImplementationIdentity,
    started_at: datetime,
    finished_at: datetime,
) -> ExecutionRecord:
    """Map one unchanged runner result into project-neutral typed rows."""

    status = result.get("status")
    states = {"completed": "completed", "not_run": "not_run", "not_applicable": "not_applicable"}
    try:
        state = states[status]
    except (KeyError, TypeError) as exc:
        raise ValueError(f"runner returned unknown case status: {status!r}") from exc
    observations = tuple(_runner_observation(item) for item in result.get("observations", []))
    details = _to_store_tree(
        {key: value for key, value in result.items() if key not in {"case_id", "status", "observations"}}
    )
    return ExecutionRecord(
        identity_digest=identity.identity_digest,
        project_id=case.project_id,
        specification_digest=case.specification_digest,
        case_id=case.case_id,
        case_digest=case.case_digest,
        state=state,
        started_at=started_at,
        finished_at=finished_at,
        duration_ns=None,
        observations=observations,
        reason=result.get("reason"),
        details=details,
    )


def execution_to_runner_result(
    case: CaseRecord, execution: ExecutionRecord
) -> dict[str, Any]:
    """Reconstruct the parity runner's comparator input from stored records."""

    result = _from_store_tree(execution.details)
    if not isinstance(result, dict):
        raise TypeError("stored Pillow execution details are not a typed object")
    result.update(
        {
            "case_id": case.case_id,
            "status": execution.state,
            "observations": [
                _observation_to_runner(item) for item in execution.observations
            ],
        }
    )
    if execution.reason is not None:
        result["reason"] = execution.reason
    return result


def _runner_observation(value: dict[str, Any]) -> ObservationRecord:
    step_id = value["step_id"]
    status = value.get("status")
    if status == "ok":
        return ObservationRecord(step_id, "ok", _to_store_tree(value.get("value")))
    if status == "error":
        error = value.get("error")
        if not isinstance(error, dict):
            raise ValueError(f"error observation {step_id!r} has no public error record")
        return ObservationRecord(
            step_id=step_id,
            state="error",
            error_class=error.get("class"),
            error_kind=error.get("kind"),
            error_message=error.get("message"),
            error_stage=error.get("stage"),
            error_code=error.get("code"),
        )
    if status == "not_run":
        return ObservationRecord(
            step_id=step_id,
            state="not_run",
            reason=value.get("reason") or "workflow observation was not run",
        )
    raise ValueError(f"runner returned unknown observation status: {status!r}")


def _observation_to_runner(value: ObservationRecord) -> dict[str, Any]:
    if value.state == "ok":
        return {"step_id": value.step_id, "status": "ok", "value": _from_store_tree(value.value)}
    if value.state == "error":
        return {
            "step_id": value.step_id,
            "status": "error",
            "error": {
                "class": value.error_class,
                "kind": value.error_kind,
                "message": value.error_message,
                "stage": value.error_stage,
                "code": value.error_code,
            },
        }
    return {"step_id": value.step_id, "status": value.state, "reason": value.reason}


def _make_identity(
    *,
    kind: str,
    implementation_id: str,
    profile_id: str | None,
    revision: str,
    dirty: bool,
    configuration: dict[str, Any],
    attributes: dict[str, str | int | bool],
) -> ImplementationIdentity:
    configuration_digest = canonical_value_digest(configuration)
    identity_fields = {
        "project_id": PROJECT_ID,
        "implementation_kind": kind,
        "implementation_id": implementation_id,
        "profile_id": profile_id,
        "revision": revision,
        "runtime": f"CPython {platform.python_version()}",
        "dirty": dirty,
        "configuration_digest": configuration_digest,
    }
    return ImplementationIdentity(
        identity_digest=canonical_value_digest(identity_fields),
        project_id=PROJECT_ID,
        implementation_kind=kind,
        implementation_id=implementation_id,
        profile_id=profile_id,
        revision=revision,
        runtime=f"CPython {platform.python_version()}",
        dirty=dirty,
        configuration_digest=configuration_digest,
        attributes=attributes,
    )


def _adapter_digest() -> bytes:
    digest = hashlib.sha256()
    for path in (Path(__file__), Path(parity.__file__)):
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.digest()
