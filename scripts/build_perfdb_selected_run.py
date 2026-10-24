#!/usr/bin/env python3
"""Build one hash-bound PerfDB selected-run admission package.

The package carries every report-referenced input document, deduplicated by
content hash, plus the manifest and build witness. Report and attestation files
remain at their existing paths so callers can upload them without duplicating
large reports.
"""

from __future__ import annotations

import argparse
import copy
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
from pathlib import PurePosixPath
import shutil
import tomllib
from typing import Any

import yaml

try:
    import migration_execution_attestation
    import migration_producer_identity
    import run_migration_benchmark
except ModuleNotFoundError:
    from scripts import migration_execution_attestation, migration_producer_identity, run_migration_benchmark


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
BENCHMARK_SCHEMA = "migration-parity/benchmark-result@1"
PARITY_SCHEMAS = {
    "migration-parity/parity-result@1",
    "migration-parity/parity-result@2",
}
BENCHMARK_INPUT_SCHEMA = "migration-parity/benchmark-input@1"
PARITY_INPUT_SCHEMA = "migration-parity/parity-input@2"
MANIFEST_SCHEMA = "migration-parity/manifest@3"
EXECUTION_SCHEMA = "migration-parity/execution-attestation@1"
PRODUCER_SCHEMA_V1 = "migration-parity/producer-attestation@1"
PRODUCER_SCHEMA_V2 = "migration-parity/producer-attestation@2"
PRODUCER_SCHEMA = PRODUCER_SCHEMA_V2
PRODUCER_BUNDLE_SCHEMA = "migration-parity/producer-bundle@1"
BUILD_WITNESS_SCHEMA = "pillow-rs/build-witness@1"
MATERIALIZED_BUILD_WITNESS_SCHEMA = "pillow-rs/build-witness@2"
BUILD_WITNESS_SCHEMAS = {BUILD_WITNESS_SCHEMA, MATERIALIZED_BUILD_WITNESS_SCHEMA}
BACKEND_FOR_PROFILE = {
    "python-cpu": "cpu",
    "python-simd": "simd",
    "python-gpu": "gpu",
    "python-parallel-cpu": "cpu",
}


def read_json(path: Path) -> dict[str, Any]:
    opener = gzip.open if path.name.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as stream:
        value = json.load(stream)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def report_fingerprint(path: Path) -> dict[str, Any]:
    container = path.read_bytes()
    canonical = gzip.decompress(container) if path.name.endswith(".gz") else container
    return {
        "path": str(path.resolve()),
        "canonical_sha256": sha256_bytes(canonical),
        "canonical_bytes": len(canonical),
        "container_sha256": sha256_bytes(container),
        "container_bytes": len(container),
    }


def manifest_identity(report: dict[str, Any], label: str) -> dict[str, str]:
    identity = report.get("identity")
    manifest = identity.get("manifest") if isinstance(identity, dict) else None
    if not isinstance(manifest, dict):
        raise ValueError(f"{label} report has no manifest identity")
    if manifest.get("schema") != MANIFEST_SCHEMA:
        raise ValueError(f"{label} report references an unsupported manifest")
    path = manifest.get("path")
    digest = manifest.get("sha256")
    if not isinstance(path, str) or not isinstance(digest, str):
        raise ValueError(f"{label} report has an invalid manifest reference")
    return {"path": path, "schema": MANIFEST_SCHEMA, "sha256": digest}


def input_references(report: dict[str, Any], label: str) -> list[dict[str, str]]:
    identity = report.get("identity")
    inputs = identity.get("inputs") if isinstance(identity, dict) else None
    if not isinstance(inputs, list) or not inputs:
        raise ValueError(f"{label} report has no input references")
    references = []
    for reference in inputs:
        if (
            not isinstance(reference, dict)
            or not isinstance(reference.get("path"), str)
            or not isinstance(reference.get("schema"), str)
            or not isinstance(reference.get("sha256"), str)
        ):
            raise ValueError(f"{label} report has an invalid input reference")
        references.append(
            {
                "path": reference["path"],
                "schema": reference["schema"],
                "sha256": reference["sha256"],
            }
        )
    return references


def resolve_report_inputs(
    report: dict[str, Any], fixture_root: Path, label: str
) -> list[tuple[dict[str, str], Path, dict[str, Any]]]:
    resolved = []
    for reference in input_references(report, label):
        path = resolve_input(reference, fixture_root, label)
        document = read_json(path)
        if document.get("schema") != reference["schema"]:
            raise ValueError(f"{label} input document schema differs from its reference")
        resolved.append((reference, path, document))
    return resolved


