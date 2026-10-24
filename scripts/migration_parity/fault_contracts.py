"""Runner-owned identifiers and assertions for target-only fault cases."""

from __future__ import annotations

from typing import Any


FAULT_POINTS: tuple[dict[str, str], ...] = (
    {
        "id": "gpu_batch.admission.device_failure",
        "target_profile": "python-gpu-fault-contract",
        "description": "Make the next GPU batch admission observe a device failure.",
    },
)

FAULT_CONTRACTS: dict[str, dict[str, Any]] = {
    "gpu_batch.admission.execution_error": {
        "operation": "PIL.GpuBatchExecutor.submit",
        "step_id": "submit",
        "required_steps": ("executor", "submit"),
        "observed_fields": ("kind", "stage", "job_id", "input_key"),
        "expected": {
            "kind": "Execution",
            "stage": "admission",
            "job_id": 0,
            "input_key": None,
        },
        "description": (
            "A device failure during batch admission raises the public Execution "
            "BatchError with its stable admission metadata."
        ),
    },
}


def manifest_fault_points() -> list[dict[str, str]]:
    """Return the declarative allow-list embedded in the fixed manifest."""

    return [dict(item) for item in FAULT_POINTS]


def manifest_fault_contracts() -> list[dict[str, str]]:
    """Return public contract descriptions without expected result values."""

    return [
        {
            "id": contract_id,
            "operation": contract["operation"],
            "fault_point": next(iter(FAULT_POINTS))["id"],
            "description": contract["description"],
        }
        for contract_id, contract in FAULT_CONTRACTS.items()
    ]


def validate_fault_registries(
    points: list[dict[str, Any]], contracts: list[dict[str, Any]]
) -> None:
    """Reject manifest drift from the runner's deterministic assertions."""

    expected_points = manifest_fault_points()
    expected_contracts = manifest_fault_contracts()
    if points != expected_points:
        raise ValueError("manifest fault-point allow-list differs from the runner")
    if contracts != expected_contracts:
        raise ValueError("manifest fault contracts differ from runner-owned contracts")
