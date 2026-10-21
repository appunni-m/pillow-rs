"""Map declared parity cases and benchmark workloads to public operations.

The index describes fixture declarations only. It never claims that a parity
case or workload was selected or executed.
"""

from __future__ import annotations

import hashlib
import json
from collections import defaultdict
from pathlib import Path
from typing import Any, Mapping

FIXTURE_ROOT = Path("pillow-rs/tests/fixtures")


def _fixture_input_path(root: Path, relative: Any) -> tuple[Path, str]:
    if not isinstance(relative, str) or not relative:
        raise ValueError(f"fixture input path must be a non-empty string: {relative!r}")
    relative_path = Path(relative)
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise ValueError(f"fixture input path escapes its owner: {relative!r}")

    repository_root = root.resolve()
    fixture_root = (repository_root / FIXTURE_ROOT).resolve()
    path = (fixture_root / relative_path).resolve()
    try:
        path.relative_to(fixture_root)
    except ValueError as error:
        raise ValueError(f"fixture input path escapes its owner: {relative!r}") from error
    if not path.is_file():
        raise FileNotFoundError(f"indexed fixture input does not exist: {relative!r}")
    return path, path.relative_to(repository_root).as_posix()


def _operation_id(surface: Any, operation: Any) -> str | None:
    if not isinstance(surface, str) or not surface or not isinstance(operation, str) or not operation:
        return None
    return f"{surface}.{operation}"


def _string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{context} must be a list of non-empty strings")
    return value


def _profiles(subjects: Any) -> set[str]:
    if not isinstance(subjects, list):
        raise ValueError("benchmark subjects must be a list")
    profiles: set[str] = set()
    for item in subjects:
        if not isinstance(item, dict):
            raise ValueError("benchmark subject entries must be mappings")
        if item.get("kind") == "target_profile":
            profile_id = item.get("id")
            if not isinstance(profile_id, str) or not profile_id:
                raise ValueError("benchmark target_profile subject needs a non-empty id")
            profiles.add(profile_id)
    return profiles


def _steps(container: Mapping[str, Any], key: str, context: str) -> list[Mapping[str, Any]]:
    raw_steps = container.get(key, [])
    if not isinstance(raw_steps, list) or any(not isinstance(step, dict) for step in raw_steps):
        raise ValueError(f"{context}.{key} must be a list of mappings")
    return raw_steps


def _input_paths(
    root: Path,
    manifest: Mapping[str, Any],
    input_kind: str,
) -> list[tuple[Path, str]]:
    index = manifest.get("input_index", {})
    if not isinstance(index, dict):
        raise ValueError("manifest input_index must be a mapping")
    paths = index.get(input_kind, [])
    if not isinstance(paths, list):
        raise ValueError(f"manifest input_index.{input_kind} must be a list")
    resolved = [_fixture_input_path(root, item) for item in paths]
    repository_paths = [item[1] for item in resolved]
    if len(repository_paths) != len(set(repository_paths)):
        raise ValueError(f"manifest input_index.{input_kind} contains duplicate paths")
    return resolved


def _empty_reference(input_path: str) -> dict[str, Any]:
    return {
        "input_path": input_path,
        "relations": set(),
        "step_ids": set(),
        "target_profiles": set(),
        "parity_case_ids": set(),
        "measurement_boundaries": set(),
    }


def _serialize_references(records: Mapping[str, Mapping[str, Any]], id_field: str) -> list[dict[str, Any]]:
    serialized = []
    for reference_id, record in sorted(records.items()):
        row = {
            id_field: reference_id,
            "input_path": record["input_path"],
            "relations": sorted(record["relations"]),
            "target_profiles": sorted(record["target_profiles"]),
        }
        if record["step_ids"]:
            row["step_ids"] = sorted(record["step_ids"])
        if record["parity_case_ids"]:
            row["parity_case_ids"] = sorted(record["parity_case_ids"])
        if record["measurement_boundaries"]:
            row["measurement_boundaries"] = sorted(record["measurement_boundaries"])
        serialized.append(row)
    return serialized


