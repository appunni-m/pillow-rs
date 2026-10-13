#!/usr/bin/env python3
"""Collect parity-gated x86 timings for Pillow-SIMD's material workloads.

Pillow-SIMD and Pillow share the ``PIL`` import namespace, so this driver runs
them in isolated virtual environments.  It keeps the paired Pillow and
pillow-rs measurements in one benchmark cohort and exports the second source
timing as a separate Pillow-SIMD subject.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import gzip
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
    from run_migration_benchmark import (
        DEFAULT_MANIFEST,
        load_benchmarks,
        load_manifest,
        load_parity_cases,
        profile_applies_to_case,
        target_profile_for_backend,
    )
    from run_migration_parity import build_operation_index, compare_case
except ModuleNotFoundError:  # imported as ``scripts.run_pillow_simd_benchmark`` in tests
    from scripts.docs_evidence import project_pillow, validate
    from scripts.run_migration_benchmark import (
        DEFAULT_MANIFEST,
        load_benchmarks,
        load_manifest,
        load_parity_cases,
        profile_applies_to_case,
        target_profile_for_backend,
    )
    from scripts.run_migration_parity import build_operation_index, compare_case


ROOT = Path(__file__).resolve().parents[1]
WORKLOAD_IDS = (
    "pipeline-op.boxblur.material-l-noise-1024x768-radius-1",
    "pipeline-op.boxblur.material-rgb-noise-1024x768-radius-1",
    "pipeline-op.gaussianblur.material-l-noise-1024x768",
    "pipeline-op.gaussianblur.material-la-noise-1024x768",
    "pipeline-op.unsharpmask.material-rgb-noise-1024x768-radius-2",
    "pipeline-op.convert-mode-cmyk-to-rgb-1024x768",
    "pipeline-op.thumbnail.material-rgb-1024x768",
    "pipeline-op.merge.material-rgb-l-1024x768",
    "pipeline-op.posterize.material-l-noise-1024x768",
    "pipeline-op.posterize.material-rgb-noise-1024x768",
    "pipeline-op.medianfilter.material-l-noise-1024x768",
    "pipeline-op.medianfilter.material-la-noise-1024x768",
    "pipeline-op.medianfilter.material-rgb-noise-1024x768",
    "pipeline-op.maxfilter.material-rgb-noise-1024x768",
    "pipeline-op.minfilter.material-rgb-noise-1024x768",
    "pipeline-op.maxfilter.material-l-noise-1024x768",
    "pipeline-op.maxfilter.material-la-noise-1024x768",
    "pipeline-op.minfilter.material-l-noise-1024x768",
    "pipeline-op.minfilter.material-la-noise-1024x768",
    "pipeline-op.grayscale.material-rgb-noise-1024x768",
    "pipeline-op.grayscale.material-ycbcr-noise-1024x768",
    "pipeline-op.thumbnail.native-f32-1024x768",
    "pipeline-op.thumbnail.native-i32-1024x768",
    "pipeline-op.filter.material-l-noise-5x5-1024x768",
    "pipeline-op.filter.material-l-noise-flat-5x5-1024x768",
    "pipeline-op.filter.material-la-noise-5x5-1024x768",
    "pipeline-op.filter.material-la-noise-3x3-1024x768",
    "pipeline-op.filter.material-i-noise-5x5-1024x768",
    "pipeline-op.filter.material-i-bounded-noise-5x5-1024x768",
    "pil-imagefilter.gaussianblur.standard",
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
    oracle_python: str,
    output: Path,
    parity_output: Path,
    workload_ids: list[str],
    reuse_parity: Path | None = None,
) -> tuple[dict[str, Any], str]:
    env = os.environ.copy()
    env.update(
        {
            "MIGRATION_BENCHMARK_COHORT": "pillow-simd",
            "MIGRATION_BENCHMARK_BACKENDS": "cpu,simd",
            "MIGRATION_ORACLE_PYTHON": oracle_python,
            "MIGRATION_ORACLE_NAME": "Pillow",
            "MIGRATION_ORACLE_VERSION": PILLOW_VERSION,
            "MIGRATION_BENCHMARK_SOURCE_PYTHON": source_python,
            "MIGRATION_BENCHMARK_SOURCE_NAME": source_name,
            "MIGRATION_BENCHMARK_SOURCE_VERSION": source_version,
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
    if reuse_parity is not None:
        command.extend(("--reuse-parity", str(reuse_parity)))
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
    try:
        validate_run(
            result,
            parity,
            source_name,
            source_version,
            workload_ids,
            oracle_name="Pillow",
            oracle_version=PILLOW_VERSION,
        )
    except ValueError:
        if parity.get("summary", {}).get("failed", 0) or any(
            comparison.get("outcome") != "pass"
            for comparison in parity.get("comparisons", [])
        ):
            source_slug = source_name.lower().replace(" ", "-")
            diagnostic_path = parity_output.with_name(
                f"pillow-simd-parity-failure-{source_slug}.json"
            )
            write_parity_failure_diagnostics(
                diagnostic_path, source_name, source_version, parity
            )
        raise
    return result, str(parity["identity"]["run_id"])


def validate_run(
    result: dict[str, Any],
    parity: dict[str, Any],
    source_name: str,
    source_version: str,
    workload_ids: list[str],
    *,
    oracle_name: str = "Pillow",
    oracle_version: str = PILLOW_VERSION,
) -> None:
    identity = result.get("identity", {})
    oracles = identity.get("oracles", [])
    if result.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError(f"{source_name} result has an unknown benchmark schema")
    if result.get("status") != "completed" or not oracles or (
        oracles[0].get("name"), oracles[0].get("version")
    ) != (oracle_name, oracle_version):
        raise ValueError(
            f"{source_name} result identity/status is invalid: expected parity oracle "
            f"{oracle_name} {oracle_version}"
        )
    if [item.get("workload_id") for item in result.get("workloads", [])] != workload_ids:
        raise ValueError(f"{source_name} result changed the selected workload set")
    if parity.get("status") != "completed":
        raise ValueError(f"{source_name} parity preflight did not complete")
    failed = [
        summarize_failed_comparison(comparison)
        for comparison in parity.get("comparisons", [])
        if comparison.get("outcome") != "pass"
    ]
    reported_failures = parity.get("summary", {}).get("failed")
    if reported_failures != 0 or failed:
        raise ValueError(
            f"{source_name} parity preflight had {reported_failures} reported failing "
            f"comparison(s), with {len(failed)} non-passing row(s): {failed[:40]}"
        )
    if parity.get("infrastructure_errors"):
        raise ValueError(f"{source_name} parity preflight has infrastructure errors")
    timed_source_id = "pillow-simd" if source_name == "Pillow-SIMD" else "pillow"
    for workload in result["workloads"]:
        if workload["correctness"].get("outcome") != "pass":
            raise ValueError(
                f"{source_name} parity gate failed for {workload['workload_id']}"
            )
        subjects = {item["id"]: item for item in workload["subjects"]}
        if set(subjects) != {timed_source_id, "python-cpu", "python-simd"}:
            raise ValueError(f"{source_name} result has an unexpected subject set")
        if any(subjects[name]["status"] != "completed" for name in subjects):
            raise ValueError(
                f"{source_name} has an incomplete subject for {workload['workload_id']}"
            )


def summarize_failed_comparison(comparison: dict[str, Any]) -> dict[str, Any]:
    """Keep failure summaries small while reporting exact byte mismatch locations."""

    summary: dict[str, Any] = {
        "case_id": comparison.get("case_id"),
        "target_profile": comparison.get("target_profile"),
        "outcome": comparison.get("outcome"),
    }
    source_observations = {
        item.get("step_id"): item
        for item in comparison.get("source", {}).get("observations", [])
    }
    target_observations = {
        item.get("step_id"): item
        for item in comparison.get("target", {}).get("observations", [])
    }
    compact_diffs: list[dict[str, Any]] = []
    for diff in comparison.get("diffs", [])[:3]:
        compact: dict[str, Any] = {
            key: diff[key]
            for key in ("path", "kind", "message")
            if key in diff
        }
        if diff.get("kind") == "bytes_mismatch":
            step_id = diff.get("step_id")
            source_value = source_observations.get(step_id, {}).get("value")
            target_value = target_observations.get(step_id, {}).get("value")
            source_data = _decode_byte_observation(source_value)
            target_data = _decode_byte_observation(target_value)
            if source_data is not None and target_data is not None:
                mismatch_count = 0
                first_differences = []
                for index, (source_byte, target_byte) in enumerate(
                    zip(source_data, target_data)
                ):
                    if source_byte == target_byte:
                        continue
                    mismatch_count += 1
                    if len(first_differences) < 8:
                        first_differences.append(
                            {
                                "offset": index,
                                "source": source_byte,
                                "target": target_byte,
                            }
                        )
                compact["byte_comparison"] = {
                    "source_length": len(source_data),
                    "target_length": len(target_data),
                    "different_bytes_in_shared_range": mismatch_count,
                    "first_differences": first_differences,
                }
        compact_diffs.append(compact)
    if compact_diffs:
        summary["diffs"] = compact_diffs
    return summary


def write_parity_failure_diagnostics(
    path: Path,
    source_name: str,
    source_version: str,
    parity: dict[str, Any],
) -> None:
    """Persist compact mismatch summaries without copying full image payloads."""

    failures = [
        summarize_failed_comparison(comparison)
        for comparison in parity.get("comparisons", [])
        if comparison.get("outcome") != "pass"
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "pillow-simd/parity-failure-diagnostics@1",
                "source": {"name": source_name, "version": source_version},
                "parity_identity": parity.get("identity", {}),
                "failed_comparisons": failures,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


def _decode_byte_observation(value: Any) -> bytes | None:
    if (
        not isinstance(value, dict)
        or value.get("kind") != "bytes"
        or value.get("encoding") != "base64"
        or not isinstance(value.get("data"), str)
    ):
        return None
    try:
        return base64.b64decode(value["data"], validate=True)
    except (ValueError, binascii.Error):
        return None


def read_parity_artifact(path: Path) -> dict[str, Any]:
    if path.name.endswith(".gz"):
        with gzip.open(path, "rt", encoding="utf-8") as stream:
            return json.load(stream)
    return json.loads(path.read_text(encoding="utf-8"))


def compare_source_compatibility(
    case: dict[str, Any],
    pillow_source: dict[str, Any],
    pillow_simd_source: dict[str, Any],
    operation_index: dict[tuple[str, str], dict[str, Any]],
) -> tuple[str, list[dict[str, Any]]]:
    """Apply the declared observation comparator to the two source outputs."""

    return compare_case(case, pillow_source, pillow_simd_source, operation_index)


def run_source_compatibility(
    *,
    pillow_python: str,
    pillow_simd_python: str,
    pillow_parity_path: Path,
    workload_ids: list[str],
    output: Path,
) -> tuple[dict[str, dict[str, Any]], list[Path]]:
    """Compare Pillow-SIMD's outputs directly with Pillow before using its times."""

    manifest_data = load_manifest(DEFAULT_MANIFEST)
    cases_by_id, _ = load_parity_cases(manifest_data)
    workloads = workload_contracts()
    selected_case_ids = list(
        dict.fromkeys(workloads[item]["input"]["case_id"] for item in workload_ids)
    )
    cases = {case_id: cases_by_id[case_id] for case_id in selected_case_ids}
    operation_index = build_operation_index(manifest_data)
    pillow_parity = read_parity_artifact(pillow_parity_path)
    pillow_comparisons = {
        (item["case_id"], item["target_profile"]): item
        for item in pillow_parity.get("comparisons", [])
    }

    compatibility_by_case: dict[str, dict[str, Any]] = {}
    run_ids: list[dict[str, str]] = []
    artifacts: list[Path] = []
    for backend in ("cpu", "simd"):
        profile = target_profile_for_backend(backend)
        backend_cases = [
            cases[case_id]
            for case_id in selected_case_ids
            if profile_applies_to_case(profile, cases[case_id])
        ]
        if not backend_cases:
            continue
        artifact_path = output.parent / (
            f"benchmark-source-compatibility-{profile}.json.gz"
        )
        artifacts.append(artifact_path)
        command = [
            sys.executable,
            str(ROOT / "scripts" / "run_migration_parity.py"),
            "--manifest",
            str(DEFAULT_MANIFEST),
            "--output",
            str(artifact_path),
            "--timeout",
            "7200",
        ]
        for case in backend_cases:
            command.extend(("--case-id", case["case_id"]))
        env = os.environ.copy()
        env.update(
            {
                "MIGRATION_ORACLE_PYTHON": pillow_simd_python,
                "MIGRATION_ORACLE_NAME": "Pillow-SIMD",
                "MIGRATION_ORACLE_VERSION": PILLOW_SIMD_VERSION,
                "MIGRATION_TARGET_BACKEND": backend,
                "MIGRATION_STRICT_TARGET_BACKEND": "1",
            }
        )
        env.pop("MIGRATION_PARITY_TIMED_SOURCE", None)
        target_python = str((ROOT / "pillow-rs-py" / "python").resolve())
        inherited_pythonpath = env.get("PYTHONPATH", "")
        env["PYTHONPATH"] = (
            target_python + os.pathsep + inherited_pythonpath
            if inherited_pythonpath
            else target_python
        )
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            check=False,
            text=True,
            capture_output=True,
            timeout=7200,
        )
        if not artifact_path.is_file():
            detail = (completed.stderr or completed.stdout)[-3000:]
            raise RuntimeError(
                f"Pillow-SIMD compatibility parity did not emit {profile}: {detail}"
            )
        parity = read_parity_artifact(artifact_path)
        if (
            parity.get("status") != "completed"
            or parity.get("infrastructure_errors")
            or completed.returncode not in {0, 1}
        ):
            raise RuntimeError(
                f"Pillow-SIMD compatibility parity infrastructure failed for {profile}: "
                f"{parity.get('infrastructure_errors', [])}"
            )
        identity = parity.get("identity", {})
        oracle = (identity.get("oracles") or [{}])[0]
        if (oracle.get("name"), oracle.get("version")) != (
            "Pillow-SIMD",
            PILLOW_SIMD_VERSION,
        ):
            raise ValueError(f"{profile} compatibility run used the wrong source")
        run_ids.append({"target_profile": profile, "run_id": identity.get("run_id", "")})

        expected_keys = {
            (case["case_id"], profile) for case in backend_cases
        }
        actual_keys = {
            (item.get("case_id"), item.get("target_profile"))
            for item in parity.get("comparisons", [])
        }
        if actual_keys != expected_keys:
            raise ValueError(
                f"Pillow-SIMD compatibility receipt has the wrong case set for {profile}"
            )

        for item in parity.get("comparisons", []):
            case_id = item["case_id"]
            reference = pillow_comparisons.get((case_id, profile))
            if reference is None:
                raise ValueError(
                    f"Pillow parity receipt is missing {case_id} for {profile}"
                )
            if reference.get("outcome") != "pass":
                raise ValueError(
                    f"regular Pillow parity did not pass for {case_id} on {profile}"
                )
            stable_target, stable_diffs = compare_case(
                cases[case_id], reference["target"], item["target"], operation_index
            )
            if stable_target != "pass":
                raise RuntimeError(
                    f"target output changed between parity cohorts for {case_id} on "
                    f"{profile}: {stable_diffs[:3]}"
                )
            outcome, diffs = compare_source_compatibility(
                cases[case_id], reference["source"], item["source"], operation_index
            )
            if (outcome == "pass") != (item.get("outcome") == "pass"):
                raise RuntimeError(
                    f"Pillow-SIMD source compatibility conflicts with target comparison "
                    f"for {case_id} on {profile}"
                )
            compact_diffs = [
                {
                    key: diff[key]
                    for key in ("path", "kind", "message")
                    if key in diff
                }
                for diff in diffs
            ]
            previous = compatibility_by_case.get(case_id)
            current = {"outcome": outcome, "differences": compact_diffs}
            if previous is not None and previous != current:
                raise RuntimeError(
                    f"Pillow-SIMD compatibility changed across target profiles for {case_id}"
                )
            compatibility_by_case[case_id] = current

    report = {
        "schema": "pillow-simd/source-compatibility@1",
        "reference": {"name": "Pillow", "version": PILLOW_VERSION},
        "subject": {"name": "Pillow-SIMD", "version": PILLOW_SIMD_VERSION},
        "status": "completed",
        "parity_runs": run_ids,
        "workloads": [
            {
                "workload_id": workload_id,
                "case_id": workloads[workload_id]["input"]["case_id"],
                **compatibility_by_case.get(
                    workloads[workload_id]["input"]["case_id"],
                    {"outcome": "not_proven", "differences": []},
                ),
            }
            for workload_id in workload_ids
        ],
    }
    missing = [
        workload_id
        for workload_id in workload_ids
        if workloads[workload_id]["input"]["case_id"] not in compatibility_by_case
    ]
    if missing:
        raise ValueError(
            f"Pillow-SIMD compatibility is unproven for selected workloads: {missing}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return (
        {
            item["workload_id"]: {
                "outcome": item["outcome"],
                "differences": item["differences"],
            }
            for item in report["workloads"]
        },
        [*artifacts, output],
    )


