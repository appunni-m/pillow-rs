"""Pillow fixture adapter for the project-neutral PostgreSQL parity store."""

from __future__ import annotations

import base64
import binascii
import codecs
import hashlib
import json
import mmap
from collections.abc import Iterable
from pathlib import Path
from typing import Any

from scripts import run_migration_parity as parity
from scripts.parity_store import (
    AssetRecord,
    CaseRecord,
    CaseStep,
    ExpectedObservation,
    FaultContract,
    PostgresStore,
    ObservationSelector,
    canonical_value_digest,
)
from scripts.migration_parity.streaming_json import iter_object_array

PROJECT_ID = "pillow-rs"
IMPORTER_SCHEMA = "pillow-rs-parity-import@2"
MAX_IMPORT_CASES = 200
MAX_IMPORT_BYTES = 64 * 1024 * 1024
MAX_DIRECT_CASE_LOOKUP = 64


class _HashingTextReader:
    """Decode streamed UTF-8 while hashing the exact bytes consumed."""

    def __init__(self, source: Any) -> None:
        self._source = source
        self._digest = hashlib.sha256()
        self._decoder = codecs.getincrementaldecoder("utf-8")()
        self._finalized = False

    def read(self, size: int = -1) -> str:
        if size == 0:
            return ""
        while True:
            chunk = self._source.read(size)
            if chunk:
                self._digest.update(chunk)
                text = self._decoder.decode(chunk, final=False)
                if text:
                    return text
                continue
            if not self._finalized:
                self._finalized = True
                return self._decoder.decode(b"", final=True)
            return ""

    def digest(self) -> bytes:
        return self._digest.digest()


