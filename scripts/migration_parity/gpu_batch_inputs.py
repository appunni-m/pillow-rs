"""Generated public workflow for the deterministic GPU batch fault contract."""

from __future__ import annotations

from typing import Any


FAULT_POINT = "gpu_batch.admission.device_failure"
FAULT_CONTRACT = "gpu_batch.admission.execution_error"
CASE_ID = "PIL.GpuBatchExecutor.submit.fault-contract.device-failure-at-admission"


def gpu_batch_fault_case(
    operation_index: dict[tuple[str, str], dict[str, Any]],
) -> dict[str, Any]:
    """Build one target-only case in the ordinary shared parity input format."""

    constructor = operation_index[("PIL", "GpuBatchExecutor")]
    submit = operation_index[("PIL.GpuBatchExecutor", "submit")]
    covers = [
        requirement["id"]
        for operation in (constructor, submit)
        for requirement in operation["requirements"]
        if "fault_contract" in requirement["lanes"]
    ]
    return {
        "case_id": CASE_ID,
        "surface": "PIL.GpuBatchExecutor",
        "operation": "submit",
        "verification": "fault-contract",
        "fault": {"point": FAULT_POINT, "contract": FAULT_CONTRACT},
        "covers": covers,
        "assets": [],
        "steps": [
            {
                "step_id": "image",
                "surface": "PIL.Image",
                "operation": "new",
                "receiver": None,
                "arguments": {
                    "mode": {"kind": "literal", "value": "L"},
                    "size": {"kind": "literal", "value": [2, 2]},
                    "color": {"kind": "literal", "value": 7},
                },
            },
            {
                "step_id": "executor",
                "surface": "PIL",
                "operation": "GpuBatchExecutor",
                "receiver": None,
                "arguments": {},
            },
            {
                "step_id": "submit",
                "surface": "PIL.GpuBatchExecutor",
                "operation": "submit",
                "receiver": {"kind": "binding", "step_id": "executor"},
                "arguments": {"pipeline": {"kind": "binding", "step_id": "image"}},
            },
        ],
        "observations": ["executor", "submit"],
    }
