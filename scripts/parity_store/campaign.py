"""Project-neutral, bounded orchestration for parity campaign profiles."""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Protocol
import uuid

from .model import (
    CaseRecord,
    ComparisonDecision,
    ComparisonRecord,
    ExecutionRecord,
    ImplementationIdentity,
)
from .postgres import PostgresStore
from .values import case_set_digest


class BatchExecutor(Protocol):
    """Execute a bounded set of cases through one public implementation."""

    identity: ImplementationIdentity

    def execute_batch(
        self, cases: Sequence[CaseRecord]
    ) -> Mapping[str, ExecutionRecord]:
        """Return exactly one execution record per supplied case ID."""


class Comparator(Protocol):
    """Apply one project's pinned public-result and error comparison policy."""

    identity: ImplementationIdentity

    def compare(
        self,
        case: CaseRecord,
        source: ExecutionRecord | None,
        target: ExecutionRecord,
    ) -> ComparisonDecision:
        """Compare a reference pair or a target-only contract case."""


@dataclass(frozen=True, slots=True)
class CampaignSummary:
    run_id: str
    profile_id: str
    selected_cases: int
    completed_batches: int
    outcomes: Mapping[str, int]


def run_target_profile(
    store: PostgresStore,
    *,
    project_id: str,
    specification_digest: bytes,
    profile_id: str,
    target: BatchExecutor,
    comparator: Comparator,
    source: BatchExecutor | None = None,
    host_description: str,
    run_id: str | None = None,
    case_ids: Sequence[str] | None = None,
    maximum_cases: int = 128,
    maximum_estimated_bytes: int = 32 * 1024 * 1024,
) -> CampaignSummary:
    """Run one target profile incrementally and persist typed evidence.

    Project adapters own case hydration, public execution, and comparison.
    This coordinator only selects profile-compatible cases, reuses executions
    with matching identities and digests, and persists their outcomes.
    """

    _validate_identities(project_id, profile_id, source, target, comparator)
    if maximum_cases <= 0 or maximum_estimated_bytes <= 0:
        raise ValueError("campaign batch limits must be positive")

    with store.connection.transaction():
        selected = store.selected_case_digests(
            project_id,
            specification_digest,
            profile_id=profile_id,
            case_ids=case_ids,
        )
    if not selected:
        raise ValueError(f"profile {profile_id!r} selects no imported cases")
    if case_ids is not None and {case_id for case_id, _digest in selected} != set(case_ids):
        raise ValueError("explicit case selection includes unknown or profile-ineligible IDs")
    selected_digest = case_set_digest(selected)
    run_id = run_id or str(uuid.uuid4())
    identities = [target.identity.identity_digest]

    with store.connection.transaction():
        if source is not None:
            store.register_identity(source.identity)
        store.register_identity(target.identity)
        store.register_identity(comparator.identity)
        store.create_run(
            run_id,
            project_id,
            specification_digest,
            selected_digest,
            host_description,
            identities,
            comparator.identity.identity_digest,
            [case_id for case_id, _digest in selected],
            source_identity_digest=(
                None if source is None else source.identity.identity_digest
            ),
        )

    outcomes: Counter[str] = Counter()
    completed_batches = 0
    error_recorded = False
    try:
        batches = store.iter_case_batches(
            project_id,
            specification_digest,
            maximum_cases=maximum_cases,
            maximum_estimated_bytes=maximum_estimated_bytes,
            profile_id=profile_id,
            case_ids=case_ids,
        )
        for batch_number, (cases, estimated_bytes) in enumerate(batches):
            case_ids = [case.case_id for case in cases]
            with store.connection.transaction():
                batch_id = store.start_batch(
                    run_id,
                    target.identity.identity_digest,
                    batch_number,
                    case_ids,
                    estimated_bytes,
                )
            try:
                _run_batch(
                    store,
                    run_id=run_id,
                    project_id=project_id,
                    specification_digest=specification_digest,
                    target=target,
                    source=source,
                    comparator=comparator,
                    cases=cases,
                    outcomes=outcomes,
                )
            except Exception as exc:
                with store.connection.transaction():
                    store.finish_batch(
                        batch_id,
                        error_kind=type(exc).__name__,
                        error_message=str(exc),
                    )
                    store.results.record_run_error(
                        run_id,
                        scope="batch",
                        external_id=str(batch_number),
                        batch_id=batch_id,
                        error_kind=type(exc).__name__,
                        message=str(exc),
                    )
                error_recorded = True
                raise
            with store.connection.transaction():
                store.finish_batch(batch_id)
            completed_batches += 1
    except Exception as exc:
        with store.connection.transaction():
            if not error_recorded:
                store.results.record_run_error(
                    run_id,
                    scope="run",
                    error_kind=type(exc).__name__,
                    message=str(exc),
                )
            store.set_run_state(run_id, "failed")
        raise

    try:
        with store.connection.transaction():
            store.verify_run_profile_complete(run_id, target.identity.identity_digest)
            store.set_run_state(run_id, "completed")
    except Exception as exc:
        with store.connection.transaction():
            store.results.record_run_error(
                run_id,
                scope="finalization",
                error_kind=type(exc).__name__,
                message=str(exc),
            )
            store.set_run_state(run_id, "failed")
        raise
    return CampaignSummary(
        run_id=run_id,
        profile_id=profile_id,
        selected_cases=len(selected),
        completed_batches=completed_batches,
        outcomes=dict(sorted(outcomes.items())),
    )


