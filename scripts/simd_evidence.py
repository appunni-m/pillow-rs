"""Validate architecture-specific SIMD execution receipts."""

from __future__ import annotations

from typing import Any


# A positive vector count on a new or control-only path must not silently
# become accelerated-SIMD evidence.
SIMD_POSITIVE_VECTOR_PATHS = frozenset(
    {
        "native-copy",
        "neon-blend",
        "neon-cmyk-l-blend",
        "neon-copy",
        "neon-deinterleave",
        "neon-f64-resize",
        "neon-interleave",
        "neon-la-l-blend",
        "neon-rgb-l-blend",
        "neon-rgba-l-blend",
        "avx2-transpose",
        "rgba-l-mask-vector",
        "ssse3-deinterleave",
        "ssse3-gather",
        "ssse3-interleave",
        "vector",
        "vector-gather",
        "vector-rgb-to-rgba-copy",
        "wide-shuffle",
    }
)


def workflow_step_owners(
    steps: Any,
) -> dict[str, tuple[str, str]]:
    """Map workflow step IDs to their declared public API owners."""
    if not isinstance(steps, list) or not steps:
        return {}
    owners: dict[str, tuple[str, str]] = {}
    for step in steps:
        if not isinstance(step, dict):
            return {}
        step_id = step.get("step_id")
        surface = step.get("surface")
        operation = step.get("operation")
        if (
            not isinstance(step_id, str)
            or not step_id
            or not isinstance(surface, str)
            or not surface
            or not isinstance(operation, str)
            or not operation
            or step_id in owners
        ):
            return {}
        owners[step_id] = (surface, operation)
    return owners


def mapped_operation_owners(
    covers: Any,
    requirement_owner: dict[str, str],
) -> set[tuple[str, str]]:
    """Resolve the manifest operation owners selected by workload coverage."""
    if not isinstance(covers, list) or not covers:
        return set()
    owners = set()
    for requirement in covers:
        owner = requirement_owner.get(str(requirement))
        if owner is None or "." not in owner:
            continue
        surface, operation = owner.rsplit(".", 1)
        owners.add((surface, operation))
    return owners


def simd_vector_evidence(
    row: dict[str, Any] | None,
    expected_sources: dict[str, tuple[str, str]] | None = None,
    mapped_owners: set[tuple[str, str]] | None = None,
) -> str:
    """Distinguish SIMD routing from observed architecture-specific work."""
    if row is None or row.get("subject") != "python-simd":
        return "not_applicable"
    operations = row.get("operation_telemetry")
    if operations is None:
        return "missing"
    if not isinstance(operations, list) or not operations:
        return "missing_or_invalid"
    selected_step_ids = row.get("selected_step_ids")
    if (
        not isinstance(selected_step_ids, list)
        or not selected_step_ids
        or any(not isinstance(step_id, str) or not step_id for step_id in selected_step_ids)
        or len(set(selected_step_ids)) != len(selected_step_ids)
    ):
        return "invalid"
    selected_steps = set(selected_step_ids)
    vector_work = False
    for operation in operations:
        if not isinstance(operation, dict):
            return "invalid"
        count = operation.get("vector_block_count")
        path = operation.get("path")
        name = operation.get("operation")
        step_id = operation.get("step_id")
        counters = [
            operation.get(field)
            for field in (
                "vector_block_count",
                "scalar_tail_count",
                "mode_conversion_count",
                "handoff_count",
            )
        ]
        source_owner: tuple[str, str] | None = None
        if expected_sources is not None:
            source = operation.get("source")
            if not isinstance(source, dict) or set(source) != {
                "step_id",
                "surface",
                "operation",
            }:
                return "invalid"
            source_step_id = source.get("step_id")
            source_surface = source.get("surface")
            source_operation = source.get("operation")
            if (
                not isinstance(source_step_id, str)
                or not source_step_id
                or not isinstance(source_surface, str)
                or not source_surface
                or not isinstance(source_operation, str)
                or not source_operation
                or expected_sources.get(source_step_id)
                != (source_surface, source_operation)
            ):
                return "invalid"
            source_owner = (source_surface, source_operation)
        if (
            any(type(value) is not int or value < 0 for value in counters)
            or not isinstance(path, str)
            or not path
            or not isinstance(name, str)
            or not name
            or not isinstance(step_id, str)
            or step_id not in selected_steps
        ):
            return "invalid"
        if count > 0:
            if path not in SIMD_POSITIVE_VECTOR_PATHS:
                return "invalid"
            if expected_sources is not None and (
                mapped_owners is None or source_owner not in mapped_owners
            ):
                return "invalid"
            vector_work = True
    return "positive_vector_work" if vector_work else "no_vector_work"
