"""Declared parity inputs for the opt-in Parallel CPU profile."""

from __future__ import annotations

from typing import Any


PARALLEL_CPU_PROFILE = "python-parallel-cpu"

PARALLEL_CPU_PARITY_CASES: dict[str, dict[str, str]] = {
    "PIL.ImageOps.posterize": {
        "surface": "PIL.ImageOps",
        "operation": "posterize",
        "case_id": (
            "PIL.ImageOps.posterize.nuanced."
            "performance-large-l-noise-1024x768"
        ),
        "requirement_id": "PIL.ImageOps.posterize.backend.parallel-cpu",
        "description": "Exact L-mode posterize output through opt-in Parallel CPU.",
    },
}


def parallel_cpu_contract_for_operation(
    operation_id: str,
) -> dict[str, str] | None:
    """Return the focused Parallel CPU parity contract for a public operation."""

    return PARALLEL_CPU_PARITY_CASES.get(operation_id)


def add_parallel_cpu_parity_profiles(
    surface_id: str,
    cases: list[dict[str, Any]],
    case_by_requirement: dict[str, str],
) -> None:
    """Bind declared large-input cases to their Parallel CPU requirements."""

    for contract in PARALLEL_CPU_PARITY_CASES.values():
        if contract["surface"] != surface_id:
            continue
        matching = [case for case in cases if case["case_id"] == contract["case_id"]]
        if len(matching) != 1:
            raise ValueError(
                f"Parallel CPU parity requires one case {contract['case_id']}; "
                f"found {len(matching)}"
            )
        case = matching[0]
        if (case["surface"], case["operation"]) != (
            contract["surface"],
            contract["operation"],
        ):
            raise ValueError(
                f"Parallel CPU parity case {contract['case_id']} changed operation"
            )
        profiles = case["target_profiles"]
        if "python-cpu" not in profiles:
            raise ValueError(
                f"Parallel CPU parity case {contract['case_id']} must retain CPU"
            )
        if PARALLEL_CPU_PROFILE not in profiles:
            profiles.append(PARALLEL_CPU_PROFILE)
        covers = case["covers"]
        if contract["requirement_id"] not in covers:
            covers.append(contract["requirement_id"])
        for other_case in cases:
            if other_case is not case:
                other_case["covers"] = [
                    requirement_id
                    for requirement_id in other_case["covers"]
                    if requirement_id != contract["requirement_id"]
                ]
        case_by_requirement[contract["requirement_id"]] = contract["case_id"]