def select_input_for_item(
    inputs: list[tuple[dict[str, str], Path, dict[str, Any]]],
    *,
    schema: str,
    collection: str,
    identifier_field: str,
    identifier: str,
    label: str,
) -> tuple[dict[str, str], Path, dict[str, Any]]:
    selected = []
    for reference, path, document in inputs:
        if reference["schema"] != schema:
            continue
        rows = document.get(collection, [])
        if any(
            isinstance(row, dict) and row.get(identifier_field) == identifier
            for row in rows
        ):
            selected.append((reference, path, document))
    if len(selected) != 1:
        raise ValueError(
            f"{label} report must reference exactly one {schema} input containing "
            f"{identifier_field} {identifier}"
        )
    return selected[0]


def resolve_input(reference: dict[str, str], fixture_root: Path, label: str) -> Path:
    relative = Path(reference["path"])
    if relative.is_absolute():
        raise ValueError(f"{label} input path must be relative to the fixture root")
    path = (fixture_root / relative).resolve(strict=True)
    try:
        path.relative_to(fixture_root.resolve(strict=True))
    except ValueError as exc:
        raise ValueError(f"{label} input escapes the fixture root") from exc
    if sha256_bytes(path.read_bytes()) != reference["sha256"]:
        raise ValueError(f"{label} input bytes differ from the report reference")
    return path


def verify_execution_attestation(
    path: Path, report_path: Path, report: dict[str, Any], label: str
) -> dict[str, Any]:
    attestation = read_json(path)
    if attestation.get("schema") != EXECUTION_SCHEMA or attestation.get("status") != "complete":
        raise ValueError(f"{label} execution attestation is not complete")
    if attestation.get("errors") != []:
        raise ValueError(f"{label} execution attestation contains errors")
    identity = report.get("identity", {})
    actual_report = report_fingerprint(report_path)
    attested_report = attestation.get("report")
    if not isinstance(attested_report, dict):
        raise ValueError(f"{label} execution attestation has no report binding")
    original_path = attested_report.get("path")
    if not isinstance(original_path, str) or not Path(original_path).is_absolute():
        raise ValueError(f"{label} execution attestation has an invalid original report path")
    expected_report = {
        **actual_report,
        "path": original_path,
        "schema": report.get("schema"),
        "run_id": identity.get("run_id"),
    }
    if attested_report != expected_report:
        raise ValueError(f"{label} execution attestation does not bind its report bytes")
    if attestation.get("manifest") != identity.get("manifest"):
        raise ValueError(f"{label} execution attestation manifest differs from its report")
    if attestation.get("inputs") != identity.get("inputs"):
        raise ValueError(f"{label} execution attestation inputs differ from its report")
    try:
        migration_execution_attestation.validate_attestation(
            attestation,
            result=report,
            report_binding=expected_report,
            allow_duplicate_parity_sources=(
                run_migration_benchmark.is_benchmark_runner_argv(
                    report.get("identity", {}).get("command", {}).get("argv")
                )
            ),
        )
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
        raise ValueError(f"{label} execution closure is invalid: {exc}") from exc
    return attestation