def import_manifest(
    store: PostgresStore,
    manifest_path: Path,
    *,
    case_ids: Iterable[str] | None = None,
) -> tuple[dict[str, Any], bytes, dict[str, int]]:
    """Validate and incrementally import one pinned Pillow fixture snapshot."""

    manifest_path = manifest_path.resolve()
    manifest = parity.load_manifest(manifest_path)
    parity.ensure_generated_parity_inputs(manifest)
    requested_case_id_list = None if case_ids is None else tuple(case_ids)
    if (
        requested_case_id_list is not None
        and len(requested_case_id_list) != len(set(requested_case_id_list))
    ):
        raise ValueError("case import selection contains duplicate IDs")
    requested_case_ids = (
        None if requested_case_id_list is None else set(requested_case_id_list)
    )
    if requested_case_ids is not None and not requested_case_ids:
        raise ValueError("case import selection must not be empty")

    manifest_bytes = manifest_path.read_bytes()
    manifest_digest = hashlib.sha256(manifest_bytes).digest()
    input_paths = tuple(manifest["input_index"]["parity"])
    input_docs: list[tuple[str, str | None, bytes]] = []
    for relative in input_paths:
        document_path = parity.FIXTURE_ROOT / relative
        input_docs.append(
            (
                relative,
                "migration-parity/parity-input@3",
                _sha256_file(document_path),
            )
        )
    specification_digest = canonical_value_digest(
        {
            "importer_schema": IMPORTER_SCHEMA,
            "manifest_sha256": manifest_digest,
            "input_documents": [
                {"path": path, "schema": schema, "sha256": digest}
                for path, schema, digest in input_docs
            ],
        }
    )
    source_ref = str(manifest_path.relative_to(parity.ROOT))
    documents = [(source_ref, manifest["schema"], manifest_digest), *input_docs]
    validated_snapshot = store.specification_documents_match(
        PROJECT_ID, specification_digest, documents
    )
    if not validated_snapshot:
        parity.validate_fixed_inputs(
            manifest, parity.FIXTURE_ROOT, lane="parity", retain_documents=False
        )

    target_profiles = {item["id"]: item for item in manifest["target_profiles"]}
    target_definitions = {item["id"]: item for item in manifest["targets"]}
    document_digests = {path: digest for path, _schema, digest in input_docs}

    with store.connection.transaction():
        store.register_project(PROJECT_ID, "pillow-rs")
        store.register_specification(
            PROJECT_ID, specification_digest, manifest["schema"], source_ref
        )
        store.register_specification_documents(
            PROJECT_ID, specification_digest, documents
        )
        _register_catalog(
            store, manifest, specification_digest, target_profiles, target_definitions
        )

    operation_index = parity.build_operation_index(manifest)
    existing_case_ids = (
        set()
        if requested_case_ids is None
        else store.existing_case_ids(
            PROJECT_ID, specification_digest, tuple(requested_case_ids)
        )
    )
    pending_case_ids = (
        None if requested_case_ids is None else requested_case_ids - existing_case_ids
    )
    if pending_case_ids is None:
        scan_paths = input_paths
    elif not pending_case_ids:
        scan_paths = ()
    else:
        scan_paths = _candidate_case_documents(
            input_paths, pending_case_ids, parity.FIXTURE_ROOT
        )
    imported = 0
    skipped_existing = len(existing_case_ids)
    total_cases: int | None = 0 if requested_case_ids is None else None
    selected_cases = 0
    found_case_ids = set(existing_case_ids)
    for relative in scan_paths:
        document_path = parity.FIXTURE_ROOT / relative
        document_digest = document_digests[relative]
        batch: list[CaseRecord] = []
        batch_bytes = 0
        with document_path.open("rb") as source:
            hashing_source = _HashingTextReader(source)
            for source_ordinal, raw_case in enumerate(
                iter_object_array(hashing_source, "cases")
            ):
                if requested_case_ids is None:
                    assert total_cases is not None
                    total_cases += 1
                elif raw_case["case_id"] not in pending_case_ids:
                    continue
                case_id = raw_case["case_id"]
                if case_id in found_case_ids:
                    raise ValueError(f"parity input repeats case ID {case_id!r}")
                found_case_ids.add(case_id)
                selected_cases += 1
                record = _make_case_record(
                    raw_case,
                    project_id=PROJECT_ID,
                    specification_digest=specification_digest,
                    source_document=relative,
                    source_document_digest=document_digest,
                    source_ordinal=source_ordinal,
                    operation_index=operation_index,
                    all_profiles=tuple(target_profiles),
                )
                if batch and (
                    len(batch) >= MAX_IMPORT_CASES
                    or batch_bytes + record.estimated_bytes > MAX_IMPORT_BYTES
                ):
                    attempted = len(batch)
                    with store.connection.transaction():
                        added = store.insert_cases(batch)
                    imported += added
                    skipped_existing += attempted - added
                    batch = []
                    batch_bytes = 0
                batch.append(record)
                batch_bytes += record.estimated_bytes
            if hashing_source.digest() != document_digest:
                raise ValueError(
                    f"parity input changed after validation: {document_path}"
                )
        if batch:
            attempted = len(batch)
            with store.connection.transaction():
                added = store.insert_cases(batch)
            imported += added
            skipped_existing += attempted - added

    if requested_case_ids is not None and found_case_ids != requested_case_ids:
        missing = sorted(requested_case_ids - found_case_ids)
        raise ValueError("unknown parity case ID(s): " + ", ".join(missing))

    return manifest, specification_digest, {
        "documents": len(input_docs),
        "cases_in_manifest": total_cases,
        "selected_cases": (
            len(requested_case_ids) if requested_case_ids is not None else selected_cases
        ),
        "filtered_cases": (
            None
            if requested_case_ids is not None
            else total_cases - selected_cases
        ),
        "import_scope": "selected" if requested_case_ids is not None else "full",
        "new_cases": imported,
        "already_imported_cases": skipped_existing,
    }


def _sha256_file(path: Path) -> bytes:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.digest()


def _candidate_case_documents(
    input_paths: tuple[str, ...], case_ids: set[str], fixture_root: Path
) -> tuple[str, ...]:
    """Locate small explicit selections without decoding unrelated case files."""

    if len(case_ids) > MAX_DIRECT_CASE_LOOKUP:
        return input_paths
    tokens = {
        token
        for case_id in case_ids
        for token in {
            json.dumps(case_id, ensure_ascii=False).encode("utf-8"),
            json.dumps(case_id, ensure_ascii=True).encode("utf-8"),
        }
    }
    candidates = []
    for relative in input_paths:
        path = fixture_root / relative
        if path.stat().st_size == 0:
            continue
        with path.open("rb") as source:
            with mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as contents:
                if any(contents.find(token) >= 0 for token in tokens):
                    candidates.append(relative)
    return tuple(candidates)


