#!/usr/bin/env python3
"""Validate generated migration-parity lane artifacts with exact key sets."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def exact(value: Any, keys: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        actual = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise ValueError(f"{label}: expected keys {sorted(keys)}, got {actual}")


def identity(value: dict[str, Any]) -> None:
    exact(value, {"run_id", "started_at", "finished_at", "manifest", "inputs", "assets", "oracles", "targets", "command"}, "identity")
    exact(value["manifest"], {"path", "schema", "sha256"}, "identity.manifest")
    for index, item in enumerate(value["inputs"]):
        exact(item, {"path", "schema", "sha256"}, f"identity.inputs[{index}]")
    for index, item in enumerate(value["assets"]):
        exact(item, {"input_path", "item_id", "asset_id", "kind", "locator", "sha256"}, f"identity.assets[{index}]")
    for index, item in enumerate(value["oracles"]):
        exact(item, {"oracle_id", "name", "version", "runtime"}, f"identity.oracles[{index}]")
    for index, item in enumerate(value["targets"]):
        exact(item, {"target_profile", "target_id", "revision", "dirty", "runtime", "backend", "features"}, f"identity.targets[{index}]")
    exact(value["command"], {"command_id", "argv", "cwd", "timeout_seconds"}, "identity.command")


def infrastructure_errors(value: list[dict[str, Any]]) -> None:
    for index, item in enumerate(value):
        exact(item, {"scope", "id", "kind", "message"}, f"infrastructure_errors[{index}]")


def workflow(value: dict[str, Any], label: str) -> None:
    exact(value, {"case_id", "status", "observations"}, label)
    if value["status"] not in {"completed", "not_run"}:
        raise ValueError(f"{label}.status: invalid workflow status")
    for index, observation in enumerate(value["observations"]):
        prefix = f"{label}.observations[{index}]"
        exact(observation, {"step_id", "status", "value"} if observation["status"] == "ok" else {"step_id", "status", "error"} if observation["status"] == "error" else {"step_id", "status", "reason"}, prefix)
        if observation["status"] == "error":
            exact(observation["error"], {"class", "kind", "message", "stage", "code"}, f"{prefix}.error")


def parity(result: dict[str, Any]) -> None:
    exact(result, {"schema", "identity", "status", "summary", "comparisons", "infrastructure_errors"}, "parity")
    if result["schema"] != "migration-parity/parity-result@1":
        raise ValueError("parity.schema: unsupported schema")
    identity(result["identity"])
    exact(result["summary"], {"selected", "executed", "passed", "failed", "not_run", "infrastructure_errors"}, "parity.summary")
    if result["summary"]["selected"] != len(result["comparisons"]):
        raise ValueError("parity.summary.selected does not equal comparison count")
    infrastructure_errors(result["infrastructure_errors"])
    for index, comparison in enumerate(result["comparisons"]):
        prefix = f"parity.comparisons[{index}]"
        exact(comparison, {"case_id", "target_profile", "requirements", "source", "target", "outcome", "diffs"}, prefix)
        workflow(comparison["source"], f"{prefix}.source")
        workflow(comparison["target"], f"{prefix}.target")
        for diff_index, diff in enumerate(comparison["diffs"]):
            exact(diff, {"step_id", "path", "kind", "source", "target", "message"}, f"{prefix}.diffs[{diff_index}]")


def coverage(result: dict[str, Any]) -> None:
    exact(result, {"schema", "identity", "status", "collector", "summary", "plans", "infrastructure_errors"}, "coverage")
    if result["schema"] != "migration-parity/coverage-result@1":
        raise ValueError("coverage.schema: unsupported schema")
    identity(result["identity"])
    exact(result["collector"], {"name", "version", "snapshot_id", "artifact_ingested"}, "coverage.collector")
    exact(result["summary"], {"plans_selected", "plans_executed", "plans_not_run", "tests_passed", "tests_failed"}, "coverage.summary")
    infrastructure_errors(result["infrastructure_errors"])
    for index, plan in enumerate(result["plans"]):
        prefix = f"coverage.plans[{index}]"
        exact(plan, {"plan_id", "target_profile", "requirements", "selected", "execution", "components"}, prefix)
        exact(plan["selected"], {"parity_case_ids", "command_ids"}, f"{prefix}.selected")
        exact(plan["execution"], {"status", "tests_passed", "tests_failed"}, f"{prefix}.execution")
        for component_index, component in enumerate(plan["components"]):
            cprefix = f"{prefix}.components[{component_index}]"
            exact(component, {"component_id", "files", "thresholds"}, cprefix)
            for file_index, file in enumerate(component["files"]):
                fprefix = f"{cprefix}.files[{file_index}]"
                exact(file, {"path", "dimensions"}, fprefix)
                for dimension_index, dimension in enumerate(file["dimensions"]):
                    exact(dimension, {"dimension", "covered", "total", "uncovered"}, f"{fprefix}.dimensions[{dimension_index}]")
            for threshold_index, threshold in enumerate(component["thresholds"]):
                exact(threshold, {"dimension", "minimum_percent", "covered", "total", "outcome"}, f"{cprefix}.thresholds[{threshold_index}]")


def benchmark(result: dict[str, Any]) -> None:
    exact(result, {"schema", "identity", "status", "environment", "summary", "workloads", "suites", "infrastructure_errors"}, "benchmark")
    if result["schema"] != "migration-parity/benchmark-result@1":
        raise ValueError("benchmark.schema: unsupported schema")
    identity(result["identity"])
    exact(result["environment"], {"machine_id", "os", "architecture", "cpu", "memory_bytes", "power_mode", "toolchain"}, "benchmark.environment")
    exact(result["summary"], {"workloads_selected", "workloads_measured", "workloads_not_run", "budgets_passed", "budgets_failed", "budgets_not_proven"}, "benchmark.summary")
    infrastructure_errors(result["infrastructure_errors"])
    for index, workload in enumerate(result["workloads"]):
        prefix = f"benchmark.workloads[{index}]"
        exact(workload, {"workload_id", "requirements", "measurement_policy", "correctness", "subjects", "budgets"}, prefix)
        exact(workload["correctness"], {"gate", "outcome", "evidence_id"}, f"{prefix}.correctness")
        for subject_index, subject in enumerate(workload["subjects"]):
            sprefix = f"{prefix}.subjects[{subject_index}]"
            exact(subject, {"kind", "id", "status", "measurements"}, sprefix)
            for measurement_index, measurement in enumerate(subject["measurements"]):
                mprefix = f"{sprefix}.measurements[{measurement_index}]"
                exact(measurement, {"metric", "unit", "sample_count", "statistics", "raw_samples_ref"}, mprefix)
                exact(measurement["statistics"], {"min", "median", "mean", "p95", "p99", "max", "total", "weighted_mean", "standard_deviation"}, f"{mprefix}.statistics")
        for budget_index, budget in enumerate(workload["budgets"]):
            exact(budget, {"requirement_id", "subject_id", "baseline_subject", "metric", "statistic", "operator", "required", "observed", "unit", "outcome"}, f"{prefix}.budgets[{budget_index}]")
    for index, suite in enumerate(result["suites"]):
        prefix = f"benchmark.suites[{index}]"
        exact(suite, {"suite_id", "members", "subjects", "comparisons"}, prefix)
        for member_index, member in enumerate(suite["members"]):
            exact(member, {"workload_id", "weight"}, f"{prefix}.members[{member_index}]")
        for subject_index, subject in enumerate(suite["subjects"]):
            sprefix = f"{prefix}.subjects[{subject_index}]"
            exact(subject, {"kind", "id", "status", "measurements"}, sprefix)
            for measurement_index, measurement in enumerate(subject["measurements"]):
                exact(measurement, {"metric", "unit", "weighted_mean"}, f"{sprefix}.measurements[{measurement_index}]")
        for comparison_index, comparison in enumerate(suite["comparisons"]):
            exact(comparison, {"baseline_subject", "subject_id", "metric", "baseline_value", "subject_value", "unit", "ratio"}, f"{prefix}.comparisons[{comparison_index}]")


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
    parser.add_argument("lane", choices=("parity", "coverage", "benchmark", "status"))
    parser.add_argument("result", type=Path)
    args = parser.parse_args()
    result = json.loads(args.result.read_text(encoding="utf-8"))
    {"parity": parity, "coverage": coverage, "benchmark": benchmark, "status": status_report}[args.lane](result)
    print(f"{args.lane} result schema valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