def verify_producer_attestation(
    producer_path: Path,
    bundle_path: Path,
    execution_path: Path,
    report: dict[str, Any],
    label: str,
    producer_path_maps: dict[str, str] | None = None,
) -> None:
    producer = read_json(producer_path)
    bundle = read_json(bundle_path)
    execution = read_json(execution_path)
    producer_schema = producer.get("schema")
    if producer_schema not in {PRODUCER_SCHEMA_V1, PRODUCER_SCHEMA_V2} or producer.get("status") != "complete":
        raise ValueError(f"{label} producer attestation is not complete")
    if producer.get("errors") != []:
        raise ValueError(f"{label} producer attestation contains errors")
    if bundle.get("schema") != PRODUCER_BUNDLE_SCHEMA:
        raise ValueError(f"{label} producer bundle schema is unsupported")
    if producer.get("producer_bundle_sha256") != sha256_bytes(bundle_path.read_bytes()):
        raise ValueError(f"{label} producer bundle hash differs from its bytes")
    bundle, producer = rehost_producer_evidence_for_validation(
        bundle, producer, producer_path_maps or {}
    )
    try:
        migration_producer_identity.validate_producer_bundle(bundle)
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
        raise ValueError(f"{label} producer bundle is invalid: {exc}") from exc
    if producer.get("execution_attestation_sha256") != sha256_bytes(execution_path.read_bytes()):
        raise ValueError(f"{label} producer attestation execution hash differs")
    identity = report.get("identity", {})
    if producer.get("report") != {
        "canonical_sha256": report_fingerprint_from_attestation(execution)["canonical_sha256"],
        "schema": report.get("schema"),
        "run_id": identity.get("run_id"),
    }:
        raise ValueError(f"{label} producer attestation report binding differs")
    producer_fields = {
        "schema",
        "status",
        "report",
        "execution_attestation_sha256",
        "producer_bundle_sha256",
        "parent_start",
        "parent_end",
        "invocations",
        "errors",
    }
    if producer_schema == PRODUCER_SCHEMA_V2:
        producer_fields.add("observed_invocation")
        try:
            from migration_producer_attestation import validate_invocation_context
        except ModuleNotFoundError:
            from scripts.migration_producer_attestation import validate_invocation_context
        try:
            validate_invocation_context(producer.get("observed_invocation"))
        except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
            raise ValueError(f"{label} observed invocation is invalid: {exc}") from exc
    if set(producer) != producer_fields:
        raise ValueError(f"{label} producer attestation fields are unsupported")
    parent_start = producer.get("parent_start")
    parent_end = producer.get("parent_end")
    try:
        migration_producer_identity.validate_producer_identity(parent_start, bundle)
        migration_producer_identity.validate_producer_identity(parent_end, bundle)
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
        raise ValueError(f"{label} producer parent identity is invalid: {exc}") from exc
    if parent_start.get("role") != "parent" or parent_end.get("role") != "parent":
        raise ValueError(f"{label} producer parent identity has the wrong role")
    end_modules = {
        record["module"]: record for record in parent_end["loaded_modules"]
    }
    if any(
        end_modules.get(record["module"]) != record
        for record in parent_start["loaded_modules"]
    ):
        raise ValueError(f"{label} producer parent modules changed during capture")

    execution_invocations = execution.get("invocations")
    producer_invocations = producer.get("invocations")
    if not isinstance(execution_invocations, list) or not isinstance(producer_invocations, list):
        raise ValueError(f"{label} producer invocation closure is invalid")
    expected_ids = [record.get("invocation_id") for record in execution_invocations]
    actual_ids = []
    for record in producer_invocations:
        if (
            not isinstance(record, dict)
            or set(record) != {"invocation_id", "receipt", "errors"}
            or record.get("errors") != []
        ):
            raise ValueError(f"{label} producer invocation is not proven")
        invocation_id = record["invocation_id"]
        receipt = record["receipt"]
        if (
            not isinstance(invocation_id, str)
            or not invocation_id
            or not isinstance(receipt, dict)
            or set(receipt)
            != {
                "schema",
                "status",
                "invocation_id",
                "producer_bundle_sha256",
                "identity",
                "error",
            }
            or receipt.get("schema") != "migration-parity/child-producer-attestation@1"
            or receipt.get("status") != "complete"
            or receipt.get("invocation_id") != invocation_id
            or receipt.get("producer_bundle_sha256") != producer.get("producer_bundle_sha256")
            or receipt.get("error") is not None
        ):
            raise ValueError(f"{label} producer child receipt is not proven")
        try:
            migration_producer_identity.validate_producer_identity(
                receipt.get("identity"), bundle
            )
        except (OSError, RuntimeError, ValueError, TypeError, KeyError) as exc:
            raise ValueError(f"{label} producer child identity is invalid: {exc}") from exc
        if receipt["identity"].get("role") != "child":
            raise ValueError(f"{label} producer child identity has the wrong role")
        actual_ids.append(invocation_id)
    if not expected_ids or actual_ids != expected_ids:
        raise ValueError(f"{label} producer invocation membership differs from execution")


