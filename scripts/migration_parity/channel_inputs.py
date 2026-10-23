"""Input-only composed workloads for the public GetChannel family."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any


GETCHANNEL_SURFACE = "PIL.Image.Image"
GETCHANNEL_CASE_REQUIREMENTS = (
    f"{GETCHANNEL_SURFACE}.getchannel.behavior.default",
    f"{GETCHANNEL_SURFACE}.getchannel.parameter.channel",
    f"{GETCHANNEL_SURFACE}.getchannel.mode.rgb",
    f"{GETCHANNEL_SURFACE}.getchannel.performance.standard",
)
SEEDED_CORNER_CASE_IDS = (
    f"{GETCHANNEL_SURFACE}.getchannel.nuanced.pipeline-seeded-corners-16x16-all-bands",
    f"{GETCHANNEL_SURFACE}.getchannel.nuanced.pipeline-seeded-corners-1024x768-all-bands",
)


def _literal(value: Any) -> dict[str, Any]:
    return {"kind": "literal", "value": value}


def _binding(step_id: str) -> dict[str, str]:
    return {"kind": "binding", "step_id": step_id}


def _step(
    step_id: str,
    surface: str,
    operation: str,
    receiver: dict[str, str] | None,
    arguments: dict[str, Any],
) -> dict[str, Any]:
    return {
        "step_id": step_id,
        "surface": surface,
        "operation": operation,
        "receiver": receiver,
        "arguments": arguments,
    }


def _seeded_corner_case(
    size: tuple[int, int],
) -> dict[str, Any]:
    width, height = size
    size_label = f"{width}x{height}"
    case_id = (
        f"{GETCHANNEL_SURFACE}.getchannel.nuanced."
        f"pipeline-seeded-corners-{size_label}-all-bands"
    )
    steps = [
        _step(
            "rgba-input",
            "PIL.Image",
            "new",
            None,
            {
                "mode": _literal("RGBA"),
                "size": _literal([width, height]),
                "color": _literal([3, 5, 7, 11]),
            },
        )
    ]
    corners = (
        ((0, 0), [17, 43, 91, 137]),
        ((width - 1, 0), [61, 127, 193, 239]),
        ((0, height - 1), [109, 181, 37, 83]),
        ((width - 1, height - 1), [233, 71, 149, 29]),
    )
    for index, (xy, rgba) in enumerate(corners):
        steps.append(
            _step(
                f"seed-corner-{index}",
                GETCHANNEL_SURFACE,
                "putpixel",
                _binding("rgba-input"),
                {"xy": _literal(list(xy)), "value": _literal(rgba)},
            )
        )

    steps.append(
        _step(
            "convert-rgb",
            GETCHANNEL_SURFACE,
            "convert",
            _binding("rgba-input"),
            {"mode": _literal("RGB")},
        )
    )
    observations: list[str] = []
    for channel in range(3):
        result_id = f"channel-{channel}"
        bytes_id = f"channel-{channel}-bytes"
        steps.extend(
            [
                _step(
                    result_id,
                    GETCHANNEL_SURFACE,
                    "getchannel",
                    _binding("convert-rgb"),
                    {"channel": _literal(channel)},
                ),
                _step(
                    bytes_id,
                    GETCHANNEL_SURFACE,
                    "tobytes",
                    _binding(result_id),
                    {},
                ),
            ]
        )
        observations.extend([result_id, bytes_id])

    return {
        "case_id": case_id,
        "surface": GETCHANNEL_SURFACE,
        "operation": "getchannel",
        "covers": list(GETCHANNEL_CASE_REQUIREMENTS),
        "assets": [],
        "steps": steps,
        "observations": observations,
    }


def seeded_corner_parity_cases(
    surface_id: str,
) -> list[dict[str, Any]]:
    """Build parity-only small and material RGBA→RGB→GetChannel workflows."""

    if surface_id != GETCHANNEL_SURFACE:
        return []
    return [
        _seeded_corner_case(size)
        for size in ((16, 16), (1024, 768))
    ]


def seeded_corner_benchmark_workloads(
    *,
    requirement_id: str,
    cases_by_id: dict[str, dict[str, Any]],
    subjects: Sequence[dict[str, str]],
    context_builder: Callable[..., dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Build full-workflow, parity-gated GetChannel benchmark rows."""

    workloads: list[dict[str, Any]] = []
    for case_id in SEEDED_CORNER_CASE_IDS:
        case = cases_by_id.get(case_id)
        if case is None:
            raise ValueError(f"seeded-corner GetChannel case is missing: {case_id}")
        size = next(
            step["arguments"]["size"]["value"]
            for step in case["steps"]
            if step["operation"] == "new"
        )
        size_label = f"{size[0]}x{size[1]}"
        workload_id = f"pipeline-chain.getchannel.seeded-corners-{size_label}-all-bands"
        workloads.append(
            {
                "workload_id": workload_id,
                "covers": [requirement_id],
                "subjects": [dict(subject) for subject in subjects],
                "input": {"kind": "parity_case", "case_id": case_id},
                "measurement": {
                    "boundary": "whole_workflow",
                    "step_ids": [],
                    "metrics": ["latency", "throughput"],
                    "warmup_iterations": 5,
                    "measurement_iterations": 20,
                    "samples": 5,
                    "concurrency": 1,
                    "cache_state": "warm",
                    "correctness_gate": "parity_pass",
                },
                "context": context_builder(
                    case,
                    variant="seeded-corners-getchannel",
                    surface=GETCHANNEL_SURFACE,
                    operation="getchannel",
                ),
            }
        )

    suite = {
        "suite_id": "pipeline-operations.getchannel-seeded-corners-suite",
        "description": (
            "Parity-gated 16x16 and 1024x768 seeded-corner RGBA-to-RGB "
            "workflows that extract all three bands."
        ),
        "members": [
            {"workload_id": workload["workload_id"], "weight": 1}
            for workload in workloads
        ],
    }
    return workloads, suite