def iter_profile_batches(
    store: PostgresStore,
    specification_digest: bytes,
    profile_id: str,
    *,
    maximum_cases: int = 128,
    maximum_estimated_bytes: int = 32 * 1024 * 1024,
) -> Iterable[tuple[list[dict[str, Any]], int, list[CaseRecord]]]:
    """Yield bounded runner inputs and their typed database records."""

    for records, estimated_bytes in store.iter_case_batches(
        PROJECT_ID,
        specification_digest,
        maximum_cases=maximum_cases,
        maximum_estimated_bytes=maximum_estimated_bytes,
        profile_id=profile_id,
    ):
        yield [record_to_runner_case(record) for record in records], estimated_bytes, records


def record_to_runner_case(record: CaseRecord) -> dict[str, Any]:
    """Restore one project fixture input from the neutral stored value tree."""

    stored = _from_store_tree(record.payload)
    if not isinstance(stored, dict) or not isinstance(stored.get("template"), dict):
        raise TypeError(f"stored case {record.case_id!r} did not restore to an object")
    case = stored["template"]
    step_templates = stored.get("step_templates")
    if not isinstance(step_templates, list) or len(step_templates) != len(record.steps):
        raise ValueError(f"stored case {record.case_id!r} has inconsistent workflow steps")
    case["steps"] = []
    for metadata, step in zip(step_templates, record.steps, strict=True):
        if not isinstance(metadata, dict):
            raise TypeError(f"stored case {record.case_id!r} has invalid step metadata")
        runner_step = dict(metadata)
        runner_step["arguments"] = _from_store_tree(step.arguments)
        case["steps"].append(runner_step)

    asset_templates = stored.get("asset_templates")
    if not isinstance(asset_templates, list):
        raise ValueError(f"stored case {record.case_id!r} has invalid asset metadata")
    stored_assets = {asset.asset_id: asset for asset in record.assets}
    case["assets"] = []
    asset_overrides = {
        asset_id: base64.b64encode(asset.content).decode("ascii")
        for asset_id, asset in stored_assets.items()
        if asset.asset_kind in {"ref", "ref_bytes"} and asset.content is not None
    }
    for metadata in asset_templates:
        if not isinstance(metadata, dict) or not isinstance(metadata.get("id"), str):
            raise TypeError(f"stored case {record.case_id!r} has invalid asset metadata")
        asset = dict(metadata)
        stored_asset = stored_assets.get(asset["id"])
        if stored_asset is None:
            raise ValueError(f"stored case {record.case_id!r} references an unknown asset")
        if asset.get("kind") == "inline":
            if stored_asset.content is None:
                raise ValueError(f"inline asset {asset['id']!r} has no stored content")
            encoding = asset.get("encoding")
            if encoding == "base64":
                asset["data"] = base64.b64encode(stored_asset.content).decode("ascii")
            elif encoding == "utf8":
                asset["data"] = stored_asset.content.decode("utf-8")
            else:
                raise ValueError(f"inline asset {asset['id']!r} has an unknown encoding")
        case["assets"].append(asset)

    if record.expected_observations:
        case["target_expectation"] = {
            "observations": [
                {
                    "step_id": item.step_id,
                    "value": _from_store_tree(item.value),
                }
                for item in record.expected_observations
            ]
        }
    if record.fault_contract is not None:
        case["fault"] = _from_store_tree(record.fault_contract.fault_value)
    if asset_overrides:
        case["_parity_asset_contents"] = asset_overrides
    return case


def _register_catalog(
    store: PostgresStore,
    manifest: dict[str, Any],
    specification_digest: bytes,
    profiles: dict[str, dict[str, Any]],
    targets: dict[str, dict[str, Any]],
) -> None:
    operation_rows = []
    requirement_ids: set[str] = set()
    operation_requirements = []
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            operation_rows.append((surface["id"], operation["id"]))
            for requirement in operation["requirements"]:
                requirement_ids.add(requirement["id"])
                operation_requirements.append(
                    (surface["id"], operation["id"], requirement["id"])
                )

    store.register_operations(PROJECT_ID, specification_digest, operation_rows)
    store.register_requirements(PROJECT_ID, specification_digest, sorted(requirement_ids))
    store.register_operation_requirements(
        PROJECT_ID, specification_digest, operation_requirements
    )

    for profile_id, profile in profiles.items():
        target = targets[profile["target_id"]]
        attributes = {
            key: value
            for key, value in profile.items()
            if isinstance(value, (str, int, bool)) and key not in {"id", "target_id"}
        }
        attributes["features"] = ";".join(profile.get("features", []))
        attributes["target_name"] = target["name"]
        store.register_target_profile(
            PROJECT_ID,
            specification_digest,
            profile_id,
            profile["target_id"],
            attributes,
        )

    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            target_support = {
                item["target_id"]: item["support"] for item in operation["targets"]
            }
            for profile_id, profile in profiles.items():
                support = target_support[profile["target_id"]]
                store.register_operation_support(
                    PROJECT_ID,
                    specification_digest,
                    surface["id"],
                    operation["id"],
                    profile_id,
                    support["status"],
                    support.get("reason"),
                    {"backend": profile["backend"]},
                )


