#!/usr/bin/env python3
"""Keep every declared operation visible against the user's performance goals.

This reads benchmark/parity artifacts only. It does not collect coverage, alter
workloads, or treat successful execution or reciprocal latency as parity or
sustained-throughput evidence. Ratios remain diagnostic until the associated
input, parity, backend, and provenance checks pass.
"""

from __future__ import annotations

import argparse
from collections import Counter
import gzip
import hashlib
import json
import math
from pathlib import Path
from typing import Any

import yaml

try:
    from run_migration_benchmark import suite_subject_is_comparable
except ModuleNotFoundError:
    from scripts.run_migration_benchmark import suite_subject_is_comparable


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "pillow-rs/tests/fixtures/manifest.yaml"
PROFILES = ("python-cpu", "python-simd", "python-gpu")


def read_json(path: Path) -> dict[str, Any]:
    # Some older receipts used a .gz suffix without compression. Inspect the
    # bytes instead of assuming that the historical filename is accurate.
    with path.open("rb") as stream:
        compressed = stream.read(2) == b"\x1f\x8b"
    opener = gzip.open if compressed else open
    with opener(path, "rt", encoding="utf-8") as stream:
        return json.load(stream)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def input_issues(identity: dict[str, Any], manifest: Path) -> list[str]:
    issues = []
    if identity.get("manifest", {}).get("sha256") != digest(manifest):
        issues.append("manifest_hash_mismatch")
    for item in identity.get("inputs", []):
        path = manifest.parent / item["path"]
        if not path.is_file() or digest(path) != item.get("sha256"):
            issues.append(f"input_hash_mismatch:{item['path']}")
    return issues


def latency(subject: dict[str, Any]) -> float | None:
    if subject.get("status") != "completed":
        return None
    for measurement in subject.get("measurements", []):
        value = measurement.get("statistics", {}).get("median")
        if (measurement.get("metric") == "latency"
                and measurement.get("unit") == "millisecond"
                and isinstance(value, (int, float)) and math.isfinite(value) and value > 0):
            return float(value)
    return None


def ratio(numerator: float | None, denominator: float | None) -> float | None:
    return numerator / denominator if numerator is not None and denominator else None


def assess_workload(
    spec: dict[str, Any], result: dict[str, Any] | None,
    parity: dict[tuple[str, str], str], parity_id: str | None,
    artifact_issues: list[str],
) -> dict[str, Any]:
    result = result or {}
    subjects = {item["id"]: item for item in result.get("subjects", [])}
    times = {name: latency(subjects.get(name, {})) for name in ("pillow", *PROFILES)}
    observed = {
        "cpu_over_pillow": ratio(times["python-cpu"], times["pillow"]),
        "simd_speedup_over_pillow": ratio(times["pillow"], times["python-simd"]),
        "gpu_over_simd": ratio(times["python-gpu"], times["python-simd"]),
    }
    common_issues = list(artifact_issues)
    if result.get("measurement_policy") != spec["measurement"]:
        common_issues.append("missing_or_changed_measurement_policy")
    if result.get("context") != spec["context"]:
        common_issues.append("missing_or_changed_context")
    if spec["measurement"]["cache_state"] == "resident":
        common_issues.append("resident_boundary_does_not_prove_fresh_end_to_end_latency")
    if not suite_subject_is_comparable(subjects.get("pillow", {}), "pillow"):
        common_issues.append("missing_oracle_timing")
    case_id = spec["input"].get("case_id")
    states = {}
    for profile in PROFILES:
        reasons = list(common_issues)
        outcome = parity.get((case_id, profile)) if case_id else None
        if outcome != "pass":
            reasons.append("parity_failed" if outcome == "fail" else "matching_output_not_proven")
        if result.get("correctness", {}).get("evidence_id") != parity_id or parity_id is None:
            reasons.append("unlinked_parity_evidence")
        subject = subjects.get(profile, {})
        expected_samples = (spec["measurement"].get("measurement_iterations", 0)
                            * spec["measurement"].get("samples", 0))
        if not expected_samples or not any(
            item.get("metric") == "latency" and item.get("sample_count") == expected_samples
            for item in subject.get("measurements", [])
        ):
            reasons.append("incomplete_declared_samples")
        if not suite_subject_is_comparable(subject, profile):
            reasons.append("requested_backend_execution_not_proven")
        if times[profile] is None:
            reasons.append("missing_latency")
        target_ms = times["pillow"] if profile == "python-cpu" else (
            times["pillow"] / 5 if profile == "python-simd" and times["pillow"] else
            times["python-simd"] if profile == "python-gpu" else None
        )
        if profile == "python-gpu" and not suite_subject_is_comparable(
            subjects.get("python-simd", {}), "python-simd"
        ):
            reasons.append("simd_comparator_execution_not_proven")
        if profile == "python-gpu" and parity.get((case_id, "python-simd")) != "pass":
            reasons.append("simd_comparator_parity_not_proven")
        meets = times[profile] <= target_ms if times[profile] is not None and target_ms else None
        states[profile] = {
            "latency_ms": times[profile], "target_ms": target_ms,
            "observed_target_met": meets,
            "status": "not_proven" if reasons else "sample_meets_target" if meets else "miss",
            "reasons": sorted(set(reasons)),
            "parity": outcome or "not_proven",
            "execution": subject.get("execution", {}),
        }
    factors = [observed["cpu_over_pillow"], observed["gpu_over_simd"]]
    if observed["simd_speedup_over_pillow"]:
        factors.append(5 / observed["simd_speedup_over_pillow"])
    return {
        "workload_id": spec["workload_id"], "requirements": spec["covers"],
        "context": spec["context"], "measurement_policy": spec["measurement"],
        "oracle_latency_ms": times["pillow"], "observed_ratios": observed,
        "subjects": states,
        "sustained_gpu_throughput": {
            "status": "not_proven",
            "reason": "Benchmark throughput is reciprocal latency; requires completed-work windows and concurrency evidence.",
        },
        "worst_observed_target_factor": max((v for v in factors if v is not None), default=None),
    }


