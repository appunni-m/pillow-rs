#!/usr/bin/env python3
"""Strict validators for the fixed migration-parity manifest and inputs.

The manifest and lane inputs are specifications.  This module intentionally
contains no runner policy and no result interpretation: it checks only the
fixed public shapes, references, and identity invariants that make the three
lane inputs executable.  Unknown fields are rejected so a typo cannot create
an untracked claim.
"""

from __future__ import annotations

import json
import hashlib
from pathlib import Path
from typing import Any, Iterable


SCHEMA_MANIFEST = "migration-parity/manifest@2"
SCHEMA_BY_LANE = {
    "parity": "migration-parity/parity-input@1",
    "coverage": "migration-parity/coverage-input@1",
    "benchmark": "migration-parity/benchmark-input@1",
}
LANES = tuple(SCHEMA_BY_LANE)
SURFACE_KINDS = {"namespace", "type", "format", "abi", "cli", "protocol", "service"}
OPERATION_KINDS = {
    "function",
    "method",
    "constructor",
    "property_get",
    "property_set",
    "command",
    "abi_function",
    "protocol_operation",
    "format_operation",
    "constant",
    "type",
    "enum",
    "enum_variant",
    "flag",
    "macro",
    "record",
    "tag",
    "error",
    "namespace",
}
VALUE_TYPES = {
    "null",
    "boolean",
    "integer",
    "number",
    "string",
    "bytes",
    "path",
    "enum",
    "sequence",
    "mapping",
    "record",
    "image",
    "font",
    "stream",
    "handle",
    "any_json",
}
PARAMETER_STYLES = {
    "receiver",
    "positional",
    "positional_or_keyword",
    "keyword",
    "variadic_positional",
    "variadic_keyword",
    "input_asset",
    "stdin",
    "environment",
    "option",
}
RESULT_SHAPES = {
    "none",
    "scalar",
    "sequence",
    "mapping",
    "record",
    "bytes",
    "image",
    "mask",
    "encoded_file",
    "metrics",
    "handle",
    "iterator",
    "stream",
    "cli",
    "protocol",
    "filesystem",
}
COMPARISON_KINDS = {"exact", "ordered", "unordered", "bytes", "numeric", "text", "image", "filesystem"}
REQUIREMENT_DIMENSIONS = {
    "parameter",
    "parameter_combination",
    "input_family",
    "success_path",
    "error_path",
    "mode",
    "format",
    "protocol_variant",
    "abi_variant",
    "asset_family",
    "boundary",
    "backend",
    "runtime",
    "feature",
    "historical_divergence",
    "code_path",
    "performance",
    "documentation",
}
SUPPORT_STATUSES = {
    "supported",
    "partial",
    "unimplemented",
    "intentionally_unsupported",
    "out_of_scope",
    "not_applicable",
}
OUTPUT_KEYS = {"expected_output", "oracle_output", "target_output", "expected_result"}


def _error(path: str, message: str) -> ValueError:
    return ValueError(f"{path}: {message}")


