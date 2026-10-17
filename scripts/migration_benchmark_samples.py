"""Optional parent-side retention for benchmark timing samples.

Only timing arrays returned after a worker has exited are retained here. This
module never imports image implementations or participates in a timed call.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any


SCHEMA = "migration-parity/benchmark-raw-samples@1"


class RawSampleCaptureError(RuntimeError):
    """Raised when an explicitly requested sample artifact cannot be trusted."""


def _require_nonempty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise RawSampleCaptureError(f"invalid raw-sample {field}")
    return value


def _integer_samples(value: Any, field: str) -> list[int]:
    if not isinstance(value, list):
        raise RawSampleCaptureError(f"invalid raw-sample {field}")
    samples: list[int] = []
    for sample in value:
        if type(sample) is not int or sample < 0:
            raise RawSampleCaptureError(f"invalid raw-sample {field} value")
        samples.append(sample)
    return samples


def _phase_samples(value: Any) -> list[dict[str, int]]:
    if not isinstance(value, list):
        raise RawSampleCaptureError("invalid raw-sample phase telemetry")
    phases: list[dict[str, int]] = []
    for phase in value:
        if not isinstance(phase, dict):
            raise RawSampleCaptureError("invalid raw-sample phase telemetry row")
        normalized: dict[str, int] = {}
        for name, duration in phase.items():
            if (
                not isinstance(name, str)
                or not name
                or type(duration) is not int
                or duration < 0
            ):
                raise RawSampleCaptureError("invalid raw-sample phase duration")
            normalized[name] = duration
        phases.append(normalized)
    return phases


def _repeat_for(policy: dict[str, Any]) -> int:
    try:
        warmup = policy["warmup_iterations"]
        iterations = policy["measurement_iterations"]
        samples = policy["samples"]
    except (KeyError, TypeError) as exc:
        raise RawSampleCaptureError("report has an invalid measurement policy") from exc
    if any(type(value) is not int or value < 0 for value in (warmup, iterations, samples)):
        raise RawSampleCaptureError("report has invalid measurement repeat counts")
    repeat = warmup + iterations * samples
    if repeat <= 0:
        raise RawSampleCaptureError("report measurement policy is empty")
    return repeat


def _json_bytes(document: dict[str, Any]) -> bytes:
    try:
        return (json.dumps(document, sort_keys=True, indent=2, allow_nan=False) + "\n").encode(
            "utf-8"
        )
    except (TypeError, ValueError) as exc:
        raise RawSampleCaptureError(f"raw-sample document is not JSON-safe: {exc}") from exc


def _publish_exclusive(path: Path, payload: bytes) -> None:
    """Create and fsync a new artifact without replacing an existing file.

    A failed write may leave a partial artifact at ``path``. It is preserved,
    and the caller attaches no reference unless this function completes.
    """

    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        raise FileExistsError(f"raw-sample artifact already exists: {path}") from None
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())


class RawSampleCollector:
    """Collect validated timed-child arrays and publish one bound sidecar."""

    def __init__(self, output: Path):
        self.output = Path(output).resolve()
        if self.output.exists():
            raise FileExistsError(f"raw-sample artifact already exists: {self.output}")
        self._records: list[dict[str, Any]] = []
        self._next_child_sequence = 0
        self._published = False

    def record_timed_child(
        self,
        *,
        group_id: str,
        side: str,
        profile: str,
        cases: list[dict[str, Any]],
        workload_ids_by_case: dict[str, list[str]],
        planned_repeat: int,
        child_repeat: int,
        lifecycle: str,
        boundary: str,
        step_ids: list[str],
        timings_ns: dict[str, Any],
        telemetry: dict[str, Any],
        child_invocation_id: str | None,
    ) -> None:
        """Retain one already-decoded and identity-checked timed child result."""

        if self._published:
            raise RawSampleCaptureError("raw-sample artifact is already published")
        group_id = _require_nonempty_string(group_id, "group ID")
        profile = _require_nonempty_string(profile, "profile")
        if side not in {"source", "target"}:
            raise RawSampleCaptureError("invalid raw-sample side")
        if lifecycle not in {"warm", "cold", "resident"}:
            raise RawSampleCaptureError("invalid raw-sample lifecycle")
        if boundary not in {"observed_steps", "whole_workflow"}:
            raise RawSampleCaptureError("invalid raw-sample timing boundary")
        if (
            type(planned_repeat) is not int
            or planned_repeat <= 0
            or type(child_repeat) is not int
            or child_repeat <= 0
        ):
            raise RawSampleCaptureError("invalid raw-sample repeat count")
        if lifecycle == "cold" and child_repeat != 1:
            raise RawSampleCaptureError("cold raw-sample child must repeat once")
        if not isinstance(step_ids, list) or any(
            not isinstance(item, str) or not item for item in step_ids
        ):
            raise RawSampleCaptureError("invalid raw-sample timing step IDs")
        if not isinstance(cases, list) or not cases:
            raise RawSampleCaptureError("timed child has no raw-sample cases")
        if not isinstance(timings_ns, dict) or not isinstance(telemetry, dict):
            raise RawSampleCaptureError("timed child has invalid raw-sample arrays")
        if child_invocation_id is not None and (
            not isinstance(child_invocation_id, str) or not child_invocation_id
        ):
            raise RawSampleCaptureError("invalid child attestation invocation ID")

        case_ids = [
            _require_nonempty_string(case.get("case_id"), "case ID")
            for case in cases
            if isinstance(case, dict)
        ]
        if len(case_ids) != len(cases) or len(case_ids) != len(set(case_ids)):
            raise RawSampleCaptureError("timed child has invalid or duplicate case IDs")

        child_sequence = self._next_child_sequence
        self._next_child_sequence += 1
        subject_kind = "oracle" if side == "source" else "target_profile"
        for case_id in case_ids:
            if case_id not in timings_ns or case_id not in telemetry:
                raise RawSampleCaptureError(
                    f"timed child omitted raw-sample arrays: {case_id}"
                )
            raw_durations = timings_ns[case_id]
            raw_phases = telemetry[case_id]
            durations = _integer_samples(raw_durations, "timing array")
            phases = _phase_samples(raw_phases)
            if len(durations) != len(phases) or len(durations) != child_repeat:
                raise RawSampleCaptureError(
                    f"timed child sample counts differ from repeat: {case_id}"
                )
            workload_ids = workload_ids_by_case.get(case_id, [])
            if (
                not isinstance(workload_ids, list)
                or any(not isinstance(item, str) or not item for item in workload_ids)
                or len(workload_ids) != len(set(workload_ids))
            ):
                raise RawSampleCaptureError("invalid workload IDs for timed case")
            self._records.append(
                {
                    "group_id": group_id,
                    "child_sequence": child_sequence,
                    "child_invocation_id": child_invocation_id,
                    "side": side,
                    "subject_kind": subject_kind,
                    "profile": profile,
                    "case_id": case_id,
                    "workload_ids": sorted(workload_ids),
                    "boundary": boundary,
                    "lifecycle": lifecycle,
                    "step_ids": list(step_ids),
                    "planned_repeat": planned_repeat,
                    "child_repeat": child_repeat,
                    "timings_ns": durations,
                    "phase_telemetry_ns": phases,
                }
            )

    def publish(
        self,
        *,
        report: dict[str, Any],
        report_path: Path,
        workload_case_ids: dict[str, str],
    ) -> str:
        """Publish the sidecar and bind completed report measurements to it.

        The reference is a relative path followed by the sidecar SHA-256. The
        report's run ID and input identities are included in the sidecar; its
        own final SHA cannot be included without creating a hash cycle.
        """

        if self._published:
            raise RawSampleCaptureError("raw-sample artifact is already published")
        report_path = Path(report_path).resolve()
        if self.output == report_path:
            raise RawSampleCaptureError("raw-sample and benchmark report paths collide")
        if not isinstance(report, dict) or not isinstance(report.get("identity"), dict):
            raise RawSampleCaptureError("benchmark report identity is missing")
        identity = report["identity"]
        run_id = _require_nonempty_string(identity.get("run_id"), "report run ID")
        report_schema = _require_nonempty_string(report.get("schema"), "report schema")
        if not isinstance(identity.get("inputs"), list) or not isinstance(
            identity.get("manifest"), dict
        ):
            raise RawSampleCaptureError("benchmark report input identity is invalid")
        if not isinstance(workload_case_ids, dict):
            raise RawSampleCaptureError("workload-to-case map is invalid")

        workloads = report.get("workloads")
        if not isinstance(workloads, list):
            raise RawSampleCaptureError("benchmark report workloads are missing")
        report_workloads: dict[str, dict[str, Any]] = {}
        for workload in workloads:
            if not isinstance(workload, dict):
                raise RawSampleCaptureError("invalid benchmark workload row")
            workload_id = _require_nonempty_string(
                workload.get("workload_id"), "workload ID"
            )
            if workload_id in report_workloads:
                raise RawSampleCaptureError("duplicate workload ID in benchmark report")
            report_workloads[workload_id] = workload
        if set(workload_case_ids) != set(report_workloads):
            raise RawSampleCaptureError("workload-to-case map does not match report")

        # Validate each captured record against the exact report workload
        # policy before deriving warm-up and measured-sample labels.
        normalized_records: list[dict[str, Any]] = []
        counts_by_group_case: dict[tuple[str, str], int] = {}
        groups: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
        for record in self._records:
            case_id = record["case_id"]
            if not record["workload_ids"]:
                raise RawSampleCaptureError(
                    f"timed case has no workload binding: {case_id}"
                )
            policies: list[dict[str, Any]] = []
            for workload_id in record["workload_ids"]:
                workload = report_workloads.get(workload_id)
                if workload is None or workload_case_ids.get(workload_id) != case_id:
                    raise RawSampleCaptureError(
                        f"timed case/workload binding mismatch: {workload_id}"
                    )
                policy = workload.get("measurement_policy")
                if not isinstance(policy, dict):
                    raise RawSampleCaptureError(
                        f"workload policy is missing: {workload_id}"
                    )
                policies.append(policy)
            policy_bytes = {
                _json_bytes(policy) for policy in policies
            }
            if len(policy_bytes) != 1:
                raise RawSampleCaptureError(
                    "one timed case group has multiple measurement policies"
                )
            policy = policies[0]
            expected_repeat = _repeat_for(policy)
            if (
                record["planned_repeat"] != expected_repeat
                or record["boundary"] != policy.get("boundary")
                or record["lifecycle"] != policy.get("cache_state")
                or record["step_ids"] != policy.get("step_ids")
            ):
                raise RawSampleCaptureError(
                    f"timed sample context differs from workload policy: {case_id}"
                )
            if len(record["timings_ns"]) != len(record["phase_telemetry_ns"]):
                raise RawSampleCaptureError(
                    f"timing and phase sample counts differ: {case_id}"
                )
            if record["child_repeat"] != len(record["timings_ns"]):
                raise RawSampleCaptureError(
                    f"timing sample count differs from child repeat: {case_id}"
                )

            group_case = (record["group_id"], case_id)
            ordinal = counts_by_group_case.get(group_case, 0)
            warmup = int(policy["warmup_iterations"])
            samples_per_iteration = int(policy["samples"])
            sample_labels = []
            for offset in range(len(record["timings_ns"])):
                sample_ordinal = ordinal + offset
                if sample_ordinal < warmup:
                    sample_labels.append(
                        {
                            "ordinal": sample_ordinal,
                            "role": "warmup",
                            "measurement_iteration": None,
                            "sample_in_iteration": None,
                        }
                    )
                else:
                    measured_ordinal = sample_ordinal - warmup
                    sample_labels.append(
                        {
                            "ordinal": sample_ordinal,
                            "role": "measurement",
                            "measurement_iteration": (
                                measured_ordinal // samples_per_iteration + 1
                                if samples_per_iteration
                                else None
                            ),
                            "sample_in_iteration": (
                                measured_ordinal % samples_per_iteration + 1
                                if samples_per_iteration
                                else None
                            ),
                        }
                    )
            counts_by_group_case[group_case] = ordinal + len(record["timings_ns"])
            normalized = {
                **record,
                "sample_labels": sample_labels,
                "capture_errors": [],
                "throughput_operations_per_second": [
                    (1_000_000_000 / duration if duration > 0 else None)
                    for duration in record["timings_ns"]
                ],
            }
            normalized_records.append(normalized)
            groups.setdefault(
                (
                    record["group_id"],
                    case_id,
                    record["subject_kind"],
                    record["profile"],
                ),
                [],
            ).append(normalized)

        relative_path = os.path.relpath(self.output, report_path.parent).replace(
            os.sep, "/"
        )
        document = {
            "schema": SCHEMA,
            "report": {
                "path": str(report_path),
                "schema": report_schema,
                "run_id": run_id,
                "manifest": identity["manifest"],
                "inputs": identity["inputs"],
            },
            "workloads": [
                {
                    "workload_id": workload_id,
                    "case_id": workload_case_ids[workload_id],
                    "measurement_policy": report_workloads[workload_id][
                        "measurement_policy"
                    ],
                }
                for workload_id in sorted(report_workloads)
            ],
            "records": normalized_records,
        }
        payload = _json_bytes(document)
        digest = hashlib.sha256(payload).hexdigest()
        reference = f"{relative_path}#sha256={digest}"

        # Completed measurement rows must be represented exactly. Incomplete
        # rows retain their existing null references and never receive an
        # inferred or partial reference.
        pending_measurements: list[dict[str, Any]] = []
        for workload_id, workload in report_workloads.items():
            case_id = workload_case_ids[workload_id]
            policy = workload["measurement_policy"]
            expected_repeat = _repeat_for(policy)
            expected_measured = expected_repeat - int(policy["warmup_iterations"])
            expected_children = expected_repeat if policy["cache_state"] == "cold" else 1
            for subject in workload.get("subjects", []):
                if not isinstance(subject, dict):
                    continue
                if subject.get("status") != "completed" or not subject.get(
                    "measurements"
                ):
                    continue
                subject_kind = subject.get("kind")
                subject_id = subject.get("id")
                matching = [
                    records
                    for (group_id, grouped_case, grouped_kind, profile), records in groups.items()
                    if grouped_case == case_id
                    and grouped_kind == subject_kind
                    and profile == subject_id
                    and any(
                        workload_id in record["workload_ids"] for record in records
                    )
                ]
                if len(matching) != 1:
                    raise RawSampleCaptureError(
                        f"completed subject has no unique sample group: {workload_id}/{subject_id}"
                    )
                child_records = sorted(matching[0], key=lambda record: record["child_sequence"])
                if len(child_records) != expected_children:
                    raise RawSampleCaptureError(
                        f"sample child count differs from policy: {workload_id}/{subject_id}"
                    )
                if policy["cache_state"] == "cold":
                    if any(record["child_repeat"] != 1 for record in child_records):
                        raise RawSampleCaptureError("cold sample child repeat is not one")
                elif (
                    len(child_records) != 1
                    or child_records[0]["child_repeat"] != expected_repeat
                ):
                    raise RawSampleCaptureError(
                        f"warm sample repeat differs from policy: {workload_id}/{subject_id}"
                    )
                measured_durations = [
                    duration
                    for record in child_records
                    for duration, label in zip(
                        record["timings_ns"], record["sample_labels"]
                    )
                    if label["role"] == "measurement"
                ]
                if len(measured_durations) != expected_measured:
                    raise RawSampleCaptureError(
                        f"measured sample count differs from policy: {workload_id}/{subject_id}"
                    )
                for measurement in subject["measurements"]:
                    metric = measurement.get("metric")
                    if metric == "latency":
                        expected_count = expected_measured
                    elif metric == "throughput":
                        expected_count = sum(value > 0 for value in measured_durations)
                    else:
                        raise RawSampleCaptureError(
                            f"unsupported raw-sample metric: {metric}"
                        )
                    if measurement.get("sample_count") != expected_count:
                        raise RawSampleCaptureError(
                            f"measurement sample count differs from retained data: {workload_id}/{subject_id}/{metric}"
                        )
                    pending_measurements.append(measurement)

        _publish_exclusive(self.output, payload)
        for measurement in pending_measurements:
            measurement["raw_samples_ref"] = reference
        self._published = True
        return reference