def build_declared_evidence_index(
    root: Path,
    manifest: Mapping[str, Any],
    operation_paths: Mapping[str, str],
) -> dict[str, Any]:
    """Map manifest operations to declared parity and benchmark inputs."""
    operation_ids = set(operation_paths)
    requirement_operations: dict[str, set[str]] = defaultdict(set)
    for surface in manifest.get("surfaces", []):
        for operation in surface.get("operations", []):
            operation_id = _operation_id(surface.get("id"), operation.get("id"))
            if operation_id not in operation_ids:
                continue
            for requirement in operation.get("requirements", []):
                requirement_id = requirement.get("id") if isinstance(requirement, dict) else None
                if isinstance(requirement_id, str):
                    requirement_operations[requirement_id].add(operation_id)

    parity_by_operation: dict[str, dict[str, dict[str, Any]]] = {}
    case_operation_links: dict[str, set[str]] = {}
    parity_files: list[dict[str, str]] = []
    unmapped_operation_references: set[tuple[str, str, str, str, str | None]] = set()
    unmapped_requirement_references: set[tuple[str, str, str, str]] = set()
    seen_case_ids: set[str] = set()
    unmapped_parity_cases: set[tuple[str, str]] = set()
    parity_case_count = 0

    for path, repository_path in _input_paths(root, manifest, "parity"):
        contents = path.read_bytes()
        parity_files.append({
            "path": repository_path,
            "sha256": hashlib.sha256(contents).hexdigest(),
        })
        document = json.loads(contents)
        cases = document.get("cases") if isinstance(document, dict) else None
        if not isinstance(cases, list):
            raise ValueError(f"parity input must contain a cases list: {repository_path}")
        for case in cases:
            if not isinstance(case, dict) or not isinstance(case.get("case_id"), str):
                raise ValueError(f"parity input contains a case without case_id: {repository_path}")
            case_id = case["case_id"]
            if not case_id:
                raise ValueError(f"parity input contains an empty case_id: {repository_path}")
            if case_id in seen_case_ids:
                raise ValueError(f"duplicate parity case ID {case_id!r} in indexed inputs")
            seen_case_ids.add(case_id)
            parity_case_count += 1
            profiles = set(_string_list(case.get("target_profiles", []), f"{case_id}.target_profiles"))
            links: dict[str, dict[str, set[str]]] = defaultdict(
                lambda: {"relations": set(), "step_ids": set()}
            )

            target_operation = _operation_id(case.get("surface"), case.get("operation"))
            if target_operation:
                links[target_operation]["relations"].add("case_target")
            for step in _steps(case, "steps", case_id):
                stepped_operation = _operation_id(step.get("surface"), step.get("operation"))
                if stepped_operation:
                    links[stepped_operation]["relations"].add("workflow_step")
                    step_id = step.get("step_id")
                    if step_id is not None:
                        if not isinstance(step_id, str) or not step_id:
                            raise ValueError(f"{case_id}.steps contains an invalid step_id")
                        links[stepped_operation]["step_ids"].add(step_id)
            for requirement_id in _string_list(case.get("covers", []), f"{case_id}.covers"):
                candidates = requirement_operations.get(requirement_id, set())
                if len(candidates) == 1:
                    links[next(iter(candidates))]["relations"].add("covers_requirement")
                elif len(candidates) > 1:
                    unmapped_requirement_references.add(
                        ("parity", case_id, repository_path, requirement_id)
                    )
                elif isinstance(requirement_id, str):
                    unmapped_requirement_references.add(
                        ("parity", case_id, repository_path, requirement_id)
                    )

            mapped_operation_ids: set[str] = set()
            for linked_operation, link in links.items():
                if linked_operation not in operation_ids:
                    unmapped_operation_references.add(
                        (
                            "parity",
                            case_id,
                            repository_path,
                            linked_operation,
                            min(link["step_ids"]) if link["step_ids"] else None,
                        )
                    )
                    continue
                mapped_operation_ids.add(linked_operation)
                record = parity_by_operation.setdefault(linked_operation, {}).setdefault(
                    case_id, _empty_reference(repository_path)
                )
                if record["input_path"] != repository_path:
                    raise ValueError(f"duplicate parity case ID {case_id!r}")
                record["relations"].update(link["relations"])
                record["step_ids"].update(link["step_ids"])
                record["target_profiles"].update(profiles)
            case_operation_links[case_id] = mapped_operation_ids
            if not mapped_operation_ids:
                unmapped_parity_cases.add((case_id, repository_path))

    benchmark_by_operation: dict[str, dict[str, dict[str, Any]]] = {}
    benchmark_files: list[dict[str, str]] = []
    unsupported_input_kinds: set[tuple[str, str, str, str]] = set()
    unresolved_case_references: set[tuple[str, str, str]] = set()
    seen_workload_ids: set[str] = set()
    unmapped_benchmark_workloads: set[tuple[str, str]] = set()
    benchmark_workload_count = 0

    for path, repository_path in _input_paths(root, manifest, "benchmark"):
        contents = path.read_bytes()
        benchmark_files.append({
            "path": repository_path,
            "sha256": hashlib.sha256(contents).hexdigest(),
        })
        document = json.loads(contents)
        workloads = document.get("workloads") if isinstance(document, dict) else None
        if not isinstance(workloads, list):
            raise ValueError(f"benchmark input must contain a workloads list: {repository_path}")
        for workload in workloads:
            if not isinstance(workload, dict) or not isinstance(workload.get("workload_id"), str):
                raise ValueError(f"benchmark input contains a workload without workload_id: {repository_path}")
            workload_id = workload["workload_id"]
            if not workload_id:
                raise ValueError(f"benchmark input contains an empty workload_id: {repository_path}")
            if workload_id in seen_workload_ids:
                raise ValueError(f"duplicate benchmark workload ID {workload_id!r} in indexed inputs")
            seen_workload_ids.add(workload_id)
            benchmark_workload_count += 1
            profiles = _profiles(workload.get("subjects"))
            measurement = workload.get("measurement", {})
            boundary = measurement.get("boundary") if isinstance(measurement, dict) else None
            input_spec = workload.get("input", {})
            if not isinstance(input_spec, dict):
                raise ValueError(f"{workload_id}.input must be a mapping")
            input_kind = input_spec.get("kind")
            links: dict[str, dict[str, set[str]]] = defaultdict(
                lambda: {"relations": set(), "step_ids": set(), "parity_case_ids": set()}
            )

            if input_kind == "parity_case":
                case_id = input_spec.get("case_id")
                if not isinstance(case_id, str) or case_id not in case_operation_links:
                    unresolved_case_references.add(
                        (workload_id, repository_path, str(case_id))
                    )
                else:
                    for linked_operation in case_operation_links[case_id]:
                        links[linked_operation]["relations"].add("parity_case_workflow")
                        links[linked_operation]["parity_case_ids"].add(case_id)
            elif input_kind == "workflow":
                for step in _steps(input_spec, "steps", workload_id):
                    stepped_operation = _operation_id(step.get("surface"), step.get("operation"))
                    if stepped_operation:
                        links[stepped_operation]["relations"].add("workflow_step")
                        step_id = step.get("step_id")
                        if step_id is not None:
                            if not isinstance(step_id, str) or not step_id:
                                raise ValueError(f"{workload_id}.input.steps contains an invalid step_id")
                            links[stepped_operation]["step_ids"].add(step_id)
            else:
                unsupported_input_kinds.add(
                    (workload_id, repository_path, str(input_kind), "unsupported input kind")
                )

            for requirement_id in _string_list(workload.get("covers", []), f"{workload_id}.covers"):
                candidates = requirement_operations.get(requirement_id, set())
                if len(candidates) == 1:
                    links[next(iter(candidates))]["relations"].add("covers_requirement")
                elif isinstance(requirement_id, str):
                    unmapped_requirement_references.add(
                        ("benchmark", workload_id, repository_path, requirement_id)
                    )

            for linked_operation, link in links.items():
                if linked_operation not in operation_ids:
                    unmapped_operation_references.add(
                        (
                            "benchmark",
                            workload_id,
                            repository_path,
                            linked_operation,
                            min(link["step_ids"]) if link["step_ids"] else None,
                        )
                    )
                    continue
                record = benchmark_by_operation.setdefault(linked_operation, {}).setdefault(
                    workload_id, _empty_reference(repository_path)
                )
                if record["input_path"] != repository_path:
                    raise ValueError(f"duplicate benchmark workload ID {workload_id!r}")
                record["relations"].update(link["relations"])
                record["step_ids"].update(link["step_ids"])
                record["parity_case_ids"].update(link["parity_case_ids"])
                record["target_profiles"].update(profiles)
                if isinstance(boundary, str):
                    record["measurement_boundaries"].add(boundary)
            if not any(linked_operation in operation_ids for linked_operation in links):
                unmapped_benchmark_workloads.add((workload_id, repository_path))

    operations = []
    no_parity_case = []
    no_benchmark_workload = []
    for operation_id, public_path in sorted(operation_paths.items()):
        parity_cases = _serialize_references(parity_by_operation.get(operation_id, {}), "case_id")
        benchmark_workloads = _serialize_references(
            benchmark_by_operation.get(operation_id, {}), "workload_id"
        )
        if not parity_cases:
            no_parity_case.append({
                "operation_id": operation_id,
                "public_target_path": public_path,
                "review_only": True,
                "reason_for_review": "No indexed parity input declares this public operation or one of its workflow steps.",
            })
        if not benchmark_workloads:
            no_benchmark_workload.append({
                "operation_id": operation_id,
                "public_target_path": public_path,
                "review_only": True,
                "reason_for_review": "No indexed benchmark workload declares this public operation or one of its workflow steps.",
            })
        operations.append({
            "operation_id": operation_id,
            "public_target_path": public_path,
            "parity_case_count": len(parity_cases),
            "parity_cases": parity_cases,
            "benchmark_workload_count": len(benchmark_workloads),
            "benchmark_workloads": benchmark_workloads,
        })

    unmapped_operation_rows = [
        {
            "input_kind": kind,
            "input_id": input_id,
            "input_path": input_path,
            "public_api_path": api_path,
            "step_id": step_id,
            "review_only": True,
            "reason_for_review": "The declared fixture references a public API path absent from the fixed manifest operation set.",
        }
        for kind, input_id, input_path, api_path, step_id in sorted(unmapped_operation_references)
    ]
    unresolved_case_rows = [
        {
            "workload_id": workload_id,
            "input_path": input_path,
            "case_id": case_id,
            "review_only": True,
            "reason_for_review": "The benchmark references a parity case absent from the manifest parity input index.",
        }
        for workload_id, input_path, case_id in sorted(unresolved_case_references)
    ]
    requirement_rows = [
        {
            "input_kind": kind,
            "input_id": input_id,
            "input_path": input_path,
            "requirement_id": requirement_id,
            "review_only": True,
            "reason_for_review": "The declared evidence requirement did not resolve to exactly one manifest operation.",
        }
        for kind, input_id, input_path, requirement_id in sorted(unmapped_requirement_references)
    ]
    unsupported_rows = [
        {
            "workload_id": workload_id,
            "input_path": input_path,
            "input_kind": input_kind,
            "review_only": True,
            "reason_for_review": reason,
        }
        for workload_id, input_path, input_kind, reason in sorted(unsupported_input_kinds)
    ]
    unmapped_parity_rows = [
        {
            "case_id": case_id,
            "input_path": input_path,
            "review_only": True,
            "reason_for_review": "The declared parity case did not resolve to any operation in the fixed manifest set.",
        }
        for case_id, input_path in sorted(unmapped_parity_cases)
    ]
    unmapped_benchmark_rows = [
        {
            "workload_id": workload_id,
            "input_path": input_path,
            "review_only": True,
            "reason_for_review": "The declared benchmark workload did not resolve to any operation in the fixed manifest set.",
        }
        for workload_id, input_path in sorted(unmapped_benchmark_workloads)
    ]

    return {
        "scope": {
            "evidence_is_declared_input_only": True,
            "parity_execution_proven": False,
            "benchmark_execution_proven": False,
            "backend_execution_proven": False,
        },
        "summary": {
            "manifest_operations": len(operation_ids),
            "parity_input_files": len(parity_files),
            "parity_cases": parity_case_count,
            "operations_with_parity_cases": sum(bool(parity_by_operation.get(item)) for item in operation_ids),
            "operations_without_parity_cases": len(no_parity_case),
            "benchmark_input_files": len(benchmark_files),
            "benchmark_workloads": benchmark_workload_count,
            "operations_with_benchmark_workloads": sum(bool(benchmark_by_operation.get(item)) for item in operation_ids),
            "operations_without_benchmark_workloads": len(no_benchmark_workload),
            "unmapped_fixture_operation_references": len(unmapped_operation_rows),
            "parity_cases_without_public_operation": len(unmapped_parity_rows),
            "benchmark_workloads_without_public_operation": len(unmapped_benchmark_rows),
            "unresolved_benchmark_parity_case_references": len(unresolved_case_rows),
            "unmapped_requirement_references": len(requirement_rows),
            "unsupported_benchmark_input_kinds": len(unsupported_rows),
        },
        "input_files": sorted(parity_files + benchmark_files, key=lambda item: item["path"]),
        "operations": operations,
        "review_queues": {
            "public_operation_without_parity_case": no_parity_case,
            "public_operation_without_benchmark_workload": no_benchmark_workload,
            "unmapped_fixture_operation_reference": unmapped_operation_rows,
            "parity_case_without_public_operation": unmapped_parity_rows,
            "benchmark_workload_without_public_operation": unmapped_benchmark_rows,
            "unresolved_benchmark_parity_case_reference": unresolved_case_rows,
            "unmapped_requirement_reference": requirement_rows,
            "unsupported_benchmark_input_kind": unsupported_rows,
        },
        "limits": [
            "Declared parity inputs do not prove that a parity runner selected or executed the case.",
            "Declared benchmark workloads do not prove a benchmark run, backend receipt, parity admission, or materialized result.",
            "Fixture-to-operation links are static surface, operation, step, and requirement references; they do not prove the Rust source route executed.",
        ],
    }