def append_github_failure_summary(source_name: str, detail: object) -> None:
    """Expose concise benchmark failure context on public Actions run summaries."""
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    text = str(detail).strip()[-5000:].replace("```", "` ` `")
    with Path(summary_path).open("a", encoding="utf-8") as summary:
        summary.write(f"### {source_name} benchmark failed\n\n```text\n{text}\n```\n")


def append_github_source_compatibility_summary(
    source_compatibility: dict[str, dict[str, Any]],
) -> None:
    """Show baseline output incompatibilities without turning them into target failures."""

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    incompatible = [
        (workload, result.get("outcome", "not_proven"))
        for workload, result in sorted(source_compatibility.items())
        if result.get("outcome") != "pass"
    ]
    with Path(summary_path).open("a", encoding="utf-8") as summary:
        summary.write(
            "### Pillow-SIMD source compatibility\n\n"
            "pillow-rs correctness remains gated against regular Pillow. "
            "Pillow-SIMD speed ratios require exact source-output compatibility.\n\n"
        )
        if incompatible:
            summary.write("| Workload | Pillow-SIMD compatibility |\n| --- | --- |\n")
            for workload, outcome in incompatible:
                summary.write(f"| {workload} | {outcome} |\n")
        else:
            summary.write("All selected Pillow-SIMD outputs matched regular Pillow.\n")


