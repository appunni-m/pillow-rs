"""Persistence for executions, observations, comparisons, and run errors."""

from __future__ import annotations

from datetime import datetime
import hashlib
from typing import Any

from .model import ComparisonRecord, DifferenceRecord, ExecutionRecord, ObservationRecord
from .values import canonical_value_digest, flatten_value


class ResultStore:
    """Write and reuse typed parity evidence through a PostgreSQL case store."""

    def __init__(self, store: Any):
        self.store = store
        self.connection = store.connection

    def record_execution(self, record: ExecutionRecord) -> int:
        self._validate_execution(record)
        result_digest = canonical_value_digest(_execution_payload(record))
        existing = self.connection.execute(
            "SELECT execution_id, result_digest FROM parity_execution "
            "WHERE identity_digest = %s AND project_id = %s AND specification_digest = %s "
            "AND case_id = %s AND case_digest = %s",
            (
                record.identity_digest,
                record.project_id,
                record.specification_digest,
                record.case_id,
                record.case_digest,
            ),
        ).fetchone()
        if existing is not None:
            if bytes(existing[1]) != result_digest:
                raise ValueError(
                    "the same immutable execution identity produced different observations"
                )
            return int(existing[0])

        ok_observations = [item for item in record.observations if item.state == "ok"]
        trees = [flatten_value(item.value) for item in ok_observations]
        details_tree_index = None
        if record.details:
            details_tree_index = len(trees)
            trees.append(flatten_value(record.details))
        with self.connection.transaction():
            value_maps = self.store._insert_value_trees(trees)
            value_ids = iter(value_map[()] for value_map in value_maps)
            identity = self.connection.execute(
                "SELECT project_id, implementation_kind FROM parity_implementation_identity "
                "WHERE identity_digest = %s",
                (record.identity_digest,),
            ).fetchone()
            if identity is None or identity[0] != record.project_id or identity[1] == "comparator":
                raise ValueError("execution identity is missing or belongs to another project")
            case = self.connection.execute(
                "SELECT case_digest FROM parity_case WHERE project_id = %s "
                "AND specification_digest = %s AND case_id = %s",
                (record.project_id, record.specification_digest, record.case_id),
            ).fetchone()
            if case is None or bytes(case[0]) != record.case_digest:
                raise ValueError("execution does not match the imported case digest")
            execution_id = int(
                self.connection.execute(
                    "INSERT INTO parity_execution "
                    "(identity_digest, project_id, specification_digest, case_id, case_digest, "
                    "execution_state, result_digest, started_at, finished_at, duration_ns, execution_reason) "
                    "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s) RETURNING execution_id",
                    (
                        record.identity_digest,
                        record.project_id,
                        record.specification_digest,
                        record.case_id,
                        record.case_digest,
                        record.state,
                        result_digest,
                        record.started_at,
                        record.finished_at,
                        record.duration_ns,
                        record.reason,
                    ),
                ).fetchone()[0]
            )
            observations = []
            values = iter(value_ids)
            for ordinal, item in enumerate(record.observations):
                value_id = next(values) if item.state == "ok" else None
                observations.append(
                    (
                        execution_id,
                        ordinal,
                        item.step_id,
                        item.state,
                        value_id,
                        item.error_class,
                        item.error_kind,
                        item.error_message,
                        item.error_stage,
                        item.error_code,
                        item.reason,
                    )
                )
            self.store._executemany(
                "INSERT INTO parity_observation "
                "(execution_id, observation_ordinal, step_id, observation_state, value_id, "
                "error_class, error_kind, error_message, error_stage, error_code, reason) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)",
                observations,
            )
            if details_tree_index is not None:
                self.connection.execute(
                    "INSERT INTO parity_execution_details (execution_id, value_id) VALUES (%s, %s)",
                    (execution_id, value_maps[details_tree_index][()]),
                )
            return execution_id

    def find_execution(
        self,
        identity_digest: bytes,
        project_id: str,
        specification_digest: bytes,
        case_id: str,
        case_digest: bytes,
    ) -> tuple[int, ExecutionRecord] | None:
        row = self.connection.execute(
            "SELECT execution_id, execution_state, started_at, finished_at, duration_ns, "
            "result_digest, execution_reason "
            "FROM parity_execution WHERE identity_digest = %s AND project_id = %s "
            "AND specification_digest = %s AND case_id = %s AND case_digest = %s",
            (identity_digest, project_id, specification_digest, case_id, case_digest),
        ).fetchone()
        if row is None:
            return None
        execution_id, state, started_at, finished_at, duration_ns, result_digest, reason = row
        observation_rows = self.connection.execute(
            "SELECT observation_ordinal, step_id, observation_state, value_id, error_class, "
            "error_kind, error_message, error_stage, error_code, reason "
            "FROM parity_observation WHERE execution_id = %s ORDER BY observation_ordinal",
            (execution_id,),
        ).fetchall()
        value_ids = [int(item[3]) for item in observation_rows if item[3] is not None]
        values = self.store._load_value_roots(value_ids)
        details_row = self.connection.execute(
            "SELECT value_id FROM parity_execution_details WHERE execution_id = %s",
            (execution_id,),
        ).fetchone()
        details = (
            {}
            if details_row is None
            else self.store._load_value_roots([int(details_row[0])])[int(details_row[0])]
        )
        observations = tuple(
            ObservationRecord(
                step_id=item[1],
                state=item[2],
                value=(None if item[3] is None else values[int(item[3])]),
                error_class=item[4],
                error_kind=item[5],
                error_message=item[6],
                error_stage=item[7],
                error_code=item[8],
                reason=item[9],
            )
            for item in observation_rows
        )
        record = ExecutionRecord(
            identity_digest=identity_digest,
            project_id=project_id,
            specification_digest=specification_digest,
            case_id=case_id,
            case_digest=case_digest,
            state=state,
            started_at=started_at,
            finished_at=finished_at,
            duration_ns=(None if duration_ns is None else int(duration_ns)),
            observations=observations,
            reason=reason,
            details=details,
        )
        if canonical_value_digest(_execution_payload(record)) != bytes(result_digest):
            raise ValueError("stored execution observation digest does not match its rows")
        return int(execution_id), record

    def record_comparison(self, record: ComparisonRecord) -> bytes:
        self._validate_comparison(record)
        digest = canonical_value_digest(_comparison_payload(record))
        existing = self.connection.execute(
            "SELECT comparison_digest FROM parity_comparison WHERE run_id = %s "
            "AND target_identity_digest = %s AND project_id = %s "
            "AND specification_digest = %s AND case_id = %s",
            (
                record.run_id,
                record.target_identity_digest,
                record.project_id,
                record.specification_digest,
                record.case_id,
            ),
        ).fetchone()
        if existing is not None:
            if bytes(existing[0]) != digest:
                raise ValueError("the same run/profile/case has a different comparison result")
            return digest

        value_slots: list[tuple[int, str, int]] = []
        trees = []
        for diff_index, difference in enumerate(record.differences):
            if difference.source_value_present:
                value_slots.append((diff_index, "source", len(trees)))
                trees.append(flatten_value(difference.source_value))
            if difference.target_value_present:
                value_slots.append((diff_index, "target", len(trees)))
                trees.append(flatten_value(difference.target_value))

        with self.connection.transaction():
            value_maps = self.store._insert_value_trees(trees)
            value_ids = {index: values[()] for index, values in enumerate(value_maps)}
            run_comparator = self.connection.execute(
                "SELECT identity_digest FROM parity_run_comparator WHERE run_id = %s",
                (record.run_id,),
            ).fetchone()
            if run_comparator is None or bytes(run_comparator[0]) != record.comparator_identity_digest:
                raise ValueError("comparison comparator does not match the run identity")
            target = self.connection.execute(
                "SELECT 1 FROM parity_run_profile WHERE run_id = %s AND identity_digest = %s",
                (record.run_id, record.target_identity_digest),
            ).fetchone()
            if target is None:
                raise ValueError("comparison target identity is not part of the run")
            run = self.connection.execute(
                "SELECT project_id, specification_digest FROM parity_run WHERE run_id = %s",
                (record.run_id,),
            ).fetchone()
            if run != (record.project_id, record.specification_digest):
                raise ValueError("comparison project or specification differs from the run")
            selected_case = self.connection.execute(
                "SELECT c.verification_kind FROM parity_run_case AS rc "
                "JOIN parity_case AS c ON c.project_id = rc.project_id "
                "AND c.specification_digest = rc.specification_digest AND c.case_id = rc.case_id "
                "WHERE rc.run_id = %s AND rc.project_id = %s "
                "AND rc.specification_digest = %s AND rc.case_id = %s",
                (
                    record.run_id,
                    record.project_id,
                    record.specification_digest,
                    record.case_id,
                ),
            ).fetchone()
            if selected_case is None:
                raise ValueError("comparison case is not part of the frozen run selection")
            run_source = self.connection.execute(
                "SELECT identity_digest FROM parity_run_oracle WHERE run_id = %s",
                (record.run_id,),
            ).fetchone()
            if record.source_execution_id is None and selected_case[0] == "parity":
                raise ValueError("reference parity comparison is missing its source execution")
            target_execution = self.connection.execute(
                "SELECT identity_digest, project_id, specification_digest, case_id "
                "FROM parity_execution WHERE execution_id = %s",
                (record.target_execution_id,),
            ).fetchone()
            if target_execution != (
                record.target_identity_digest,
                record.project_id,
                record.specification_digest,
                record.case_id,
            ):
                raise ValueError("comparison target execution does not match its case/profile")
            if record.source_execution_id is not None:
                source_execution = self.connection.execute(
                    "SELECT identity_digest, project_id, specification_digest, case_id "
                    "FROM parity_execution WHERE execution_id = %s",
                    (record.source_execution_id,),
                ).fetchone()
                if (
                    run_source is None
                    or source_execution is None
                    or bytes(run_source[0]) != bytes(source_execution[0])
                    or source_execution[1:] != (
                        record.project_id,
                        record.specification_digest,
                        record.case_id,
                    )
                ):
                    raise ValueError("comparison reference execution does not match its case")
            comparison_row = self.connection.execute(
                "INSERT INTO parity_comparison "
                "(run_id, target_identity_digest, comparator_identity_digest, project_id, "
                "specification_digest, case_id, source_execution_id, target_execution_id, "
                "outcome, difference_count, comparison_digest, compared_at) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s) "
                "RETURNING comparison_digest",
                (
                    record.run_id,
                    record.target_identity_digest,
                    record.comparator_identity_digest,
                    record.project_id,
                    record.specification_digest,
                    record.case_id,
                    record.source_execution_id,
                    record.target_execution_id,
                    record.outcome,
                    len(record.differences),
                    digest,
                    record.compared_at,
                ),
            ).fetchone()
            if bytes(comparison_row[0]) != digest:
                raise ValueError("stored comparison digest changed during insertion")
            slot_ids = {
                (difference_index, side): value_ids[tree_index]
                for difference_index, side, tree_index in value_slots
            }
            difference_rows = []
            for ordinal, difference in enumerate(record.differences):
                difference_rows.append(
                    (
                        record.run_id,
                        record.target_identity_digest,
                        record.project_id,
                        record.specification_digest,
                        record.case_id,
                        ordinal,
                        difference.step_id,
                        difference.value_path,
                        difference.kind,
                        difference.message,
                        difference.source_value_present,
                        slot_ids.get((ordinal, "source")),
                        difference.target_value_present,
                        slot_ids.get((ordinal, "target")),
                    )
                )
            self.store._executemany(
                "INSERT INTO parity_difference "
                "(run_id, target_identity_digest, project_id, specification_digest, case_id, "
                "difference_ordinal, step_id, value_path, difference_kind, message, "
                "source_value_present, source_value_id, target_value_present, target_value_id) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)",
                difference_rows,
            )
        return digest

    def record_run_error(
        self,
        run_id: str,
        *,
        scope: str,
        error_kind: str,
        message: str,
        external_id: str | None = None,
        batch_id: int | None = None,
    ) -> int:
        if batch_id is not None:
            row = self.connection.execute(
                "SELECT 1 FROM parity_batch WHERE batch_id = %s AND run_id = %s",
                (batch_id, run_id),
            ).fetchone()
            if row is None:
                raise ValueError("run error batch does not belong to the requested run")
        ordinal = int(
            self.connection.execute(
                "SELECT COALESCE(MAX(error_ordinal) + 1, 0) FROM parity_run_error WHERE run_id = %s",
                (run_id,),
            ).fetchone()[0]
        )
        self.connection.execute(
            "INSERT INTO parity_run_error "
            "(run_id, error_ordinal, batch_id, scope, external_id, error_kind, message) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s)",
            (run_id, ordinal, batch_id, scope, external_id, error_kind, message),
        )
        return ordinal

    def _validate_execution(self, record: ExecutionRecord) -> None:
        if record.state not in {
            "completed",
            "not_run",
            "not_applicable",
            "infrastructure_error",
        }:
            raise ValueError(f"invalid execution state: {record.state}")
        if record.duration_ns is not None and record.duration_ns < 0:
            raise ValueError("execution duration cannot be negative")
        if len(record.identity_digest) != 32 or len(record.specification_digest) != 32:
            raise ValueError("execution identities must be SHA-256 digests")
        if len(record.case_digest) != 32:
            raise ValueError("case identity must be a SHA-256 digest")
        for item in record.observations:
            _validate_observation(item)

    def _validate_comparison(self, record: ComparisonRecord) -> None:
        if record.outcome not in {
            "pass",
            "fail",
            "not_run",
            "not_applicable",
            "infrastructure_error",
        }:
            raise ValueError(f"invalid comparison outcome: {record.outcome}")
        for digest in (
            record.target_identity_digest,
            record.comparator_identity_digest,
            record.specification_digest,
        ):
            if len(digest) != 32:
                raise ValueError("comparison identities must be SHA-256 digests")


