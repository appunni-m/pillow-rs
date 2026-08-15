#!/usr/bin/env python3
"""Report the maintained PipelineOp benchmark workload coverage.

This is a benchmark-input audit, not an LLVM coverage denominator.  It checks
that every PipelineOp variant has exactly one operation-matrix workload and
reports the separately measured composition-workflow population.  When the
managed benchmark result exists, it also reports successful execution by
subject; unsupported backend operations remain visible in that receipt.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

try:
    from build_migration_parity_inputs import PIPELINE_OP_BENCHMARK_SPECS, slug
except ModuleNotFoundError:  # imported as ``scripts...`` by tooling
    from scripts.build_migration_parity_inputs import (
        PIPELINE_OP_BENCHMARK_SPECS,
        slug,
    )


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT = (
    ROOT
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "inputs"
    / "benchmark"
    / "pipeline-operations.json"
)
DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "benchmark-result-pipeline-allops-20260812.json"


def pipeline_op_variants() -> set[str]:
    """Read the authoritative top-level ``PipelineOp`` variant names."""

    source = (ROOT / "pillow-rs" / "src" / "pipeline.rs").read_text(
        encoding="utf-8"
    )
    enum_start = source.index("pub enum PipelineOp {")
    body_start = source.index("{", enum_start) + 1
    depth = 1
    body_end = body_start
    while depth:
        if source[body_end] == "{":
            depth += 1
        elif source[body_end] == "}":
            depth -= 1
        body_end += 1
    body = source[body_start : body_end - 1]
    return {
        match.group(1)
        for match in re.finditer(
            r"^    ([A-Z][A-Za-z0-9_]*)\s*(?:\{|,)", body, re.MULTILINE
        )
    }


def report(path: Path, result_path: Path | None = None) -> dict[str, object]:
    document = json.loads(path.read_text(encoding="utf-8"))
    workloads = document["workloads"]
    source_variants = pipeline_op_variants()
    spec_variants = set(PIPELINE_OP_BENCHMARK_SPECS)
    expected_ids = {
        f"pipeline-op.{slug(variant)}.benchmark-materialized"
        for variant in spec_variants
    }
    operation_items = [
        item
        for item in workloads
        if item["workload_id"].startswith("pipeline-op.")
    ]
    base_operation_items = [
        item
        for item in operation_items
        if item["workload_id"].endswith(".benchmark-materialized")
    ]
    matrix_operation_items = [
        item
        for item in operation_items
        if item["workload_id"].endswith(".matrix-32x24")
    ]
    expanded_matrix_items = [
        item
        for item in workloads
        if item["workload_id"].startswith("pipeline-matrix.expanded.")
    ]
    long_point_chain_items = [
        item
        for item in workloads
        if item["workload_id"].startswith("pipeline-chain.long-point.")
    ]
    actual_ids = {
        item["workload_id"]
        for item in base_operation_items
    }
    all_ids = [item["workload_id"] for item in workloads]
    duplicate_ids = sorted(
        workload_id for workload_id in set(all_ids) if all_ids.count(workload_id) != 1
    )
    composition_count = sum(
        item["workload_id"].startswith("pipeline-chain.") for item in workloads
    )
    lifecycle_items = [
        item
        for item in workloads
        if item["workload_id"].startswith("pipeline-lifecycle.")
    ]
    quick_items = [
        item
        for item in workloads
        if item["workload_id"].startswith("pipeline.quick.")
    ]
    covered = expected_ids & actual_ids
    required_context_keys = {
        "size",
        "mode",
        "chain_length",
        "operation_class",
        "cache_state",
        "build_profile",
    }
    context_complete = [
        item
        for item in workloads
        if isinstance(item.get("context"), dict)
        and set(item["context"]) == required_context_keys
    ]
    operation_classes = sorted(
        {
            item["context"]["operation_class"]
            for item in operation_items
            if isinstance(item.get("context"), dict)
        }
    )
    matrix_context_items = [
        *operation_items,
        *[
            item
            for item in workloads
            if item["workload_id"].endswith(".matrix-32x24")
        ],
        *expanded_matrix_items,
    ]
    sizes = sorted(
        {
            tuple(item["context"]["size"])
            for item in matrix_context_items
            if isinstance(item.get("context"), dict)
        }
    )
    modes = sorted(
        {
            item["context"]["mode"]
            for item in matrix_context_items
            if isinstance(item.get("context"), dict)
        }
    )
    report: dict[str, object] = {
        "schema": "pillow-rs/pipeline-benchmark-coverage@1",
        "input": str(path.relative_to(ROOT)),
        "source_pipeline_op_variants": len(source_variants),
        "benchmark_spec_variants": len(spec_variants),
        "missing_benchmark_specs": sorted(source_variants - spec_variants),
        "unexpected_benchmark_specs": sorted(spec_variants - source_variants),
        "operation_variants_total": len(source_variants),
        "operation_variants_benchmarked": len(covered),
        "operation_coverage_percent": 100.0 * len(covered) / len(source_variants),
        "composition_workflows": composition_count,
        "lifecycle_workflows": len(lifecycle_items),
        "quick_workflows": len(quick_items),
        "lifecycle_cache_states": sorted(
            {
                item["context"]["cache_state"]
                for item in lifecycle_items
                if isinstance(item.get("context"), dict)
            }
        ),
        "size_matrix_workflows": len(matrix_operation_items) + len(expanded_matrix_items),
        "expanded_size_matrix_workflows": len(expanded_matrix_items),
        "long_point_chain_workflows": len(long_point_chain_items),
        "long_point_chain_lengths": sorted(
            item["context"]["chain_length"]
            for item in long_point_chain_items
            if isinstance(item.get("context"), dict)
        ),
        "pipeline_workloads_total": len(workloads),
        "missing_operation_workloads": sorted(expected_ids - actual_ids),
        "unexpected_operation_workloads": sorted(actual_ids - expected_ids),
        "duplicate_workload_ids": duplicate_ids,
        "context_complete_workloads": len(context_complete),
        "context_missing_workloads": sorted(
            item["workload_id"]
            for item in workloads
            if item not in context_complete
        ),
        "operation_classes": operation_classes,
        "operation_sizes": [list(size) for size in sizes],
        "operation_modes": modes,
    }
    if result_path is not None and result_path.is_file():
        result = json.loads(result_path.read_text(encoding="utf-8"))
        operation_workloads = [
            item
            for item in result.get("workloads", [])
            if item.get("workload_id", "").endswith(".benchmark-materialized")
        ]
        matrix_workloads = [
            item
            for item in result.get("workloads", [])
            if item.get("workload_id", "").endswith(".matrix-32x24")
        ]
        expanded_matrix_results = [
            item
            for item in result.get("workloads", [])
            if item.get("workload_id", "").startswith("pipeline-matrix.expanded.")
        ]
        quick_results = [
            item
            for item in result.get("workloads", [])
            if item.get("workload_id", "").startswith("pipeline.quick.")
        ]
        long_point_chain_results = [
            item
            for item in result.get("workloads", [])
            if item.get("workload_id", "").startswith("pipeline-chain.long-point.")
        ]
        subjects = ["pillow", "python-cpu", "python-simd", "python-gpu"]
        operation_status_by_subject: dict[str, dict[str, object]] = {}
        for subject_id in subjects:
            completed_ids = sorted(
                item["workload_id"]
                for item in operation_workloads
                for subject in item.get("subjects", [])
                if subject.get("id") == subject_id
                and subject.get("status") == "completed"
            )
            incomplete_ids = sorted(
                item["workload_id"]
                for item in operation_workloads
                for subject in item.get("subjects", [])
                if subject.get("id") == subject_id
                and subject.get("status") != "completed"
            )
            operation_status_by_subject[subject_id] = {
                "selected": len(operation_workloads),
                "completed": len(completed_ids),
                "incomplete": len(incomplete_ids),
                "incomplete_workload_ids": incomplete_ids,
            }
        report["execution"] = {
            "artifact": str(result_path.relative_to(ROOT)),
            "operation_workloads_selected": len(operation_workloads),
            "size_matrix_workloads_selected": len(matrix_workloads),
            "expanded_size_matrix_workloads_selected": len(expanded_matrix_results),
            "quick_workloads_selected": len(quick_results),
            "quick_workloads_all_subjects_completed": sum(
                all(
                    subject.get("id") in subjects
                    and subject.get("status") == "completed"
                    for subject in item.get("subjects", [])
                )
                for item in quick_results
            ),
            "long_point_chain_workloads_selected": len(long_point_chain_results),
            "long_point_chain_workloads_all_subjects_completed": sum(
                all(
                    subject.get("id") in subjects
                    and subject.get("status") == "completed"
                    for subject in item.get("subjects", [])
                )
                for item in long_point_chain_results
            ),
            "operation_workloads_all_subjects_completed": sum(
                all(
                    subject.get("id") in subjects
                    and subject.get("status") == "completed"
                    for subject in item.get("subjects", [])
                )
                for item in operation_workloads
            ),
            "size_matrix_workloads_all_subjects_completed": sum(
                all(
                    subject.get("id") in subjects
                    and subject.get("status") == "completed"
                    for subject in item.get("subjects", [])
                )
                for item in matrix_workloads
            ),
            "expanded_size_matrix_workloads_all_subjects_completed": sum(
                all(
                    subject.get("id") in subjects
                    and subject.get("status") == "completed"
                    for subject in item.get("subjects", [])
                )
                for item in expanded_matrix_results
            ),
            "completed_by_subject": {
                subject_id: sum(
                    any(
                        subject.get("id") == subject_id
                        and subject.get("status") == "completed"
                        for subject in item.get("subjects", [])
                    )
                    for item in operation_workloads
                )
                for subject_id in subjects
            },
            "operation_status_by_subject": operation_status_by_subject,
        }
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--result", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = report(args.input.resolve(), args.result.resolve())
    if args.output is not None:
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps(result, sort_keys=True))
    return 0 if (
        not result["missing_operation_workloads"]
        and not result["unexpected_operation_workloads"]
        and not result["duplicate_workload_ids"]
        and not result["context_missing_workloads"]
        and not result["missing_benchmark_specs"]
        and not result["unexpected_benchmark_specs"]
        and result["operation_variants_benchmarked"]
        == result["operation_variants_total"]
    ) else 1


if __name__ == "__main__":
    raise SystemExit(main())
