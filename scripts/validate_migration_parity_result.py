#!/usr/bin/env python3
"""Validate generated migration-parity lane artifacts with exact key sets."""

from __future__ import annotations

import argparse
import gzip
import json
import re
from pathlib import Path
from typing import Any

try:
    from migration_parity.fault_contracts import FAULT_CONTRACTS
except ModuleNotFoundError:
    from scripts.migration_parity.fault_contracts import FAULT_CONTRACTS


def exact(value: Any, keys: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        actual = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise ValueError(f"{label}: expected keys {sorted(keys)}, got {actual}")


def non_negative_int(value: Any, label: str) -> None:
    if type(value) is not int or value < 0:
        raise ValueError(f"{label}: expected non-negative integer")


def string(value: Any, label: str, *, nullable: bool = False, allow_empty: bool = False) -> None:
    if nullable and value is None:
        return
    if not isinstance(value, str) or (not value and not allow_empty):
        raise ValueError(f"{label}: expected non-empty string")


def unique(values: list[Any], label: str) -> None:
    try:
        if len(values) != len(set(values)):
            raise ValueError(f"{label}: duplicate ID")
    except TypeError as exc:
        raise ValueError(f"{label}: IDs must be scalar values") from exc


def id_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        raise ValueError(f"{label}: expected string array")
    result = []
    for index, item in enumerate(value):
        string(item, f"{label}[{index}]")
        result.append(item)
    unique(result, label)
    return result


def identity(value: dict[str, Any]) -> None:
    exact(value, {"run_id", "started_at", "finished_at", "manifest", "inputs", "assets", "oracles", "targets", "command"}, "identity")
    string(value["run_id"], "identity.run_id")
    string(value["started_at"], "identity.started_at")
    string(value["finished_at"], "identity.finished_at")
    exact(value["manifest"], {"path", "schema", "sha256"}, "identity.manifest")
    string(value["manifest"]["path"], "identity.manifest.path")
    string(value["manifest"]["schema"], "identity.manifest.schema")
    if not re.fullmatch(r"[0-9a-f]{64}", value["manifest"]["sha256"]):
        raise ValueError("identity.manifest.sha256: expected lowercase sha256")
    input_paths: list[str] = []
    for index, item in enumerate(value["inputs"]):
        exact(item, {"path", "schema", "sha256"}, f"identity.inputs[{index}]")
        string(item["path"], f"identity.inputs[{index}].path")
        string(item["schema"], f"identity.inputs[{index}].schema")
        if not re.fullmatch(r"[0-9a-f]{64}", item["sha256"]):
            raise ValueError(f"identity.inputs[{index}].sha256: expected lowercase sha256")
        input_paths.append(item["path"])
    unique(input_paths, "identity.inputs")
    asset_keys: list[tuple[Any, ...]] = []
    for index, item in enumerate(value["assets"]):
        exact(item, {"input_path", "item_id", "asset_id", "kind", "locator", "sha256"}, f"identity.assets[{index}]")
        for field in ("input_path", "item_id", "asset_id", "kind"):
            string(item[field], f"identity.assets[{index}].{field}")
        string(item["locator"], f"identity.assets[{index}].locator", nullable=True)
        if item["sha256"] is not None and not re.fullmatch(r"[0-9a-f]{64}", item["sha256"]):
            raise ValueError(f"identity.assets[{index}].sha256: expected lowercase sha256 or null")
        asset_keys.append((item["input_path"], item["item_id"], item["asset_id"]))
    unique(asset_keys, "identity.assets")
    oracle_ids: list[str] = []
    for index, item in enumerate(value["oracles"]):
        exact(item, {"oracle_id", "name", "version", "runtime"}, f"identity.oracles[{index}]")
        for field in ("oracle_id", "name", "version", "runtime"):
            string(item[field], f"identity.oracles[{index}].{field}")
        oracle_ids.append(item["oracle_id"])
    unique(oracle_ids, "identity.oracles")
    target_keys: list[tuple[str, str]] = []
    for index, item in enumerate(value["targets"]):
        exact(item, {"target_profile", "target_id", "revision", "dirty", "runtime", "backend", "features"}, f"identity.targets[{index}]")
        for field in ("target_profile", "target_id", "revision", "runtime", "backend"):
            string(item[field], f"identity.targets[{index}].{field}")
        if not isinstance(item["dirty"], bool):
            raise ValueError(f"identity.targets[{index}].dirty: expected boolean")
        if not isinstance(item["features"], list) or not all(isinstance(feature, str) and feature for feature in item["features"]):
            raise ValueError(f"identity.targets[{index}].features: expected string array")
        target_keys.append((item["target_profile"], item["target_id"]))
    unique(target_keys, "identity.targets")
    exact(value["command"], {"command_id", "argv", "cwd", "timeout_seconds"}, "identity.command")
    string(value["command"]["command_id"], "identity.command.command_id")
    if not isinstance(value["command"]["argv"], list) or not value["command"]["argv"] or not all(isinstance(arg, str) and arg for arg in value["command"]["argv"]):
        raise ValueError("identity.command.argv: expected non-empty string array")
    string(value["command"]["cwd"], "identity.command.cwd")
    if type(value["command"]["timeout_seconds"]) is not int or value["command"]["timeout_seconds"] <= 0:
        raise ValueError("identity.command.timeout_seconds: expected positive integer")


def infrastructure_errors(value: list[dict[str, Any]]) -> None:
    if not isinstance(value, list):
        raise ValueError("infrastructure_errors: expected array")
    for index, item in enumerate(value):
        exact(item, {"scope", "id", "kind", "message"}, f"infrastructure_errors[{index}]")
        if item["scope"] not in {"oracle", "target", "collector", "runner", "artifact", "aggregation"}:
            raise ValueError(f"infrastructure_errors[{index}].scope: invalid scope")
        string(item["id"], f"infrastructure_errors[{index}].id", nullable=True)
        string(item["kind"], f"infrastructure_errors[{index}].kind")
        string(item["message"], f"infrastructure_errors[{index}].message")


def execution_receipt(
    value: dict[str, Any],
    label: str,
    *,
    allowed_step_ids: set[str] | None = None,
) -> None:
    required = {
        "status",
        "requested_backend",
        "actual_backend",
        "actual_backend_counts",
        "fallback_reason_counts",
        "operation_count",
        "dispatch_count",
        "resize_coeff_cache_hits",
        "resize_coeff_cache_misses",
        "phase_timings_ns",
        "resource",
        "sample_count",
        "cached_sample_count",
    }
    optional = {
        "terminal_complete",
        "errors",
        "operation_telemetry",
        "selected_step_ids",
    }
    if (
        not isinstance(value, dict)
        or not required <= set(value)
        or not set(value) <= required | optional
    ):
        raise ValueError(
            f"{label}: expected execution receipt keys {sorted(required)}"
            " with optional terminal_complete, errors, operation_telemetry, "
            "and selected_step_ids"
        )
    selected_step_ids = (
        set(id_array(value["selected_step_ids"], f"{label}.selected_step_ids"))
        if "selected_step_ids" in value
        else None
    )
    if (
        selected_step_ids is not None
        and allowed_step_ids is not None
        and not selected_step_ids <= allowed_step_ids
    ):
        raise ValueError(
            f"{label}.selected_step_ids: outside workload measurement_policy.step_ids"
        )
    explicit_terminal_state = "terminal_complete" in value
    if explicit_terminal_state and type(value["terminal_complete"]) is not bool:
        raise ValueError(f"{label}.terminal_complete: expected boolean")
    if value["status"] not in {
        "completed",
        "partial",
        "unsupported",
        "not_proven",
        "not_applicable",
    }:
        raise ValueError(f"{label}.status: invalid status")
    string(value["requested_backend"], f"{label}.requested_backend")
    string(value["actual_backend"], f"{label}.actual_backend", nullable=True)
    for field in ("actual_backend_counts", "fallback_reason_counts"):
        counts = value[field]
        if not isinstance(counts, dict):
            raise ValueError(f"{label}.{field}: expected object")
        for key, count in counts.items():
            string(key, f"{label}.{field} key")
            if type(count) is not int or count <= 0:
                raise ValueError(f"{label}.{field}.{key}: expected positive integer")

    def summary(summary_value: dict[str, Any], summary_label: str) -> None:
        exact(
            summary_value,
            {"sample_count", "min", "median", "mean", "max", "total"},
            summary_label,
        )
        non_negative_int(summary_value["sample_count"], f"{summary_label}.sample_count")
        for field in ("min", "median", "mean", "max", "total"):
            number = summary_value[field]
            if number is not None and (isinstance(number, bool) or not isinstance(number, (int, float))):
                raise ValueError(f"{summary_label}.{field}: expected number or null")
            if number is not None and number < 0:
                raise ValueError(f"{summary_label}.{field}: expected non-negative number")

    summary(value["operation_count"], f"{label}.operation_count")
    summary(value["dispatch_count"], f"{label}.dispatch_count")
    summary(
        value["resize_coeff_cache_hits"],
        f"{label}.resize_coeff_cache_hits",
    )
    summary(
        value["resize_coeff_cache_misses"],
        f"{label}.resize_coeff_cache_misses",
    )
    exact(value["phase_timings_ns"], {"route_ns", "validation_ns", "backend_ns"}, f"{label}.phase_timings_ns")
    for phase, phase_summary in value["phase_timings_ns"].items():
        summary(phase_summary, f"{label}.phase_timings_ns.{phase}")
    legacy_resource_fields = {
        "sample_count",
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
    }
    checked_allocation_fields = {"host_allocation_count", "host_allocated_bytes"}
    resource_keys = set(value["resource"]) if isinstance(value["resource"], dict) else set()
    if checked_allocation_fields.intersection(resource_keys):
        resource_fields = legacy_resource_fields | checked_allocation_fields
    else:
        # Receipts written before checked host-allocation counters were exposed
        # remain readable; new benchmark runs always include both fields.
        resource_fields = legacy_resource_fields
    exact(value["resource"], resource_fields, f"{label}.resource")
    non_negative_int(value["resource"]["sample_count"], f"{label}.resource.sample_count")
    for field in (
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
        *(sorted(checked_allocation_fields) if checked_allocation_fields <= resource_keys else ()),
    ):
        summary(value["resource"][field], f"{label}.resource.{field}")
    non_negative_int(value["sample_count"], f"{label}.sample_count")
    non_negative_int(value["cached_sample_count"], f"{label}.cached_sample_count")
    if "operation_telemetry" in value:
        telemetry = value["operation_telemetry"]
        if not isinstance(telemetry, list):
            raise ValueError(f"{label}.operation_telemetry: expected array")
        for index, item in enumerate(telemetry):
            telemetry_label = f"{label}.operation_telemetry[{index}]"
            telemetry_keys = {
                "operation",
                "path",
                "vector_block_count",
                "scalar_tail_count",
                "mode_conversion_count",
                "handoff_count",
            }
            has_source = isinstance(item, dict) and "source" in item
            if selected_step_ids is None:
                exact(
                    item,
                    telemetry_keys | ({"source"} if has_source else set()),
                    telemetry_label,
                )
            else:
                exact(
                    item,
                    telemetry_keys
                    | {"step_id"}
                    | ({"source"} if has_source else set()),
                    telemetry_label,
                )
                string(item["step_id"], f"{telemetry_label}.step_id")
                if item["step_id"] not in selected_step_ids:
                    raise ValueError(
                        f"{telemetry_label}.step_id: not in selected_step_ids"
                    )
            if has_source and item["source"] is not None:
                source_label = f"{telemetry_label}.source"
                exact(
                    item["source"],
                    {"step_id", "surface", "operation"},
                    source_label,
                )
                for field in ("step_id", "surface", "operation"):
                    string(item["source"][field], f"{source_label}.{field}")
            string(item["operation"], f"{telemetry_label}.operation")
            string(item["path"], f"{telemetry_label}.path")
            for field in (
                "vector_block_count",
                "scalar_tail_count",
                "mode_conversion_count",
                "handoff_count",
            ):
                non_negative_int(item[field], f"{telemetry_label}.{field}")
    if "errors" in value:
        if not isinstance(value["errors"], list):
            raise ValueError(f"{label}.errors: expected array")
        for index, item in enumerate(value["errors"]):
            error_label = f"{label}.errors[{index}]"
            exact(item, {"step_id", "error"}, error_label)
            string(item["step_id"], f"{error_label}.step_id", nullable=True)
            exact(
                item["error"],
                {"class", "kind", "message", "stage", "code"},
                f"{error_label}.error",
            )
            for field in ("class", "kind", "message", "stage"):
                string(
                    item["error"][field],
                    f"{error_label}.error.{field}",
                    allow_empty=field == "message",
                )
            string(item["error"]["code"], f"{error_label}.error.code", nullable=True)
    if explicit_terminal_state:
        terminal_complete = value["terminal_complete"]
        if terminal_complete and value["status"] != "completed":
            raise ValueError(
                f"{label}: terminal_complete=true requires status=completed"
            )
        if value["status"] == "completed" and not terminal_complete:
            raise ValueError(
                f"{label}: status=completed requires terminal_complete=true"
            )
        if terminal_complete and value.get("errors"):
            raise ValueError(
                f"{label}: terminal_complete=true cannot carry execution errors"
            )


def workflow(value: dict[str, Any], label: str) -> None:
    required = {"case_id", "status", "observations"}
    optional = {"reason", "fault_contract_observation"}
    if not isinstance(value, dict) or not required <= set(value) or not set(value) <= required | optional:
        raise ValueError(f"{label}: invalid workflow fields")
    string(value["case_id"], f"{label}.case_id")
    if value["status"] not in {"completed", "not_run", "not_applicable"}:
        raise ValueError(f"{label}.status: invalid workflow status")
    if value["status"] == "not_applicable":
        if "reason" not in value:
            raise ValueError(f"{label}.reason: missing target-only reason")
        string(value["reason"], f"{label}.reason")
        if value["observations"]:
            raise ValueError(f"{label}: not-applicable workflow cannot contain observations")
    elif "reason" in value:
        raise ValueError(f"{label}.reason: only not-applicable workflows carry a reason")
    if not isinstance(value["observations"], list):
        raise ValueError(f"{label}.observations: expected array")
    observation_ids: list[str] = []
    for index, observation in enumerate(value["observations"]):
        prefix = f"{label}.observations[{index}]"
        if not isinstance(observation, dict) or observation.get("status") not in {"ok", "error", "not_run"}:
            raise ValueError(f"{prefix}.status: invalid observation status")
        exact(observation, {"step_id", "status", "value"} if observation["status"] == "ok" else {"step_id", "status", "error"} if observation["status"] == "error" else {"step_id", "status", "reason"}, prefix)
        string(observation["step_id"], f"{prefix}.step_id")
        observation_ids.append(observation["step_id"])
        if observation["status"] == "ok":
            if "value" not in observation:
                raise ValueError(f"{prefix}.value: missing value")
        if observation["status"] == "error":
            exact(observation["error"], {"class", "kind", "message", "stage", "code"}, f"{prefix}.error")
            for field in ("class", "kind", "message", "stage"):
                string(observation["error"][field], f"{prefix}.error.{field}", allow_empty=field == "message")
            string(observation["error"]["code"], f"{prefix}.error.code", nullable=True)
        if observation["status"] == "not_run":
            string(observation["reason"], f"{prefix}.reason")
    unique(observation_ids, f"{label}.observations")
    if "fault_contract_observation" in value:
        observation = value["fault_contract_observation"]
        if observation is None:
            return
        exact(
            observation,
            {"contract_id", "step_id", "observed"},
            f"{label}.fault_contract_observation",
        )
        string(observation["contract_id"], f"{label}.fault_contract_observation.contract_id")
        if observation["contract_id"] not in FAULT_CONTRACTS:
            raise ValueError(f"{label}.fault_contract_observation: unknown contract")
        string(observation["step_id"], f"{label}.fault_contract_observation.step_id")
        contract = FAULT_CONTRACTS[observation["contract_id"]]
        if observation["step_id"] != contract["step_id"]:
            raise ValueError(f"{label}.fault_contract_observation.step_id: wrong public step")
        exact(
            observation["observed"],
            set(contract["observed_fields"]),
            f"{label}.fault_contract_observation.observed",
        )


def execution_evidence_summary(value: dict[str, Any], label: str) -> None:
    """Validate sidecar receipt counts, accepting the pre-bit summary shape."""

    legacy = {
        "selected",
        "receipt_cases",
        "not_recorded_cases",
        "completed_receipts",
        "actual_backend_counts",
        "fallback_reason_counts",
    }
    current = legacy | {"terminal_complete_receipts", "terminal_incomplete_cases"}
    versioned = current | {
        "pipeline_applicable_cases",
        "pipeline_complete_cases",
        "pipeline_missing_receipt_cases",
        "pipeline_partial_receipt_cases",
        "pipeline_not_applicable_cases",
        "pipeline_indeterminate_cases",
    }
    if not isinstance(value, dict) or set(value) not in (legacy, current, versioned):
        raise ValueError(
            f"{label}: expected legacy, terminal-complete, or partitioned summary keys"
        )
    for field in (
        "selected",
        "receipt_cases",
        "not_recorded_cases",
        "completed_receipts",
    ):
        non_negative_int(value[field], f"{label}.{field}")
    for field in ("actual_backend_counts", "fallback_reason_counts"):
        counts = value[field]
        if not isinstance(counts, dict):
            raise ValueError(f"{label}.{field}: expected object")
        for key, count in counts.items():
            string(key, f"{label}.{field} key")
            non_negative_int(count, f"{label}.{field}[{key}]")
    if (
        value["receipt_cases"] + value["not_recorded_cases"]
        != value["selected"]
    ):
        raise ValueError(f"{label}: receipt case counts are inconsistent")
    if set(value) in (current, versioned):
        non_negative_int(
            value["terminal_complete_receipts"],
            f"{label}.terminal_complete_receipts",
        )
        non_negative_int(
            value["terminal_incomplete_cases"],
            f"{label}.terminal_incomplete_cases",
        )
        if value["terminal_complete_receipts"] > value["completed_receipts"]:
            raise ValueError(
                f"{label}: terminal receipts cannot exceed completed receipts"
            )
        if value["terminal_incomplete_cases"] > value["receipt_cases"]:
            raise ValueError(
                f"{label}: incomplete cases cannot exceed receipt cases"
            )
        backend_denominator = value["terminal_complete_receipts"]
    else:
        backend_denominator = value["completed_receipts"]
    if set(value) == versioned:
        for field in versioned - current:
            non_negative_int(value[field], f"{label}.{field}")
        if (
            value["pipeline_applicable_cases"]
            != value["pipeline_complete_cases"]
            + value["pipeline_missing_receipt_cases"]
            + value["pipeline_partial_receipt_cases"]
        ):
            raise ValueError(f"{label}: pipeline-applicable case counts are inconsistent")
        if (
            value["pipeline_applicable_cases"]
            + value["pipeline_not_applicable_cases"]
            + value["pipeline_indeterminate_cases"]
            != value["selected"]
        ):
            raise ValueError(f"{label}: pipeline case partition does not match selected")
        if value["pipeline_partial_receipt_cases"] != value[
            "terminal_incomplete_cases"
        ]:
            raise ValueError(f"{label}: partial receipt count is inconsistent")
        if value["pipeline_complete_cases"] > value["terminal_complete_receipts"]:
            raise ValueError(f"{label}: complete case count exceeds terminal receipts")
    if sum(value["actual_backend_counts"].values()) != backend_denominator:
        raise ValueError(f"{label}: backend counts are inconsistent")


def parity(result: dict[str, Any]) -> None:
    exact(result, {"schema", "identity", "status", "summary", "comparisons", "infrastructure_errors"}, "parity")
    schema = result["schema"]
    if schema not in {"migration-parity/parity-result@1", "migration-parity/parity-result@2"}:
        raise ValueError("parity.schema: unsupported schema")
    if result["status"] not in {"completed", "infrastructure_failed", "cancelled", "invalid"}:
        raise ValueError("parity.status: invalid artifact status")
    identity(result["identity"])
    legacy_fields = {"selected", "executed", "passed", "failed", "not_run", "infrastructure_errors"}
    v2_fields = legacy_fields | {"parity_cases", "fault_contracts"}
    exact(result["summary"], legacy_fields if schema.endswith("@1") else v2_fields, "parity.summary")
    for field in legacy_fields:
        non_negative_int(result["summary"][field], f"parity.summary.{field}")
    if schema.endswith("@2"):
        for label in ("parity_cases", "fault_contracts"):
            counts = result["summary"][label]
            exact(counts, {"selected", "executed", "passed", "failed", "not_run"}, f"parity.summary.{label}")
            for field, value in counts.items():
                non_negative_int(value, f"parity.summary.{label}.{field}")
            if counts["passed"] + counts["failed"] + counts["not_run"] != counts["selected"]:
                raise ValueError(f"parity.summary.{label}: selected cases are not partitioned")
            if counts["executed"] > counts["selected"]:
                raise ValueError(f"parity.summary.{label}: executed exceeds selected")
        for field in ("selected", "executed", "passed", "failed", "not_run"):
            if result["summary"][field] != sum(result["summary"][kind][field] for kind in ("parity_cases", "fault_contracts")):
                raise ValueError(f"parity.summary.{field}: verification partitions disagree")
    if result["summary"]["executed"] != len(result["comparisons"]):
        raise ValueError("parity.summary.executed does not equal comparison count")
    if schema.endswith("@1"):
        if result["status"] == "completed":
            if result["summary"]["selected"] != result["summary"]["executed"]:
                raise ValueError("completed parity must execute every selected case")
            if result["summary"]["passed"] + result["summary"]["failed"] + result["summary"]["not_run"] != result["summary"]["executed"]:
                raise ValueError("parity.summary: executed must equal passed plus failed plus not_run")
        else:
            if result["summary"]["passed"] + result["summary"]["failed"] != result["summary"]["executed"]:
                raise ValueError("incomplete parity must classify every executed comparison")
            if result["summary"]["not_run"] != result["summary"]["selected"] - result["summary"]["executed"]:
                raise ValueError("incomplete parity must classify every unexecuted case as not_run")
    else:
        if result["summary"]["passed"] + result["summary"]["failed"] + result["summary"]["not_run"] != result["summary"]["selected"]:
            raise ValueError("parity.summary: selected cases are not partitioned")
        if result["status"] == "completed" and result["summary"]["selected"] != result["summary"]["executed"]:
            raise ValueError("completed parity must execute every selected case")
    if result["summary"]["infrastructure_errors"] != len(result["infrastructure_errors"]):
        raise ValueError("parity.summary.infrastructure_errors does not equal error count")
    infrastructure_errors(result["infrastructure_errors"])
    comparison_ids: list[tuple[str, str]] = []
    kind_counts = {"parity": 0, "fault_contract": 0}
    kind_outcomes = {
        kind: {"pass": 0, "fail": 0, "not_run": 0}
        for kind in kind_counts
    }
    for index, comparison in enumerate(result["comparisons"]):
        prefix = f"parity.comparisons[{index}]"
        base_fields = {"case_id", "target_profile", "requirements", "source", "target", "outcome", "diffs"}
        if schema.endswith("@2"):
            if not isinstance(comparison, dict) or comparison.get("verification") not in kind_counts:
                raise ValueError(f"{prefix}.verification: invalid verification kind")
            kind = comparison["verification"]
            exact(
                comparison,
                base_fields | {"verification", "fault_contract_id"}
                if kind == "fault_contract"
                else base_fields | {"verification"},
                prefix,
            )
            kind_counts[kind] += 1
            if kind == "fault_contract":
                string(comparison["fault_contract_id"], f"{prefix}.fault_contract_id")
                if comparison["fault_contract_id"] not in FAULT_CONTRACTS:
                    raise ValueError(f"{prefix}.fault_contract_id: unknown contract")
        else:
            exact(comparison, base_fields, prefix)
        string(comparison["case_id"], f"{prefix}.case_id")
        string(comparison["target_profile"], f"{prefix}.target_profile")
        requirements = id_array(comparison["requirements"], f"{prefix}.requirements")
        if comparison["outcome"] not in {"pass", "fail", "not_run"}:
            raise ValueError(f"{prefix}.outcome: invalid outcome")
        if schema.endswith("@2"):
            kind_outcomes[comparison["verification"]][comparison["outcome"]] += 1
        workflow(comparison["source"], f"{prefix}.source")
        workflow(comparison["target"], f"{prefix}.target")
        if schema.endswith("@2") and comparison["verification"] == "fault_contract":
            if comparison["source"]["status"] != "not_applicable":
                raise ValueError(f"{prefix}: target-only fault contract must mark oracle N/A")
            if "fault_contract_observation" not in comparison["target"]:
                raise ValueError(f"{prefix}: missing runner-owned fault observation")
        elif comparison["source"]["status"] == "not_applicable":
            raise ValueError(f"{prefix}: parity case cannot mark its oracle N/A")
        if comparison["source"]["case_id"] != comparison["case_id"] or comparison["target"]["case_id"] != comparison["case_id"]:
            raise ValueError(f"{prefix}: workflow case IDs do not match comparison")
        comparison_ids.append((comparison["case_id"], comparison["target_profile"]))
        if not isinstance(comparison["diffs"], list):
            raise ValueError(f"{prefix}.diffs: expected array")
        for diff_index, diff in enumerate(comparison["diffs"]):
            exact(diff, {"step_id", "path", "kind", "source", "target", "message"}, f"{prefix}.diffs[{diff_index}]")
            for field in ("step_id", "path", "kind", "message"):
                string(diff[field], f"{prefix}.diffs[{diff_index}].{field}")
    unique(comparison_ids, "parity.comparisons")
    if schema.endswith("@2"):
        for label, kind in (("parity_cases", "parity"), ("fault_contracts", "fault_contract")):
            counts = result["summary"][label]
            if counts["executed"] != kind_counts[kind]:
                raise ValueError(f"parity summary {label}.executed does not match rows")
            for outcome in ("pass", "fail"):
                if counts["passed" if outcome == "pass" else "failed"] != kind_outcomes[kind][outcome]:
                    raise ValueError(f"parity summary {label} {outcome} count does not match rows")
            expected_not_run = kind_outcomes[kind]["not_run"] + counts["selected"] - counts["executed"]
            if counts["not_run"] != expected_not_run:
                raise ValueError(f"parity summary {label}.not_run does not match rows and selection")


def coverage_backend_executions(executions: Any, plans: list[dict[str, Any]], summary: dict[str, int]) -> None:
    """Validate additive backend evidence without changing the unique-plan scope."""
    if not isinstance(executions, list) or not executions:
        raise ValueError("coverage.backend_executions: expected non-empty array")
    expected = {plan["plan_id"]: plan for plan in plans}
    totals = {plan_id: {"tests_passed": 0, "tests_failed": 0} for plan_id in expected}
    backends = []
    for index, item in enumerate(executions):
        prefix = f"coverage.backend_executions[{index}]"
        exact(item, {"backend", "run_id", "status", "summary", "plans", "infrastructure_errors"}, prefix)
        if item["backend"] not in {"cpu", "simd", "gpu"}:
            raise ValueError(f"{prefix}.backend: invalid backend")
        backends.append(item["backend"])
        string(item["run_id"], f"{prefix}.run_id")
        if item["status"] not in {"completed", "infrastructure_failed", "cancelled", "invalid", "not_ingested"}:
            raise ValueError(f"{prefix}.status: invalid status")
        infrastructure_errors(item["infrastructure_errors"])
        exact(item["summary"], {"plans_selected", "plans_executed", "plans_not_run", "tests_passed", "tests_failed"}, f"{prefix}.summary")
        for field, value in item["summary"].items():
            non_negative_int(value, f"{prefix}.summary.{field}")
        selected = []
        for plan in item["plans"]:
            exact(plan, {"plan_id", "selected", "execution"}, f"{prefix}.plans")
            plan_id = plan["plan_id"]
            selected.append(plan_id)
            if plan_id not in expected or plan["selected"] != expected[plan_id]["selected"]:
                raise ValueError(f"{prefix}: backend selected a different plan/case scope")
            exact(plan["execution"], {"status", "tests_passed", "tests_failed"}, f"{prefix}.execution")
            if plan["execution"]["status"] not in {"completed", "failed", "not_run"}:
                raise ValueError(f"{prefix}.execution.status: invalid status")
            for field in ("tests_passed", "tests_failed"):
                non_negative_int(plan["execution"][field], f"{prefix}.execution.{field}")
                totals[plan_id][field] += plan["execution"][field]
        unique(selected, f"{prefix}.plans")
        executed = sum(plan["execution"]["status"] != "not_run" for plan in item["plans"])
        if set(selected) != set(expected) or (
            item["summary"]["plans_selected"], item["summary"]["plans_executed"], item["summary"]["plans_not_run"]
        ) != (len(expected), executed, len(expected) - executed):
            raise ValueError(f"{prefix}: backend summary disagrees with plan scope")
        for field in ("tests_passed", "tests_failed"):
            if item["summary"][field] != sum(plan["execution"][field] for plan in item["plans"]):
                raise ValueError(f"{prefix}: backend summary disagrees with {field}")
    unique(backends, "coverage.backend_executions")
    for field in ("tests_passed", "tests_failed"):
        if summary[field] != sum(total[field] for total in totals.values()) or any(
            plan["execution"][field] != totals[plan["plan_id"]][field] for plan in plans
        ):
            raise ValueError(f"coverage: merged {field} disagrees with backend executions")


def coverage(result: dict[str, Any]) -> None:
    required = {"schema", "identity", "status", "collector", "summary", "plans", "infrastructure_errors"}
    exact(result, required | ({"backend_executions"} if "backend_executions" in result else set()), "coverage")
    if result["schema"] != "migration-parity/coverage-result@1":
        raise ValueError("coverage.schema: unsupported schema")
    if result["status"] not in {"completed", "infrastructure_failed", "cancelled", "invalid", "not_ingested"}:
        raise ValueError("coverage.status: invalid artifact status")
    identity(result["identity"])
    exact(result["collector"], {"name", "version", "snapshot_id", "artifact_ingested"}, "coverage.collector")
    string(result["collector"]["name"], "coverage.collector.name")
    string(result["collector"]["version"], "coverage.collector.version")
    string(result["collector"]["snapshot_id"], "coverage.collector.snapshot_id", nullable=True)
    if not isinstance(result["collector"]["artifact_ingested"], bool):
        raise ValueError("coverage.collector.artifact_ingested: expected boolean")
    exact(result["summary"], {"plans_selected", "plans_executed", "plans_not_run", "tests_passed", "tests_failed"}, "coverage.summary")
    for field in result["summary"]:
        non_negative_int(result["summary"][field], f"coverage.summary.{field}")
    if result["summary"]["plans_executed"] + result["summary"]["plans_not_run"] != result["summary"]["plans_selected"]:
        raise ValueError("coverage.summary: selected must equal executed plus not_run")
    infrastructure_errors(result["infrastructure_errors"])
    if "backend_executions" in result:
        coverage_backend_executions(result["backend_executions"], result["plans"], result["summary"])
    plan_ids: list[str] = []
    for index, plan in enumerate(result["plans"]):
        prefix = f"coverage.plans[{index}]"
        exact(plan, {"plan_id", "target_profile", "requirements", "selected", "execution", "components"}, prefix)
        string(plan["plan_id"], f"{prefix}.plan_id")
        string(plan["target_profile"], f"{prefix}.target_profile")
        id_array(plan["requirements"], f"{prefix}.requirements")
        plan_ids.append(plan["plan_id"])
        exact(plan["selected"], {"parity_case_ids", "command_ids"}, f"{prefix}.selected")
        id_array(plan["selected"]["parity_case_ids"], f"{prefix}.selected.parity_case_ids")
        id_array(plan["selected"]["command_ids"], f"{prefix}.selected.command_ids")
        exact(plan["execution"], {"status", "tests_passed", "tests_failed"}, f"{prefix}.execution")
        if plan["execution"]["status"] not in {"completed", "failed", "not_run"}:
            raise ValueError(f"{prefix}.execution.status: invalid status")
        non_negative_int(plan["execution"]["tests_passed"], f"{prefix}.execution.tests_passed")
        non_negative_int(plan["execution"]["tests_failed"], f"{prefix}.execution.tests_failed")
        for component_index, component in enumerate(plan["components"]):
            cprefix = f"{prefix}.components[{component_index}]"
            exact(component, {"component_id", "files", "thresholds"}, cprefix)
            string(component["component_id"], f"{cprefix}.component_id")
            for file_index, file in enumerate(component["files"]):
                fprefix = f"{cprefix}.files[{file_index}]"
                exact(file, {"path", "dimensions"}, fprefix)
                string(file["path"], f"{fprefix}.path")
                for dimension_index, dimension in enumerate(file["dimensions"]):
                    dpath = f"{fprefix}.dimensions[{dimension_index}]"
                    exact(dimension, {"dimension", "covered", "total", "uncovered"}, dpath)
                    string(dimension["dimension"], f"{dpath}.dimension")
                    non_negative_int(dimension["covered"], f"{dpath}.covered")
                    non_negative_int(dimension["total"], f"{dpath}.total")
                    if dimension["covered"] > dimension["total"]:
                        raise ValueError(f"{dpath}: covered exceeds total")
                    if not isinstance(dimension["uncovered"], list):
                        raise ValueError(f"{dpath}.uncovered: expected array")
            for threshold_index, threshold in enumerate(component["thresholds"]):
                exact(threshold, {"dimension", "minimum_percent", "covered", "total", "outcome"}, f"{cprefix}.thresholds[{threshold_index}]")
                if threshold["dimension"] not in {"function", "line", "branch", "region"}:
                    raise ValueError(f"{cprefix}.thresholds[{threshold_index}].dimension: invalid dimension")
                if type(threshold["minimum_percent"]) is not int or not 0 <= threshold["minimum_percent"] <= 100:
                    raise ValueError(f"{cprefix}.thresholds[{threshold_index}].minimum_percent: invalid threshold")
                non_negative_int(threshold["covered"], f"{cprefix}.thresholds[{threshold_index}].covered")
                non_negative_int(threshold["total"], f"{cprefix}.thresholds[{threshold_index}].total")
                if threshold["covered"] > threshold["total"]:
                    raise ValueError(f"{cprefix}.thresholds[{threshold_index}]: covered exceeds total")
                if threshold["outcome"] not in {"pass", "fail", "not_proven"}:
                    raise ValueError(f"{cprefix}.thresholds[{threshold_index}].outcome: invalid outcome")
    unique(plan_ids, "coverage.plans")
    if result["summary"]["plans_selected"] != len(result["plans"]):
        raise ValueError("coverage.summary.plans_selected does not equal plan count")
    if result["summary"]["tests_passed"] + result["summary"]["tests_failed"] < result["summary"]["plans_executed"]:
        raise ValueError("coverage.summary: test counts cannot be below executed plan count")


def all_backends(result: dict[str, Any]) -> None:
    """Validate the one-command CPU/SIMD/GPU/Python/JS test evidence."""

    def validate_scope(value: Any, label: str, *, execution: bool) -> None:
        required = {"kind", "selected", "case_ids_sha256", "filter"}
        if execution:
            required |= {"executed", "pending"}
        if not isinstance(value, dict) or not required.issubset(value):
            raise ValueError(f"{label}: invalid public parity scope")
        if value["kind"] != "public-parity-corpus":
            raise ValueError(f"{label}.kind: unsupported scope")
        for field in ("selected", "executed", "pending"):
            if field in value:
                non_negative_int(value[field], f"{label}.{field}")
        if execution and value["executed"] + value["pending"] != value["selected"]:
            raise ValueError(f"{label}: selected must equal executed plus pending")
        if not re.fullmatch(r"[0-9a-f]{64}", value["case_ids_sha256"]):
            raise ValueError(f"{label}.case_ids_sha256: expected lowercase sha256")
        if value["filter"] is not None:
            id_array(value["filter"], f"{label}.filter")
        if "selection" in value and value["selection"] not in {
            "all-public-cases",
            "explicit-case-filter",
        }:
            raise ValueError(f"{label}.selection: unsupported selection mode")
        if "pending_reasons" in value:
            if not isinstance(value["pending_reasons"], dict):
                raise ValueError(f"{label}.pending_reasons: expected object")
            for reason, count in value["pending_reasons"].items():
                string(reason, f"{label}.pending_reasons key")
                non_negative_int(count, f"{label}.pending_reasons[{reason}]")
        if "preflight" in value:
            preflight = value["preflight"]
            if not isinstance(preflight, dict) or set(preflight) != {"unrepresentable", "reasons"}:
                raise ValueError(f"{label}.preflight: invalid preflight classification")
            non_negative_int(preflight["unrepresentable"], f"{label}.preflight.unrepresentable")
            if not isinstance(preflight["reasons"], dict):
                raise ValueError(f"{label}.preflight.reasons: expected object")
            reason_total = 0
            for reason, count in preflight["reasons"].items():
                string(reason, f"{label}.preflight.reasons key")
                non_negative_int(count, f"{label}.preflight.reasons[{reason}]")
                reason_total += count
            if reason_total != preflight["unrepresentable"]:
                raise ValueError(f"{label}.preflight: reason counts do not add to unrepresentable")
        if "diagnostics" in value:
            diagnostics = value["diagnostics"]
            required_diagnostics = {
                "kind",
                "does_not_filter_or_change_execution",
                "not_pending",
                "hints",
            }
            if not isinstance(diagnostics, dict) or set(diagnostics) != required_diagnostics:
                raise ValueError(f"{label}.diagnostics: invalid diagnostic classification")
            string(diagnostics["kind"], f"{label}.diagnostics.kind")
            if not diagnostics["does_not_filter_or_change_execution"]:
                raise ValueError(
                    f"{label}.diagnostics: execution-affecting diagnostics are not allowed"
                )
            if not diagnostics["not_pending"]:
                raise ValueError(f"{label}.diagnostics: hints cannot be pending")
            if not isinstance(diagnostics["hints"], dict):
                raise ValueError(f"{label}.diagnostics.hints: expected object")
            for reason, count in diagnostics["hints"].items():
                string(reason, f"{label}.diagnostics.hints key")
                non_negative_int(count, f"{label}.diagnostics.hints[{reason}]")

    exact(
        result,
        {
            "schema",
            "status",
            "started_at",
            "finished_at",
            "revision",
            "input_scope",
            "gpu_gate",
            "gpu_full_requested",
            "backend_coverage",
            "lanes",
        },
        "all_backends",
    )
    if result["schema"] != "migration-parity/all-backends-test-result@3":
        raise ValueError("all_backends.schema: unsupported schema")
    if result["status"] not in {
        "passed",
        "passed_with_gpu_skipped",
        "passed_with_backend_gaps",
        "failed",
    }:
        raise ValueError("all_backends.status: invalid status")
    for field in ("started_at", "finished_at", "revision"):
        string(result[field], f"all_backends.{field}")
    if not isinstance(result["gpu_full_requested"], bool):
        raise ValueError("all_backends.gpu_full_requested: expected boolean")
    validate_scope(result["input_scope"], "all_backends.input_scope", execution=False)

    exact(
        result["gpu_gate"],
        {"case_id", "status", "timeout_seconds"},
        "all_backends.gpu_gate",
    )
    string(result["gpu_gate"]["case_id"], "all_backends.gpu_gate.case_id")
    if result["gpu_gate"]["status"] not in {"passed", "failed", "skipped"}:
        raise ValueError("all_backends.gpu_gate.status: invalid status")
    if type(result["gpu_gate"]["timeout_seconds"]) is not int or result["gpu_gate"]["timeout_seconds"] <= 0:
        raise ValueError("all_backends.gpu_gate.timeout_seconds: expected positive integer")

    expected_ids = {
        "parity-cpu",
        "parity-simd",
        "parity-gpu-smoke",
        "parity-gpu",
        "js-wasm-parity",
        "browser-wasm-parity",
    }
    lane_ids: list[str] = []
    lanes_by_id: dict[str, dict[str, Any]] = {}
    failed_required = False
    smoke_status: str | None = None
    full_gpu_status: str | None = None
    if not isinstance(result["lanes"], list) or not result["lanes"]:
        raise ValueError("all_backends.lanes: expected a non-empty list")
    for index, lane in enumerate(result["lanes"]):
        prefix = f"all_backends.lanes[{index}]"
        allowed = {
            "lane_id",
            "kind",
            "backend",
            "command",
            "status",
            "returncode",
            "timed_out",
            "artifact",
            "summary",
            "scope",
            "shader_coverage",
            "shader_coverage_artifact",
            "execution_evidence",
            "capabilities",
            "reason",
            "output_tail",
        }
        if not isinstance(lane, dict) or not set(lane).issubset(allowed):
            raise ValueError(f"{prefix}: unknown lane fields")
        for field in ("lane_id", "kind", "status"):
            string(lane.get(field), f"{prefix}.{field}")
        lane_ids.append(lane["lane_id"])
        lanes_by_id[lane["lane_id"]] = lane
        if lane["status"] not in {"passed", "failed", "skipped"}:
            raise ValueError(f"{prefix}.status: invalid status")
        if lane["kind"] not in {
            "python-py3-parity",
            "javascript-wasm-parity",
            "browser-wasm-parity",
        }:
            raise ValueError(f"{prefix}.kind: invalid kind")
        if lane.get("backend") not in {None, "cpu", "simd", "gpu"}:
            raise ValueError(f"{prefix}.backend: invalid backend")
        if not isinstance(lane.get("command"), list) or not lane["command"] or not all(isinstance(value, str) and value for value in lane["command"]):
            raise ValueError(f"{prefix}.command: expected non-empty string array")
        returncode = lane.get("returncode")
        if returncode is not None and (type(returncode) is not int or returncode < 0):
            raise ValueError(f"{prefix}.returncode: expected non-negative integer or null")
        if not isinstance(lane.get("timed_out"), bool):
            raise ValueError(f"{prefix}.timed_out: expected boolean")
        if "artifact" in lane:
            string(lane["artifact"], f"{prefix}.artifact")
        if "summary" in lane:
            summary = lane["summary"]
            summary_fields = {
                "selected",
                "executed",
                "passed",
                "failed",
                "not_run",
                "infrastructure_errors",
            }
            versioned_summary_fields = summary_fields | {
                "parity_cases",
                "fault_contracts",
            }
            if not isinstance(summary, dict) or frozenset(summary) not in {
                frozenset(summary_fields),
                frozenset(versioned_summary_fields),
            }:
                raise ValueError(
                    f"{prefix}.summary: unsupported parity summary fields"
                )
            for field, value in summary.items():
                if field not in {"parity_cases", "fault_contracts"}:
                    non_negative_int(value, f"{prefix}.summary.{field}")
            if set(summary) == versioned_summary_fields:
                outcome_fields = {
                    "selected",
                    "executed",
                    "passed",
                    "failed",
                    "not_run",
                }
                for kind in ("parity_cases", "fault_contracts"):
                    kind_summary = summary[kind]
                    exact(
                        kind_summary,
                        outcome_fields,
                        f"{prefix}.summary.{kind}",
                    )
                    for field in outcome_fields:
                        non_negative_int(
                            kind_summary[field],
                            f"{prefix}.summary.{kind}.{field}",
                        )
                    if kind_summary["selected"] != (
                        kind_summary["executed"] + kind_summary["not_run"]
                    ):
                        raise ValueError(
                            f"{prefix}.summary.{kind}: selected count is inconsistent"
                        )
                    if kind_summary["executed"] != (
                        kind_summary["passed"] + kind_summary["failed"]
                    ):
                        raise ValueError(
                            f"{prefix}.summary.{kind}: executed count is inconsistent"
                        )
                    for field in outcome_fields:
                        if summary[field] != sum(
                            summary[other][field]
                            for other in ("parity_cases", "fault_contracts")
                        ):
                            raise ValueError(
                                f"{prefix}.summary.{field} does not match its lane counts"
                            )
                if summary["fault_contracts"]["selected"] != 0:
                    raise ValueError(
                        f"{prefix}.summary: target-only fault contracts cannot enter all-backend parity lanes"
                    )
        if "scope" in lane:
            validate_scope(lane["scope"], f"{prefix}.scope", execution=True)
        if "shader_coverage" in lane:
            exact(lane["shader_coverage"], {"status", "reason"}, f"{prefix}.shader_coverage")
            if lane["shader_coverage"]["status"] not in {"measured", "not_measured"}:
                raise ValueError(f"{prefix}.shader_coverage.status: invalid status")
            string(lane["shader_coverage"]["reason"], f"{prefix}.shader_coverage.reason")
        if "shader_coverage_artifact" in lane:
            string(
                lane["shader_coverage_artifact"],
                f"{prefix}.shader_coverage_artifact",
            )
        if "execution_evidence" in lane:
            evidence = lane["execution_evidence"]
            if not isinstance(evidence, dict):
                raise ValueError(f"{prefix}.execution_evidence: expected object")
            evidence_status = evidence.get("status")
            if evidence_status not in {"measured", "not_measured"}:
                raise ValueError(
                    f"{prefix}.execution_evidence.status: invalid status"
                )
            exact(
                evidence,
                {"status", "reason", "artifact", "summary"}
                if evidence_status == "measured"
                else {"status", "reason", "artifact"},
                f"{prefix}.execution_evidence",
            )
            string(
                evidence["reason"],
                f"{prefix}.execution_evidence.reason",
                allow_empty=True,
            )
            string(evidence["artifact"], f"{prefix}.execution_evidence.artifact")
            if evidence_status == "measured":
                execution_summary = evidence["summary"]
                execution_evidence_summary(
                    execution_summary,
                    f"{prefix}.execution_evidence.summary",
                )
        if "capabilities" in lane:
            capabilities = lane["capabilities"]
            exact(capabilities, {"webgpu"}, f"{prefix}.capabilities")
            webgpu = capabilities["webgpu"]
            if not isinstance(webgpu, dict):
                raise ValueError(f"{prefix}.capabilities.webgpu: expected object")
            required_webgpu = {"api", "adapter", "device", "shader_dispatch", "reason"}
            if not required_webgpu.issubset(webgpu):
                raise ValueError(
                    f"{prefix}.capabilities.webgpu: missing required fields"
                )
            for field in required_webgpu:
                string(webgpu[field], f"{prefix}.capabilities.webgpu.{field}")
            if "adapter_info" in webgpu:
                exact(
                    webgpu["adapter_info"],
                    {"vendor", "architecture", "device", "description"},
                    f"{prefix}.capabilities.webgpu.adapter_info",
                )
                for field, value in webgpu["adapter_info"].items():
                    string(
                        value,
                        f"{prefix}.capabilities.webgpu.adapter_info.{field}",
                        nullable=True,
                        allow_empty=True,
                    )
        if lane["status"] in {"failed", "skipped"}:
            string(lane.get("reason"), f"{prefix}.reason")
        if "output_tail" in lane:
            string(lane["output_tail"], f"{prefix}.output_tail", allow_empty=True)
        if lane["lane_id"] == "parity-gpu-smoke":
            smoke_status = lane["status"]
        if lane["lane_id"] == "parity-gpu":
            full_gpu_status = lane["status"]
        if lane["status"] == "failed" and lane["lane_id"] != "parity-gpu-smoke":
            failed_required = True
    unique(lane_ids, "all_backends.lanes")
    if set(lane_ids) != expected_ids:
        raise ValueError(
            "all_backends.lanes: expected exactly CPU, SIMD, GPU gate/full, "
            "Node WASM, and browser WASM parity lanes"
        )
    wasm_lanes = {
        lane["lane_id"]: lane
        for lane in result["lanes"]
        if lane["lane_id"] in {"js-wasm-parity", "browser-wasm-parity"}
    }
    wasm_digests = {
        lane.get("scope", {}).get("case_ids_sha256")
        for lane in wasm_lanes.values()
        if isinstance(lane.get("scope"), dict)
    }
    if len(wasm_digests) > 1:
        raise ValueError("all_backends: Node and browser WASM lanes used different input scopes")
    if wasm_digests and wasm_digests != {result["input_scope"]["case_ids_sha256"]}:
        raise ValueError("all_backends: WASM lane scope does not match the common input scope")
    browser_lane = wasm_lanes.get("browser-wasm-parity")
    if browser_lane and browser_lane["status"] == "passed" and "capabilities" not in browser_lane:
        raise ValueError("all_backends: a passed browser WASM lane must report WebGPU capability")
    if smoke_status != result["gpu_gate"]["status"]:
        raise ValueError("all_backends.gpu_gate.status does not match GPU smoke lane")
    if smoke_status == "passed" and result["gpu_full_requested"] and full_gpu_status == "skipped":
        raise ValueError("all_backends: a requested full GPU lane must execute after a passed GPU gate")
    if smoke_status == "passed" and not result["gpu_full_requested"] and full_gpu_status != "skipped":
        raise ValueError("all_backends: the non-requested full GPU lane must be recorded as skipped")
    if smoke_status in {"failed", "skipped"} and full_gpu_status != "skipped":
        raise ValueError("all_backends: a non-passing GPU gate must skip the full GPU lane")

    # The public parity lanes and the backend-proof lanes are intentionally
    # separate claims.  Recompute the receipt verdict from the lane evidence
    # instead of trusting a producer-supplied status: a plain ``passed`` result
    # must never hide missing terminal receipts, CPU controls in a SIMD/GPU
    # lane, or non-empty fallback taxonomy.
    backend_coverage = result["backend_coverage"]
    exact(
        backend_coverage,
        {"status", "target_lanes"},
        "all_backends.backend_coverage",
    )
    if backend_coverage["status"] not in {"proven", "not_proven"}:
        raise ValueError("all_backends.backend_coverage.status: invalid status")
    if not isinstance(backend_coverage["target_lanes"], list):
        raise ValueError(
            "all_backends.backend_coverage.target_lanes: expected list"
        )
    coverage_lane_ids: list[str] = []
    for index, coverage_lane in enumerate(backend_coverage["target_lanes"]):
        prefix = f"all_backends.backend_coverage.target_lanes[{index}]"
        exact(
            coverage_lane,
            {
                "lane_id",
                "requested_backend",
                "status",
                "selected",
                "terminal_complete_receipts",
                "terminal_incomplete_cases",
                "not_recorded_cases",
                "pipeline_applicable_cases",
                "pipeline_complete_cases",
                "pipeline_missing_receipt_cases",
                "pipeline_partial_receipt_cases",
                "pipeline_not_applicable_cases",
                "pipeline_indeterminate_cases",
                "actual_backend_counts",
                "fallback_reason_counts",
                "reasons",
            },
            prefix,
        )
        string(coverage_lane["lane_id"], f"{prefix}.lane_id")
        coverage_lane_ids.append(coverage_lane["lane_id"])
        string(coverage_lane["requested_backend"], f"{prefix}.requested_backend")
        if coverage_lane["status"] not in {"proven", "not_proven"}:
            raise ValueError(f"{prefix}.status: invalid status")
        for field in (
            "selected",
            "terminal_complete_receipts",
            "terminal_incomplete_cases",
            "not_recorded_cases",
            "pipeline_applicable_cases",
            "pipeline_complete_cases",
            "pipeline_missing_receipt_cases",
            "pipeline_partial_receipt_cases",
            "pipeline_not_applicable_cases",
            "pipeline_indeterminate_cases",
        ):
            non_negative_int(coverage_lane[field], f"{prefix}.{field}")
        if (
            coverage_lane["pipeline_applicable_cases"]
            != coverage_lane["pipeline_complete_cases"]
            + coverage_lane["pipeline_missing_receipt_cases"]
            + coverage_lane["pipeline_partial_receipt_cases"]
        ):
            raise ValueError(f"{prefix}: pipeline-applicable counts are inconsistent")
        if (
            coverage_lane["pipeline_applicable_cases"]
            + coverage_lane["pipeline_not_applicable_cases"]
            + coverage_lane["pipeline_indeterminate_cases"]
            != coverage_lane["selected"]
        ):
            raise ValueError(f"{prefix}: pipeline case partition does not match selected")
        if coverage_lane["pipeline_partial_receipt_cases"] != coverage_lane[
            "terminal_incomplete_cases"
        ]:
            raise ValueError(f"{prefix}: partial receipt count is inconsistent")
        for field in ("actual_backend_counts", "fallback_reason_counts"):
            counts = coverage_lane[field]
            if not isinstance(counts, dict):
                raise ValueError(f"{prefix}.{field}: expected object")
            for key, count in counts.items():
                string(key, f"{prefix}.{field} key")
                non_negative_int(count, f"{prefix}.{field}[{key}]")
        if not isinstance(coverage_lane["reasons"], list) or not all(
            isinstance(reason, str) and reason for reason in coverage_lane["reasons"]
        ):
            raise ValueError(f"{prefix}.reasons: expected non-empty string array")
        if coverage_lane["status"] == "proven" and coverage_lane["reasons"]:
            raise ValueError(f"{prefix}: proven lane cannot have reasons")
        if coverage_lane["status"] == "not_proven" and not coverage_lane["reasons"]:
            raise ValueError(f"{prefix}: not-proven lane requires reasons")
    unique(coverage_lane_ids, "all_backends.backend_coverage.target_lanes")
    expected_coverage_lane_ids = {
        "parity-cpu",
        "parity-simd",
        "parity-gpu",
    }
    if set(coverage_lane_ids) != expected_coverage_lane_ids:
        raise ValueError(
            "all_backends.backend_coverage.target_lanes: expected CPU, SIMD, and GPU lanes"
        )

    try:
        from run_all_backend_tests import backend_coverage_report
    except ModuleNotFoundError:  # imported as ``scripts.validate_migration_parity_result``
        from scripts.run_all_backend_tests import backend_coverage_report

    expected_backend_coverage = backend_coverage_report(
        [lanes_by_id[lane_id] for lane_id in sorted(lanes_by_id)]
    )
    if backend_coverage != expected_backend_coverage:
        raise ValueError(
            "all_backends.backend_coverage does not match lane receipt evidence"
        )
    expected_status = (
        "failed"
        if failed_required
        else "passed_with_gpu_skipped"
        if smoke_status != "passed" or not result["gpu_full_requested"]
        else "passed_with_backend_gaps"
        if backend_coverage["status"] != "proven"
        else "passed"
    )
    if result["status"] != expected_status:
        raise ValueError("all_backends.status does not match lane outcomes")


def benchmark(result: dict[str, Any]) -> None:
    exact(result, {"schema", "identity", "status", "environment", "summary", "workloads", "suites", "infrastructure_errors"}, "benchmark")
    if result["schema"] != "migration-parity/benchmark-result@1":
        raise ValueError("benchmark.schema: unsupported schema")
    if result["status"] not in {"completed", "infrastructure_failed", "cancelled", "invalid"}:
        raise ValueError("benchmark.status: invalid artifact status")
    identity(result["identity"])
    environment_fields = {"machine_id", "os", "architecture", "cpu", "memory_bytes", "power_mode", "toolchain"}
    environment = result["environment"]
    if not isinstance(environment, dict) or set(environment) not in (
        environment_fields,
        environment_fields | {"runner_id"},
    ):
        actual = sorted(environment) if isinstance(environment, dict) else type(environment).__name__
        raise ValueError(
            f"benchmark.environment: expected keys {sorted(environment_fields)} "
            f"with optional runner_id, got {actual}"
        )
    if "runner_id" in environment:
        string(environment["runner_id"], "benchmark.environment.runner_id")
    for field in ("machine_id", "os", "architecture", "cpu", "power_mode", "toolchain"):
        string(result["environment"][field], f"benchmark.environment.{field}")
    non_negative_int(result["environment"]["memory_bytes"], "benchmark.environment.memory_bytes")
    exact(result["summary"], {"workloads_selected", "workloads_measured", "workloads_not_run", "budgets_passed", "budgets_failed", "budgets_not_proven"}, "benchmark.summary")
    for field in result["summary"]:
        non_negative_int(result["summary"][field], f"benchmark.summary.{field}")
    if result["summary"]["workloads_measured"] + result["summary"]["workloads_not_run"] != result["summary"]["workloads_selected"]:
        raise ValueError("benchmark.summary: selected must equal measured plus not_run")
    infrastructure_errors(result["infrastructure_errors"])
    workload_ids: list[str] = []
    for index, workload in enumerate(result["workloads"]):
        prefix = f"benchmark.workloads[{index}]"
        exact(workload, {"workload_id", "requirements", "measurement_policy", "context", "correctness", "subjects", "budgets"}, prefix)
        string(workload["workload_id"], f"{prefix}.workload_id")
        workload_ids.append(workload["workload_id"])
        id_array(workload["requirements"], f"{prefix}.requirements")
        exact(workload["context"], {"size", "mode", "chain_length", "operation_class", "cache_state", "build_profile"}, f"{prefix}.context")
        if (
            not isinstance(workload["context"]["size"], list)
            or len(workload["context"]["size"]) != 2
            or any(type(value) is not int or value < 0 for value in workload["context"]["size"])
        ):
            raise ValueError(f"{prefix}.context.size: expected two non-negative integer dimensions")
        if not isinstance(workload["context"]["mode"], str) or not workload["context"]["mode"]:
            raise ValueError(f"{prefix}.context.mode: expected a non-empty mode")
        if type(workload["context"]["chain_length"]) is not int or workload["context"]["chain_length"] < 0:
            raise ValueError(f"{prefix}.context.chain_length: expected a non-negative integer")
        if workload["context"]["operation_class"] not in {"point", "neighborhood", "geometry", "draw", "multi_image", "generator", "terminal"}:
            raise ValueError(f"{prefix}.context.operation_class: unsupported operation class")
        if workload["context"]["cache_state"] not in {"cold", "warm", "resident", "mixed"}:
            raise ValueError(f"{prefix}.context.cache_state: unsupported cache state")
        if workload["context"]["build_profile"] not in {"debug", "release"}:
            raise ValueError(f"{prefix}.context.build_profile: unsupported build profile")
        if not isinstance(workload["measurement_policy"], dict):
            raise ValueError(f"{prefix}.measurement_policy: expected object")
        execution_scope = workload["measurement_policy"].get(
            "execution_scope", "compute_backend"
        )
        if execution_scope not in {"compute_backend", "host_boundary"}:
            raise ValueError(
                f"{prefix}.measurement_policy.execution_scope: invalid scope"
            )
        exact(workload["correctness"], {"gate", "outcome", "evidence_id"}, f"{prefix}.correctness")
        if workload["correctness"]["gate"] not in {"parity_pass", "source_target_match", "successful_execution", "not_applicable"}:
            raise ValueError(f"{prefix}.correctness.gate: invalid gate")
        if workload["correctness"]["outcome"] not in {"pass", "fail", "not_proven"}:
            raise ValueError(f"{prefix}.correctness.outcome: invalid outcome")
        string(workload["correctness"]["evidence_id"], f"{prefix}.correctness.evidence_id", nullable=True)
        subject_ids: list[str] = []
        for subject_index, subject in enumerate(workload["subjects"]):
            sprefix = f"{prefix}.subjects[{subject_index}]"
            exact(subject, {"kind", "id", "status", "measurements", "phases", "execution"}, sprefix)
            if subject["kind"] not in {"oracle", "target_profile"}:
                raise ValueError(f"{sprefix}.kind: invalid subject kind")
            string(subject["id"], f"{sprefix}.id")
            subject_ids.append(subject["id"])
            if subject["status"] not in {"completed", "failed", "not_run"}:
                raise ValueError(f"{sprefix}.status: invalid status")
            for measurement_index, measurement in enumerate(subject["measurements"]):
                mprefix = f"{sprefix}.measurements[{measurement_index}]"
                exact(measurement, {"metric", "unit", "sample_count", "statistics", "raw_samples_ref"}, mprefix)
                string(measurement["metric"], f"{mprefix}.metric")
                string(measurement["unit"], f"{mprefix}.unit")
                non_negative_int(measurement["sample_count"], f"{mprefix}.sample_count")
                exact(measurement["statistics"], {"min", "median", "mean", "p95", "p99", "max", "total", "weighted_mean", "standard_deviation"}, f"{mprefix}.statistics")
                for field, value in measurement["statistics"].items():
                    if value is not None and not isinstance(value, (int, float)):
                        raise ValueError(f"{mprefix}.statistics.{field}: expected number or null")
                string(measurement["raw_samples_ref"], f"{mprefix}.raw_samples_ref", nullable=True)
            exact(subject["phases"], {"setup", "pipeline", "terminal", "total"}, f"{sprefix}.phases")
            for phase_name, phase in subject["phases"].items():
                pprefix = f"{sprefix}.phases.{phase_name}"
                exact(phase, {"sample_count", "statistics"}, pprefix)
                non_negative_int(phase["sample_count"], f"{pprefix}.sample_count")
                exact(phase["statistics"], {"min", "median", "mean", "p95", "p99", "max", "total", "weighted_mean", "standard_deviation"}, f"{pprefix}.statistics")
                for field, value in phase["statistics"].items():
                    if value is not None and not isinstance(value, (int, float)):
                        raise ValueError(f"{pprefix}.statistics.{field}: expected number or null")
            policy = workload["measurement_policy"]
            allowed_step_ids = None
            if policy.get("boundary") == "observed_steps":
                allowed_step_ids = set(
                    id_array(
                        policy.get("step_ids"),
                        f"{prefix}.measurement_policy.step_ids",
                    )
                )
            execution_receipt(
                subject["execution"],
                f"{sprefix}.execution",
                allowed_step_ids=allowed_step_ids,
            )
            if execution_scope == "host_boundary" and subject["kind"] == "target_profile":
                receipt = subject["execution"]
                if (
                    receipt["status"] != "not_applicable"
                    or receipt["actual_backend"] is not None
                    or receipt["actual_backend_counts"] != {}
                    or receipt["sample_count"] != 0
                    or receipt["fallback_reason_counts"] != {}
                    or receipt.get("errors")
                ):
                    raise ValueError(
                        f"{sprefix}.execution: host-boundary timings cannot claim "
                        "compute-backend attribution"
                    )
        unique(subject_ids, f"{prefix}.subjects")
        for budget_index, budget in enumerate(workload["budgets"]):
            exact(budget, {"requirement_id", "subject_id", "baseline_subject", "metric", "statistic", "operator", "required", "observed", "unit", "outcome"}, f"{prefix}.budgets[{budget_index}]")
            for field in ("requirement_id", "subject_id", "baseline_subject", "metric", "statistic", "operator", "unit"):
                string(budget[field], f"{prefix}.budgets[{budget_index}].{field}", nullable=field in {"baseline_subject"})
            if not isinstance(budget["required"], (int, float)) or not isinstance(budget["observed"], (int, float)):
                raise ValueError(f"{prefix}.budgets[{budget_index}]: required and observed must be numeric")
            if budget["operator"] not in {"less_than_or_equal", "greater_than_or_equal"}:
                raise ValueError(f"{prefix}.budgets[{budget_index}].operator: invalid operator")
            if budget["outcome"] not in {"pass", "fail", "not_proven"}:
                raise ValueError(f"{prefix}.budgets[{budget_index}].outcome: invalid outcome")
    for index, suite in enumerate(result["suites"]):
        prefix = f"benchmark.suites[{index}]"
        exact(suite, {"suite_id", "members", "subjects", "comparisons"}, prefix)
        for member_index, member in enumerate(suite["members"]):
            exact(member, {"workload_id", "weight"}, f"{prefix}.members[{member_index}]")
            string(member["workload_id"], f"{prefix}.members[{member_index}].workload_id")
            if not isinstance(member["weight"], (int, float)) or member["weight"] <= 0:
                raise ValueError(f"{prefix}.members[{member_index}].weight: expected positive number")
        for subject_index, subject in enumerate(suite["subjects"]):
            sprefix = f"{prefix}.subjects[{subject_index}]"
            exact(subject, {"kind", "id", "status", "measurements"}, sprefix)
            for measurement_index, measurement in enumerate(subject["measurements"]):
                exact(measurement, {"metric", "unit", "weighted_mean"}, f"{sprefix}.measurements[{measurement_index}]")
        for comparison_index, comparison in enumerate(suite["comparisons"]):
            comparison_label = f"{prefix}.comparisons[{comparison_index}]"
            legacy_keys = {
                "baseline_subject",
                "subject_id",
                "metric",
                "baseline_value",
                "subject_value",
                "unit",
                "ratio",
            }
            evidence_keys = legacy_keys | {
                "declared_member_count",
                "common_member_count",
                "common_member_ids_sha256",
                "excluded_members",
                "status",
            }
            if set(comparison) not in (legacy_keys, evidence_keys):
                raise ValueError(f"{comparison_label}: invalid comparison fields")
            for field in ("baseline_subject", "subject_id", "metric", "unit"):
                string(comparison[field], f"{comparison_label}.{field}")
            for field in ("baseline_value", "subject_value", "ratio"):
                number = comparison[field]
                if number is not None and (
                    isinstance(number, bool) or not isinstance(number, (int, float))
                ):
                    raise ValueError(f"{comparison_label}.{field}: expected number or null")
            if evidence_keys == set(comparison):
                for field in ("declared_member_count", "common_member_count"):
                    non_negative_int(comparison[field], f"{comparison_label}.{field}")
                if comparison["common_member_count"] > comparison["declared_member_count"]:
                    raise ValueError(f"{comparison_label}: common count exceeds declared count")
                if not re.fullmatch(r"[0-9a-f]{64}", comparison["common_member_ids_sha256"]):
                    raise ValueError(f"{comparison_label}.common_member_ids_sha256: expected lowercase sha256")
                if comparison["status"] not in {"comparable", "not_comparable"}:
                    raise ValueError(f"{comparison_label}.status: invalid status")
                if not isinstance(comparison["excluded_members"], list):
                    raise ValueError(f"{comparison_label}.excluded_members: expected array")
                for excluded_index, excluded in enumerate(comparison["excluded_members"]):
                    excluded_label = f"{comparison_label}.excluded_members[{excluded_index}]"
                    exact(excluded, {"workload_id", "baseline_status", "subject_status"}, excluded_label)
                    for field in ("workload_id", "baseline_status", "subject_status"):
                        string(excluded[field], f"{excluded_label}.{field}")


def status_report(result: dict[str, Any]) -> None:
    """Validate the generated aggregate without accepting open-ended fields."""

    exact(
        result,
        {
            "schema",
            "manifest",
            "target_profiles",
            "evidence",
            "completeness",
            "operations",
            "stale_or_incompatible_evidence",
        },
        "status",
    )
    if result["schema"] != "migration-parity/status-report@1":
        raise ValueError("status.schema: unsupported schema")
    exact(result["manifest"], {"path", "schema", "sha256"}, "status.manifest")
    for index, target in enumerate(result["target_profiles"]):
        exact(
            target,
            {"target_profile", "target_id", "revision", "dirty", "runtime", "backend", "features"},
            f"status.target_profiles[{index}]",
        )
    for index, item in enumerate(result["evidence"]):
        exact(item, {"lane", "run_id", "snapshot_id"}, f"status.evidence[{index}]")
        if item["lane"] not in {"parity", "coverage", "benchmark"}:
            raise ValueError(f"status.evidence[{index}].lane: invalid lane")
    dimensions = {
        "inventory_representation",
        "operation_contracts",
        "parity_input_mapping",
        "coverage_input_mapping",
        "benchmark_input_mapping",
        "parity_outcome",
        "function_coverage",
        "line_coverage",
        "branch_coverage",
        "region_coverage",
        "benchmark_budget_outcome",
        "documentation_freshness",
    }
    for index, item in enumerate(result["completeness"]):
        exact(
            item,
            {"dimension", "target_profile", "numerator", "denominator", "evidence_id"},
            f"status.completeness[{index}]",
        )
        if item["dimension"] not in dimensions:
            raise ValueError(f"status.completeness[{index}].dimension: invalid dimension")
        if not isinstance(item["numerator"], int) or not isinstance(item["denominator"], int):
            raise ValueError(f"status.completeness[{index}]: counts must be integers")
        if item["numerator"] < 0 or item["denominator"] < 0 or item["numerator"] > item["denominator"]:
            raise ValueError(f"status.completeness[{index}]: invalid counts")
    for index, item in enumerate(result["operations"]):
        prefix = f"status.operations[{index}]"
        exact(
            item,
            {"surface", "operation", "target_profile", "classification", "support", "requirements", "parity", "coverage", "benchmark"},
            prefix,
        )
        if item["support"] not in {"supported", "partial", "unsupported"}:
            raise ValueError(f"{prefix}.support: invalid support")
        for lane in ("parity", "coverage", "benchmark"):
            lane_prefix = f"{prefix}.{lane}"
            exact(item[lane], {"applicability", "input_ids", "outcome", "evidence_id", "details"}, lane_prefix)
            if item[lane]["outcome"] not in {"pass", "fail", "not_run", "not_proven", "not_applicable"}:
                raise ValueError(f"{lane_prefix}.outcome: invalid outcome")
            if not isinstance(item[lane]["details"], list) or not all(isinstance(value, str) for value in item[lane]["details"]):
                raise ValueError(f"{lane_prefix}.details: expected string array")
    for index, item in enumerate(result["stale_or_incompatible_evidence"]):
        exact(item, {"lane", "run_id", "reason", "identity_diff"}, f"status.stale_or_incompatible_evidence[{index}]")
        if not isinstance(item["identity_diff"], list) or not all(isinstance(value, str) for value in item["identity_diff"]):
            raise ValueError(f"status.stale_or_incompatible_evidence[{index}].identity_diff: expected string array")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("lane", choices=("parity", "coverage", "all_backends", "benchmark", "status"))
    parser.add_argument("result", type=Path)
    args = parser.parse_args()
    if args.result.name.endswith(".gz"):
        with gzip.open(args.result, "rt", encoding="utf-8") as handle:
            result = json.load(handle)
    else:
        result = json.loads(args.result.read_text(encoding="utf-8"))
    {"parity": parity, "coverage": coverage, "all_backends": all_backends, "benchmark": benchmark, "status": status_report}[args.lane](result)
    print(f"{args.lane} result schema valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
