#!/usr/bin/env python3
"""Compare compatible pipeline benchmark lineages against a guarded budget.

Only completed subjects with matching actual backends and latency statistics
are compared. Missing GPU adapters, unsupported operations, and backend
identity changes remain explicit ``not_comparable`` records instead of being
treated as passes or failures.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CURRENT = ROOT / "build" / "migration-parity" / "benchmark-result.json"
DEFAULT_BASELINE = ROOT / "build" / "migration-parity" / "benchmark-result-baseline.json"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "pipeline-budget-check.json"
SUBJECTS = ("pillow", "python-cpu", "python-simd", "python-gpu")


def relative(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT.resolve()))
    except ValueError:
        return str(path.resolve())


def latency(subject: dict[str, Any]) -> dict[str, float] | None:
    if subject.get("status") != "completed":
        return None
    for measurement in subject.get("measurements", []):
        if measurement.get("metric") != "latency":
            continue
        statistics = measurement.get("statistics")
        if not isinstance(statistics, dict):
            return None
        values = ("median", "p95", "standard_deviation")
        if any(not isinstance(statistics.get(key), (int, float)) for key in values):
            return None
        sample_count = measurement.get("sample_count")
        if not isinstance(sample_count, int) or sample_count <= 0:
            return None
        return {
            "median": float(statistics["median"]),
            "p95": float(statistics["p95"]),
            "standard_deviation": float(statistics["standard_deviation"]),
            "sample_count": float(sample_count),
        }
    return None


def execution_comparability(
    subject: dict[str, Any], subject_id: str, stats: dict[str, float]
) -> tuple[bool, str]:
    """Require a terminal, backend-identifying receipt before comparing time.

    A benchmark subject can have a latency measurement even when its pipeline
    execution was never observed (for example a drained prefix, a resident
    cache hit, or a missing adapter receipt).  Treating that timing as a
    backend cohort would make ``actual_backend: null`` look comparable.  The
    source oracle has no pipeline telemetry by design, so its explicit
    Pillow receipt is the only special case.
    """

    execution = subject.get("execution")
    if not isinstance(execution, dict):
        return False, "missing_execution_receipt"
    if execution.get("errors"):
        return False, "execution_errors"
    fallback_counts = execution.get("fallback_reason_counts")
    if fallback_counts != {}:
        return False, "execution_fallback"

    expected_backend = (
        "pillow" if subject_id == "pillow" else subject_id.removeprefix("python-")
    )
    if subject_id == "pillow":
        if execution.get("status") != "not_applicable":
            return False, "oracle_execution_not_applicable_missing"
        if (
            execution.get("requested_backend") != "pillow"
            or execution.get("actual_backend") != "pillow"
        ):
            return False, "oracle_backend_identity_invalid"
        return True, ""

    if execution.get("status") != "completed":
        return False, "execution_not_completed"
    # New receipts must explicitly prove that the final public observation
    # completed.  Missing is deliberately not treated as legacy success: an
    # old artifact cannot prove that a drained prefix was terminal.
    if execution.get("terminal_complete") is not True:
        return False, "terminal_receipt_incomplete"
    if execution.get("requested_backend") != expected_backend:
        return False, "requested_backend_changed"
    if execution.get("actual_backend") != expected_backend:
        return False, "actual_backend_not_proven"
    actual_counts = execution.get("actual_backend_counts")
    if not isinstance(actual_counts, dict) or set(actual_counts) != {expected_backend}:
        return False, "actual_backend_counts_invalid"
    count = actual_counts.get(expected_backend)
    if not isinstance(count, int) or count <= 0:
        return False, "actual_backend_counts_invalid"
    sample_count = execution.get("sample_count")
    if not isinstance(sample_count, int) or sample_count <= 0:
        return False, "execution_samples_missing"
    if int(sample_count) != int(stats["sample_count"]):
        return False, "execution_sample_count_mismatch"
    return True, ""


def subject_map(workload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {str(subject.get("id")): subject for subject in workload.get("subjects", [])}


def compare(current_path: Path, baseline_path: Path, budget: float) -> dict[str, Any]:
    current = json.loads(current_path.read_text(encoding="utf-8"))
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    if current.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError("current result has an incompatible schema")
    if baseline.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError("baseline result has an incompatible schema")

    current_by_id = {item["workload_id"]: item for item in current.get("workloads", [])}
    baseline_by_id = {item["workload_id"]: item for item in baseline.get("workloads", [])}
    comparisons: list[dict[str, Any]] = []
    violations: list[dict[str, Any]] = []
    for workload_id in sorted(current_by_id):
        current_workload = current_by_id[workload_id]
        baseline_workload = baseline_by_id.get(workload_id)
        for subject_id in SUBJECTS:
            key = {"workload_id": workload_id, "subject_id": subject_id}
            if baseline_workload is None:
                comparisons.append({**key, "status": "not_comparable", "reason": "missing_baseline_workload"})
                continue
            current_subject = subject_map(current_workload).get(subject_id)
            baseline_subject = subject_map(baseline_workload).get(subject_id)
            if current_subject is None or baseline_subject is None:
                comparisons.append({**key, "status": "not_comparable", "reason": "missing_subject"})
                continue
            current_stats = latency(current_subject)
            baseline_stats = latency(baseline_subject)
            if current_stats is None or baseline_stats is None:
                comparisons.append({**key, "status": "not_comparable", "reason": "subject_not_completed"})
                continue
            current_comparable, current_reason = execution_comparability(
                current_subject, subject_id, current_stats
            )
            baseline_comparable, baseline_reason = execution_comparability(
                baseline_subject, subject_id, baseline_stats
            )
            if not current_comparable or not baseline_comparable:
                comparisons.append(
                    {
                        **key,
                        "status": "not_comparable",
                        "reason": (
                            f"current_{current_reason}"
                            if not current_comparable
                            else f"baseline_{baseline_reason}"
                        ),
                        "current_reason": current_reason,
                        "baseline_reason": baseline_reason,
                    }
                )
                continue
            current_backend = current_subject["execution"]["actual_backend"]
            baseline_backend = baseline_subject["execution"]["actual_backend"]
            if current_backend != baseline_backend:
                comparisons.append(
                    {
                        **key,
                        "status": "not_comparable",
                        "reason": "actual_backend_changed",
                        "baseline_actual_backend": baseline_backend,
                        "current_actual_backend": current_backend,
                    }
                )
                continue

            delta = current_stats["median"] / baseline_stats["median"] - 1.0
            p95_delta = current_stats["p95"] / baseline_stats["p95"] - 1.0
            baseline_se = baseline_stats["standard_deviation"] / math.sqrt(
                baseline_stats["sample_count"]
            )
            current_se = current_stats["standard_deviation"] / math.sqrt(
                current_stats["sample_count"]
            )
            standard_error = math.hypot(baseline_se, current_se)
            difference = current_stats["median"] - baseline_stats["median"]
            significant = difference > 2.0 * standard_error
            comparison = {
                **key,
                "status": "regression" if delta > budget and significant else "pass",
                "actual_backend": current_backend,
                "baseline_median_ms": baseline_stats["median"],
                "current_median_ms": current_stats["median"],
                "delta_fraction": delta,
                "baseline_p95_ms": baseline_stats["p95"],
                "current_p95_ms": current_stats["p95"],
                "p95_delta_fraction": p95_delta,
                "pooled_standard_error_ms": standard_error,
                "statistically_credible": significant,
                "budget_fraction": budget,
            }
            comparisons.append(comparison)
            if comparison["status"] == "regression":
                violations.append(comparison)

    return {
        "schema": "pillow-rs/pipeline-performance-budget@1",
        "current_result": relative(current_path),
        "baseline_result": relative(baseline_path),
        "budget_fraction": budget,
        "comparison_count": len(comparisons),
        "comparable_count": sum(item["status"] in {"pass", "regression"} for item in comparisons),
        "not_comparable_count": sum(item["status"] == "not_comparable" for item in comparisons),
        "violation_count": len(violations),
        "violations": violations,
        "comparisons": comparisons,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--current", type=Path, default=DEFAULT_CURRENT)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--budget-percent", type=float, default=5.0)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    current_path = args.current.resolve()
    baseline_path = args.baseline.resolve()
    output_path = args.output.resolve()
    if not current_path.is_file():
        raise SystemExit(f"current result does not exist: {current_path}")
    if not baseline_path.is_file():
        raise SystemExit(f"baseline result does not exist: {baseline_path}")
    document = compare(current_path, baseline_path, args.budget_percent / 100.0)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(
        json.dumps(
            {
                "comparable": document["comparable_count"],
                "not_comparable": document["not_comparable_count"],
                "violations": document["violation_count"],
                "output": relative(output_path),
            },
            sort_keys=True,
        )
    )
    return 1 if args.check and document["violation_count"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