def _validate_observation(item: ObservationRecord) -> None:
    if item.state == "ok":
        invalid = any(
            value is not None
            for value in (
                item.error_class,
                item.error_kind,
                item.error_message,
                item.error_stage,
                item.error_code,
                item.reason,
            )
        )
    elif item.state == "error":
        invalid = (
            item.error_class is None
            or item.error_kind is None
            or item.error_message is None
            or item.error_stage is None
            or item.reason is not None
        )
    elif item.state == "not_run":
        invalid = (
            item.reason is None
            or any(
                value is not None
                for value in (
                    item.error_class,
                    item.error_kind,
                    item.error_message,
                    item.error_stage,
                    item.error_code,
                )
            )
        )
    else:
        raise ValueError(f"invalid observation state: {item.state}")
    if invalid:
        raise ValueError(f"observation {item.step_id!r} has an invalid typed state")


def _execution_payload(record: ExecutionRecord) -> dict[str, Any]:
    return {
        "case_digest": record.case_digest,
        "identity_digest": record.identity_digest,
        "state": record.state,
        "reason": record.reason,
        "details": record.details,
        "observations": [
            {
                "step_id": item.step_id,
                "state": item.state,
                "value": item.value if item.state == "ok" else None,
                "error_class": item.error_class,
                "error_kind": item.error_kind,
                "error_message": item.error_message,
                "error_stage": item.error_stage,
                "error_code": item.error_code,
                "reason": item.reason,
            }
            for item in record.observations
        ],
    }


def _comparison_payload(record: ComparisonRecord) -> dict[str, Any]:
    return {
        "run_id": record.run_id,
        "target_identity_digest": record.target_identity_digest,
        "comparator_identity_digest": record.comparator_identity_digest,
        "project_id": record.project_id,
        "specification_digest": record.specification_digest,
        "case_id": record.case_id,
        "source_execution_id": record.source_execution_id,
        "target_execution_id": record.target_execution_id,
        "outcome": record.outcome,
        "differences": [
            {
                "step_id": item.step_id,
                "value_path": item.value_path,
                "kind": item.kind,
                "message": item.message,
                "source_value_present": item.source_value_present,
                "source_value": item.source_value,
                "target_value_present": item.target_value_present,
                "target_value": item.target_value,
            }
            for item in record.differences
        ],
    }
