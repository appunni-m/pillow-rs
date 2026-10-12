"""Shared interpreter and environment selection for benchmark adapters."""

from __future__ import annotations

import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def benchmark_adapter_python(
    side: str,
    inherited_environment: dict[str, str] | None = None,
) -> str:
    """Select the same source or target interpreter as managed benchmarks."""

    if side not in {"source", "target"}:
        raise ValueError("benchmark adapter side must be source or target")
    environment = (
        os.environ if inherited_environment is None else inherited_environment
    )
    if side == "source":
        return environment.get(
            "MIGRATION_BENCHMARK_SOURCE_PYTHON",
            environment.get("MIGRATION_ORACLE_PYTHON", sys.executable),
        )
    return sys.executable


def benchmark_adapter_environment(
    side: str,
    backend: str,
    inherited_environment: dict[str, str] | None = None,
) -> dict[str, str]:
    """Build the shared strict-backend environment for benchmark adapters."""

    if side not in {"source", "target"}:
        raise ValueError("benchmark adapter side must be source or target")
    if backend not in {"cpu", "simd", "gpu"}:
        raise ValueError("benchmark adapter backend must be cpu, simd, or gpu")

    inherited = os.environ if inherited_environment is None else inherited_environment
    environment = dict(inherited)
    environment.update(
        {
            "MIGRATION_TARGET_BACKEND": backend,
            "MIGRATION_STRICT_TARGET_BACKEND": "1",
            "MIGRATION_PARITY_BENCHMARK_ADAPTER": "1",
        }
    )
    target_python = str((ROOT / "pillow-rs-py" / "python").resolve())
    if side == "target":
        inherited_pythonpath = environment.get("PYTHONPATH", "")
        environment["PYTHONPATH"] = (
            target_python + os.pathsep + inherited_pythonpath
            if inherited_pythonpath
            else target_python
        )
    else:
        # Keep the source adapter on the separately installed Pillow oracle.
        # The checkout's python directory contains the target PIL facade.
        environment["MIGRATION_PARITY_TIMED_SOURCE"] = "1"
        inherited_pythonpath = environment.get("PYTHONPATH", "")
        retained = [
            item
            for item in inherited_pythonpath.split(os.pathsep)
            if item and str(Path(item).resolve()) != target_python
        ]
        if retained:
            environment["PYTHONPATH"] = os.pathsep.join(retained)
        else:
            environment.pop("PYTHONPATH", None)
    return environment