def _mapping(value: Any, path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise _error(path, "expected object")
    return value


def _list(value: Any, path: str) -> list[Any]:
    if not isinstance(value, list):
        raise _error(path, "expected array")
    return value


def _string(value: Any, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise _error(path, "expected non-empty string")
    return value


def _exact(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    obj = _mapping(value, path)
    actual = set(obj)
    if actual != keys:
        raise _error(path, f"expected keys {sorted(keys)}, got {sorted(actual)}")
    return obj


def _optional_exact(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    obj = _mapping(value, path)
    if not set(obj).issubset(keys):
        raise _error(path, f"unknown keys {sorted(set(obj) - keys)}")
    return obj


def _unique(values: Iterable[str], path: str) -> None:
    values = list(values)
    if len(values) != len(set(values)):
        seen: set[str] = set()
        duplicate = next(value for value in values if value in seen or seen.add(value))
        raise _error(path, f"duplicate ID {duplicate!r}")


def _id_array(value: Any, path: str) -> list[str]:
    values = _list(value, path)
    result = [_string(item, f"{path}[{index}]") for index, item in enumerate(values)]
    _unique(result, path)
    return result


def _sha(value: Any, path: str) -> None:
    text = _string(value, path)
    if len(text) != 64 or any(char not in "0123456789abcdef" for char in text):
        raise _error(path, "expected lowercase sha256")


def _relative_path(value: Any, path: str) -> str:
    text = _string(value, path)
    candidate = Path(text)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise _error(path, "path must be repository-relative without traversal")
    return candidate.as_posix()


def _manifest_operation_index(manifest: dict[str, Any]) -> dict[tuple[str, str], dict[str, Any]]:
    return {
        (surface["id"], operation["id"]): operation
        for surface in manifest["surfaces"]
        for operation in surface["operations"]
    }


def _requirement_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            for requirement in operation["requirements"]:
                result[requirement["id"]] = requirement
    return result


def _validate_manifest_operation(operation: dict[str, Any], surface_id: str, requirement_ids: set[str], known_requirement_ids: set[str], command_ids: set[str], profile_ids: set[str], target_ids: set[str], component_ids: set[str]) -> None:
    path = f"surfaces[{surface_id}].operations[{operation.get('id', '?')}]"
    _exact(operation, {"id", "kind", "classification", "lifecycle", "source", "targets", "requirements", "parity", "coverage", "benchmark"}, path)
    operation_id = _string(operation["id"], f"{path}.id")
    if operation["kind"] not in OPERATION_KINDS:
        raise _error(f"{path}.kind", "unsupported operation kind")
    if operation["classification"] not in {"endpoint", "non_endpoint"}:
        raise _error(f"{path}.classification", "unsupported classification")
    lifecycle = _exact(operation["lifecycle"], {"status"}, f"{path}.lifecycle")
    if lifecycle["status"] != "current":
        raise _error(f"{path}.lifecycle.status", "active manifest only accepts current operations")
    source = _exact(operation["source"], {"oracle_id", "path", "signature", "parameters", "result"}, f"{path}.source")
    _string(source["oracle_id"], f"{path}.source.oracle_id")
    _string(source["path"], f"{path}.source.path")
    if not isinstance(source["signature"], str):
        raise _error(f"{path}.source.signature", "expected string or null")
    parameters = _list(source["parameters"], f"{path}.source.parameters")
    parameter_ids: set[str] = set()
    for index, parameter in enumerate(parameters):
        ppath = f"{path}.source.parameters[{index}]"
        parameter = _exact(parameter, {"id", "style", "value_types", "omission"}, ppath)
        parameter_id = _string(parameter["id"], f"{ppath}.id")
        if parameter_id in parameter_ids:
            raise _error(f"{ppath}.id", "duplicate parameter ID")
        parameter_ids.add(parameter_id)
        if parameter["style"] not in PARAMETER_STYLES:
            raise _error(f"{ppath}.style", "unsupported parameter style")
        types = _id_array(parameter["value_types"], f"{ppath}.value_types")
        if any(item not in VALUE_TYPES for item in types):
            raise _error(f"{ppath}.value_types", "unsupported value type")
        omission = _mapping(parameter["omission"], f"{ppath}.omission")
        kind = omission.get("kind")
        if kind == "required":
            _exact(omission, {"kind"}, f"{ppath}.omission")
        elif kind == "literal":
            _exact(omission, {"kind", "value"}, f"{ppath}.omission")
        elif kind == "sentinel":
            _exact(omission, {"kind", "name", "semantics"}, f"{ppath}.omission")
            _string(omission["name"], f"{ppath}.omission.name")
            _string(omission["semantics"], f"{ppath}.omission.semantics")
        else:
            raise _error(f"{ppath}.omission.kind", "unsupported omission variant")
    result = _exact(source["result"], {"shape", "observations", "error"}, f"{path}.source.result")
    if result["shape"] not in RESULT_SHAPES:
        raise _error(f"{path}.source.result.shape", "unsupported result shape")
    observations = _list(result["observations"], f"{path}.source.result.observations")
    observation_paths: set[str] = set()
    for index, observation in enumerate(observations):
        opath = f"{path}.source.result.observations[{index}]"
        observation = _exact(observation, {"path", "value_types", "comparison"}, opath)
        observation_path = _string(observation["path"], f"{opath}.path")
        if observation_path in observation_paths:
            raise _error(f"{opath}.path", "duplicate observation path")
        observation_paths.add(observation_path)
        types = _id_array(observation["value_types"], f"{opath}.value_types")
        if any(item not in VALUE_TYPES for item in types):
            raise _error(f"{opath}.value_types", "unsupported value type")
        _validate_comparison(observation["comparison"], opath)
    error = _exact(result["error"], {"fields", "message"}, f"{path}.source.result.error")
    fields = _id_array(error["fields"], f"{path}.source.result.error.fields")
    if any(field not in {"class", "kind", "message", "stage", "code"} for field in fields):
        raise _error(f"{path}.source.result.error.fields", "unsupported error field")
    message = _exact(error["message"], {"mode", "transforms", "reason"}, f"{path}.source.result.error.message")
    if message["mode"] not in {"exact", "normalized", "ignored"}:
        raise _error(f"{path}.source.result.error.message.mode", "unsupported error message mode")
    transforms = _id_array(message["transforms"], f"{path}.source.result.error.message.transforms")
    if any(item not in {"normalize_newlines", "normalize_path_separators", "strip_runtime_addresses", "unicode_nfc"} for item in transforms):
        raise _error(f"{path}.source.result.error.message.transforms", "unsupported transform")
    if message["mode"] == "normalized" and (not transforms or not isinstance(message["reason"], str) or not message["reason"]):
        raise _error(f"{path}.source.result.error.message", "normalized messages require transforms and a reason")
    if message["mode"] == "ignored" and (transforms or not isinstance(message["reason"], str) or not message["reason"]):
        raise _error(f"{path}.source.result.error.message", "ignored messages require a reason and no transforms")
    if message["mode"] == "exact" and (transforms or message["reason"] is not None):
        raise _error(f"{path}.source.result.error.message", "exact messages require empty transforms and null reason")
    targets = _list(operation["targets"], f"{path}.targets")
    if not targets:
        raise _error(f"{path}.targets", "at least one target binding is required")
    seen_target_ids: set[str] = set()
    for index, target in enumerate(targets):
        tpath = f"{path}.targets[{index}]"
        target = _exact(target, {"target_id", "path", "signature", "support"}, tpath)
        target_id = _string(target["target_id"], f"{tpath}.target_id")
        if target_id in seen_target_ids:
            raise _error(f"{tpath}.target_id", "duplicate target binding")
        seen_target_ids.add(target_id)
        if target_id not in target_ids:
            raise _error(f"{tpath}.target_id", "unknown target")
        if not isinstance(target["path"], str) or not isinstance(target["signature"], str):
            raise _error(tpath, "target path and signature must be strings")
        support = _mapping(target["support"], f"{tpath}.support")
        status = support.get("status")
        if status not in SUPPORT_STATUSES:
            raise _error(f"{tpath}.support.status", "unsupported support status")
        if status == "supported":
            _exact(support, {"status"}, f"{tpath}.support")
        elif status == "partial":
            _exact(support, {"status", "reason", "missing_requirements"}, f"{tpath}.support")
            _string(support["reason"], f"{tpath}.support.reason")
            missing = _id_array(support["missing_requirements"], f"{tpath}.support.missing_requirements")
            if any(item not in known_requirement_ids for item in missing):
                raise _error(f"{tpath}.support.missing_requirements", "unknown requirement")
        else:
            _optional_exact(support, {"status", "reason", "blocker", "authority"}, f"{tpath}.support")
            _string(support.get("reason"), f"{tpath}.support.reason")
    requirements = _list(operation["requirements"], f"{path}.requirements")
    for index, requirement in enumerate(requirements):
        rpath = f"{path}.requirements[{index}]"
        requirement = _exact(requirement, {"id", "dimension", "description", "lanes", "target_profiles"}, rpath)
        requirement_id = _string(requirement["id"], f"{rpath}.id")
        if requirement_id in requirement_ids:
            raise _error(f"{rpath}.id", "duplicate requirement ID")
        requirement_ids.add(requirement_id)
        if requirement["dimension"] not in REQUIREMENT_DIMENSIONS:
            raise _error(f"{rpath}.dimension", "unsupported requirement dimension")
        _string(requirement["description"], f"{rpath}.description")
        lanes = _id_array(requirement["lanes"], f"{rpath}.lanes")
        if any(lane not in LANES for lane in lanes):
            raise _error(f"{rpath}.lanes", "unsupported lane")
        profiles = _id_array(requirement["target_profiles"], f"{rpath}.target_profiles")
        if any(profile not in profile_ids for profile in profiles):
            raise _error(f"{rpath}.target_profiles", "unknown target profile")
    for lane in LANES:
        policy = _mapping(operation[lane], f"{path}.{lane}")
        applicability = policy.get("applicability")
        if applicability == "not_applicable":
            _exact(policy, {"applicability", "reason"}, f"{path}.{lane}")
            _string(policy["reason"], f"{path}.{lane}.reason")
        elif applicability in {"required", "optional"}:
            expected = {"applicability", "target_profiles"}
            if lane == "coverage":
                expected.add("component_ids")
            if lane == "benchmark":
                expected.add("metrics")
            _exact(policy, expected, f"{path}.{lane}")
            profiles = _id_array(policy["target_profiles"], f"{path}.{lane}.target_profiles")
            if any(profile not in profile_ids for profile in profiles):
                raise _error(f"{path}.{lane}.target_profiles", "unknown target profile")
            if lane == "coverage":
                components = _id_array(policy["component_ids"], f"{path}.{lane}.component_ids")
                if any(component not in component_ids for component in components):
                    raise _error(f"{path}.{lane}.component_ids", "unknown coverage component")
            if lane == "benchmark":
                metrics = _id_array(policy["metrics"], f"{path}.{lane}.metrics")
                if any(metric not in {"latency", "throughput", "allocations", "peak_memory", "resident_memory", "artifact_size", "encoded_size", "startup_time", "cpu_time"} for metric in metrics):
                    raise _error(f"{path}.{lane}.metrics", "unsupported benchmark metric")
        else:
            raise _error(f"{path}.{lane}.applicability", "unsupported applicability")


def _validate_comparison(value: Any, path: str) -> None:
    comparison = _mapping(value, f"{path}.comparison")
    kind = comparison.get("kind")
    if kind not in COMPARISON_KINDS:
        raise _error(f"{path}.comparison.kind", "unsupported comparison kind")
    if kind in {"exact", "ordered", "unordered", "bytes"}:
        _exact(comparison, {"kind"}, f"{path}.comparison")
    elif kind == "numeric":
        _exact(comparison, {"kind", "absolute_tolerance", "relative_tolerance", "nan_policy"}, f"{path}.comparison")
        for key in ("absolute_tolerance", "relative_tolerance"):
            if not isinstance(comparison[key], (int, float)) or comparison[key] < 0:
                raise _error(f"{path}.comparison.{key}", "expected non-negative number")
        if comparison["nan_policy"] not in {"forbidden", "equal", "unequal"}:
            raise _error(f"{path}.comparison.nan_policy", "unsupported NaN policy")
    elif kind == "text":
        _exact(comparison, {"kind", "transforms", "reason"}, f"{path}.comparison")
        transforms = _id_array(comparison["transforms"], f"{path}.comparison.transforms")
        if any(item not in {"normalize_newlines", "normalize_path_separators", "strip_runtime_addresses", "unicode_nfc"} for item in transforms):
            raise _error(f"{path}.comparison.transforms", "unsupported transform")
        if transforms and (not isinstance(comparison["reason"], str) or not comparison["reason"]):
            raise _error(f"{path}.comparison.reason", "transforms require a reason")
    elif kind == "image":
        _exact(comparison, {"kind", "pixel_mode", "maximum_channel_delta", "metadata_mode", "reason"}, f"{path}.comparison")
        if comparison["pixel_mode"] not in {"exact", "bounded_delta"}:
            raise _error(f"{path}.comparison.pixel_mode", "unsupported pixel policy")
        delta = comparison["maximum_channel_delta"]
        if type(delta) is not int or delta < 0 or (comparison["pixel_mode"] == "exact" and delta != 0) or (comparison["pixel_mode"] == "bounded_delta" and delta <= 0):
            raise _error(f"{path}.comparison.maximum_channel_delta", "invalid image delta")
        if comparison["metadata_mode"] not in {"exact", "declared_only", "ignored"}:
            raise _error(f"{path}.comparison.metadata_mode", "unsupported metadata policy")
        if (comparison["pixel_mode"] != "exact" or comparison["metadata_mode"] != "exact") and (not isinstance(comparison["reason"], str) or not comparison["reason"]):
            raise _error(f"{path}.comparison.reason", "relaxed image policy requires a reason")
    else:
        _exact(comparison, {"kind", "path_mode", "ordering", "content_mode"}, f"{path}.comparison")
        if comparison["path_mode"] != "relative" or comparison["ordering"] != "sorted" or comparison["content_mode"] != "exact":
            raise _error(f"{path}.comparison", "filesystem comparison must be relative, sorted, and exact")


def validate_manifest(manifest: Any, *, manifest_path: Path | None = None) -> dict[str, Any]:
    """Validate and return a manifest object using only fixed fields."""

    manifest = _exact(
        manifest,
        {"schema", "scope", "oracles", "targets", "target_profiles", "commands", "interfaces", "input_index", "coverage_components", "surfaces", "documentation"},
        "manifest",
    )
    if manifest["schema"] != SCHEMA_MANIFEST:
        raise _error("manifest.schema", f"expected {SCHEMA_MANIFEST}")
    scope = _exact(manifest["scope"], {"id", "mode", "inventory"}, "manifest.scope")
    _string(scope["id"], "manifest.scope.id")
    if scope["mode"] not in {"full", "slice"}:
        raise _error("manifest.scope.mode", "unsupported scope mode")
    inventory = _exact(scope["inventory"], {"authority", "revision", "command_id"}, "manifest.scope.inventory")
    for key in inventory:
        _string(inventory[key], f"manifest.scope.inventory.{key}")
    oracles = _list(manifest["oracles"], "manifest.oracles")
    oracle_ids: set[str] = set()
    for index, oracle in enumerate(oracles):
        path = f"manifest.oracles[{index}]"
        oracle = _exact(oracle, {"id", "name", "version", "runtime", "identity_command_id", "contract", "components"}, path)
        oracle_id = _string(oracle["id"], f"{path}.id")
        if oracle_id in oracle_ids:
            raise _error(f"{path}.id", "duplicate oracle ID")
        oracle_ids.add(oracle_id)
        for key in ("name", "version", "runtime", "identity_command_id", "contract"):
            _string(oracle[key], f"{path}.{key}")
        for component_index, component in enumerate(_list(oracle["components"], f"{path}.components")):
            component = _exact(component, {"id", "name", "version"}, f"{path}.components[{component_index}]")
            for key in component:
                _string(component[key], f"{path}.components[{component_index}].{key}")
    targets = _list(manifest["targets"], "manifest.targets")
    target_ids: set[str] = set()
    for index, target in enumerate(targets):
        path = f"manifest.targets[{index}]"
        target = _exact(target, {"id", "name", "runtime", "identity_command_id", "contract"}, path)
        target_id = _string(target["id"], f"{path}.id")
        if target_id in target_ids:
            raise _error(f"{path}.id", "duplicate target ID")
        target_ids.add(target_id)
        for key in target:
            _string(target[key], f"{path}.{key}")
    profiles = _list(manifest["target_profiles"], "manifest.target_profiles")
    profile_ids: set[str] = set()
    for index, profile in enumerate(profiles):
        path = f"manifest.target_profiles[{index}]"
        profile = _exact(profile, {"id", "target_id", "backend", "features"}, path)
        profile_id = _string(profile["id"], f"{path}.id")
        if profile_id in profile_ids:
            raise _error(f"{path}.id", "duplicate target profile ID")
        profile_ids.add(profile_id)
        if profile["target_id"] not in target_ids:
            raise _error(f"{path}.target_id", "unknown target")
        _string(profile["backend"], f"{path}.backend")
        _id_array(profile["features"], f"{path}.features")
    commands = _list(manifest["commands"], "manifest.commands")
    command_ids: set[str] = set()
    for index, command in enumerate(commands):
        path = f"manifest.commands[{index}]"
        command = _exact(command, {"id", "argv", "cwd", "timeout_seconds"}, path)
        command_id = _string(command["id"], f"{path}.id")
        if command_id in command_ids:
            raise _error(f"{path}.id", "duplicate command ID")
        command_ids.add(command_id)
        _id_array(command["argv"], f"{path}.argv")
        _relative_path(command["cwd"], f"{path}.cwd")
        if type(command["timeout_seconds"]) is not int or command["timeout_seconds"] <= 0:
            raise _error(f"{path}.timeout_seconds", "expected positive integer")
    if inventory["command_id"] not in command_ids:
        raise _error("manifest.scope.inventory.command_id", "unknown command")
    for collection, label in ((oracles, "oracles"), (targets, "targets")):
        for index, item in enumerate(collection):
            if item["identity_command_id"] not in command_ids:
                raise _error(f"manifest.{label}[{index}].identity_command_id", "unknown command")
    interfaces = _exact(manifest["interfaces"], {"parity", "coverage", "benchmark", "aggregation"}, "manifest.interfaces")
    expected_lanes = {
        "parity": ("migration-parity/parity-input@1", "migration-parity/parity-result@1"),
        "coverage": ("migration-parity/coverage-input@1", "migration-parity/coverage-result@1"),
        "benchmark": ("migration-parity/benchmark-input@1", "migration-parity/benchmark-result@1"),
    }
    for lane, (input_schema, result_schema) in expected_lanes.items():
        item = _exact(interfaces[lane], {"input_schema", "result_schema", "command_id"}, f"manifest.interfaces.{lane}")
        if (item["input_schema"], item["result_schema"]) != (input_schema, result_schema):
            raise _error(f"manifest.interfaces.{lane}", "schema does not match lane")
        if item["command_id"] not in command_ids:
            raise _error(f"manifest.interfaces.{lane}.command_id", "unknown command")
    aggregation = _exact(interfaces["aggregation"], {"input_schemas", "result_schema", "command_id"}, "manifest.interfaces.aggregation")
    if aggregation["input_schemas"] != [item[1] for item in expected_lanes.values()] or aggregation["result_schema"] != "migration-parity/status-report@1":
        raise _error("manifest.interfaces.aggregation", "aggregation schemas do not match fixed lane results")
    if aggregation["command_id"] not in command_ids:
        raise _error("manifest.interfaces.aggregation.command_id", "unknown command")
    input_index = _exact(manifest["input_index"], set(LANES), "manifest.input_index")
    for lane in LANES:
        paths = _id_array(input_index[lane], f"manifest.input_index.{lane}")
        for relative in paths:
            if not relative.startswith(f"inputs/{lane}/") or not relative.endswith(".json"):
                raise _error(f"manifest.input_index.{lane}", "indexed path must stay beneath its lane root")
            if manifest_path is not None and not (manifest_path.parent / relative).is_file():
                raise _error(f"manifest.input_index.{lane}", f"missing indexed file {relative}")
    components = _list(manifest["coverage_components"], "manifest.coverage_components")
    component_ids: set[str] = set()
    for index, component in enumerate(components):
        path = f"manifest.coverage_components[{index}]"
        component = _exact(component, {"id", "target_profile", "paths", "dimensions", "thresholds"}, path)
        component_id = _string(component["id"], f"{path}.id")
        if component_id in component_ids:
            raise _error(f"{path}.id", "duplicate coverage component ID")
        component_ids.add(component_id)
        if component["target_profile"] not in profile_ids:
            raise _error(f"{path}.target_profile", "unknown target profile")
        for path_index, source_path in enumerate(_list(component["paths"], f"{path}.paths")):
            _relative_path(source_path, f"{path}.paths[{path_index}]")
        dimensions = _id_array(component["dimensions"], f"{path}.dimensions")
        if any(item not in {"function", "line", "branch", "region"} for item in dimensions):
            raise _error(f"{path}.dimensions", "unsupported coverage dimension")
        for threshold_index, threshold in enumerate(_list(component["thresholds"], f"{path}.thresholds")):
            tpath = f"{path}.thresholds[{threshold_index}]"
            threshold = _exact(threshold, {"dimension", "minimum_percent"}, tpath)
            if threshold["dimension"] not in dimensions or type(threshold["minimum_percent"]) is not int or not 0 <= threshold["minimum_percent"] <= 100:
                raise _error(tpath, "invalid coverage threshold")
    requirement_ids: set[str] = set()
    known_requirement_ids: set[str] = set()
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            for requirement in operation["requirements"]:
                requirement_id = requirement.get("id")
                if requirement_id in known_requirement_ids:
                    raise _error("manifest.surfaces", f"duplicate requirement ID {requirement_id!r}")
                known_requirement_ids.add(requirement_id)
    for surface_index, surface in enumerate(_list(manifest["surfaces"], "manifest.surfaces")):
        path = f"manifest.surfaces[{surface_index}]"
        surface = _exact(surface, {"id", "kind", "source_path", "storage_slug", "operations"}, path)
        surface_id = _string(surface["id"], f"{path}.id")
        if surface["kind"] not in SURFACE_KINDS:
            raise _error(f"{path}.kind", "unsupported surface kind")
        _string(surface["source_path"], f"{path}.source_path")
        _string(surface["storage_slug"], f"{path}.storage_slug")
        operation_ids: set[str] = set()
        for operation in _list(surface["operations"], f"{path}.operations"):
            operation_id = operation.get("id")
            if operation_id in operation_ids:
                raise _error(f"{path}.operations", "duplicate operation ID")
            operation_ids.add(operation_id)
            _validate_manifest_operation(operation, surface_id, requirement_ids, known_requirement_ids, command_ids, profile_ids, target_ids, component_ids)
    for index, operation in enumerate([operation for surface in manifest["surfaces"] for operation in surface["operations"]]):
        if operation["source"]["oracle_id"] not in oracle_ids:
            raise _error(f"manifest.operation[{index}].source.oracle_id", "unknown oracle")
    documentation = _exact(manifest["documentation"], {"command_id", "specification_outputs", "evidence_outputs"}, "manifest.documentation")
    if documentation["command_id"] not in command_ids:
        raise _error("manifest.documentation.command_id", "unknown command")
    for key in ("specification_outputs", "evidence_outputs"):
        for index, output in enumerate(_list(documentation[key], f"manifest.documentation.{key}")):
            _relative_path(output, f"manifest.documentation.{key}[{index}]")
    return manifest


def _validate_descriptor(value: Any, path: str, assets: set[str], prior_steps: set[str]) -> None:
    descriptor = _mapping(value, path)
    kind = descriptor.get("kind")
    if kind == "literal":
        _exact(descriptor, {"kind", "value"}, path)
    elif kind == "asset":
        _exact(descriptor, {"kind", "asset_id"}, path)
        if descriptor["asset_id"] not in assets:
            raise _error(f"{path}.asset_id", "unknown asset")
    elif kind == "binding":
        _exact(descriptor, {"kind", "step_id"}, path)
        if descriptor["step_id"] not in prior_steps:
            raise _error(f"{path}.step_id", "binding must refer to an earlier step")
    elif kind == "bindings":
        _exact(descriptor, {"kind", "step_ids"}, path)
        step_ids = _list(descriptor["step_ids"], f"{path}.step_ids")
        for index, step_id in enumerate(step_ids):
            step_id = _string(step_id, f"{path}.step_ids[{index}]")
            if step_id not in prior_steps:
                raise _error(
                    f"{path}.step_ids[{index}]",
                    "binding must refer to an earlier step",
                )
    else:
        raise _error(f"{path}.kind", "unsupported value descriptor")


def _validate_asset(asset: Any, path: str, asset_ids: set[str], fixture_root: Path | None) -> str:
    asset = _mapping(asset, path)
    kind = asset.get("kind")
    key_sets = {
        "ref": {"id", "kind", "path", "sha256", "media_type"},
        "inline": {"id", "kind", "encoding", "data", "sha256", "media_type"},
        "builtin": {"id", "kind", "name"},
        "missing": {"id", "kind", "path"},
        "generated": {"id", "kind", "path", "command_id", "seed", "sha256", "media_type"},
        "remote_mock": {"id", "kind", "path", "command_id", "endpoint", "sha256", "media_type"},
    }
    if kind not in key_sets:
        raise _error(f"{path}.kind", "unsupported asset kind")
    _exact(asset, key_sets[kind], path)
    asset_id = _string(asset["id"], f"{path}.id")
    if asset_id in asset_ids:
        raise _error(f"{path}.id", "duplicate asset ID")
    asset_ids.add(asset_id)
    if kind in {"ref", "missing", "generated", "remote_mock"}:
        _relative_path(asset["path"], f"{path}.path")
    if kind == "ref" and fixture_root is not None:
        referenced = fixture_root / "assets" / asset["path"]
        if not referenced.is_file():
            raise _error(f"{path}.path", f"missing active asset {asset['path']}")
        actual_digest = hashlib.sha256(referenced.read_bytes()).hexdigest()
        if asset["sha256"] != actual_digest:
            raise _error(f"{path}.sha256", "asset digest does not match active bytes")
    if kind == "inline":
        if asset["encoding"] not in {"base64", "utf8"}:
            raise _error(f"{path}.encoding", "unsupported inline encoding")
        if not isinstance(asset["data"], str):
            raise _error(f"{path}.data", "expected string")
        _sha(asset["sha256"], f"{path}.sha256")
        try:
            if asset["encoding"] == "base64":
                import base64

                raw = base64.b64decode(asset["data"], validate=True)
            else:
                raw = asset["data"].encode("utf-8")
        except (ValueError, UnicodeEncodeError) as exc:
            raise _error(f"{path}.data", "invalid inline encoding") from exc
        if hashlib.sha256(raw).hexdigest() != asset["sha256"]:
            raise _error(f"{path}.sha256", "inline asset digest does not match data")
    if kind in {"ref", "generated", "remote_mock"}:
        _sha(asset["sha256"], f"{path}.sha256")
    if kind in {"ref", "inline", "generated", "remote_mock"}:
        _string(asset["media_type"], f"{path}.media_type")
    return asset_id


def _literal_type(value: Any) -> str:
    if value is None:
        return "null"
    if type(value) is bool:
        return "boolean"
    if type(value) is int:
        return "integer"
    if type(value) is float:
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "sequence"
    if isinstance(value, dict):
        if value.get("protocol") == "outline":
            return "handle"
        return "mapping"
    return "any_json"


def _literal_matches(value: Any, declared: list[str]) -> bool:
    actual = _literal_type(value)
    if "any_json" in declared or actual in declared:
        return True
    if actual == "integer" and "number" in declared:
        return True
    # Python IntEnum values serialize as integers in the input language even
    # though the reflected signature exposes the enum type.
    if actual == "integer" and "enum" in declared:
        return True
    if actual == "string" and "enum" in declared:
        return True
    if actual == "mapping" and ("record" in declared or "any_json" in declared):
        return True
    return False


def validate_inputs(manifest: dict[str, Any], fixture_root: Path, *, lane: str | None = None) -> dict[str, dict[str, Any]]:
    """Validate every indexed active input document and its cross-references."""

    if lane is not None and lane not in LANES:
        raise _error("lane", "unsupported lane")
    if lane == "parity":
        lanes = ("parity",)
    elif lane == "coverage":
        lanes = ("parity", "coverage")
    elif lane == "benchmark":
        lanes = ("parity", "benchmark")
    else:
        lanes = LANES
    operation_index = _manifest_operation_index(manifest)
    requirement_index = _requirement_index(manifest)
    profile_ids = {profile["id"] for profile in manifest["target_profiles"]}
    command_ids = {command["id"] for command in manifest["commands"]}
    component_ids = {component["id"] for component in manifest["coverage_components"]}
    item_ids: set[str] = set()
    parity_case_ids: set[str] = set()
    documents: dict[str, dict[str, Any]] = {}
    for current_lane in lanes:
        seen_paths: set[str] = set()
        for relative in manifest["input_index"][current_lane]:
            if relative in seen_paths:
                raise _error(f"input_index.{current_lane}", "duplicate indexed path")
            seen_paths.add(relative)
            path = fixture_root / relative
            if not path.is_file():
                raise _error(relative, "indexed input does not exist")
            try:
                document = json.loads(path.read_text(encoding="utf-8"))
            except json.JSONDecodeError as exc:
                raise _error(relative, f"invalid JSON: {exc}") from exc
            expected_schema = SCHEMA_BY_LANE[current_lane]
            if current_lane == "parity":
                _validate_parity_document(document, relative, operation_index, requirement_index, profile_ids, fixture_root, item_ids)
                parity_case_ids.update(item_ids)
            elif current_lane == "coverage":
                _validate_coverage_document(document, relative, operation_index, requirement_index, profile_ids, command_ids, component_ids, item_ids, parity_case_ids)
            else:
                _validate_benchmark_document(document, relative, operation_index, requirement_index, profile_ids, item_ids, parity_case_ids)
            if document.get("schema") != expected_schema:
                raise _error(relative, f"expected {expected_schema}")
            documents[relative] = document
    return documents


def _reject_output_fields(value: Any, path: str) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in OUTPUT_KEYS:
                raise _error(f"{path}.{key}", "expected outputs are forbidden in active inputs")
            _reject_output_fields(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_output_fields(child, f"{path}[{index}]")


def _validate_parity_document(document: Any, relative: str, operation_index: dict[tuple[str, str], dict[str, Any]], requirement_index: dict[str, dict[str, Any]], profile_ids: set[str], fixture_root: Path, item_ids: set[str]) -> None:
    document = _exact(document, {"schema", "cases"}, relative)
    _reject_output_fields(document, relative)
    for index, case in enumerate(_list(document["cases"], f"{relative}.cases")):
        path = f"{relative}.cases[{index}]"
        case = _exact(case, {"case_id", "surface", "operation", "covers", "target_profiles", "assets", "steps", "observations"}, path)
        case_id = _string(case["case_id"], f"{path}.case_id")
        if case_id in item_ids:
            raise _error(f"{path}.case_id", "duplicate executable item ID")
        item_ids.add(case_id)
        surface = _string(case["surface"], f"{path}.surface")
        operation = _string(case["operation"], f"{path}.operation")
        if (surface, operation) not in operation_index:
            raise _error(path, "unknown primary operation")
        profiles = _id_array(case["target_profiles"], f"{path}.target_profiles")
        if not profiles or any(profile not in profile_ids for profile in profiles):
            raise _error(f"{path}.target_profiles", "unknown or missing target profile")
        covers = _id_array(case["covers"], f"{path}.covers")
        if not covers:
            raise _error(f"{path}.covers", "a parity case must cover at least one requirement")
        for requirement_id in covers:
            if requirement_id not in requirement_index:
                raise _error(f"{path}.covers", f"unknown requirement {requirement_id!r}")
            requirement = requirement_index[requirement_id]
            if "parity" not in requirement["lanes"] or not set(profiles).intersection(requirement["target_profiles"]):
                raise _error(f"{path}.covers", f"requirement {requirement_id!r} is not applicable to this case")
        assets = _list(case["assets"], f"{path}.assets")
        asset_ids: set[str] = set()
        for asset_index, asset in enumerate(assets):
            _validate_asset(asset, f"{path}.assets[{asset_index}]", asset_ids, fixture_root)
        steps = _list(case["steps"], f"{path}.steps")
        step_ids: set[str] = set()
        step_map: dict[str, dict[str, Any]] = {}
        for step_index, step in enumerate(steps):
            spath = f"{path}.steps[{step_index}]"
            step = _exact(step, {"step_id", "surface", "operation", "receiver", "arguments"}, spath)
            step_id = _string(step["step_id"], f"{spath}.step_id")
            if step_id in step_ids:
                raise _error(f"{spath}.step_id", "duplicate step ID")
            step_ids.add(step_id)
            step_surface = _string(step["surface"], f"{spath}.surface")
            step_operation = _string(step["operation"], f"{spath}.operation")
            operation_definition = operation_index.get((step_surface, step_operation))
            if operation_definition is None:
                raise _error(spath, "unknown workflow operation")
            if step["receiver"] is not None:
                _validate_descriptor(step["receiver"], f"{spath}.receiver", asset_ids, step_ids - {step_id})
            arguments = _mapping(step["arguments"], f"{spath}.arguments")
            parameters = {parameter["id"]: parameter for parameter in operation_definition["source"]["parameters"]}
            unknown = set(arguments) - set(parameters)
            if unknown:
                raise _error(f"{spath}.arguments", f"unknown parameter(s) {sorted(unknown)}")
            for parameter in parameters.values():
                if parameter["omission"]["kind"] == "required" and parameter["style"] != "receiver" and parameter["id"] not in arguments:
                    raise _error(f"{spath}.arguments", f"missing required parameter {parameter['id']!r}")
            for name, descriptor in arguments.items():
                _validate_descriptor(descriptor, f"{spath}.arguments.{name}", asset_ids, step_ids - {step_id})
                if descriptor["kind"] == "literal" and not _literal_matches(descriptor.get("value"), parameters[name]["value_types"]):
                    raise _error(f"{spath}.arguments.{name}", "literal does not match declared parameter value_types")
            step_map[step_id] = step
        observations = _id_array(case["observations"], f"{path}.observations")
        if not observations:
            raise _error(f"{path}.observations", "a parity case must observe at least one step")
        if any(observation not in step_ids for observation in observations):
            raise _error(f"{path}.observations", "unknown observed step")
        primary_observed = [step_id for step_id in observations if step_map[step_id]["surface"] == surface and step_map[step_id]["operation"] == operation]
        if not primary_observed:
            raise _error(f"{path}.observations", "primary operation must be observed")
        for requirement_id in covers:
            matching_operations = [
                key
                for key in operation_index
                if requirement_id.startswith(f"{key[0]}.{key[1]}.")
            ]
            req_surface, req_operation = max(
                matching_operations,
                key=lambda key: len(f"{key[0]}.{key[1]}"),
                default=(None, None),
            )
            if (req_surface, req_operation) not in {(step_map[step_id]["surface"], step_map[step_id]["operation"]) for step_id in observations}:
                raise _error(f"{path}.observations", f"covered requirement {requirement_id!r} has no observed operation")


def _validate_coverage_document(document: Any, relative: str, operation_index: dict[tuple[str, str], dict[str, Any]], requirement_index: dict[str, dict[str, Any]], profile_ids: set[str], command_ids: set[str], component_ids: set[str], item_ids: set[str], known_parity_case_ids: set[str] | None = None) -> None:
    document = _exact(document, {"schema", "plans"}, relative)
    _reject_output_fields(document, relative)
    for index, plan in enumerate(_list(document["plans"], f"{relative}.plans")):
        path = f"{relative}.plans[{index}]"
        plan = _exact(plan, {"plan_id", "covers", "target_profile", "selectors", "component_ids", "command_id"}, path)
        plan_id = _string(plan["plan_id"], f"{path}.plan_id")
        if plan_id in item_ids:
            raise _error(f"{path}.plan_id", "duplicate executable item ID")
        item_ids.add(plan_id)
        if plan["target_profile"] not in profile_ids:
            raise _error(f"{path}.target_profile", "unknown target profile")
        covers = _id_array(plan["covers"], f"{path}.covers")
        if not covers:
            raise _error(f"{path}.covers", "coverage plan must cover at least one requirement")
        for requirement_id in covers:
            requirement = requirement_index.get(requirement_id)
            if requirement is None or "coverage" not in requirement["lanes"] or plan["target_profile"] not in requirement["target_profiles"]:
                raise _error(f"{path}.covers", f"invalid coverage requirement {requirement_id!r}")
        selectors = _exact(plan["selectors"], {"parity_case_ids", "command_ids"}, f"{path}.selectors")
        selected_case_ids = _id_array(selectors["parity_case_ids"], f"{path}.selectors.parity_case_ids")
        selected_commands = _id_array(selectors["command_ids"], f"{path}.selectors.command_ids")
        if not selected_case_ids and not selected_commands:
            raise _error(f"{path}.selectors", "at least one selector is required")
        if known_parity_case_ids is not None and any(case_id not in known_parity_case_ids for case_id in selected_case_ids):
            raise _error(f"{path}.selectors.parity_case_ids", "unknown parity case")
        if any(command not in command_ids for command in selected_commands) or plan["command_id"] not in command_ids:
            raise _error(f"{path}.selectors", "unknown command ID")
        components = _id_array(plan["component_ids"], f"{path}.component_ids")
        if any(component not in component_ids for component in components):
            raise _error(f"{path}.component_ids", "unknown coverage component")
        if plan["command_id"] not in command_ids:
            raise _error(f"{path}.command_id", "unknown command")


def _validate_benchmark_document(document: Any, relative: str, operation_index: dict[tuple[str, str], dict[str, Any]], requirement_index: dict[str, dict[str, Any]], profile_ids: set[str], item_ids: set[str], known_parity_case_ids: set[str] | None = None) -> None:
    document = _exact(document, {"schema", "workloads", "suites"}, relative)
    _reject_output_fields(document, relative)
    workload_ids: set[str] = set()
    for index, workload in enumerate(_list(document["workloads"], f"{relative}.workloads")):
        path = f"{relative}.workloads[{index}]"
        workload = _exact(workload, {"workload_id", "covers", "subjects", "input", "measurement"}, path)
        workload_id = _string(workload["workload_id"], f"{path}.workload_id")
        if workload_id in item_ids:
            raise _error(f"{path}.workload_id", "duplicate executable item ID")
        item_ids.add(workload_id)
        workload_ids.add(workload_id)
        covers = _id_array(workload["covers"], f"{path}.covers")
        if not covers:
            raise _error(f"{path}.covers", "benchmark workload must cover at least one requirement")
        for requirement_id in covers:
            requirement = requirement_index.get(requirement_id)
            if requirement is None or "benchmark" not in requirement["lanes"]:
                raise _error(f"{path}.covers", f"invalid benchmark requirement {requirement_id!r}")
        subjects = _list(workload["subjects"], f"{path}.subjects")
        if not subjects:
            raise _error(f"{path}.subjects", "at least one subject is required")
        subject_ids: set[str] = set()
        for subject_index, subject in enumerate(subjects):
            spath = f"{path}.subjects[{subject_index}]"
            subject = _exact(subject, {"kind", "id"}, spath)
            if subject["kind"] not in {"oracle", "target_profile"}:
                raise _error(f"{spath}.kind", "unsupported benchmark subject kind")
            subject_id = _string(subject["id"], f"{spath}.id")
            if subject_id in subject_ids:
                raise _error(f"{spath}.id", "duplicate subject ID")
            subject_ids.add(subject_id)
            if subject["kind"] == "target_profile" and subject_id not in profile_ids:
                raise _error(f"{spath}.id", "unknown target profile")
        input_spec = _mapping(workload["input"], f"{path}.input")
        kind = input_spec.get("kind")
        if kind == "parity_case":
            _exact(input_spec, {"kind", "case_id"}, f"{path}.input")
            if known_parity_case_ids is not None and input_spec["case_id"] not in known_parity_case_ids:
                raise _error(f"{path}.input.case_id", "unknown parity case (inputs are validated in the same run)")
        elif kind == "workflow":
            _exact(input_spec, {"kind", "assets", "steps", "observations"}, f"{path}.input")
        elif kind == "command":
            _exact(input_spec, {"kind", "command_id"}, f"{path}.input")
        elif kind == "artifact":
            _exact(input_spec, {"kind", "path"}, f"{path}.input")
            _relative_path(input_spec["path"], f"{path}.input.path")
        else:
            raise _error(f"{path}.input.kind", "unsupported benchmark input kind")
        measurement = _exact(workload["measurement"], {"boundary", "step_ids", "metrics", "warmup_iterations", "measurement_iterations", "samples", "concurrency", "cache_state", "correctness_gate"}, f"{path}.measurement")
        if measurement["boundary"] not in {"observed_steps", "whole_workflow", "process", "artifact"}:
            raise _error(f"{path}.measurement.boundary", "unsupported measurement boundary")
        _id_array(measurement["step_ids"], f"{path}.measurement.step_ids")
        metrics = _id_array(measurement["metrics"], f"{path}.measurement.metrics")
        if any(metric not in {"latency", "throughput", "allocations", "peak_memory", "resident_memory", "artifact_size", "encoded_size", "startup_time", "cpu_time"} for metric in metrics):
            raise _error(f"{path}.measurement.metrics", "unsupported benchmark metric")
        for key in ("warmup_iterations", "measurement_iterations", "samples", "concurrency"):
            if type(measurement[key]) is not int or measurement[key] < 0 or (key != "warmup_iterations" and measurement[key] == 0):
                raise _error(f"{path}.measurement.{key}", "expected valid non-negative measurement count")
        if measurement["cache_state"] not in {"cold", "warm", "mixed"}:
            raise _error(f"{path}.measurement.cache_state", "unsupported cache state")
        if measurement["correctness_gate"] not in {"parity_pass", "source_target_match", "successful_execution", "not_applicable"}:
            raise _error(f"{path}.measurement.correctness_gate", "unsupported correctness gate")
    for index, suite in enumerate(_list(document["suites"], f"{relative}.suites")):
        path = f"{relative}.suites[{index}]"
        suite = _exact(suite, {"suite_id", "description", "members"}, path)
        suite_id = _string(suite["suite_id"], f"{path}.suite_id")
        if suite_id in item_ids:
            raise _error(f"{path}.suite_id", "duplicate executable item ID")
        item_ids.add(suite_id)
        _string(suite["description"], f"{path}.description")
        members = _list(suite["members"], f"{path}.members")
        if not members:
            raise _error(f"{path}.members", "suite must contain a workload")
        member_ids: set[str] = set()
        for member_index, member in enumerate(members):
            mpath = f"{path}.members[{member_index}]"
            member = _exact(member, {"workload_id", "weight"}, mpath)
            if member["workload_id"] not in workload_ids:
                raise _error(f"{mpath}.workload_id", "unknown workload")
            if member["workload_id"] in member_ids:
                raise _error(f"{mpath}.workload_id", "duplicate suite member")
            member_ids.add(member["workload_id"])
            if not isinstance(member["weight"], (int, float)) or member["weight"] <= 0:
                raise _error(f"{mpath}.weight", "weight must be positive")


def validate_active_tree(manifest_path: Path) -> dict[str, dict[str, Any]]:
    """Validate the active manifest and every indexed input file."""

    import yaml

    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    validate_manifest(manifest, manifest_path=manifest_path)
    return validate_inputs(manifest, manifest_path.parent)


__all__ = ["SCHEMA_MANIFEST", "SCHEMA_BY_LANE", "validate_manifest", "validate_inputs", "validate_active_tree"]