def build_report(manifest_path: Path, result_path: Path | None, parity_path: Path | None) -> dict[str, Any]:
    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    operations = {}
    requirement_owner = {}
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            name = f"{surface['id']}.{operation['id']}"
            operations[name] = {
                "operation": name, "kind": operation["kind"],
                "classification": operation["classification"], "workload_ids": [],
            }
            requirement_owner.update({item["id"]: name for item in operation["requirements"]})
    specs = {}
    spec_paths = {}
    for relative in manifest["input_index"]["benchmark"]:
        for workload in read_json(manifest_path.parent / relative)["workloads"]:
            key = workload["workload_id"]
            if key in specs:
                raise ValueError(f"duplicate workload: {key}")
            specs[key] = workload
            spec_paths[key] = relative
            for owner in {requirement_owner[item] for item in workload["covers"]}:
                operations[owner]["workload_ids"].append(key)
    result = read_json(result_path) if result_path else {}
    if result and result.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError("unexpected benchmark schema")
    identity = result.get("identity", {})
    issues = input_issues(identity, manifest_path) if result else ["missing_benchmark_artifact"]
    if any(item.get("dirty") is not False for item in identity.get("targets", [])):
        issues.append("dirty_build_provenance_requires_review")
    target_identities = {item["target_profile"]: item for item in identity.get("targets", [])}
    if set(target_identities) != set(PROFILES):
        issues.append("missing_target_identity")
    by_id = {}
    for workload in result.get("workloads", []):
        key = workload["workload_id"]
        if key in by_id:
            raise ValueError(f"duplicate measured workload: {key}")
        by_id[key] = workload
    parity = {}
    parity_identity = {}
    if parity_path:
        parity_document = read_json(parity_path)
        if parity_document.get("schema") != "migration-parity/parity-result@1":
            raise ValueError("unexpected parity schema")
        parity_identity = parity_document["identity"]
        issues.extend(input_issues(parity_identity, manifest_path))
        for item in parity_identity.get("targets", []):
            if item != target_identities.get(item["target_profile"]):
                issues.append(f"parity_target_identity_mismatch:{item['target_profile']}")
        parity = {(row["case_id"], row["target_profile"]): row["outcome"]
                  for row in parity_document["comparisons"]}
        del parity_document
    recorded_inputs = {item["path"] for item in identity.get("inputs", [])}
    rows = {key: assess_workload(spec, by_id.get(key), parity, parity_identity.get("run_id"),
                                issues + ([] if spec_paths[key] in recorded_inputs else ["missing_benchmark_input_digest"]))
            for key, spec in specs.items()}
    for operation in operations.values():
        members = [rows[key] for key in operation["workload_ids"]]
        operation["status"] = "incomplete" if members else "missing_workload"
        operation["latency_status_counts"] = {
            profile: dict(Counter(row["subjects"][profile]["status"] for row in members))
            for profile in PROFILES
        }
        operation["worst_observed_ratios"] = {
            key: (min(values) if key == "simd_speedup_over_pillow" else max(values)) if values else None
            for key in ("cpu_over_pillow", "simd_speedup_over_pillow", "gpu_over_simd")
            for values in [[row["observed_ratios"][key] for row in members
                            if row["observed_ratios"][key] is not None]]
        }
    priority = sorted(rows.values(), key=lambda row: (
        not any(s["parity"] == "fail" for s in row["subjects"].values()),
        -(row["worst_observed_target_factor"] or 0), row["workload_id"],
    ))
    return {
        "schema": "pillow-rs/optimization-goals@1",
        "scope": "Every manifest row is retained; public exports outside the selected manifest still require audit.",
        "manifest_sha256": digest(manifest_path),
        "benchmark_identity": identity, "parity_identity": parity_identity,
        "artifact_issues": sorted(set(issues)),
        "summary": {
            "public_operations": sum(op["kind"] != "constant" for op in operations.values()),
            "public_constants": sum(op["kind"] == "constant" for op in operations.values()),
            "manifest_rows": len(operations), "workloads": len(rows),
            "workloads_with_timing": sum(any(row["subjects"][p]["latency_ms"] is not None for p in PROFILES) for row in rows.values()),
            "operations_without_workloads": [name for name, op in operations.items() if not op["workload_ids"]],
            "unexpected_measured_workloads": sorted(set(by_id) - set(specs)),
            "complete_operations": 0,
        },
        "operations": list(operations.values()), "workloads": list(rows.values()),
        "priority": [{"workload_id": row["workload_id"], "worst_observed_target_factor": row["worst_observed_target_factor"],
                      "parity_failures": [p for p, state in row["subjects"].items() if state["parity"] == "fail"]}
                     for row in priority],
        "remaining_evidence": [
            "Audit public exports beyond the selected manifest.",
            "Compare exact output for every benchmark workflow on every requested backend.",
            "Measure sustained completed-work throughput with changing inputs and backend receipts.",
            "Establish representative size/mode/edge-case sufficiency and repeat passing measurements.",
        ],
    }


