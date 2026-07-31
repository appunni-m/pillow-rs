#!/usr/bin/env python3
"""Join manifest, indexed inputs, and compatible lane evidence.

The aggregate is deliberately a generated status document.  It never edits
the manifest or input documents and it never treats a static mapping as live
parity, code coverage, or benchmark proof.  Missing, dirty, stale, and
incompatible artifacts remain visible as ``not_proven`` in the operation and
completeness records.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "status-report.json"

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_migration_parity import load_manifest  # noqa: E402
from validate_migration_parity_result import (  # noqa: E402
    benchmark as validate_benchmark,
    coverage as validate_coverage,
    parity as validate_parity,
)


DIMENSIONS = (
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
)
LANES = ("parity", "coverage", "benchmark")
OUTCOMES = {"pass", "fail", "not_run", "not_proven", "not_applicable"}
TARGET_KEYS = {
    "target_profile",
    "target_id",
    "revision",
    "dirty",
    "runtime",
    "backend",
    "features",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path: Path) -> str:
    return path.resolve().relative_to(ROOT.resolve()).as_posix()


def load_indexed_documents(
    manifest: dict[str, Any], lane: str, schema: str
) -> list[tuple[str, dict[str, Any]]]:
    documents: list[tuple[str, dict[str, Any]]] = []
    for item in manifest["input_index"][lane]:
        path = FIXTURE_ROOT / item
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("schema") != schema:
            raise ValueError(f"{item}: expected {schema}")
        documents.append((item, payload))
    return documents


def load_inputs(manifest: dict[str, Any]) -> dict[str, Any]:
    parity_docs = load_indexed_documents(
        manifest, "parity", "migration-parity/parity-input@1"
    )
    coverage_docs = load_indexed_documents(
        manifest, "coverage", "migration-parity/coverage-input@1"
    )
    benchmark_docs = load_indexed_documents(
        manifest, "benchmark", "migration-parity/benchmark-input@1"
    )
    cases: dict[str, dict[str, Any]] = {}
    case_paths: dict[str, str] = {}
    for path, document in parity_docs:
        for case in document["cases"]:
            case_id = case["case_id"]
            if case_id in cases:
                raise ValueError(f"duplicate parity case: {case_id}")
            cases[case_id] = case
            case_paths[case_id] = path
    plans: dict[str, dict[str, Any]] = {}
    plan_paths: dict[str, str] = {}
    for path, document in coverage_docs:
        for plan in document["plans"]:
            plan_id = plan["plan_id"]
            if plan_id in plans:
                raise ValueError(f"duplicate coverage plan: {plan_id}")
            plans[plan_id] = plan
            plan_paths[plan_id] = path
    workloads: dict[str, dict[str, Any]] = {}
    workload_paths: dict[str, str] = {}
    suites: dict[str, dict[str, Any]] = {}
    for path, document in benchmark_docs:
        for workload in document["workloads"]:
            workload_id = workload["workload_id"]
            if workload_id in workloads:
                raise ValueError(f"duplicate benchmark workload: {workload_id}")
            workloads[workload_id] = workload
            workload_paths[workload_id] = path
        for suite in document.get("suites", []):
            suite_id = suite["suite_id"]
            if suite_id in suites:
                raise ValueError(f"duplicate benchmark suite: {suite_id}")
            suites[suite_id] = suite
    return {
        "cases": cases,
        "case_paths": case_paths,
        "plans": plans,
        "plan_paths": plan_paths,
        "workloads": workloads,
        "workload_paths": workload_paths,
        "suites": suites,
    }


def operation_records(manifest: dict[str, Any]) -> list[tuple[dict[str, Any], dict[str, Any]]]:
    return [
        (surface, operation)
        for surface in manifest["surfaces"]
        for operation in surface["operations"]
    ]


def operation_requirements(operation: dict[str, Any], profile: str) -> list[str]:
    return [
        item["id"]
        for item in operation.get("requirements", [])
        if profile in item.get("target_profiles", [])
    ]


def requirements_for_lane(
    manifest: dict[str, Any], profile: str, lane: str
) -> set[str]:
    result: set[str] = set()
    for _, operation in operation_records(manifest):
        for requirement in operation.get("requirements", []):
            if (
                lane in requirement.get("lanes", [])
                and profile in requirement.get("target_profiles", [])
            ):
                result.add(requirement["id"])
    return result


def profile_map(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {item["id"]: item for item in manifest["target_profiles"]}


def target_identity_from_profile(
    profile: dict[str, Any], target_map: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    target = target_map[profile["target_id"]]
    return {
        "target_profile": profile["id"],
        "target_id": target["id"],
        "revision": None,
        "dirty": None,
        "runtime": None,
        "backend": profile["backend"],
        "features": list(profile["features"]),
    }


def validate_result(lane: str, value: dict[str, Any]) -> None:
    {"parity": validate_parity, "coverage": validate_coverage, "benchmark": validate_benchmark}[lane](value)


def normalize_result_path(value: str) -> str:
    path = Path(value)
    if path.is_absolute():
        try:
            return relative(path)
        except ValueError:
            return path.as_posix()
    return path.as_posix()


def check_identity(
    lane: str,
    result: dict[str, Any],
    manifest: dict[str, Any],
    indexed: dict[str, Any],
    profiles: dict[str, dict[str, Any]],
    target_map: dict[str, dict[str, Any]],
) -> list[str]:
    """Return stable identity paths that prevent an evidence join."""

    identity = result["identity"]
    differences: list[str] = []
    expected_manifest = relative(DEFAULT_MANIFEST)
    if normalize_result_path(identity["manifest"]["path"]) != expected_manifest:
        differences.append("identity.manifest.path")
    if identity["manifest"]["schema"] != manifest["schema"]:
        differences.append("identity.manifest.schema")
    if identity["manifest"]["sha256"] != sha256(DEFAULT_MANIFEST):
        differences.append("identity.manifest.sha256")

    allowed: dict[str, str] = {}
    for path in manifest["input_index"]["parity"]:
        allowed[path] = "migration-parity/parity-input@1"
    if lane in {"coverage", "benchmark"}:
        for path in manifest["input_index"][lane]:
            allowed[path] = f"migration-parity/{lane}-input@1"
    for index, item in enumerate(identity["inputs"]):
        path = normalize_result_path(item["path"])
        prefix = f"identity.inputs[{index}]"
        if path not in allowed:
            differences.append(f"{prefix}.path")
            continue
        if item["schema"] != allowed[path]:
            differences.append(f"{prefix}.schema")
        actual = FIXTURE_ROOT / path
        if not actual.is_file() or item["sha256"] != sha256(actual):
            differences.append(f"{prefix}.sha256")

    if lane == "parity":
        if not identity["oracles"] or identity["oracles"][0]["version"] != "12.2.0":
            differences.append("identity.oracles[0].version")
    for index, target in enumerate(identity["targets"]):
        prefix = f"identity.targets[{index}]"
        profile_id = target["target_profile"]
        if profile_id not in profiles:
            differences.append(f"{prefix}.target_profile")
            continue
        profile = profiles[profile_id]
        expected_target = target_map[profile["target_id"]]
        if target["target_id"] != expected_target["id"]:
            differences.append(f"{prefix}.target_id")
        if target["backend"] != profile["backend"]:
            differences.append(f"{prefix}.backend")
        if list(target["features"]) != list(profile["features"]):
            differences.append(f"{prefix}.features")
        if target["dirty"] is True:
            differences.append(f"{prefix}.dirty")

    if result["status"] != "completed":
        differences.append("status")
    if lane == "coverage":
        collector = result["collector"]
        if not collector["artifact_ingested"] or not collector["snapshot_id"]:
            differences.append("collector.snapshot_id")
            differences.append("collector.artifact_ingested")
    return sorted(set(differences))


def lane_applicability(operation: dict[str, Any], lane: str, profile: str) -> str:
    declaration = operation.get(lane, {})
    if profile not in declaration.get("target_profiles", []):
        return "not_applicable"
    return declaration.get("applicability", "not_proven")


def lane_summary(
    applicability: str,
    input_ids: list[str],
    outcome: str,
    evidence_id: str | None,
    details: Iterable[str],
) -> dict[str, Any]:
    if applicability not in {"required", "optional"}:
        outcome = "not_applicable"
    if outcome not in OUTCOMES:
        raise ValueError(f"invalid aggregate outcome: {outcome}")
    return {
        "applicability": applicability,
        "input_ids": input_ids,
        "outcome": outcome,
        "evidence_id": evidence_id,
        "details": list(details),
    }


def static_mapping(
    manifest: dict[str, Any], inputs: dict[str, Any], profile: str, lane: str
) -> tuple[set[str], dict[str, list[str]]]:
    covered: set[str] = set()
    operation_inputs: dict[str, list[str]] = {}
    if lane == "parity":
        for case_id, case in inputs["cases"].items():
            if profile in case.get("target_profiles", []):
                covered.update(case.get("covers", []))
                key = f"{case['surface']}\0{case['operation']}"
                operation_inputs.setdefault(key, []).append(case_id)
    elif lane == "coverage":
        for plan_id, plan in inputs["plans"].items():
            if profile == plan["target_profile"]:
                covered.update(plan.get("covers", []))
                for _, operation in operation_records(manifest):
                    req_ids = {item["id"] for item in operation.get("requirements", [])}
                    if req_ids.intersection(plan.get("covers", [])):
                        surface = next(
                            item for item in manifest["surfaces"] if operation in item["operations"]
                        )
                        key = f"{surface['id']}\0{operation['id']}"
                        operation_inputs.setdefault(key, []).append(plan_id)
    else:
        for workload_id, workload in inputs["workloads"].items():
            for subject in workload.get("subjects", []):
                if subject.get("kind") == "target_profile" and subject.get("id") == profile:
                    covered.update(workload.get("covers", []))
                    for surface, operation in operation_records(manifest):
                        req_ids = {item["id"] for item in operation.get("requirements", [])}
                        if req_ids.intersection(workload.get("covers", [])):
                            key = f"{surface['id']}\0{operation['id']}"
                            operation_inputs.setdefault(key, []).append(workload_id)
    for key in operation_inputs:
        operation_inputs[key] = sorted(set(operation_inputs[key]))
    return covered, operation_inputs


def parity_outcome(
    case_ids: list[str], result: dict[str, Any] | None, compatible: bool
) -> tuple[str, str | None, list[str]]:
    if not case_ids:
        return "not_proven", None, ["no indexed parity case"]
    if result is None or not compatible:
        return "not_proven", None, ["compatible parity evidence unavailable"]
    by_id = {item["case_id"]: item for item in result["comparisons"]}
    selected = [by_id.get(case_id) for case_id in case_ids]
    missing = sum(item is None for item in selected)
    outcomes = [item["outcome"] for item in selected if item is not None]
    if any(item == "fail" for item in outcomes):
        outcome = "fail"
    elif missing or any(item == "not_run" for item in outcomes):
        outcome = "not_run"
    elif outcomes and all(item == "pass" for item in outcomes):
        outcome = "pass"
    else:
        outcome = "not_proven"
    details = [f"cases={len(case_ids)}", f"passed={outcomes.count('pass')}", f"failed={outcomes.count('fail')}"]
    if missing:
        details.append(f"missing={missing}")
    return outcome, result["identity"]["run_id"], details


def coverage_outcome(
    operation: dict[str, Any],
    plan_ids: list[str],
    result: dict[str, Any] | None,
    compatible: bool,
) -> tuple[str, str | None, list[str]]:
    if not plan_ids:
        return "not_proven", None, ["no indexed coverage plan"]
    if result is None or not compatible:
        return "not_proven", None, ["fresh ingested coverage evidence unavailable"]
    components = {
        item["component_id"]: item
        for plan in result["plans"]
        if plan["plan_id"] in plan_ids
        for item in plan["components"]
    }
    component_ids = operation.get("coverage", {}).get("component_ids", [])
    thresholds = [
        threshold
        for component_id in component_ids
        for threshold in components.get(component_id, {}).get("thresholds", [])
    ]
    if not thresholds:
        return "not_proven", result["identity"]["run_id"], ["no measured component threshold"]
    if any(item["outcome"] == "fail" for item in thresholds):
        outcome = "fail"
    elif all(item["outcome"] == "pass" for item in thresholds):
        outcome = "pass"
    else:
        outcome = "not_proven"
    return outcome, result["identity"]["run_id"], [f"thresholds={len(thresholds)}"]


def benchmark_outcome(
    workload_ids: list[str], result: dict[str, Any] | None, compatible: bool
) -> tuple[str, str | None, list[str]]:
    if not workload_ids:
        return "not_proven", None, ["no indexed benchmark workload"]
    if result is None or not compatible:
        return "not_proven", None, ["compatible benchmark evidence unavailable"]
    by_id = {item["workload_id"]: item for item in result["workloads"]}
    selected = [by_id.get(item) for item in workload_ids]
    if any(item is None for item in selected):
        return "not_run", result["identity"]["run_id"], ["benchmark workload result missing"]
    correctness = [item["correctness"]["outcome"] for item in selected if item is not None]
    if any(item == "fail" for item in correctness):
        outcome = "fail"
    elif all(item == "pass" for item in correctness) and all(
        any(subject["status"] == "completed" and subject["id"] != "pillow" for subject in item["subjects"])
        for item in selected
    ):
        outcome = "pass"
    else:
        outcome = "not_proven"
    return outcome, result["identity"]["run_id"], [f"workloads={len(workload_ids)}", f"correctness_pass={correctness.count('pass')}"]


def documentation_counts(manifest: dict[str, Any]) -> tuple[int, int]:
    outputs = list(manifest.get("documentation", {}).get("specification_outputs", [])) + list(
        manifest.get("documentation", {}).get("evidence_outputs", [])
    )
    total = len(outputs)
    fresh = 0
    manifest_digest = sha256(DEFAULT_MANIFEST)
    for output in outputs:
        path = ROOT / output
        if path.is_file() and f"manifest_sha256: {manifest_digest}" in path.read_text(encoding="utf-8"):
            fresh += 1
    return fresh, total


def completeness(
    manifest: dict[str, Any],
    inputs: dict[str, Any],
    profiles: dict[str, dict[str, Any]],
    results: dict[str, tuple[dict[str, Any] | None, bool]],
) -> list[dict[str, Any]]:
    operations = operation_records(manifest)
    profile_ids = list(profiles)
    rows: list[dict[str, Any]] = []
    for dimension in DIMENSIONS:
        if dimension in {"inventory_representation", "operation_contracts"}:
            denominator = len(operations)
            rows.append({"dimension": dimension, "target_profile": None, "numerator": denominator, "denominator": denominator, "evidence_id": None})
            continue
        if dimension == "documentation_freshness":
            fresh, total = documentation_counts(manifest)
            rows.append({"dimension": dimension, "target_profile": None, "numerator": fresh, "denominator": total, "evidence_id": None})
            continue
        for profile_id in profile_ids:
            if dimension == "parity_input_mapping":
                lane = "parity"
                denominator_ids = requirements_for_lane(manifest, profile_id, lane)
                covered, _ = static_mapping(manifest, inputs, profile_id, lane)
                evidence_id = None
            elif dimension == "coverage_input_mapping":
                lane = "coverage"
                denominator_ids = requirements_for_lane(manifest, profile_id, lane)
                covered, _ = static_mapping(manifest, inputs, profile_id, lane)
                evidence_id = None
            elif dimension == "benchmark_input_mapping":
                lane = "benchmark"
                denominator_ids = requirements_for_lane(manifest, profile_id, lane)
                covered, _ = static_mapping(manifest, inputs, profile_id, lane)
                evidence_id = None
            elif dimension == "parity_outcome":
                lane = "parity"
                denominator_ids = {
                    case_id
                    for case_id, case in inputs["cases"].items()
                    if profile_id in case.get("target_profiles", [])
                }
                result, compatible = results[lane]
                evidence_id = result["identity"]["run_id"] if result and compatible else None
                by_id = {item["case_id"]: item for item in result["comparisons"]} if result and compatible else {}
                covered = {case_id for case_id, item in by_id.items() if item["outcome"] == "pass"}
            elif dimension in {"function_coverage", "line_coverage", "branch_coverage", "region_coverage"}:
                lane = "coverage"
                dimension_name = dimension.removesuffix("_coverage")
                result, compatible = results[lane]
                evidence_id = (
                    result["collector"]["snapshot_id"]
                    if result and compatible and result["collector"]["snapshot_id"]
                    else None
                )
                measured: dict[tuple[str, str], tuple[int, int]] = {}
                if result and compatible:
                    for plan in result["plans"]:
                        for component in plan["components"]:
                            for file in component["files"]:
                                for item in file["dimensions"]:
                                    if item["dimension"] == dimension_name:
                                        measured[(file["path"], dimension_name)] = (
                                            int(item["covered"]),
                                            int(item["total"]),
                                        )
                # Code coverage has no static denominator.  Preserve the
                # collector's integer covered/total counts; an absent or
                # un-ingested collector therefore remains 0/0 and not proven.
                rows.append({
                    "dimension": dimension,
                    "target_profile": profile_id,
                    "numerator": sum(item[0] for item in measured.values()),
                    "denominator": sum(item[1] for item in measured.values()),
                    "evidence_id": evidence_id,
                })
                continue
            elif dimension == "benchmark_budget_outcome":
                lane = "benchmark"
                denominator_ids = {
                    item["id"]
                    for _, operation in operations
                    for item in operation.get("requirements", [])
                    if item.get("dimension") == "performance"
                    and lane in item.get("lanes", [])
                    and profile_id in item.get("target_profiles", [])
                }
                result, compatible = results[lane]
                evidence_id = result["identity"]["run_id"] if result and compatible else None
                covered = {
                    budget["requirement_id"]
                    for workload in (result["workloads"] if result and compatible else [])
                    for budget in workload["budgets"]
                    if budget["outcome"] == "pass"
                }
            else:  # pragma: no cover - guarded by DIMENSIONS
                raise AssertionError(dimension)
            rows.append({
                "dimension": dimension,
                "target_profile": profile_id,
                "numerator": len(covered.intersection(denominator_ids)),
                "denominator": len(denominator_ids),
                "evidence_id": evidence_id,
            })
    return rows


def aggregate(args: argparse.Namespace) -> dict[str, Any]:
    manifest_path = args.manifest.resolve()
    if manifest_path != DEFAULT_MANIFEST.resolve():
        raise ValueError("aggregate currently accepts the canonical active manifest only")
    manifest = load_manifest(manifest_path)
    inputs = load_inputs(manifest)
    profiles = profile_map(manifest)
    target_map = {item["id"]: item for item in manifest["targets"]}
    result_paths = {
        "parity": args.parity.resolve(),
        "coverage": args.coverage.resolve(),
        "benchmark": args.benchmark.resolve(),
    }
    results: dict[str, tuple[dict[str, Any] | None, bool]] = {}
    stale: list[dict[str, Any]] = []
    evidence: list[dict[str, Any]] = []
    for lane in LANES:
        path = result_paths[lane]
        if not path.is_file():
            results[lane] = (None, False)
            continue
        value = json.loads(path.read_text(encoding="utf-8"))
        validate_result(lane, value)
        differences = check_identity(lane, value, manifest, inputs, profiles, target_map)
        compatible = not differences
        results[lane] = (value, compatible)
        if compatible:
            evidence.append({
                "lane": lane,
                "run_id": value["identity"]["run_id"],
                "snapshot_id": value.get("collector", {}).get("snapshot_id") if lane == "coverage" else None,
            })
        else:
            stale.append({
                "lane": lane,
                "run_id": value["identity"]["run_id"],
                "reason": "identity or freshness check failed",
                "identity_diff": differences,
            })

    target_profiles: list[dict[str, Any]] = []
    for profile in manifest["target_profiles"]:
        identity = target_identity_from_profile(profile, target_map)
        for lane in LANES:
            value, compatible = results[lane]
            if not value or not compatible:
                continue
            found = next(
                (item for item in value["identity"]["targets"] if item["target_profile"] == profile["id"]),
                None,
            )
            if found:
                identity = dict(found)
                break
        target_profiles.append(identity)

    static_maps = {
        lane: {
            profile_id: static_mapping(manifest, inputs, profile_id, lane)[1]
            for profile_id in profiles
        }
        for lane in LANES
    }
    operations: list[dict[str, Any]] = []
    for surface, operation in operation_records(manifest):
        target = next(
            (item for item in operation.get("targets", []) if item["target_id"] == surface.get("target_id", "pillow-rs-python")),
            None,
        )
        # The target binding is keyed by target registry ID, not surface ID.
        target = next(
            (item for item in operation.get("targets", []) if item["target_id"] == "pillow-rs-python"),
            target,
        )
        static_support = (target or {}).get("support", {}).get("status", "unsupported")
        for profile_id in profiles:
            req_ids = operation_requirements(operation, profile_id)
            key = f"{surface['id']}\0{operation['id']}"
            parity_ids = static_maps["parity"][profile_id].get(key, [])
            coverage_ids = static_maps["coverage"][profile_id].get(key, [])
            benchmark_ids = static_maps["benchmark"][profile_id].get(key, [])
            parity_result, parity_compatible = results["parity"]
            coverage_result, coverage_compatible = results["coverage"]
            benchmark_result, benchmark_compatible = results["benchmark"]
            parity_app = lane_applicability(operation, "parity", profile_id)
            coverage_app = lane_applicability(operation, "coverage", profile_id)
            benchmark_app = lane_applicability(operation, "benchmark", profile_id)
            parity = parity_outcome(parity_ids, parity_result, parity_compatible)
            coverage = coverage_outcome(operation, coverage_ids, coverage_result, coverage_compatible)
            benchmark = benchmark_outcome(benchmark_ids, benchmark_result, benchmark_compatible)
            if static_support == "unsupported":
                support = "unsupported"
            elif static_support == "partial":
                support = "partial"
            elif parity[0] == "pass":
                support = "supported"
            else:
                support = "partial"
            operations.append({
                "surface": surface["id"],
                "operation": operation["id"],
                "target_profile": profile_id,
                "classification": operation["classification"],
                "support": support,
                "requirements": req_ids,
                "parity": lane_summary(parity_app, parity_ids, parity[0], parity[1], parity[2]),
                "coverage": lane_summary(coverage_app, coverage_ids, coverage[0], coverage[1], coverage[2]),
                "benchmark": lane_summary(benchmark_app, benchmark_ids, benchmark[0], benchmark[1], benchmark[2]),
            })

    report = {
        "schema": "migration-parity/status-report@1",
        "manifest": {
            "path": relative(manifest_path),
            "schema": manifest["schema"],
            "sha256": sha256(manifest_path),
        },
        "target_profiles": target_profiles,
        "evidence": sorted(evidence, key=lambda item: item["lane"]),
        "completeness": completeness(manifest, inputs, profiles, results),
        "operations": operations,
        "stale_or_incompatible_evidence": sorted(stale, key=lambda item: item["lane"]),
    }
    args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.output.resolve().write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "operations": len(operations),
        "evidence": len(evidence),
        "stale_or_incompatible_evidence": len(stale),
    }, sort_keys=True))
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--parity", type=Path, default=ROOT / "build" / "migration-parity" / "parity-result.json")
    parser.add_argument("--coverage", type=Path, default=ROOT / "build" / "migration-parity" / "coverage-result.json")
    parser.add_argument("--benchmark", type=Path, default=ROOT / "build" / "migration-parity" / "benchmark-result.json")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    aggregate(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