def combine_results(
    pillow: dict[str, Any],
    pillow_simd: dict[str, Any],
    source_digest: str,
    *,
    parity_run_ids: tuple[str, str] = ("", ""),
    source_compatibility: dict[str, dict[str, Any]] | None = None,
) -> dict[str, Any]:
    """Build one x86 comparison snapshot with a version-matched Pillow baseline."""
    first_identity = pillow["identity"]
    simd_identity = pillow_simd["identity"]
    if source_compatibility is None:
        raise ValueError("Pillow-SIMD source compatibility evidence is required")
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
        row = simd_rows.get((workload, "pillow-simd")) or simd_rows.get(
            (workload, "pillow")
        )
        if row is None:
            raise ValueError(f"Pillow-SIMD timing is missing for {workload}")
        normal_baseline = normal_rows.get((workload, "pillow"))
        if normal_baseline is None:
            raise ValueError(f"Pillow timing is missing for {workload}")
        for field in ("policy", "context", "sample_unit", "sample_count"):
            if normal_baseline[field] != row[field]:
                raise ValueError(f"Pillow and Pillow-SIMD {field} differ for {workload}")
        compatibility = source_compatibility.get(workload)
        if compatibility is None:
            raise ValueError(f"Pillow-SIMD compatibility is missing for {workload}")
        outcome = compatibility.get("outcome", "not_proven")
        if outcome not in {"pass", "fail", "not_proven"}:
            raise ValueError(
                f"invalid Pillow-SIMD compatibility outcome for {workload}: {outcome}"
            )
        row = dict(
            row,
            subject="pillow-simd",
            comparison_group="default",
            correctness=f"Pillow-SIMD compatibility: {outcome}",
        )
        snapshot["rows"].append(row)

    snapshot["measured_at"] = max(snapshot["measured_at"], simd_snapshot["measured_at"])
    snapshot["run_id"] = f"{first_identity['run_id']}+{simd_identity['run_id']}"
    snapshot["source_sha256"] = source_digest
    incompatible = sorted(
        workload
        for workload, compatibility in source_compatibility.items()
        if compatibility.get("outcome") == "fail"
    )
    not_proven = sorted(
        workload
        for workload, compatibility in source_compatibility.items()
        if compatibility.get("outcome") == "not_proven"
    )
    snapshot["policy_status"] = (
        "Paired x86 cohort; pillow-rs CPU/SIMD correctness is gated against regular "
        "Pillow 12.1.1. Pillow-SIMD timings are compared only where its outputs also "
        "match regular Pillow."
    )
    snapshot["notes"] = [
        "This x86-only cohort compares pillow-rs CPU/SIMD, ordinary Pillow 12.1.1, and Pillow-SIMD 12.1.1.post0 on identical workloads and measurement policies.",
        "Pillow-SIMD and Pillow were run in separate virtual environments because both install the PIL namespace. pillow-rs CPU/SIMD parity uses regular Pillow; Pillow-SIMD compatibility is checked separately and incompatible baseline cells remain visible with their speed ratios withheld.",
        f"The matched cohort covers {len(WORKLOAD_IDS)} full-size, parity-backed individual-operation cases: all 29 current pipeline-op cases, the established RGB GaussianBlur case, and getchannel in L, LA, RGB, and RGBA. Every workload uses identical inputs and measurement policy across the three sources.",
        "This cohort uses the Pillow-SIMD default SSE4 build. It is not comparable with the Apple ARM measurements on the main benchmark page.",
        *(
            [
                f"Pillow-SIMD output differed from regular Pillow on {len(incompatible)} workload(s); those baseline cells are retained and no Pillow-SIMD speed ratio is accepted: {', '.join(incompatible)}."
            ]
            if incompatible
            else ["Pillow-SIMD outputs matched regular Pillow on every selected workload."]
        ),
        *(
            [
                f"Pillow-SIMD compatibility was not proven on {len(not_proven)} workload(s); those baseline cells remain visible and no speed ratio is accepted: {', '.join(not_proven)}."
            ]
            if not_proven
            else []
        ),
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
            "compatibility_outcome": (
                "not_proven" if not_proven else "fail" if incompatible else "pass"
            ),
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
    pillow_parity_path = root / "benchmark-parity-result-pillow.json"
    for name, version, python_path, suffix in (
        ("Pillow", PILLOW_VERSION, args.pillow_python, "pillow"),
        ("Pillow-SIMD", PILLOW_SIMD_VERSION, args.pillow_simd_python, "pillow-simd"),
    ):
        result_path = root / f"benchmark-result-{suffix}.json"
        parity_path = root / f"benchmark-parity-result-{suffix}.json"
        try:
            result, parity_run_id = run_source_cohort(
                source_python=python_path,
                source_name=name,
                source_version=version,
                oracle_python=args.pillow_python,
                output=result_path,
                parity_output=parity_path,
                workload_ids=ids,
                reuse_parity=(pillow_parity_path if suffix == "pillow-simd" else None),
            )
        except Exception as error:
            append_github_failure_summary(name, error)
            raise
        results.append(result)
        parity_run_ids.append(parity_run_id)
    source_compatibility, compatibility_files = run_source_compatibility(
        pillow_python=args.pillow_python,
        pillow_simd_python=args.pillow_simd_python,
        pillow_parity_path=pillow_parity_path,
        workload_ids=ids,
        output=root / "benchmark-source-compatibility-pillow-simd.json",
    )
    append_github_source_compatibility_summary(source_compatibility)
    source_files = [
        root / filename
        for filename in (
            "benchmark-result-pillow.json",
            "benchmark-parity-result-pillow.json",
            "benchmark-result-pillow-simd.json",
            "benchmark-parity-result-pillow-simd.json",
        )
    ] + compatibility_files
    digest = hashlib.sha256(
        b"".join(path.name.encode() + path.read_bytes() for path in source_files)
    ).hexdigest()
    snapshot = combine_results(
        results[0],
        results[1],
        digest,
        parity_run_ids=tuple(parity_run_ids),
        source_compatibility=source_compatibility,
    )
    validate(snapshot, "appunni-m/pillow-rs")
    args.output.write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "status": "completed",
                "workloads": len(ids),
                "subjects": 4,
                "pillow_simd_compatible": sum(
                    result["outcome"] == "pass"
                    for result in source_compatibility.values()
                ),
                "pillow_simd_incompatible": sum(
                    result["outcome"] == "fail"
                    for result in source_compatibility.values()
                ),
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
