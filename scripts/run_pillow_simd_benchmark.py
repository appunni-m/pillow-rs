#!/usr/bin/env python3
"""Collect parity-gated x86 timings for Pillow-SIMD's material workloads.

Pillow-SIMD and Pillow share the ``PIL`` import namespace, so this driver runs
them in isolated virtual environments.  It keeps the paired Pillow and
pillow-rs measurements in one benchmark cohort and exports the second source
timing as a separate Pillow-SIMD subject.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
from typing import Any

# The maintained runners use script-style imports for direct command execution.
# Add that directory when this module is imported by its unit tests.
_script_dir = str(Path(__file__).resolve().parent)
if _script_dir not in sys.path:
    sys.path.insert(0, _script_dir)

try:
    from docs_evidence import project_pillow, validate
    from run_migration_benchmark import DEFAULT_MANIFEST, load_benchmarks, load_manifest
except ModuleNotFoundError:  # imported as ``scripts.run_pillow_simd_benchmark`` in tests
    from scripts.docs_evidence import project_pillow, validate
    from scripts.run_migration_benchmark import DEFAULT_MANIFEST, load_benchmarks, load_manifest


ROOT = Path(__file__).resolve().parents[1]
WORKLOAD_IDS = (
    "pipeline-op.boxblur.material-l-noise-1024x768-radius-1",
    "pipeline-op.boxblur.material-rgb-noise-1024x768-radius-1",
    "pil-imagefilter.gaussianblur.standard",
    "pipeline-op.gaussianblur.material-l-noise-1024x768",
    "pipeline-op.gaussianblur.material-la-noise-1024x768",
    "pil-image-image.getchannel.materialized.l-1024x768",
    "pil-image-image.getchannel.materialized.rgb-1024x768",
    "pil-image-image.getchannel.materialized.la-1024x768",
    "pil-image-image.getchannel.materialized.rgba-1024x768",
)
PILLOW_VERSION = "12.1.1"
PILLOW_SIMD_VERSION = "12.1.1.post0"


def workload_contracts() -> dict[str, dict[str, Any]]:
    manifest = load_manifest(DEFAULT_MANIFEST)
    workloads, _, _ = load_benchmarks(manifest)
    by_id = workloads
    missing = [workload_id for workload_id in WORKLOAD_IDS if workload_id not in by_id]
    if missing:
        raise ValueError(f"Pillow-SIMD benchmark workloads are missing: {missing}")
    selected = {workload_id: by_id[workload_id] for workload_id in WORKLOAD_IDS}
    invalid = [
        workload_id
        for workload_id, item in selected.items()
        if item["input"]["kind"] != "parity_case"
        or item["measurement"]["correctness_gate"] != "parity_pass"
        or item["context"].get("size") != [1024, 768]
    ]
    if invalid:
        raise ValueError(
            "Pillow-SIMD comparisons require full-size parity-backed workloads: "
            f"{invalid}"
        )
    return selected


def run_source_cohort(
    *,
    source_python: str,
    source_name: str,
    source_version: str,
    output: Path,
    parity_output: Path,
    workload_ids: list[str],
) -> tuple[dict[str, Any], str]:
    env = os.environ.copy()
    env.update(
        {
            "MIGRATION_BENCHMARK_COHORT": "pillow-simd",
            "MIGRATION_BENCHMARK_BACKENDS": "cpu,simd",
            "MIGRATION_ORACLE_PYTHON": source_python,
            "MIGRATION_ORACLE_NAME": source_name,
            "MIGRATION_ORACLE_VERSION": source_version,
        }
    )
    command = [
        sys.executable,
        str(ROOT / "scripts" / "run_migration_benchmark.py"),
        "--manifest",
        str(DEFAULT_MANIFEST),
        "--output",
        str(output),
        "--parity-output",
        str(parity_output),
    ]
    for workload_id in workload_ids:
        command.extend(("--workload-id", workload_id))
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode:
        detail = (completed.stderr or completed.stdout)[-4000:]
        raise RuntimeError(f"{source_name} benchmark run failed: {detail}")
    result = json.loads(output.read_text(encoding="utf-8"))
    parity = json.loads(parity_output.read_text(encoding="utf-8"))
    validate_run(result, parity, source_name, source_version, workload_ids)
    return result, str(parity["identity"]["run_id"])


def validate_run(
    result: dict[str, Any],
    parity: dict[str, Any],
    source_name: str,
    source_version: str,
    workload_ids: list[str],
) -> None:
    identity = result.get("identity", {})
    oracles = identity.get("oracles", [])
    if result.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError(f"{source_name} result has an unknown benchmark schema")
    if result.get("status") != "completed" or not oracles or (
        oracles[0].get("name"), oracles[0].get("version")
    ) != (source_name, source_version):
        raise ValueError(f"{source_name} result identity/status is invalid")
    if [item.get("workload_id") for item in result.get("workloads", [])] != workload_ids:
        raise ValueError(f"{source_name} result changed the selected workload set")
    if parity.get("status") != "completed" or parity.get("summary", {}).get("failed") != 0:
        raise ValueError(f"{source_name} parity preflight did not pass")
    if parity.get("infrastructure_errors"):
        raise ValueError(f"{source_name} parity preflight has infrastructure errors")
    for workload in result["workloads"]:
        if workload["correctness"].get("outcome") != "pass":
            raise ValueError(
                f"{source_name} parity gate failed for {workload['workload_id']}"
            )
        subjects = {item["id"]: item for item in workload["subjects"]}
        if set(subjects) != {"pillow", "python-cpu", "python-simd"}:
            raise ValueError(f"{source_name} result has an unexpected subject set")
        if any(subjects[name]["status"] != "completed" for name in subjects):
            raise ValueError(
                f"{source_name} has an incomplete subject for {workload['workload_id']}"
            )


def combine_results(
    pillow: dict[str, Any],
    pillow_simd: dict[str, Any],
    source_digest: str,
    *,
    parity_run_ids: tuple[str, str] = ("", ""),
) -> dict[str, Any]:
    """Build one x86 comparison snapshot with a version-matched Pillow baseline."""
    first_identity = pillow["identity"]
    simd_identity = pillow_simd["identity"]
    if first_identity["targets"] != simd_identity["targets"]:
        raise ValueError("Pillow and Pillow-SIMD target identities differ")
    for key in ("manifest", "inputs", "assets"):
        if first_identity.get(key) != simd_identity.get(key):
            raise ValueError(f"Pillow and Pillow-SIMD {key} differ")
    if pillow["environment"] != pillow_simd["environment"]:
        raise ValueError("Pillow and Pillow-SIMD measurements use different hosts")
    if [w["workload_id"] for w in pillow["workloads"]] != [
        w["workload_id"] for w in pillow_simd["workloads"]
    ]:
        raise ValueError("Pillow and Pillow-SIMD measurements use different workloads")

    snapshot = project_pillow(pillow)
    simd_snapshot = project_pillow(pillow_simd)
    normal_rows = {(row["workload"], row["subject"]): row for row in snapshot["rows"]}
    simd_rows = {(row["workload"], row["subject"]): row for row in simd_snapshot["rows"]}
    for workload in WORKLOAD_IDS:
        normal_workload = next(w for w in pillow["workloads"] if w["workload_id"] == workload)
        simd_workload = next(w for w in pillow_simd["workloads"] if w["workload_id"] == workload)
        if normal_workload["context"] != simd_workload["context"] or normal_workload["measurement_policy"] != simd_workload["measurement_policy"]:
            raise ValueError(f"comparison conditions differ for {workload}")
        row = simd_rows.get((workload, "pillow"))
        if row is None:
            raise ValueError(f"Pillow-SIMD timing is missing for {workload}")
        normal_baseline = normal_rows.get((workload, "pillow"))
        if normal_baseline is None:
            raise ValueError(f"Pillow timing is missing for {workload}")
        for field in ("policy", "context", "sample_unit", "sample_count"):
            if normal_baseline[field] != row[field]:
                raise ValueError(f"Pillow and Pillow-SIMD {field} differ for {workload}")
        row = dict(row, subject="pillow-simd", comparison_group="default")
        snapshot["rows"].append(row)

    snapshot["measured_at"] = max(snapshot["measured_at"], simd_snapshot["measured_at"])
    snapshot["run_id"] = f"{first_identity['run_id']}+{simd_identity['run_id']}"
    snapshot["source_sha256"] = source_digest
    snapshot["policy_status"] = "Paired x86 cohort; Pillow 12.1.1 and Pillow-SIMD 12.1.1.post0 are separately parity-gated against pillow-rs."
    snapshot["notes"] = [
        "This x86-only cohort compares pillow-rs CPU/SIMD, ordinary Pillow 12.1.1, and Pillow-SIMD 12.1.1.post0 on identical workloads and measurement policies.",
        "Pillow-SIMD and Pillow were run in separate virtual environments because both install the PIL namespace. Each source was independently parity-gated against pillow-rs before timings were accepted.",
        "Rows cover only currently maintained, full-size parity-backed workloads for BoxBlur, GaussianBlur, and getchannel. Operations with no comparable full-size workload are listed as not measured on the published page.",
        "This cohort uses the Pillow-SIMD default SSE4 build. It is not comparable with the Apple ARM measurements on the main benchmark page.",
    ]
    snapshot["cohorts"] = [
        {
            "id": "pillow",
            "run_id": first_identity["run_id"],
            "parity_run_id": parity_run_ids[0],
            "version": PILLOW_VERSION,
        },
        {
            "id": "pillow-simd",
            "run_id": simd_identity["run_id"],
            "parity_run_id": parity_run_ids[1],
            "version": PILLOW_SIMD_VERSION,
        },
    ]
    snapshot.update(
        schema="public-docs/benchmark-snapshot@1",
        repository="appunni-m/pillow-rs",
        source_sha256=source_digest,
    )
    return snapshot


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pillow-python", default=os.environ.get("PILLOW_ORACLE_PYTHON"))
    parser.add_argument("--pillow-simd-python", default=os.environ.get("PILLOW_SIMD_PYTHON"))
    parser.add_argument("--output", type=Path, default=ROOT / "build/migration-parity/pillow-simd-public-snapshot.json")
    args = parser.parse_args()
    if not args.pillow_python or not args.pillow_simd_python:
        parser.error("set PILLOW_ORACLE_PYTHON and PILLOW_SIMD_PYTHON to isolated interpreter paths")
    if platform.machine().lower() not in {"x86_64", "amd64"}:
        raise SystemExit("Pillow-SIMD comparison benchmarks require an x86_64 host")
    workloads = workload_contracts()
    ids = list(workloads)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    root = ROOT / "build" / "migration-parity"
    results = []
    parity_run_ids = []
    for name, version, python_path, suffix in (
        ("Pillow", PILLOW_VERSION, args.pillow_python, "pillow"),
        ("Pillow-SIMD", PILLOW_SIMD_VERSION, args.pillow_simd_python, "pillow-simd"),
    ):
        result_path = root / f"benchmark-result-{suffix}.json"
        parity_path = root / f"benchmark-parity-result-{suffix}.json"
        result, parity_run_id = run_source_cohort(
            source_python=python_path,
            source_name=name,
            source_version=version,
            output=result_path,
            parity_output=parity_path,
            workload_ids=ids,
        )
        results.append(result)
        parity_run_ids.append(parity_run_id)
    source_files = [
        root / filename
        for filename in (
            "benchmark-result-pillow.json",
            "benchmark-parity-result-pillow.json",
            "benchmark-result-pillow-simd.json",
            "benchmark-parity-result-pillow-simd.json",
        )
    ]
    digest = hashlib.sha256(
        b"".join(path.name.encode() + path.read_bytes() for path in source_files)
    ).hexdigest()
    snapshot = combine_results(
        results[0], results[1], digest, parity_run_ids=tuple(parity_run_ids)
    )
    validate(snapshot, "appunni-m/pillow-rs")
    args.output.write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "completed", "workloads": len(ids), "subjects": 4}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