def _run_batch(
    store: PostgresStore,
    *,
    run_id: str,
    project_id: str,
    specification_digest: bytes,
    target: BatchExecutor,
    source: BatchExecutor | None,
    comparator: Comparator,
    cases: Sequence[CaseRecord],
    outcomes: Counter[str],
) -> None:
    for case in cases:
        if case.project_id != project_id or case.specification_digest != specification_digest:
            raise ValueError(f"case {case.case_id!r} is outside the selected specification")
        if target.identity.profile_id not in case.profiles:
            raise ValueError(f"case {case.case_id!r} is not declared for the target profile")
        if case.verification_kind not in {"parity", "expected_output", "fault_contract"}:
            raise ValueError(
                f"case {case.case_id!r} has unsupported verification kind "
                f"{case.verification_kind!r}"
            )

    source_cases = [case for case in cases if case.verification_kind == "parity"]
    if source_cases and source is None:
        raise ValueError("reference parity cases require a source executor")

    source_records = _existing_executions(store, source, source_cases)
    target_records = _existing_executions(store, target, cases)
    missing_source = [case for case in source_cases if case.case_id not in source_records]
    missing_target = [case for case in cases if case.case_id not in target_records]

    if missing_source:
        assert source is not None
        source_records.update(_execute_and_store(store, source, missing_source))
    if missing_target:
        target_records.update(_execute_and_store(store, target, missing_target))

    for case in cases:
        source_execution = source_records.get(case.case_id)
        source_record = None if source_execution is None else source_execution[1]
        target_execution_id, target_record = target_records[case.case_id]
        if source_record is not None and source_record.state == "infrastructure_error":
            raise RuntimeError(f"source execution for {case.case_id!r} is an infrastructure error")
        if target_record.state == "infrastructure_error":
            raise RuntimeError(f"target execution for {case.case_id!r} is an infrastructure error")
        decision = comparator.compare(case, source_record, target_record)
        if decision.outcome not in {
            "pass",
            "fail",
            "not_run",
            "not_applicable",
            "infrastructure_error",
        }:
            raise ValueError(f"comparator returned unknown outcome {decision.outcome!r}")
        if decision.outcome == "infrastructure_error":
            raise RuntimeError(f"comparator reported infrastructure failure for {case.case_id!r}")

        record = ComparisonRecord(
            run_id=run_id,
            target_identity_digest=target.identity.identity_digest,
            comparator_identity_digest=comparator.identity.identity_digest,
            project_id=project_id,
            specification_digest=specification_digest,
            case_id=case.case_id,
            source_execution_id=(None if source_execution is None else source_execution[0]),
            target_execution_id=target_execution_id,
            outcome=decision.outcome,
            differences=decision.differences,
            compared_at=datetime.now(timezone.utc),
        )
        with store.connection.transaction():
            store.results.record_comparison(record)
        outcomes[decision.outcome] += 1


def _existing_executions(
    store: PostgresStore,
    executor: BatchExecutor | None,
    cases: Sequence[CaseRecord],
) -> dict[str, tuple[int, ExecutionRecord]]:
    if executor is None or not cases:
        return {}
    records: dict[str, tuple[int, ExecutionRecord]] = {}
    with store.connection.transaction():
        for case in cases:
            found = store.results.find_execution(
                executor.identity.identity_digest,
                case.project_id,
                case.specification_digest,
                case.case_id,
                case.case_digest,
            )
            if found is not None:
                _validate_execution(executor.identity, case, found[1])
                records[case.case_id] = found
    return records


def _execute_and_store(
    store: PostgresStore,
    executor: BatchExecutor,
    cases: Sequence[CaseRecord],
) -> dict[str, tuple[int, ExecutionRecord]]:
    received = executor.execute_batch(cases)
    expected_ids = {case.case_id for case in cases}
    if set(received) != expected_ids:
        missing = sorted(expected_ids - set(received))
        extra = sorted(set(received) - expected_ids)
        raise ValueError(f"executor case set mismatch; missing={missing}, extra={extra}")

    result: dict[str, tuple[int, ExecutionRecord]] = {}
    for case in cases:
        execution = received[case.case_id]
        _validate_execution(executor.identity, case, execution)
        if execution.state == "infrastructure_error":
            raise RuntimeError(
                f"executor returned infrastructure error for {case.case_id!r}: "
                f"{execution.reason or 'no reason supplied'}"
            )
        with store.connection.transaction():
            execution_id = store.results.record_execution(execution)
        result[case.case_id] = (execution_id, execution)
    return result


def _validate_execution(
    identity: ImplementationIdentity,
    case: CaseRecord,
    execution: ExecutionRecord,
) -> None:
    expected = (
        identity.identity_digest,
        case.project_id,
        case.specification_digest,
        case.case_id,
        case.case_digest,
    )
    actual = (
        execution.identity_digest,
        execution.project_id,
        execution.specification_digest,
        execution.case_id,
        execution.case_digest,
    )
    if actual != expected:
        raise ValueError(f"execution identity does not match case {case.case_id!r}")


def _validate_identities(
    project_id: str,
    profile_id: str,
    source: BatchExecutor | None,
    target: BatchExecutor,
    comparator: Comparator,
) -> None:
    if target.identity.project_id != project_id or target.identity.implementation_kind != "target":
        raise ValueError("target executor identity does not match the project or target role")
    if target.identity.profile_id != profile_id:
        raise ValueError("target executor identity does not match the selected profile")
    if source is not None and (
        source.identity.project_id != project_id
        or source.identity.implementation_kind != "oracle"
    ):
        raise ValueError("source executor identity does not match the project or oracle role")
    if (
        comparator.identity.project_id != project_id
        or comparator.identity.implementation_kind != "comparator"
    ):
        raise ValueError("comparator identity does not match the project or comparator role")
    for identity in (
        [target.identity, comparator.identity]
        if source is None
        else [source.identity, target.identity, comparator.identity]
    ):
        if len(identity.identity_digest) != 32 or len(identity.configuration_digest) != 32:
            raise ValueError("implementation identities must use SHA-256 digests")
