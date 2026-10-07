#!/usr/bin/env python3
"""Flatten published benchmark snapshots into a complete operation matrix.

The CSV preserves every row, including execution-only, failed, not-run,
fallback, and dirty-snapshot observations. Only exact-parity rows with a clean
source and a completed matching-backend receipt receive verified speedup or
goal-status fields. Sustained throughput is not inferred from latency.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "docs/evidence/performance-optimization-matrix.csv"
DEFAULT_OPERATION_OUTPUT = ROOT / "docs/evidence/performance-optimization-operation-matrix.csv"
BENCHMARK_URL = "https://appunni-m.github.io/pillow-rs/assets/benchmark.json"
PILLOW_SIMD_URL = "https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json"
EXPECTED_REPOSITORY = "appunni-m/pillow-rs"

FIELDS = (
    "source_snapshot",
    "source_url",
    "source_schema",
    "source_json_sha256",
    "source_revision",
    "source_clean",
    "measured_at",
    "runner",
    "comparison_group",
    "workload",
    "scope",
    "operation",
    "public_apis",
    "input_kind",
    "operation_class",
    "mode",
    "width",
    "height",
    "chain_length",
    "cache_state",
    "build_profile",
    "profile",
    "status",
    "correctness",
    "correctness_gate",
    "measurement_boundary",
    "warmup_iterations",
    "measurement_iterations",
    "samples",
    "concurrency",
    "sample_unit",
    "sample_count",
    "median_us",
    "p90_us",
    "p95_us",
    "observed_speedup_vs_pillow",
    "verified_speedup_vs_pillow",
    "observed_speedup_vs_pillow_simd",
    "verified_speedup_vs_pillow_simd",
    "observed_gpu_latency_speedup_vs_simd",
    "verified_gpu_latency_speedup_vs_simd",
    "actual_backend",
    "requested_backend",
    "terminal_complete",
    "fallback_reasons",
    "evidence_status",
    "cpu_goal_status",
    "simd_2x_goal_status",
    "simd_5x_goal_status",
    "gpu_latency_goal_status",
    "gpu_sustained_throughput_status",
)

OPERATION_FIELDS = (
    "full_snapshot_revision",
    "full_snapshot_clean",
    "full_snapshot_measured_at",
    "full_snapshot_sha256",
    "pillow_simd_snapshot_revision",
    "pillow_simd_snapshot_clean",
    "pillow_simd_snapshot_measured_at",
    "pillow_simd_snapshot_sha256",
    "operation",
    "api_path",
    "target_api_paths",
    "kind",
    "classification",
    "profile",
    "baseline",
    "goal_threshold_speedup",
    "profile_applicability",
    "profile_status",
    "declared_profiles",
    "manifest_target_profiles",
    "workload_target_profiles",
    "benchmark_workloads",
    "published_workloads",
    "missing_published_workloads",
    "published_profile_rows",
    "verified_comparisons",
    "below_baseline_comparisons",
    "below_2x_comparisons",
    "below_5x_comparisons",
    "worst_verified_speedup",
    "worst_verified_workload",
    "worst_verified_runner",
    "execution_only_rows",
    "parity_not_proven_rows",
    "failed_rows",
    "not_run_rows",
    "fallback_rows",
    "wrong_backend_rows",
    "pillow_simd_snapshot_workloads",
    "verified_pillow_simd_comparisons",
    "worst_verified_speedup_vs_pillow_simd",
    "gpu_sustained_throughput_status",
    "benchmark_workload_ids",
)

PROFILES = ("python-cpu", "python-simd", "python-gpu", "python-parallel-cpu")


def read_snapshot(path: Path, name: str) -> tuple[dict[str, Any], str]:
    raw = path.read_bytes()
    snapshot = json.loads(raw)
    if snapshot.get("schema") not in {
        "public-docs/benchmark-snapshot@1",
        "public-docs/benchmark-snapshot@2",
    }:
        raise ValueError(f"{path}: unsupported public benchmark schema")
    if snapshot.get("repository") != EXPECTED_REPOSITORY:
        raise ValueError(f"{path}: unexpected repository identity")
    if not snapshot.get("revision") or not isinstance(snapshot.get("rows"), list):
        raise ValueError(f"{path}: snapshot is missing its revision or rows")
    if not snapshot["rows"]:
        raise ValueError(f"{path}: refusing to build a matrix from no rows")
    return {**snapshot, "_source_snapshot": name}, hashlib.sha256(raw).hexdigest()


def read_workload_specs() -> dict[str, dict[str, Any]]:
    specs: dict[str, dict[str, Any]] = {}
    input_root = ROOT / "pillow-rs/tests/fixtures/inputs/benchmark"
    for path in sorted(input_root.glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        for workload in document.get("workloads", []):
            workload_id = workload.get("workload_id")
            if not isinstance(workload_id, str):
                continue
            previous = specs.get(workload_id)
            if previous is not None and previous != workload:
                raise ValueError(f"duplicate workload specification: {workload_id}")
            specs[workload_id] = workload
    return specs


def read_operation_catalog(
    specs: dict[str, dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, str]]:
    manifest_path = ROOT / "pillow-rs/tests/fixtures/manifest.yaml"
    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    operations: dict[str, dict[str, Any]] = {}
    requirement_owner: dict[str, str] = {}
    for surface in manifest["surfaces"]:
        for operation in surface.get("operations", []):
            name = f"{surface['id']}.{operation['id']}"
            api_path = str(operation.get("source", {}).get("path") or name)
            declared_profiles: set[str] = set()
            requirements = []
            manifest_profiles: set[str] = set()
            for requirement in operation.get("requirements", []):
                requirement_id = requirement.get("id")
                if isinstance(requirement_id, str):
                    requirement_owner[requirement_id] = name
                    requirements.append(requirement_id)
                if requirement.get("dimension") == "performance":
                    manifest_profiles.update(requirement.get("target_profiles", []))
            operations[name] = {
                "operation": name,
                "api_path": api_path,
                "kind": str(operation.get("kind", "")),
                "classification": str(operation.get("classification", "")),
                "manifest_profiles": manifest_profiles,
                "workload_profiles": set(),
                "requirements": requirements,
                "workload_ids": [],
            }

    for workload_id, spec in specs.items():
        workload_profiles = {
            str(subject["id"])
            for subject in spec.get("subjects", [])
            if subject.get("kind") == "target_profile" and subject.get("id")
        }
        for requirement in spec.get("covers", []):
            owner = requirement_owner.get(str(requirement))
            if owner is not None:
                operations[owner]["workload_ids"].append(workload_id)
                operations[owner]["workload_profiles"].update(workload_profiles)
    for operation in operations.values():
        operation["workload_ids"] = sorted(set(operation["workload_ids"]))
    for operation in operations.values():
        operation["declared_profiles"] = sorted(
            operation["manifest_profiles"] | operation["workload_profiles"]
        )
    return operations, requirement_owner


def classify(row: dict[str, Any], source_clean: bool) -> str:
    if row is None:
        return "not_compared"
    if row.get("status") != "completed" or row.get("median_us") is None:
        return row.get("status") or "not_measured"
    if row.get("correctness") != "parity_pass: pass":
        if str(row.get("correctness", "")).endswith("not_proven"):
            return "parity_not_proven"
        if str(row.get("correctness", "")).startswith("successful_execution"):
            return "execution_only"
        return "correctness_not_proven"
    if row.get("policy", {}).get("correctness_gate") != "parity_pass":
        return "parity_gate_mismatch"
    if row.get("subject") in {"pillow", "pillow-simd"}:
        if not source_clean:
            return "dirty_pillow_reference"
        return "pillow-simd_reference" if row.get("subject") == "pillow-simd" else "pillow_reference"
    if not source_clean:
        return "dirty_snapshot"
    if row.get("fallback_reasons"):
        return "fallback"
    if row.get("actual_backend") != row.get("requested_backend"):
        return "wrong_or_unconfirmed_backend"
    if not row.get("terminal_complete"):
        return "terminal_not_complete"
    return "parity_backend_verified"


def compatible(left: dict[str, Any] | None, right: dict[str, Any] | None) -> bool:
    if left is None or right is None:
        return False
    return all(left.get(field) == right.get(field) for field in ("policy", "context", "sample_unit"))


def finite_latency(row: dict[str, Any] | None) -> float | None:
    value = row.get("median_us") if row else None
    return float(value) if isinstance(value, (int, float)) and value > 0 else None


def ratio(baseline: dict[str, Any] | None, target: dict[str, Any] | None) -> float | None:
    before, after = finite_latency(baseline), finite_latency(target)
    if before is None or after is None or not compatible(baseline, target):
        return None
    return before / after


def fmt(value: float | None) -> str:
    return "" if value is None else f"{value:.6f}"


def workload_scope(workload_id: str) -> str:
    if workload_id.startswith("pipeline-op."):
        return "operation"
    if workload_id.startswith("pipeline-lifecycle."):
        return "lifecycle"
    if workload_id.startswith("pipeline-matrix."):
        return "matrix"
    if workload_id.startswith("pipeline-chain.") or workload_id.startswith("pipeline.quick."):
        return "composed_pipeline"
    return "benchmark"


def workload_operation(
    workload_id: str,
    spec: dict[str, Any],
    requirement_owner: dict[str, str],
) -> str:
    if workload_id.startswith("pipeline-op."):
        apis = workload_apis(spec, requirement_owner).split(";")
        if apis and apis[0]:
            return apis[0].rsplit(".", 1)[-1]
        return workload_id.removeprefix("pipeline-op.").split(".", 1)[0]
    return "composed"


def workload_apis(spec: dict[str, Any], requirement_owner: dict[str, str]) -> str:
    prefixes = (
        "PIL.Image.Image.",
        "PIL.ImageOps.",
        "PIL.ImageChops.",
        "PIL.ImageEnhance.",
        "PIL.ImageColor.",
        "PIL.ImageFilter.",
        "PIL.ImageFont.",
        "PIL.ImageDraw.",
        "PIL.ImagePalette.",
        "PIL.ImageStat.",
        "PIL.ImageSequence.",
        "PIL.Image.",
    )
    apis = []
    for requirement in spec.get("covers", []):
        value = str(requirement)
        if value in requirement_owner:
            apis.append(requirement_owner[value])
            continue
        for prefix in prefixes:
            if value.startswith(prefix):
                apis.append(prefix[:-1] + "." + value[len(prefix):].split(".", 1)[0])
                break
        else:
            apis.append(value)
    return ";".join(dict.fromkeys(apis))


def pick_rows(snapshots: list[dict[str, Any]]) -> dict[tuple[str, str, str, str], dict[str, Any]]:
    indexed: dict[tuple[str, str, str, str], dict[str, Any]] = {}
    for snapshot in snapshots:
        source = snapshot["_source_snapshot"]
        default_machine = (snapshot.get("machines") or [snapshot.get("machine") or {}])
        default_id = default_machine[0].get("id") if len(default_machine) == 1 else None
        for row in snapshot["rows"]:
            machine_id = row.get("machine_id") or default_id
            if not machine_id:
                raise ValueError(f"{source}: row has no unambiguous runner: {row.get('workload')}")
            key = (source, str(machine_id), str(row.get("comparison_group", "default")), str(row["workload"]))
            subject = str(row["subject"])
            bucket = indexed.setdefault(key, {})
            if subject in bucket:
                raise ValueError(f"{source}: duplicate row for {key} subject {subject}")
            bucket[subject] = row
    return indexed


def goal_status(speedup: float | None, evidence: str, threshold: float) -> str:
    if speedup is not None:
        return "pass" if speedup >= threshold else "below_target"
    if evidence in {"execution_only", "parity_not_proven", "fallback", "dirty_snapshot"}:
        return evidence
    if evidence == "parity_backend_verified":
        return "not_compared"
    return "not_measured_or_receipt_missing"


def matrix_rows(
    snapshots: list[dict[str, Any]],
    hashes: dict[str, str],
    specs: dict[str, dict[str, Any]],
    requirement_owner: dict[str, str],
) -> list[dict[str, str]]:
    indexed = pick_rows(snapshots)
    source_by_name = {item["_source_snapshot"]: item for item in snapshots}
    output: list[dict[str, str]] = []
    for (source, runner, comparison_group, workload_id), subjects in sorted(indexed.items()):
        snapshot = source_by_name[source]
        source_clean = snapshot.get("clean") is True
        spec = specs.get(workload_id, {})
        pillow = subjects.get("pillow")
        pillow_simd = subjects.get("pillow-simd")
        simd = subjects.get("python-simd")
        for profile, row in sorted(subjects.items()):
            evidence = classify(row, source_clean)
            observed_pillow = ratio(pillow, row) if profile != "pillow" else None
            verified_pillow = (
                observed_pillow
                if evidence == "parity_backend_verified"
                and classify(pillow, source_clean) == "pillow_reference"
                else None
            )
            observed_pillow_simd = (
                ratio(pillow_simd, row)
                if profile == "python-simd" and source == "github-pages/pillow-simd-benchmark.json"
                else None
            )
            verified_pillow_simd = (
                observed_pillow_simd
                if evidence == "parity_backend_verified"
                and classify(pillow_simd, source_clean) == "pillow-simd_reference"
                else None
            )
            observed_gpu = (
                ratio(simd, row)
                if profile == "python-gpu"
                else None
            )
            verified_gpu = (
                observed_gpu
                if evidence == "parity_backend_verified"
                and classify(simd, source_clean) == "parity_backend_verified"
                else None
            )
            context = row.get("context", {})
            size = context.get("size", [])
            width = size[0] if len(size) > 0 else ""
            height = size[1] if len(size) > 1 else ""
            policy = row.get("policy", {})
            source_url = PILLOW_SIMD_URL if source.endswith("pillow-simd-benchmark.json") else BENCHMARK_URL
            output.append({
                "source_snapshot": source,
                "source_url": source_url,
                "source_schema": str(snapshot.get("schema", "")),
                "source_json_sha256": hashes[source],
                "source_revision": str(snapshot.get("revision", "")),
                "source_clean": str(source_clean).lower(),
                "measured_at": str(snapshot.get("measured_at", "")),
                "runner": runner,
                "comparison_group": comparison_group,
                "workload": workload_id,
                "scope": workload_scope(workload_id),
                "operation": workload_operation(workload_id, spec, requirement_owner),
                "public_apis": workload_apis(spec, requirement_owner),
                "input_kind": str(spec.get("input", {}).get("kind", "")),
                "mode": str(context.get("mode", "")),
                "width": str(width),
                "height": str(height),
                "operation_class": str(context.get("operation_class", "")),
                "chain_length": str(context.get("chain_length", "")),
                "cache_state": str(context.get("cache_state", policy.get("cache_state", ""))),
                "build_profile": str(context.get("build_profile", "")),
                "profile": profile,
                "status": str(row.get("status", "")),
                "correctness": str(row.get("correctness", "")),
                "correctness_gate": str(policy.get("correctness_gate", "")),
                "measurement_boundary": str(policy.get("boundary", "")),
                "warmup_iterations": str(policy.get("warmup_iterations", "")),
                "measurement_iterations": str(policy.get("measurement_iterations", "")),
                "samples": str(policy.get("samples", "")),
                "concurrency": str(policy.get("concurrency", "")),
                "sample_unit": str(row.get("sample_unit", "")),
                "sample_count": str(row.get("sample_count") or ""),
                "median_us": fmt(finite_latency(row)),
                "p90_us": fmt(float(row["p90_us"]) if isinstance(row.get("p90_us"), (int, float)) else None),
                "p95_us": fmt(float(row["p95_us"]) if isinstance(row.get("p95_us"), (int, float)) else None),
                "observed_speedup_vs_pillow": fmt(observed_pillow),
                "verified_speedup_vs_pillow": fmt(verified_pillow),
                "observed_speedup_vs_pillow_simd": fmt(observed_pillow_simd),
                "verified_speedup_vs_pillow_simd": fmt(verified_pillow_simd),
                "observed_gpu_latency_speedup_vs_simd": fmt(observed_gpu),
                "verified_gpu_latency_speedup_vs_simd": fmt(verified_gpu),
                "actual_backend": str(row.get("actual_backend") or ""),
                "requested_backend": str(row.get("requested_backend") or ""),
                "terminal_complete": str(row.get("terminal_complete", False)).lower(),
                "fallback_reasons": json.dumps(row.get("fallback_reasons", {}), sort_keys=True, separators=(",", ":")),
                "evidence_status": evidence,
                "cpu_goal_status": goal_status(verified_pillow, evidence, 1.0) if profile == "python-cpu" else "",
                "simd_2x_goal_status": goal_status(verified_pillow, evidence, 2.0) if profile == "python-simd" else "",
                "simd_5x_goal_status": goal_status(verified_pillow, evidence, 5.0) if profile == "python-simd" else "",
                "gpu_latency_goal_status": goal_status(verified_gpu, evidence, 1.0) if profile == "python-gpu" else "",
                "gpu_sustained_throughput_status": "not_measured" if profile == "python-gpu" else "",
            })
    return output


def operation_matrix_rows(
    operations: dict[str, dict[str, Any]],
    workload_rows: list[dict[str, str]],
    public_api_rows: list[dict[str, str]],
) -> list[dict[str, str]]:
    full_source = "github-pages/benchmark.json"
    pillow_simd_source = "github-pages/pillow-simd-benchmark.json"
    full_metadata = next(
        (row for row in workload_rows if row["source_snapshot"] == full_source), {}
    )
    pillow_simd_metadata = next(
        (row for row in workload_rows if row["source_snapshot"] == pillow_simd_source), {}
    )
    output: list[dict[str, str]] = []
    target_paths_by_operation: dict[str, list[str]] = {}
    for api in public_api_rows:
        operation = api.get("manifest_operation", "")
        if operation:
            target_paths_by_operation.setdefault(operation, []).append(api["public_path"])
    for operation_name, operation in sorted(operations.items()):
        workload_ids = operation["workload_ids"]
        published_workloads = {
            row["workload"]
            for row in workload_rows
            if row["source_snapshot"] == full_source and row["workload"] in workload_ids
        }
        missing_workloads = sorted(set(workload_ids) - published_workloads)
        for profile in PROFILES:
            if profile == "python-cpu":
                baseline, threshold = "Pillow", 1.0
                speedup_field = "verified_speedup_vs_pillow"
            elif profile == "python-simd":
                baseline, threshold = "Pillow", 2.0
                speedup_field = "verified_speedup_vs_pillow"
            elif profile == "python-gpu":
                baseline, threshold = "SIMD", 1.0
                speedup_field = "verified_gpu_latency_speedup_vs_simd"
            else:
                baseline, threshold = "Pillow", None
                speedup_field = "verified_speedup_vs_pillow"

            profile_rows = [
                row for row in workload_rows
                if row["source_snapshot"] == full_source
                and row["profile"] == profile
                and row["workload"] in workload_ids
            ]
            verified = []
            for row in profile_rows:
                value = row.get(speedup_field, "")
                if value:
                    verified.append((float(value), row))
            worst = min(verified, key=lambda item: item[0]) if verified else None
            manifest_applicable = profile in operation["manifest_profiles"]
            workload_applicable = profile in operation["workload_profiles"]
            if workload_applicable:
                applicability = "benchmark_workload"
            elif manifest_applicable:
                applicability = "manifest_performance_requirement"
            elif profile_rows:
                applicability = "published_rows_only"
            else:
                applicability = "not_declared"
            applicable = applicability != "not_declared"
            if not applicable:
                status = "not_declared_in_manifest_or_workload"
            elif not workload_ids:
                status = "no_benchmark_mapping"
            elif not published_workloads:
                status = "not_in_published_snapshot"
            elif not verified:
                status = "no_verified_comparison"
            elif threshold is None:
                status = "tracked_as_separate_profile"
            elif any(value < threshold for value, _ in verified):
                status = "target_unmet_in_verified_rows"
            else:
                status = "target_met_in_verified_rows"

            simd_rows = [
                row for row in workload_rows
                if row["source_snapshot"] == pillow_simd_source
                and row["profile"] == "python-simd"
                and row["workload"] in workload_ids
            ] if profile == "python-simd" else []
            simd_verified = [
                (float(row["verified_speedup_vs_pillow_simd"]), row)
                for row in simd_rows
                if row.get("verified_speedup_vs_pillow_simd", "")
            ]
            worst_simd = min(simd_verified, key=lambda item: item[0]) if simd_verified else None
            evidence_counts = {
                state: sum(row["evidence_status"] == state for row in profile_rows)
                for state in (
                    "execution_only",
                    "parity_not_proven",
                    "failed",
                    "not_run",
                    "fallback",
                    "wrong_or_unconfirmed_backend",
                )
            }
            output.append({
                "full_snapshot_revision": full_metadata.get("source_revision", ""),
                "full_snapshot_clean": full_metadata.get("source_clean", ""),
                "full_snapshot_measured_at": full_metadata.get("measured_at", ""),
                "full_snapshot_sha256": full_metadata.get("source_json_sha256", ""),
                "pillow_simd_snapshot_revision": pillow_simd_metadata.get("source_revision", ""),
                "pillow_simd_snapshot_clean": pillow_simd_metadata.get("source_clean", ""),
                "pillow_simd_snapshot_measured_at": pillow_simd_metadata.get("measured_at", ""),
                "pillow_simd_snapshot_sha256": pillow_simd_metadata.get("source_json_sha256", ""),
                "operation": operation_name,
                "api_path": operation["api_path"],
                "target_api_paths": ";".join(sorted(target_paths_by_operation.get(operation_name, []))),
                "kind": operation["kind"],
                "classification": operation["classification"],
                "profile": profile,
                "baseline": baseline,
                "goal_threshold_speedup": "" if threshold is None else f"{threshold:.1f}",
                "profile_applicability": applicability,
                "profile_status": status,
                "declared_profiles": ";".join(operation["declared_profiles"]),
                "manifest_target_profiles": ";".join(sorted(operation["manifest_profiles"])),
                "workload_target_profiles": ";".join(sorted(operation["workload_profiles"])),
                "benchmark_workloads": str(len(workload_ids)),
                "published_workloads": str(len(published_workloads)),
                "missing_published_workloads": ";".join(missing_workloads),
                "published_profile_rows": str(len(profile_rows)),
                "verified_comparisons": str(len(verified)),
                "below_baseline_comparisons": str(sum(value < 1.0 for value, _ in verified)),
                "below_2x_comparisons": str(sum(value < 2.0 for value, _ in verified)),
                "below_5x_comparisons": str(sum(value < 5.0 for value, _ in verified)) if profile == "python-simd" else "",
                "worst_verified_speedup": fmt(worst[0] if worst else None),
                "worst_verified_workload": worst[1]["workload"] if worst else "",
                "worst_verified_runner": worst[1]["runner"] if worst else "",
                "execution_only_rows": str(evidence_counts["execution_only"]),
                "parity_not_proven_rows": str(evidence_counts["parity_not_proven"]),
                "failed_rows": str(evidence_counts["failed"]),
                "not_run_rows": str(evidence_counts["not_run"]),
                "fallback_rows": str(evidence_counts["fallback"]),
                "wrong_backend_rows": str(evidence_counts["wrong_or_unconfirmed_backend"]),
                "pillow_simd_snapshot_workloads": str(len({row["workload"] for row in simd_rows})),
                "verified_pillow_simd_comparisons": str(len(simd_verified)),
                "worst_verified_speedup_vs_pillow_simd": fmt(worst_simd[0] if worst_simd else None),
                "gpu_sustained_throughput_status": "not_measured" if profile == "python-gpu" else "",
                "benchmark_workload_ids": ";".join(workload_ids),
            })

    # The manifest defines the selected correctness/performance contract. Keep
    # every other public PIL path visible as an explicit, unbenchmarked row so
    # the operation matrix does not imply coverage of only the selected subset.
    for api in public_api_rows:
        if api.get("manifest_status") == "selected_manifest":
            continue
        path = api["public_path"]
        for profile in PROFILES:
            if profile == "python-cpu":
                baseline, threshold = "Pillow", 1.0
            elif profile == "python-simd":
                baseline, threshold = "Pillow", 2.0
            elif profile == "python-gpu":
                baseline, threshold = "SIMD", 1.0
            else:
                baseline, threshold = "Pillow", None
            output.append({
                "full_snapshot_revision": full_metadata.get("source_revision", ""),
                "full_snapshot_clean": full_metadata.get("source_clean", ""),
                "full_snapshot_measured_at": full_metadata.get("measured_at", ""),
                "full_snapshot_sha256": full_metadata.get("source_json_sha256", ""),
                "pillow_simd_snapshot_revision": pillow_simd_metadata.get("source_revision", ""),
                "pillow_simd_snapshot_clean": pillow_simd_metadata.get("source_clean", ""),
                "pillow_simd_snapshot_measured_at": pillow_simd_metadata.get("measured_at", ""),
                "pillow_simd_snapshot_sha256": pillow_simd_metadata.get("source_json_sha256", ""),
                "operation": f"public-api.{path}",
                "api_path": path,
                "target_api_paths": path,
                "kind": api["entry_kind"],
                "classification": "outside_selected_manifest",
                "profile": profile,
                "baseline": baseline,
                "goal_threshold_speedup": "" if threshold is None else f"{threshold:.1f}",
                "profile_applicability": "public_api_inventory_gap",
                "profile_status": "not_mapped_to_benchmark",
                "declared_profiles": "",
                "manifest_target_profiles": "",
                "workload_target_profiles": "",
                "benchmark_workloads": "0",
                "published_workloads": "0",
                "missing_published_workloads": "",
                "published_profile_rows": "0",
                "verified_comparisons": "0",
                "below_baseline_comparisons": "0",
                "below_2x_comparisons": "0",
                "below_5x_comparisons": "" if profile != "python-simd" else "0",
                "worst_verified_speedup": "",
                "worst_verified_workload": "",
                "worst_verified_runner": "",
                "execution_only_rows": "0",
                "parity_not_proven_rows": "0",
                "failed_rows": "0",
                "not_run_rows": "0",
                "fallback_rows": "0",
                "wrong_backend_rows": "0",
                "pillow_simd_snapshot_workloads": "0",
                "verified_pillow_simd_comparisons": "0",
                "worst_verified_speedup_vs_pillow_simd": "",
                "gpu_sustained_throughput_status": "not_measured" if profile == "python-gpu" else "",
                "benchmark_workload_ids": "",
            })
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--benchmark-snapshot", type=Path, required=True,
                        help="Downloaded GitHub Pages assets/benchmark.json")
    parser.add_argument("--pillow-simd-snapshot", type=Path,
                        help="Downloaded GitHub Pages assets/pillow-simd-benchmark.json")
    parser.add_argument("--public-api-inventory", type=Path,
                        default=ROOT / "docs/evidence/performance-optimization-public-api.csv",
                        help="Public PIL API census generated by build_public_api_inventory.py")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--operation-output", type=Path, default=DEFAULT_OPERATION_OUTPUT)
    args = parser.parse_args()

    snapshots: list[dict[str, Any]] = []
    hashes: dict[str, str] = {}
    full, hashes["github-pages/benchmark.json"] = read_snapshot(
        args.benchmark_snapshot, "github-pages/benchmark.json"
    )
    snapshots.append(full)
    if args.pillow_simd_snapshot:
        simd, hashes["github-pages/pillow-simd-benchmark.json"] = read_snapshot(
            args.pillow_simd_snapshot, "github-pages/pillow-simd-benchmark.json"
        )
        snapshots.append(simd)
    specs = read_workload_specs()
    operations, requirement_owner = read_operation_catalog(specs)
    rows = matrix_rows(snapshots, hashes, specs, requirement_owner)
    with args.public_api_inventory.open(encoding="utf-8", newline="") as stream:
        public_api_rows = list(csv.DictReader(stream))
    if not public_api_rows or not {"public_path", "entry_kind", "manifest_status"}.issubset(public_api_rows[0]):
        raise ValueError(f"{args.public_api_inventory}: not a public API inventory CSV")
    public_manifest_operations = {
        row["manifest_operation"]
        for row in public_api_rows
        if row.get("manifest_status") == "selected_manifest" and row.get("manifest_operation")
    }
    if public_manifest_operations != set(operations):
        missing = sorted(set(operations) - public_manifest_operations)
        unexpected = sorted(public_manifest_operations - set(operations))
        raise ValueError(
            "public API inventory does not map every selected operation "
            f"(missing={missing}, unexpected={unexpected})"
        )
    operation_rows = operation_matrix_rows(operations, rows, public_api_rows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=FIELDS, extrasaction="raise", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    args.operation_output.parent.mkdir(parents=True, exist_ok=True)
    with args.operation_output.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=OPERATION_FIELDS, extrasaction="raise", lineterminator="\n")
        writer.writeheader()
        writer.writerows(operation_rows)

    workload_counts = {
        snapshot["_source_snapshot"]: len({row["workload"] for row in snapshot["rows"]})
        for snapshot in snapshots
    }
    print(json.dumps({
        "output": str(args.output),
        "rows": len(rows),
        "operation_output": str(args.operation_output),
        "operation_rows": len(operation_rows),
        "operations": len(operations),
        "public_api_entries": len(public_api_rows),
        "public_api_inventory_gaps": sum(
            row.get("manifest_status") != "selected_manifest" for row in public_api_rows
        ),
        "workloads": workload_counts,
        "source_revisions": {snapshot["_source_snapshot"]: snapshot["revision"] for snapshot in snapshots},
        "source_clean": {snapshot["_source_snapshot"]: snapshot.get("clean") is True for snapshot in snapshots},
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