def markdown(report: dict[str, Any]) -> str:
    def cell(value: float | None) -> str:
        return f"{value:.3f}" if value is not None else "unmeasured"
    lines = ["# Operation optimization matrix", "", report["scope"], "",
             "Ratios below are diagnostic observations, not accepted parity/backend performance claims.",
             "Targets: CPU/Pillow ≤ 1; Pillow/SIMD ≥ 5; GPU/SIMD ≤ 1. Sustained GPU throughput remains unproven.", "",
             "| Operation | Workloads | Worst CPU/Pillow | Lowest Pillow/SIMD | Worst GPU/SIMD |",
             "| --- | ---: | ---: | ---: | ---: |"]
    for operation in report["operations"]:
        r = operation["worst_observed_ratios"]
        lines.append(f"| `{operation['operation']}` | {len(operation['workload_ids'])} | "
                     f"{cell(r['cpu_over_pillow'])} | {cell(r['simd_speedup_over_pillow'])} | {cell(r['gpu_over_simd'])} |")
    lines.extend(["", "## Next investigation order", "", "Parity failures take precedence; otherwise rank measured target gaps. Missing evidence remains open.", ""])
    for index, row in enumerate(report["priority"][:25], 1):
        lines.append(f"{index}. `{row['workload_id']}` — target factor {cell(row['worst_observed_target_factor'])}; parity failures: {', '.join(row['parity_failures']) or 'none recorded'}.")
    lines.extend(["", "## Remaining evidence", "", *[f"- {item}" for item in report["remaining_evidence"]], ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--result", type=Path)
    parser.add_argument("--parity", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "build/migration-parity/optimization-goals.json")
    parser.add_argument("--markdown", type=Path, default=ROOT / "build/migration-parity/optimization-goals.md")
    args = parser.parse_args()
    report = build_report(args.manifest, args.result, args.parity)
    for path in (args.output, args.markdown):
        path.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    args.markdown.write_text(markdown(report), encoding="utf-8")
    print(json.dumps(report["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