def rehost_producer_evidence_for_validation(
    bundle: dict[str, Any],
    producer: dict[str, Any],
    path_maps: dict[str, str],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Remap evidence paths in memory while preserving attested source files.

    The original bundle and producer attestation remain untouched and their
    hashes are checked by the caller. A path map only makes the exact frozen
    producer sources available to the strict local inventory validator.
    """

    if not path_maps:
        return bundle, producer

    original_root_value = bundle.get("root")
    if not isinstance(original_root_value, str) or not Path(original_root_value).is_absolute():
        raise ValueError("producer bundle root cannot be rehosted")
    original_root = PurePosixPath(original_root_value)
    root_destination = path_maps.get(original_root_value)
    if root_destination is not None and not Path(root_destination).is_dir():
        raise ValueError("producer path map destination root is not a directory")

    expected_files_sha = migration_producer_identity._canonical_json_sha256(
        bundle.get("files")
    )
    mapped_bundle = copy.deepcopy(bundle)

    def mapped_path(value: Any) -> Any:
        if not isinstance(value, str):
            return value
        path = PurePosixPath(value)
        try:
            suffix = path.relative_to(original_root)
        except ValueError:
            return path_maps.get(value, value)
        if root_destination is None:
            return value
        return str(PurePosixPath(root_destination) / suffix)

    if root_destination is not None:
        mapped_bundle["root"] = root_destination
    for record in mapped_bundle.get("files", []):
        if isinstance(record, dict):
            record["resolved_path"] = mapped_path(record.get("resolved_path"))
    entrypoints = mapped_bundle.get("entrypoints", {})
    if isinstance(entrypoints, dict):
        for record in entrypoints.values():
            if isinstance(record, dict):
                record["path"] = mapped_path(record.get("path"))
    for record in mapped_bundle.get("extras", []):
        if isinstance(record, dict):
            record["path"] = mapped_path(record.get("path"))

    def mapped_identity(identity: Any) -> None:
        if not isinstance(identity, dict):
            return
        if identity.get("files_sha256") != expected_files_sha:
            raise ValueError("producer identity does not bind the original bundle files")
        entrypoint = identity.get("entrypoint")
        if isinstance(entrypoint, dict):
            entrypoint["path"] = mapped_path(entrypoint.get("path"))
        for record in identity.get("loaded_modules", []):
            if isinstance(record, dict):
                record["origin"] = mapped_path(record.get("origin"))
                record["path"] = mapped_path(record.get("path"))
        identity["files_sha256"] = migration_producer_identity._canonical_json_sha256(
            mapped_bundle["files"]
        )

    mapped_producer = copy.deepcopy(producer)
    for key in ("parent_start", "parent_end"):
        mapped_identity(mapped_producer.get(key))
    for invocation in mapped_producer.get("invocations", []):
        if isinstance(invocation, dict):
            receipt = invocation.get("receipt")
            if isinstance(receipt, dict):
                mapped_identity(receipt.get("identity"))
    return mapped_bundle, mapped_producer


def parse_producer_path_maps(values: list[str]) -> dict[str, str]:
    """Parse explicit ORIGINAL=LOCAL paths used only during producer checks."""

    mappings: dict[str, str] = {}
    for value in values:
        original, separator, local = value.partition("=")
        original_path = PurePosixPath(original)
        if (
            not separator
            or not original_path.is_absolute()
            or original_path.as_posix() != original
            or any(part in {".", ".."} for part in original_path.parts)
        ):
            raise ValueError("producer path maps must use canonical absolute ORIGINAL paths")
        local_path = Path(local)
        if not local_path.is_absolute():
            raise ValueError("producer path map destinations must be absolute")
        try:
            resolved_local = str(local_path.resolve(strict=True))
        except (OSError, RuntimeError, ValueError) as exc:
            raise ValueError(f"producer path map destination cannot be resolved: {exc}") from exc
        if original in mappings and mappings[original] != resolved_local:
            raise ValueError("producer path map source is duplicated")
        mappings[original] = resolved_local
    return mappings


def report_fingerprint_from_attestation(attestation: dict[str, Any]) -> dict[str, Any]:
    report = attestation.get("report")
    if not isinstance(report, dict):
        raise ValueError("execution attestation has no report binding")
    return report


def selected_target_identity(
    report: dict[str, Any], profile: str, label: str
) -> dict[str, Any]:
    targets = report.get("identity", {}).get("targets", [])
    matches = [
        row for row in targets
        if isinstance(row, dict) and row.get("target_profile") == profile
    ]
    if len(matches) != 1:
        raise ValueError(f"{label} report has no unique {profile} target identity")
    return matches[0]


def selected_target_runtimes(
    attestation: dict[str, Any], profile: str, role: str, label: str
) -> list[dict[str, Any]]:
    matches = []
    for invocation in attestation.get("invocations", []):
        if (
            invocation.get("side") == "target"
            and invocation.get("profile") == profile
            and invocation.get("role") == role
        ):
            receipt = invocation.get("receipt")
            runtime = receipt.get("runtime") if isinstance(receipt, dict) else None
            stable = runtime.get("stable") if isinstance(runtime, dict) else None
            if not isinstance(stable, dict):
                raise ValueError(f"{label} target invocation has no stable runtime identity")
            matches.append(runtime)
    if not matches:
        raise ValueError(f"{label} has no {profile} {role} runtime identity")
    return matches


def copy_new(source: Path, destination: Path) -> None:
    with source.open("rb") as input_stream, destination.open("xb") as output_stream:
        shutil.copyfileobj(input_stream, output_stream)


def build_selected_run(
    *,
    benchmark_report_path: Path,
    benchmark_attestation_path: Path,
    benchmark_producer_path: Path,
    benchmark_bundle_path: Path,
    parity_report_path: Path,
    parity_attestation_path: Path,
    parity_producer_path: Path,
    parity_bundle_path: Path,
    manifest_path: Path,
    source_build_path: Path,
    workload_id: str,
    target_profile: str,
    baseline_name: str,
    target_policy: str,
    max_regression_percent: float,
    output_dir: Path,
    fixture_root: Path = FIXTURE_ROOT,
    producer_path_maps: dict[str, str] | None = None,
) -> Path:
    """Validate and package one direct-operation report/profile selection."""

    if not workload_id.strip():
        raise ValueError("workload ID cannot be empty")
    if target_profile not in BACKEND_FOR_PROFILE:
        raise ValueError(f"unsupported target profile: {target_profile}")
    if not baseline_name.strip():
        raise ValueError("baseline name cannot be empty")
    if target_policy not in {"strict", "tracking"}:
        raise ValueError(f"unsupported target policy: {target_policy}")
    if not math.isfinite(max_regression_percent) or max_regression_percent < 0:
        raise ValueError("maximum regression percent must be finite and nonnegative")

    benchmark = read_json(benchmark_report_path)
    parity = read_json(parity_report_path)
    if benchmark.get("schema") != BENCHMARK_SCHEMA or benchmark.get("status") != "completed":
        raise ValueError("benchmark report is incomplete or unsupported")
    if parity.get("schema") not in PARITY_SCHEMAS or parity.get("status") != "completed":
        raise ValueError("parity report is incomplete or unsupported")
    parity_summary = parity.get("summary", {})
    parity_failed = (
        parity_summary.get("parity_cases", {}).get("failed")
        if parity.get("schema") == "migration-parity/parity-result@2"
        else parity_summary.get("failed")
    )
    if parity_failed != 0 or parity.get("infrastructure_errors"):
        raise ValueError("parity report contains failed or infrastructure comparisons")

    benchmark_identity = benchmark["identity"]
    parity_identity = parity["identity"]
    benchmark_manifest = manifest_identity(benchmark, "benchmark")
    parity_manifest = manifest_identity(parity, "parity")
    if benchmark_manifest != parity_manifest:
        raise ValueError("benchmark and parity reports reference different manifests")
    manifest_bytes = manifest_path.read_bytes()
    if sha256_bytes(manifest_bytes) != benchmark_manifest["sha256"]:
        raise ValueError("manifest bytes differ from report identity")
    manifest = yaml.safe_load(manifest_bytes)
    if not isinstance(manifest, dict) or manifest.get("schema") != MANIFEST_SCHEMA:
        raise ValueError("manifest is unsupported")

    benchmark_identity_inputs = resolve_report_inputs(
        benchmark, fixture_root, "benchmark"
    )
    parity_identity_inputs = resolve_report_inputs(parity, fixture_root, "parity")
    benchmark_input_ref, benchmark_input_path, benchmark_input = select_input_for_item(
        benchmark_identity_inputs,
        schema=BENCHMARK_INPUT_SCHEMA,
        collection="workloads",
        identifier_field="workload_id",
        identifier=workload_id,
        label="benchmark",
    )
    if benchmark_input.get("schema") != BENCHMARK_INPUT_SCHEMA:
        raise ValueError("benchmark input schema is unsupported")

    workload_definitions = [
        row for row in benchmark_input.get("workloads", [])
        if isinstance(row, dict) and row.get("workload_id") == workload_id
    ]
    workload_results = [
        row for row in benchmark.get("workloads", [])
        if isinstance(row, dict) and row.get("workload_id") == workload_id
    ]
    if len(workload_definitions) != 1 or len(workload_results) != 1:
        raise ValueError("selected workload must occur exactly once in input and report")
    workload_definition = workload_definitions[0]
    workload_result = workload_results[0]
    input_spec = workload_definition.get("input", {})
    if input_spec.get("kind") != "parity_case":
        raise ValueError("selected-run direct-operation admission requires a parity_case workload")
    case_id = input_spec.get("case_id")
    if not isinstance(case_id, str) or not case_id:
        raise ValueError("selected workload has no parity case ID")
    parity_input_ref, parity_input_path, parity_input = select_input_for_item(
        parity_identity_inputs,
        schema=PARITY_INPUT_SCHEMA,
        collection="cases",
        identifier_field="case_id",
        identifier=case_id,
        label="parity",
    )
    if parity_input.get("schema") != PARITY_INPUT_SCHEMA:
        raise ValueError("parity input schema is unsupported")
    if parity_input_ref not in benchmark_identity.get("inputs", []):
        raise ValueError("benchmark and parity reports do not share the selected parity input")
    if parity_input_ref not in parity_identity.get("inputs", []):
        raise ValueError("parity report identity omits its selected parity input")
    if workload_result.get("correctness", {}).get("outcome") != "pass":
        raise ValueError("selected workload has no passing correctness result")
    if workload_result.get("correctness", {}).get("evidence_id") != parity_identity.get("run_id"):
        raise ValueError("selected benchmark correctness does not reference the supplied parity run")
    profile_subjects = [
        row for row in workload_result.get("subjects", [])
        if isinstance(row, dict)
        and row.get("kind") == "target_profile"
        and row.get("id") == target_profile
    ]
    if len(profile_subjects) != 1 or profile_subjects[0].get("status") != "completed":
        raise ValueError("selected benchmark target profile was not measured completely")
    actual_backend = profile_subjects[0].get("execution", {}).get("actual_backend")
    expected_backend = BACKEND_FOR_PROFILE[target_profile]
    if actual_backend != expected_backend:
        raise ValueError(
            f"selected profile executed {actual_backend!r}, expected {expected_backend!r}"
        )

    input_cases = [
        row for row in parity_input.get("cases", [])
        if isinstance(row, dict) and row.get("case_id") == case_id
    ]
    parity_comparisons = [
        row for row in parity.get("comparisons", [])
        if isinstance(row, dict)
        and row.get("case_id") == case_id
        and row.get("target_profile") == target_profile
        and row.get("verification", "parity") == "parity"
    ]
    if len(input_cases) != 1:
        raise ValueError("selected parity case must occur exactly once in its input")
    if len(parity_comparisons) != 1 or parity_comparisons[0].get("outcome") != "pass":
        raise ValueError("selected case/profile has no unique passing parity comparison")

    benchmark_execution = verify_execution_attestation(
        benchmark_attestation_path, benchmark_report_path, benchmark, "benchmark"
    )
    parity_execution = verify_execution_attestation(
        parity_attestation_path, parity_report_path, parity, "parity"
    )
    verify_producer_attestation(
        benchmark_producer_path,
        benchmark_bundle_path,
        benchmark_attestation_path,
        benchmark,
        "benchmark",
        producer_path_maps,
    )
    verify_producer_attestation(
        parity_producer_path,
        parity_bundle_path,
        parity_attestation_path,
        parity,
        "parity",
        producer_path_maps,
    )
    build_witness = read_json(source_build_path)
    build_witness_schema = build_witness.get("schema")
    if (
        build_witness_schema not in BUILD_WITNESS_SCHEMAS
        or build_witness.get("status") != "complete"
        or build_witness.get("errors") != []
    ):
        raise ValueError("source-build witness is unsupported or incomplete")
    if build_witness_schema == MATERIALIZED_BUILD_WITNESS_SCHEMA:
        materialization = build_witness.get("materialization")
        runtime_artifact = (
            materialization.get("module_artifact")
            if isinstance(materialization, dict)
            else None
        )
        if (
            not isinstance(materialization, dict)
            or materialization.get("schema") != "pillow-rs/maturin-materialization@1"
            or materialization.get("status") != "complete"
            or not isinstance(runtime_artifact, dict)
        ):
            raise ValueError("source-build witness has no complete Maturin module artifact")
    else:
        source = build_witness.get("source")
        runtime_artifact = build_witness.get("artifact")
        if (
            not isinstance(source, dict)
            or not isinstance(source.get("before"), dict)
            or source.get("before") != source.get("after")
            or not isinstance(runtime_artifact, dict)
            or runtime_artifact.get("target") != "_core"
            or "cdylib" not in runtime_artifact.get("crate_types", [])
        ):
            raise ValueError("source-build witness has no stable direct Cargo artifact")
    if (
        not isinstance(runtime_artifact.get("path"), str)
        or not os.path.isabs(runtime_artifact["path"])
        or not isinstance(runtime_artifact.get("sha256"), str)
        or len(runtime_artifact["sha256"]) != 64
        or any(char not in "0123456789abcdef" for char in runtime_artifact["sha256"])
        or type(runtime_artifact.get("size")) is not int
        or runtime_artifact["size"] <= 0
    ):
        raise ValueError("source-build witness runtime artifact identity is invalid")
    profile_targets = [
        row for row in benchmark_identity.get("targets", [])
        if isinstance(row, dict) and row.get("target_profile") == target_profile
    ]
    if len(profile_targets) != 1 or profile_targets[0].get("backend") != expected_backend:
        raise ValueError("selected report target identity does not match its profile")
    workspace_identity = build_witness.get("workspace", {})
    if profile_targets[0].get("revision") != workspace_identity.get("git_head"):
        raise ValueError("source-build witness revision differs from the selected report")
    if profile_targets[0].get("dirty") != workspace_identity.get("dirty"):
        raise ValueError("source-build witness dirty state differs from the selected report")
    parity_target = selected_target_identity(parity, target_profile, "parity")
    identity_fields = (
        "target_profile",
        "target_id",
        "revision",
        "dirty",
        "runtime",
        "backend",
        "features",
    )
    if any(
        profile_targets[0].get(field) != parity_target.get(field)
        for field in identity_fields
    ):
        raise ValueError("benchmark and parity target identities differ")
    profile_features = profile_targets[0].get("features")
    if not isinstance(profile_features, list) or any(not isinstance(item, str) for item in profile_features):
        raise ValueError("selected report has an invalid target feature identity")
    build_features = build_witness.get("build", {}).get("features", {})
    if target_profile == "python-parallel-cpu":
        required_features = {"pillow-rs-py/parallel", "pillow-rs/parallel"}
        if not required_features.issubset(profile_features):
            raise ValueError("Parallel CPU target identity omits its opt-in feature")
        if not all(
            isinstance(build_features.get(package), list)
            and "parallel" in build_features[package]
            for package in ("pillow-rs-py", "pillow-rs")
        ):
            raise ValueError("Parallel CPU build witness omits its opt-in feature")
    elif any("/parallel" in feature for feature in profile_features):
        raise ValueError("serial profile unexpectedly declares the Parallel CPU feature")
    target_profiles = manifest.get("target_profiles", [])
    manifest_profiles = [
        row for row in target_profiles
        if isinstance(row, dict) and row.get("id") == target_profile
    ]
    if len(manifest_profiles) != 1:
        raise ValueError("selected target profile is absent or duplicated in manifest")

    runner_id = benchmark.get("environment", {}).get("runner_id")
    if not isinstance(runner_id, str) or not runner_id:
        raise ValueError("selected benchmark has no stable runner ID")
    parity_runtimes = selected_target_runtimes(
        parity_execution, target_profile, "parity", "parity attestation"
    )
    benchmark_runtimes = selected_target_runtimes(
        benchmark_execution,
        target_profile,
        "benchmark_execution_proof",
        "benchmark attestation",
    )
    target_runtime_hashes = {
        runtime.get("stable_sha256")
        for runtime in (*parity_runtimes, *benchmark_runtimes)
    }
    if len(target_runtime_hashes) != 1:
        raise ValueError("benchmark and parity target runtime identities differ")
    target_runtime_stables = [
        runtime["stable"] for runtime in (*parity_runtimes, *benchmark_runtimes)
    ]
    target_core_files = [
        stable.get("target_execution", {}).get("selected_core", {}).get("file", {})
        for stable in target_runtime_stables
    ]
    if build_witness_schema == BUILD_WITNESS_SCHEMA:
        mappings = [
            stable.get("target_execution", {}).get("selected_core", {}).get("mapping", {})
            for stable in target_runtime_stables
        ]
        if any(
            not isinstance(mapping, dict)
            or mapping.get("status") != "verified"
            or mapping.get("reason") is not None
            for mapping in mappings
        ):
            raise ValueError("direct Cargo artifact is not verified as the loaded target core")
    target_runtime_paths = {
        core_file.get("path") for core_file in target_core_files
    }
    target_runtime_sizes = {
        core_file.get("size") for core_file in target_core_files
    }
    target_core_hashes = {core_file.get("sha256") for core_file in target_core_files}
    runner_ids = {
        stable.get("host", {}).get("runner_id")
        for stable in target_runtime_stables
    }
    if runner_ids != {runner_id}:
        raise ValueError("benchmark report and parity/benchmark runtimes have different runner IDs")
    if target_core_hashes != {runtime_artifact["sha256"]}:
        raise ValueError("loaded target core hash differs from the source-build witness artifact")
    if target_runtime_paths != {runtime_artifact["path"]}:
        raise ValueError("loaded target core path differs from the source-build witness artifact")
    if target_runtime_sizes != {runtime_artifact["size"]}:
        raise ValueError("loaded target core size differs from the source-build witness artifact")

    output_dir = output_dir.resolve()
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    output_dir.mkdir(exist_ok=False)
    input_output_dir = output_dir / "inputs"
    input_output_dir.mkdir()
    manifest_output = output_dir / "manifest.yaml"
    source_build_output = output_dir / "source-build-witness.json"
    copy_new(manifest_path, manifest_output)
    copy_new(source_build_path, source_build_output)

    def relative(path: Path) -> str:
        return Path(os.path.relpath(path.resolve(), output_dir)).as_posix()

    packaged_inputs: dict[str, tuple[str, Path]] = {}

    def package_identity_inputs(
        inputs: list[tuple[dict[str, str], Path, dict[str, Any]]],
    ) -> list[dict[str, str]]:
        packaged = []
        for reference, source, _document in inputs:
            digest = reference["sha256"]
            if digest not in packaged_inputs:
                suffix = ".json.gz" if source.name.endswith(".gz") else ".json"
                destination = input_output_dir / f"{digest}{suffix}"
                copy_new(source, destination)
                packaged_inputs[digest] = (reference["schema"], destination)
            else:
                existing_schema, destination = packaged_inputs[digest]
                if existing_schema != reference["schema"]:
                    raise ValueError(
                        "one input content hash is referenced with multiple schemas"
                    )
                if destination.read_bytes() != source.read_bytes():
                    raise ValueError("input hash collision while packaging report inputs")
            packaged.append({**reference, "file": relative(destination)})
        return packaged

    benchmark_packaged_inputs = package_identity_inputs(benchmark_identity_inputs)
    parity_packaged_inputs = package_identity_inputs(parity_identity_inputs)

    def packaged_selected_input(
        packaged: list[dict[str, str]], reference: dict[str, str], label: str
    ) -> str:
        matches = [
            item["file"]
            for item in packaged
            if all(item.get(key) == reference.get(key) for key in ("path", "schema", "sha256"))
        ]
        if len(matches) != 1:
            raise ValueError(f"{label} selected input is absent or duplicated in its identity closure")
        return matches[0]

    benchmark_input_file = packaged_selected_input(
        benchmark_packaged_inputs, benchmark_input_ref, "benchmark"
    )
    parity_input_file = packaged_selected_input(
        parity_packaged_inputs, parity_input_ref, "parity"
    )

    project = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    producer_version = str(project["workspace"]["package"]["version"])
    selection = {
        "schema": "pillow-perfdb/selected-run@2",
        "kind": "direct-operation",
        "workload_id": workload_id,
        "case_id": case_id,
        "target_profile": target_profile,
        "producer": {"tool": "pillow-rs", "version": producer_version},
        "benchmark": {
            "report": relative(benchmark_report_path),
            "input": benchmark_input_file,
            "identity_inputs": benchmark_packaged_inputs,
            "attestation": relative(benchmark_attestation_path),
            "producer": relative(benchmark_producer_path),
            "bundle": relative(benchmark_bundle_path),
        },
        "parity": {
            "report": relative(parity_report_path),
            "input": parity_input_file,
            "identity_inputs": parity_packaged_inputs,
            "attestation": relative(parity_attestation_path),
            "producer": relative(parity_producer_path),
            "bundle": relative(parity_bundle_path),
        },
        "manifest": manifest_output.name,
        "source_build": source_build_output.name,
        "link_output": "selected-run-link.json",
        "baseline": {
            "name": baseline_name,
            "source_kind": "benchmark-result",
            "target_policy": target_policy,
            "max_regression_percent": max_regression_percent,
        },
    }
    selection_path = output_dir / "selected-run.json"
    with selection_path.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(selection, indent=2) + "\n")
    return selection_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for option in (
        "benchmark-report",
        "benchmark-attestation",
        "benchmark-producer",
        "benchmark-bundle",
        "parity-report",
        "parity-attestation",
        "parity-producer",
        "parity-bundle",
        "manifest",
        "source-build",
        "workload-id",
        "target-profile",
        "baseline-name",
        "target-policy",
        "output-dir",
    ):
        parser.add_argument(f"--{option}", required=True)
    parser.add_argument(
        "--fixture-root",
        type=Path,
        default=FIXTURE_ROOT,
        help="root containing the exact input files referenced by the selected reports",
    )
    parser.add_argument(
        "--producer-path-map",
        action="append",
        default=[],
        metavar="ORIGINAL=LOCAL",
        help="rehost an attested producer root or exact external file for validation",
    )
    parser.add_argument("--max-regression-percent", type=float, default=0.0)
    args = parser.parse_args()
    output = build_selected_run(
        benchmark_report_path=Path(args.benchmark_report),
        benchmark_attestation_path=Path(args.benchmark_attestation),
        benchmark_producer_path=Path(args.benchmark_producer),
        benchmark_bundle_path=Path(args.benchmark_bundle),
        parity_report_path=Path(args.parity_report),
        parity_attestation_path=Path(args.parity_attestation),
        parity_producer_path=Path(args.parity_producer),
        parity_bundle_path=Path(args.parity_bundle),
        manifest_path=Path(args.manifest),
        source_build_path=Path(args.source_build),
        workload_id=args.workload_id,
        target_profile=args.target_profile,
        baseline_name=args.baseline_name,
        target_policy=args.target_policy,
        max_regression_percent=args.max_regression_percent,
        output_dir=Path(args.output_dir),
        fixture_root=args.fixture_root,
        producer_path_maps=parse_producer_path_maps(args.producer_path_map),
    )
    print(json.dumps({"status": "complete", "selected_run": str(output)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
