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


def simd_vector_evidence(row: dict[str, Any] | None) -> str:
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
            vector_work = True
    return "positive_vector_work" if vector_work else "no_vector_work"