def _make_case_record(
    raw_case: dict[str, Any],
    *,
    project_id: str,
    specification_digest: bytes,
    source_document: str,
    source_document_digest: bytes,
    source_ordinal: int,
    operation_index: dict[tuple[str, str], dict[str, Any]],
    all_profiles: tuple[str, ...],
) -> CaseRecord:
    assets = tuple(_make_asset_record(item) for item in raw_case["assets"])
    case_digest = canonical_value_digest(
        {
            "source_document_digest": source_document_digest,
            "source_ordinal": source_ordinal,
            "case_id": raw_case["case_id"],
            "asset_digests": [
                {
                    "asset_id": asset.asset_id,
                    "sha256": (
                        None
                        if asset.content is None
                        else hashlib.sha256(asset.content).digest()
                    ),
                }
                for asset in assets
            ],
        }
    )
    payload = {
        "template": _to_store_tree(
            {
                key: value
                for key, value in raw_case.items()
                if key not in {"steps", "assets", "target_expectation", "fault"}
            }
        ),
        "step_templates": _to_store_tree(
            [
                {key: value for key, value in step.items() if key != "arguments"}
                for step in raw_case["steps"]
            ]
        ),
        "asset_templates": [
                {key: value for key, value in asset.items() if key != "data"}
                for asset in raw_case["assets"]
        ],
    }
    verification = raw_case.get("verification", "parity")
    profiles = all_profiles
    fault_contract = None
    if verification == "fault-contract":
        operation = operation_index[(raw_case["surface"], raw_case["operation"])]
        profiles = tuple(operation["fault_contract"]["target_profiles"])
        fault = raw_case["fault"]
        fault_contract = FaultContract(
            contract_id=fault["contract"],
            injection_point=fault["point"],
            fault_value=_to_store_tree(fault),
        )

    steps = tuple(
        CaseStep(
            step_id=step["step_id"],
            surface_id=step["surface"],
            operation_id=step["operation"],
            arguments=_to_store_tree(step["arguments"]),
            receiver_step_id=(
                step["receiver"].get("step_id")
                if isinstance(step.get("receiver"), dict)
                and step["receiver"].get("kind") == "binding"
                else None
            ),
        )
        for step in raw_case["steps"]
    )
    selectors = tuple(
        ObservationSelector(
            step_id=(item if isinstance(item, str) else item["step_id"]),
            field_path=(None if isinstance(item, str) else item.get("field_path")),
        )
        for item in raw_case["observations"]
    )
    expected_observations = tuple(
        ExpectedObservation(
            step_id=item["step_id"], value=_to_store_tree(item["value"])
        )
        for item in raw_case.get("target_expectation", {}).get("observations", [])
    )
    estimated_bytes = len(
        json.dumps(raw_case, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    ) + sum(len(asset.content or b"") for asset in assets)

    return CaseRecord(
        project_id=project_id,
        specification_digest=specification_digest,
        case_id=raw_case["case_id"],
        surface_id=raw_case["surface"],
        operation_id=raw_case["operation"],
        verification_kind=verification.replace("-", "_"),
        source_document=source_document,
        source_document_digest=source_document_digest,
        source_ordinal=source_ordinal,
        case_digest=case_digest,
        estimated_bytes=estimated_bytes,
        dimensions=_case_dimensions(raw_case),
        requirements=tuple(raw_case.get("covers", [])),
        profiles=profiles,
        payload=payload,
        steps=steps,
        observation_selectors=selectors,
        expected_observations=expected_observations,
        fault_contract=fault_contract,
        assets=assets,
    )


def _make_asset_record(asset: dict[str, Any]) -> AssetRecord:
    asset_id = asset["id"]
    kind = asset["kind"]
    content: bytes | None = None
    locator = asset.get("path") or asset.get("name")
    if kind == "inline":
        if asset["encoding"] == "base64":
            try:
                content = base64.b64decode(asset["data"], validate=True)
            except (ValueError, binascii.Error) as exc:
                raise ValueError(f"inline asset {asset_id!r} has invalid base64") from exc
        elif asset["encoding"] == "utf8":
            content = asset["data"].encode("utf-8")
        else:
            raise ValueError(f"inline asset {asset_id!r} has an unsupported encoding")
    elif kind in {"ref", "ref_bytes"}:
        path = (parity.FIXTURE_ROOT / "assets" / asset["path"]).resolve()
        content = path.read_bytes()
    elif kind == "builtin" and asset.get("name") in parity.ENCODED_INPUTS:
        content = parity.ENCODED_INPUTS[asset["name"]]

    expected_sha = asset.get("sha256")
    expected_digest = None if expected_sha is None else bytes.fromhex(expected_sha)
    if content is not None and expected_digest is not None:
        actual_digest = hashlib.sha256(content).digest()
        if actual_digest != expected_digest:
            raise ValueError(f"fixture asset {asset_id!r} failed its SHA-256 check")
    return AssetRecord(
        asset_id=asset_id,
        asset_kind=kind,
        locator=locator,
        media_type=asset.get("media_type"),
        expected_sha256=expected_digest,
        content=content,
    )


def _to_store_tree(value: Any) -> Any:
    if isinstance(value, list):
        return [_to_store_tree(item) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: _to_store_tree(item) for key, item in value.items()}
    encoding = value.get("encoding")
    kind = value.get("kind")
    data_key = None
    if kind == "inline" and encoding == "base64":
        data_key = "data"
        result["encoding"] = "raw-bytes"
    elif kind == "image" and isinstance(value.get("bytes"), str):
        data_key = "bytes"
    elif kind == "packed_pixels" and isinstance(value.get("data"), str):
        data_key = "data"
    elif kind == "bytes" and encoding == "base64" and isinstance(value.get("data"), str):
        data_key = "data"
        result["encoding"] = "raw-bytes"
    if data_key is not None:
        try:
            result[data_key] = base64.b64decode(value[data_key], validate=True)
        except (ValueError, binascii.Error) as exc:
            raise ValueError(f"invalid base64 parity value at {data_key!r}") from exc
    return result


def _from_store_tree(value: Any) -> Any:
    if isinstance(value, list):
        return [_from_store_tree(item) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: _from_store_tree(item) for key, item in value.items()}
    kind = value.get("kind")
    encoding = value.get("encoding")
    data_key = None
    if kind == "inline" and encoding == "raw-bytes":
        data_key = "data"
        result["encoding"] = "base64"
    elif kind == "image" and isinstance(value.get("bytes"), bytes):
        data_key = "bytes"
    elif kind == "packed_pixels" and isinstance(value.get("data"), bytes):
        data_key = "data"
    elif kind == "bytes" and encoding == "raw-bytes" and isinstance(value.get("data"), bytes):
        data_key = "data"
        result["encoding"] = "base64"
    if data_key is not None:
        result[data_key] = base64.b64encode(value[data_key]).decode("ascii")
    return result


def _case_dimensions(case: dict[str, Any]) -> dict[str, str | int | bool]:
    dimensions: dict[str, str | int | bool] = {
        "verification": case.get("verification", "parity"),
    }
    modes: set[str] = set()
    sizes: set[tuple[int, int]] = set()
    for step in case.get("steps", []):
        arguments = step.get("arguments", {})
        mode = arguments.get("mode")
        if isinstance(mode, dict) and mode.get("kind") == "literal":
            if isinstance(mode.get("value"), str):
                modes.add(mode["value"])
        size = arguments.get("size")
        if isinstance(size, dict) and size.get("kind") == "literal":
            value = size.get("value")
            if (
                isinstance(value, list)
                and len(value) == 2
                and all(isinstance(item, int) and not isinstance(item, bool) for item in value)
            ):
                sizes.add((value[0], value[1]))
    if len(modes) == 1:
        dimensions["mode"] = next(iter(modes))
    elif modes:
        dimensions["modes"] = ",".join(sorted(modes))
    if len(sizes) == 1:
        width, height = next(iter(sizes))
        dimensions["width"] = width
        dimensions["height"] = height
    elif sizes:
        dimensions["sizes"] = ";".join(
            f"{width}x{height}" for width, height in sorted(sizes)
        )
    return dimensions
