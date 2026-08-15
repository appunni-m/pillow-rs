#!/usr/bin/env python3
"""Generate a strict, machine-readable report from a pipeline benchmark.

This report is performance evidence, not an LLVM coverage report and not a
parity denominator.  Every selected workload and every subject is retained;
unsupported or failed subjects remain visible in ``status_counts`` and in the
per-workload receipt.  The operation denominator is delegated to the same
authoritative audit used by ``report_pipeline_benchmark_coverage.py``.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))

from report_pipeline_benchmark_coverage import (  # noqa: E402
    DEFAULT_INPUT,
    report as report_workload_coverage,
)


DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "benchmark-result.json"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "pipeline-performance-report.json"


def percentile_summary(measurements: list[dict[str, Any]], metric: str) -> dict[str, Any] | None:
    for measurement in measurements:
        if measurement.get("metric") != metric:
            continue
        statistics = measurement.get("statistics")
        if not isinstance(statistics, dict):
            return None
        return {
            "sample_count": measurement.get("sample_count"),
            "unit": measurement.get("unit"),
            "min": statistics.get("min"),
            "median": statistics.get("median"),
            "mean": statistics.get("mean"),
            "p95": statistics.get("p95"),
            "p99": statistics.get("p99"),
            "max": statistics.get("max"),
            "standard_deviation": statistics.get("standard_deviation"),
        }
    return None


def execution_summary(subject: dict[str, Any]) -> dict[str, Any]:
    execution = subject.get("execution")
    if not isinstance(execution, dict):
        return {}
    resource = execution.get("resource")
    resource_summary: dict[str, Any] = {}
    if isinstance(resource, dict):
        for name, value in resource.items():
            if not isinstance(value, dict):
                continue
            resource_summary[name] = {
                "sample_count": value.get("sample_count"),
                "median": value.get("median"),
                "mean": value.get("mean"),
                "max": value.get("max"),
                "total": value.get("total"),
            }
    dispatch = execution.get("dispatch_count")
    operation_count = execution.get("operation_count")
    return {
        "status": execution.get("status"),
        "requested_backend": execution.get("requested_backend"),
        "actual_backend": execution.get("actual_backend"),
        "actual_backend_counts": execution.get("actual_backend_counts", {}),
        "fallback_reason_counts": execution.get("fallback_reason_counts", {}),
        "operation_count": operation_count,
        "dispatch_count": dispatch,
        "resize_coeff_cache_hits": execution.get("resize_coeff_cache_hits"),
        "resize_coeff_cache_misses": execution.get("resize_coeff_cache_misses"),
        "phase_timings_ns": execution.get("phase_timings_ns", {}),
        "resource": resource_summary,
        "sample_count": execution.get("sample_count"),
        "cached_sample_count": execution.get("cached_sample_count"),
    }


def subject_summary(subject: dict[str, Any]) -> dict[str, Any]:
    measurements = subject.get("measurements")
    if not isinstance(measurements, list):
        measurements = []
    return {
        "kind": subject.get("kind"),
        "id": subject.get("id"),
        "status": subject.get("status"),
        "latency": percentile_summary(measurements, "latency"),
        "throughput": percentile_summary(measurements, "throughput"),
        "execution": execution_summary(subject),
    }


def workload_summary(workload: dict[str, Any]) -> dict[str, Any]:
    subjects = workload.get("subjects")
    if not isinstance(subjects, list):
        subjects = []
    return {
        "workload_id": workload.get("workload_id"),
        "context": workload.get("context", {}),
        "measurement_policy": workload.get("measurement_policy", {}),
        "correctness": workload.get("correctness", {}),
        "subjects": [subject_summary(subject) for subject in subjects],
    }


def subject_status_counts(workloads: list[dict[str, Any]]) -> dict[str, dict[str, int]]:
    counts: dict[str, Counter[str]] = {}
    for workload in workloads:
        for subject in workload.get("subjects", []):
            subject_id = str(subject.get("id", "unknown"))
            counts.setdefault(subject_id, Counter())[str(subject.get("status"))] += 1
    return {subject_id: dict(sorted(counter.items())) for subject_id, counter in sorted(counts.items())}


def baseline_deltas(
    current: list[dict[str, Any]], baseline: list[dict[str, Any]] | None
) -> list[dict[str, Any]] | None:
    if baseline is None:
        return None
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    for workload in baseline:
        for subject in workload.get("subjects", []):
            by_key[(str(workload.get("workload_id")), str(subject.get("id")))] = subject

    deltas: list[dict[str, Any]] = []
    for workload in current:
        workload_id = str(workload.get("workload_id"))
        for subject in workload.get("subjects", []):
            subject_id = str(subject.get("id"))
            previous = by_key.get((workload_id, subject_id))
            current_latency = percentile_summary(subject.get("measurements", []), "latency")
            previous_latency = (
                percentile_summary(previous.get("measurements", []), "latency")
                if previous is not None
                else None
            )
            if current_latency is None or previous_latency is None:
                deltas.append(
                    {
                        "workload_id": workload_id,
                        "subject_id": subject_id,
                        "status": "not_comparable",
                    }
                )
                continue
            deltas.append(
                {
                    "workload_id": workload_id,
                    "subject_id": subject_id,
                    "status": "comparable",
                    "current_median_ms": current_latency["median"],
                    "baseline_median_ms": previous_latency["median"],
                    "delta_median_ms": current_latency["median"] - previous_latency["median"],
                    "delta_fraction": (
                        current_latency["median"] / previous_latency["median"] - 1
                        if previous_latency["median"]
                        else None
                    ),
                }
            )
    return deltas


def build_report(result_path: Path, baseline_path: Path | None) -> dict[str, Any]:
    result_path = result_path.resolve()
    if baseline_path is not None:
        baseline_path = baseline_path.resolve()
    document = json.loads(result_path.read_text(encoding="utf-8"))
    if document.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError(f"unexpected benchmark schema: {document.get('schema')!r}")
    workloads = document.get("workloads")
    if not isinstance(workloads, list):
        raise ValueError("benchmark result has no workload list")

    baseline_workloads = None
    if baseline_path is not None:
        baseline_document = json.loads(baseline_path.read_text(encoding="utf-8"))
        if baseline_document.get("schema") != "migration-parity/benchmark-result@1":
            raise ValueError("baseline has an incompatible benchmark schema")
        baseline_workloads = baseline_document.get("workloads")
        if not isinstance(baseline_workloads, list):
            raise ValueError("baseline benchmark result has no workload list")

    coverage = report_workload_coverage(DEFAULT_INPUT, result_path)
    return {
        "schema": "pillow-rs/pipeline-performance-report@1",
        "source_result": str(result_path.resolve().relative_to(ROOT.resolve())),
        "baseline_result": (
            str(baseline_path.resolve().relative_to(ROOT.resolve()))
            if baseline_path is not None
            else None
        ),
        "denominator": {
            "pipeline_workloads": len(workloads),
            "operation_variants_total": coverage["operation_variants_total"],
            "operation_variants_benchmarked": coverage["operation_variants_benchmarked"],
            "operation_coverage_percent": coverage["operation_coverage_percent"],
            "missing_operation_workloads": coverage["missing_operation_workloads"],
            "unexpected_operation_workloads": coverage["unexpected_operation_workloads"],
            "duplicate_workload_ids": coverage["duplicate_workload_ids"],
            "context_complete_workloads": coverage["context_complete_workloads"],
            "context_missing_workloads": coverage["context_missing_workloads"],
            "operation_execution_by_subject": coverage.get("execution", {}).get(
                "operation_status_by_subject", {}
            ),
        },
        "status_counts": subject_status_counts(workloads),
        "correctness_outcomes": dict(
            sorted(
                Counter(
                    str(workload.get("correctness", {}).get("outcome"))
                    for workload in workloads
                ).items()
            )
        ),
        "workloads": [workload_summary(workload) for workload in workloads],
        "baseline_deltas": baseline_deltas(workloads, baseline_workloads),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--result", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    result = build_report(args.result, args.baseline)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(
        json.dumps(
            {
                "output": str(args.output),
                "pipeline_workloads": result["denominator"]["pipeline_workloads"],
                "operation_coverage_percent": result["denominator"]["operation_coverage_percent"],
                "status_counts": result["status_counts"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
