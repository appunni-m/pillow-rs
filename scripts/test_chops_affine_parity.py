#!/usr/bin/env python3
"""Run generated ImageChops offset cases through shared backend parity runners.

The active fixture owns the inputs and public observations. This focused
wrapper only selects those shared case IDs. It sends that unchanged case set
through the standard Python parity runner with CPU, SIMD, and GPU selected at
the runner boundary, then through Node and browser WASM parity. Focused SIMD
and GPU lanes require native backend receipts.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
INPUT = ROOT / "pillow-rs/tests/fixtures/inputs/parity/pil-imagechops.json"
CASE_PREFIX = "PIL.ImageChops."
CASE_MARKER = ".nuanced.offset-coercion-"
DEFAULT_OUTPUT_DIR = ROOT / "build/migration-parity/chops-affine"


def selected_case_ids() -> list[str]:
    """Return the shared generated ImageChops offset-coercion case IDs."""

    document = json.loads(INPUT.read_text(encoding="utf-8"))
    return sorted(
        case["case_id"]
        for case in document["cases"]
        if case.get("case_id", "").startswith(CASE_PREFIX)
        and CASE_MARKER in case["case_id"]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    case_ids = selected_case_ids()
    if not case_ids:
        parser.error("the generated ImageChops offset cases are missing")

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    for command in (["make", "build-parity"], ["make", "build-wasm-core"]):
        result = subprocess.run(command, cwd=ROOT, check=False)
        if result.returncode:
            return result.returncode

    case_list = ",".join(case_ids)
    for backend in ("cpu", "simd", "gpu"):
        output = output_dir / f"{backend}.json"
        environment = os.environ.copy()
        environment.pop("MIGRATION_TARGET_PROFILE", None)
        environment.update(
            {
                "MIGRATION_TARGET_BACKEND": backend,
                "MIGRATION_STRICT_TARGET_BACKEND": (
                    "1" if backend in {"simd", "gpu"} else "0"
                ),
                "MIGRATION_PARITY_BENCHMARK_ADAPTER": "0",
                "MIGRATION_PARITY_EXECUTION_OUTPUT": str(
                    output_dir / f"{backend}-execution.json"
                ),
            }
        )
        command = [
            "make",
            "migration-parity-test",
            f"MIGRATION_PARITY_OUTPUT={output}",
            f"MIGRATION_PARITY_CASE_IDS={case_list}",
        ]
        result = subprocess.run(command, cwd=ROOT, env=environment, check=False)
        if result.returncode:
            return result.returncode

    for host in ("node", "browser"):
        command = [
            sys.executable,
            "scripts/run_migration_js_parity.py",
            "--host",
            host,
            "--output",
            str(output_dir / f"{host}-wasm.json"),
            *[argument for case_id in case_ids for argument in ("--case-id", case_id)],
        ]
        environment = os.environ.copy()
        oracle_python = ROOT / ".venv/bin/python"
        if oracle_python.is_file():
            environment.setdefault("MIGRATION_ORACLE_PYTHON", str(oracle_python))
        result = subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            check=False,
        )
        if result.returncode:
            return result.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
