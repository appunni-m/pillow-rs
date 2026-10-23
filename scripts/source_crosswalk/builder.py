"""Build the operation-to-source graph and conservative review findings."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable

import yaml

from .python_bindings import (
    Binding,
    PythonBindingIndex,
    direct_class_base_names,
    direct_self_method_call_lines,
    python_method_owner,
)
from .pyo3_bindings import (
    PyO3Index,
    declaration_node,
    py_source_files,
    python_native_calls,
)
from .rust_source import (
    CPU_ROOT,
    PIPELINE_PATH,
    REGISTRATION_ROOT,
    REGISTRY_PATH,
    SHADER_ROOT,
    SIMD_ROOT,
    code_only,
    cpu_registration_imports,
    cpu_registration_references,
    enum_variants,
    function_index,
    macro_generated_functions,
    parse_registry_modules,
    variant_key_edges,
)
from .rust_modules import (
    SOURCE_AREAS,
    WORKSPACE_RUST_PACKAGES,
    build_rust_module_inventory,
)
from .rust_calls import (
    core_public_api_node,
    core_image_method_index,
    core_draw_method_index,
    direct_pipeline_variant_references,
    crate_root_api_routes,
    resolve_public_api_call,
    top_level_function_definitions,
    rust_family_for_path,
    structure_role,
)
from .evidence import build_declared_evidence_index

ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = Path("pillow-rs/tests/fixtures/manifest.yaml")
PYTHON_SOURCE = Path("pillow-rs-py/python")
CORE_LIBRARY_PATH = Path("pillow-rs/src/lib.rs")
STRUCTURE_STANDARD_PATH = Path("docs/IMPLEMENTATION_STRUCTURE.md")

def git_identity(root: Path) -> dict[str, Any]:
    def git(*args: str) -> str:
        result = subprocess.run(
            ["git", *args], cwd=root, check=True, text=True, capture_output=True
        )
        return result.stdout.strip()

    return {
        "revision": git("rev-parse", "HEAD"),
        "dirty": bool(git("status", "--porcelain")),
    }


def contextual_python_override(
    binding: Binding, public_path: str, index: PythonBindingIndex
) -> tuple[Binding, int] | None:
    """Resolve one inherited method's direct self-callback for a named subclass."""
    parts = public_path.split(".")
    if len(parts) < 4 or parts[-1] != "enhance":
        return None
    class_name = parts[-2]
    method_owner = python_method_owner(index, binding)
    class_binding = index.bindings.get(binding.module, {}).get(class_name)
    if (
        method_owner is None
        or not isinstance(class_binding, Binding)
        or class_binding.kind != "python_class"
        or method_owner[0] not in direct_class_base_names(class_binding)
    ):
        return None
    call_lines = direct_self_method_call_lines(binding, "_apply")
    override = class_binding.members.get("_apply")
    if not call_lines or override is None or override.kind != "python_method":
        return None
    return override, call_lines[0]


def crosswalk_source_identity(
    root: Path, additional_inputs: Iterable[str] = ()
) -> dict[str, Any]:
    """Fingerprint every source file read by the static crosswalk builder."""
    root = root.resolve()
    files: set[Path] = set()
    fixed = (
        MANIFEST_PATH,
        PIPELINE_PATH,
        REGISTRY_PATH,
        CORE_LIBRARY_PATH,
        STRUCTURE_STANDARD_PATH,
        Path("Cargo.toml"),
        Path("pillow-rs/Cargo.toml"),
        Path("pillow-rs-py/Cargo.toml"),
        Path("pillow-rs-js/Cargo.toml"),
        *(Path(package) / "build.rs" for package in WORKSPACE_RUST_PACKAGES),
        Path("scripts/build_operation_source_crosswalk.py"),
    )
    for relative in fixed:
        path = root / relative
        if path.is_file():
            files.add(path)

    trees = [
        (Path(package) / area, ".rs")
        for package in WORKSPACE_RUST_PACKAGES
        for area in SOURCE_AREAS
    ] + [
        (Path("pillow-rs-py/python/pillow_rs"), ".py"),
        (Path("pillow-rs-py/python/PIL"), ".py"),
        (SHADER_ROOT, ".wgsl"),
        (PYTHON_SOURCE, ".py"),
        (Path("scripts/source_crosswalk"), ".py"),
        (Path("scripts/implementation_structure"), ".py"),
    ]
    for relative, suffix in trees:
        directory = root / relative
        if directory.is_dir():
            files.update(path for path in directory.rglob(f"*{suffix}") if path.is_file())
    for relative in additional_inputs:
        path = root / relative
        if not path.is_file():
            raise FileNotFoundError(f"crosswalk input disappeared during identity capture: {relative}")
        files.add(path.resolve())

    digest = hashlib.sha256()
    for path in sorted(files, key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        contents = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(contents).to_bytes(8, "big"))
        digest.update(contents)
    return {
        "crosswalk_input_sha256": digest.hexdigest(),
        "crosswalk_input_file_count": len(files),
    }


def build_crosswalk(root: Path = ROOT) -> dict[str, Any]:
    root = root.resolve()
    manifest_bytes = (root / MANIFEST_PATH).read_bytes()
    manifest = yaml.safe_load(manifest_bytes)
    python_index = PythonBindingIndex(root / PYTHON_SOURCE, root)
    pyo3_index = PyO3Index(py_source_files(root), root)
    pipeline_source = (root / PIPELINE_PATH).read_text(encoding="utf-8")
    registry_source = (root / REGISTRY_PATH).read_text(encoding="utf-8")
    registration_sources = {
        path.relative_to(root).as_posix(): path.read_text(encoding="utf-8")
        for path in sorted((root / REGISTRATION_ROOT).glob("*.rs"))
    }
    variants = enum_variants(pipeline_source)
    mappings = variant_key_edges(registry_source)
    registrations, simd_entries = parse_registry_modules(registration_sources)
    cpu_imports = cpu_registration_imports(
        registration_sources[(REGISTRATION_ROOT / "prelude.rs").as_posix()]
    )
    cpu_functions = function_index(root / CPU_ROOT, root)
    simd_functions = function_index(root / SIMD_ROOT, root)
    simd_generated = macro_generated_functions(root / SIMD_ROOT, root)
    rust_modules = build_rust_module_inventory(root)
    core_api_routes = crate_root_api_routes(
        (root / CORE_LIBRARY_PATH).read_text(encoding="utf-8"),
        rust_modules["module_edges"],
        crate_root=CORE_LIBRARY_PATH.as_posix(),
        export_path=CORE_LIBRARY_PATH.as_posix(),
    )

    nodes: dict[str, dict[str, Any]] = {}
    edges: list[dict[str, Any]] = []
    findings: list[dict[str, Any]] = []
    public_operation_count = 0
    unmapped_public_operations = 0
    public_bindings: dict[str, Binding] = {}
    callback_bindings: dict[str, Binding] = {}

    for surface in manifest.get("surfaces", []):
        for operation in surface.get("operations", []):
            public_operation_count += 1
            operation_id = f"{surface['id']}.{operation['id']}"
            source_path = str(operation.get("source", {}).get("path") or operation_id)
            public_path = source_path.replace(
                "PIL.ImageDraw.ImageDraw", "PIL.ImageDraw.Draw", 1
            )
            operation_node_id = f"public-operation:{operation_id}"
            nodes[operation_node_id] = {
                "id": operation_node_id,
                "kind": "manifest_public_operation",
                "operation_id": operation_id,
                "surface": surface["id"],
                "operation": operation["id"],
                "operation_kind": operation.get("kind"),
                "source_path": source_path,
                "public_target_path": public_path,
                "manifest_path": MANIFEST_PATH.as_posix(),
            }
            binding, alias_trace = python_index.resolve_public_path(public_path)
            if not isinstance(binding, Binding):
                unmapped_public_operations += 1
                findings.append(
                    {
                        "flag": "public_operation_python_binding_unresolved",
                        "subject": operation_node_id,
                        "evidence": [
                            {
                                "path": MANIFEST_PATH.as_posix(),
                                "public_target_path": public_path,
                            }
                        ],
                        "reason_for_review": "The manifest public path did not resolve through the checked-in PIL and pillow_rs Python AST bindings.",
                    }
                )
                continue
            binding_node_id = f"python-binding:{binding.path}:{binding.line}:{binding.name}"
            public_bindings.setdefault(binding_node_id, binding)
            nodes.setdefault(
                binding_node_id,
                {
                    "id": binding_node_id,
                    **python_index.node(binding),
                },
            )
            edges.append(
                {
                    "from": operation_node_id,
                    "relation": "resolves_to_python_binding",
                    "to": binding_node_id,
                    "site": {"path": binding.path, "line": binding.line},
                    "evidence_kind": "python_ast_alias_resolution",
                    "public_target_path": public_path,
                    "alias_trace": alias_trace,
                    "status": "static_wrapper_resolution",
                }
            )
            contextual_override = contextual_python_override(
                binding, public_path, python_index
            )
            if contextual_override is not None:
                override, callback_line = contextual_override
                callback_node_id = (
                    f"python-binding:{override.path}:{override.line}:{override.name}"
                )
                callback_bindings.setdefault(callback_node_id, override)
                nodes.setdefault(
                    callback_node_id,
                    {"id": callback_node_id, **python_index.node(override)},
                )
                edges.append(
                    {
                        "from": operation_node_id,
                        "relation": "resolves_to_contextual_python_override",
                        "to": callback_node_id,
                        "site": {"path": binding.path, "line": callback_line},
                        "definition_site": {
                            "path": override.path,
                            "line": override.line,
                        },
                        "dispatch_context": public_path,
                        "dispatch_method": "_apply",
                        "evidence_kind": "manifest_class_context_plus_python_inheritance_and_override",
                        "status": "static_contextual_python_override",
                    }
                )

    dynamic_python_callback_candidates: dict[str, list[tuple[str, Binding]]] = {}
    for class_binding in python_index.bindings.get(
        "pillow_rs.imagefilter", {}
    ).values():
        if not isinstance(class_binding, Binding) or class_binding.kind != "python_class":
            continue
        for member_name, member in class_binding.members.items():
            if not member_name.startswith("_") or member.kind != "python_method":
                continue
            callback_node_id = f"python-binding:{member.path}:{member.line}:{member.name}"
            callback_bindings.setdefault(callback_node_id, member)
            nodes.setdefault(
                callback_node_id,
                {"id": callback_node_id, **python_index.node(member)},
            )
            dynamic_python_callback_candidates.setdefault(member_name, []).append(
                (callback_node_id, member)
            )

    mapped_python_native_calls = 0
    python_native_call_count = 0
    unresolved_python_native_calls = 0
    unregistered_python_native_calls = 0
    pyo3_core_public_call_count = 0
    mapped_pyo3_core_public_calls = 0
    unresolved_pyo3_core_public_calls = 0
    linked_pyo3_declarations: set[str] = set()
    pyo3_receiver_method_call_count = 0
    mapped_pyo3_receiver_method_calls = 0
    unresolved_pyo3_receiver_method_calls = 0
    core_method_pipeline_routes = 0
    receiver_method_seeds: list[dict[str, Any]] = []
    routed_python_bindings = {**public_bindings, **callback_bindings}
    for binding_node_id, binding in sorted(routed_python_bindings.items()):
        for call in python_native_calls(binding, python_index):
            python_native_call_count += 1
            declaration = pyo3_index.resolve(call.kind, call.symbol, call.owner)
            site = {"path": call.path, "line": call.line}
            if declaration is None:
                unresolved_python_native_calls += 1
                findings.append(
                    {
                        "flag": "python_native_call_pyo3_declaration_unresolved",
                        "subject": f"{binding_node_id}:{call.kind}:{call.owner or ''}:{call.symbol}",
                        "evidence": [site],
                        "reason_for_review": "A checked-in Python wrapper calls a native extension symbol that was not found in the PyO3 declarations index.",
                    }
                )
                continue
            pyo3_id = (
                f"pyo3-{call.kind}:{declaration.owner + '.' if declaration.owner else ''}"
                f"{declaration.exposed_name}"
            )
            nodes.setdefault(
                pyo3_id,
                {"id": pyo3_id, **declaration_node(declaration)},
            )
            for callback in declaration.dynamic_python_callbacks:
                candidates = dynamic_python_callback_candidates.get(
                    callback["method_name"], []
                )
                for callback_node_id, callback_binding in candidates:
                    edges.append(
                        {
                            "from": pyo3_id,
                            "relation": "may_dispatch_to_python_callback",
                            "to": callback_node_id,
                            "site": {
                                "path": declaration.path,
                                "line": callback["line"],
                            },
                            "definition_site": {
                                "path": callback_binding.path,
                                "line": callback_binding.line,
                            },
                            "call_expression": callback["expression"],
                            "callback_method": callback["method_name"],
                            "evidence_kind": "literal_pyo3_call_method_target_plus_checked_in_callback_override",
                            "status": "dynamic_python_callback_candidate",
                            "runtime_dispatch_target_proven": False,
                        }
                    )
                if candidates:
                    findings.append(
                        {
                            "flag": "dynamic_python_callback_target_review",
                            "subject": f"{pyo3_id}:{callback['method_name']}",
                            "evidence": [
                                {"path": declaration.path, "line": callback["line"]},
                                *[
                                    {"path": member.path, "line": member.line}
                                    for _, member in candidates
                                ],
                            ],
                            "reason_for_review": (
                                "PyO3 dispatches a literal method name to a runtime Python "
                                "object. Listed package methods are candidate targets; the "
                                "actual filter object is not statically fixed here."
                            ),
                        }
                    )
            if declaration.registered:
                mapped_python_native_calls += 1
            else:
                unregistered_python_native_calls += 1
                findings.append(
                    {
                        "flag": "python_native_call_pyo3_registration_unresolved",
                        "subject": pyo3_id,
                        "evidence": [site, {"path": declaration.path, "line": declaration.line}],
                        "reason_for_review": "The PyO3 declaration exists, but the static registration scan did not find its function or owner class in the extension module.",
                    }
                )
            edges.append(
                {
                    "from": binding_node_id,
                    "relation": (
                        "python_callback_candidate_calls_pyo3"
                        if call.dynamic_callback_candidate
                        else "python_wrapper_calls_pyo3"
                    ),
                    "to": pyo3_id,
                    "site": site,
                    "call_expression": call.expression,
                    "evidence_kind": (
                        "python_callback_argument_name_plus_direct_native_method_call"
                        if call.dynamic_callback_candidate
                        else "python_ast_native_call"
                    ),
                    "status": (
                        "dynamic_python_callback_candidate_call"
                        if call.dynamic_callback_candidate and declaration.registered
                        else "dynamic_python_callback_candidate_call_registration_unresolved"
                        if call.dynamic_callback_candidate
                        else "static_registered_binding_call"
                        if declaration.registered
                        else "static_call_declaration_registration_unresolved"
                    ),
                }
            )
            if pyo3_id in linked_pyo3_declarations:
                continue
            linked_pyo3_declarations.add(pyo3_id)
            for receiver_call in declaration.direct_receiver_calls:
                pyo3_receiver_method_call_count += 1
                receiver_type = declaration.receiver_field_type or {}
                if receiver_type.get("resolved_type") not in {
                    "pillow_rs::Image", "pillow_rs::Draw"
                }:
                    unresolved_pyo3_receiver_method_calls += 1
                    findings.append(
                        {
                            "flag": "pyo3_receiver_method_type_unresolved",
                            "subject": f"{pyo3_id}:{receiver_call['symbol']}",
                            "evidence": [
                                {"path": declaration.path, "line": receiver_call["line"]},
                                {"path": declaration.path, "line": declaration.line},
                            ],
                            "reason_for_review": (
                                "A direct PyO3 receiver method call was found, but its field "
                                "type did not resolve to a supported core Image or Draw type."
                            ),
                        }
                    )
                    continue
                receiver_method_seeds.append(
                    {
                        "from": pyo3_id,
                        "call": receiver_call,
                        "site_path": declaration.path,
                        "core_type": receiver_type["resolved_type"],
                        "core_owner": "Image" if receiver_type["resolved_type"] == "pillow_rs::Image" else "Draw",
                    }
                )
            for core_call in declaration.direct_core_calls:
                pyo3_core_public_call_count += 1
                core_reexport, resolution = resolve_public_api_call(core_call, core_api_routes)
                if core_reexport is None:
                    unresolved_pyo3_core_public_calls += 1
                    findings.append(
                        {
                            "flag": "pyo3_core_public_api_route_unresolved",
                            "subject": f"{pyo3_id}:{core_call['symbol']}",
                            "evidence": [
                                {"path": declaration.path, "line": core_call["line"]},
                                {"path": CORE_LIBRARY_PATH.as_posix()},
                            ],
                            "reason_for_review": (
                                "A PyO3 declaration calls a crate-root function, but the "
                                f"static crate-root API route did not resolve ({resolution}). "
                                "Check grouped exports, generated aliases, and conditional modules."
                            ),
                        }
                    )
                    continue
                mapped_pyo3_core_public_calls += 1
                core_node = core_public_api_node(
                    core_reexport,
                    f"pyo3_direct_call_plus_{core_reexport['kind']}",
                )
                core_node_id = core_node["id"]
                nodes.setdefault(core_node_id, core_node)
                edges.append(
                    {
                        "from": pyo3_id,
                        "relation": "calls_core_public_api_route",
                        "to": core_node_id,
                        "site": {"path": declaration.path, "line": core_call["line"]},
                        "reexport_site": {
                            "path": core_reexport["export_path"],
                            "line": core_reexport["export_line"],
                        },
                        "call_expression": core_call["expression"],
                        "evidence_kind": f"pyo3_call_plus_{core_reexport['kind']}",
                        "status": "static_crate_root_api_route",
                        "runtime_execution_proven": False,
                    }
                )

    variant_names = {variant["name"] for variant in variants}
    core_methods_by_type = {
        "pillow_rs::Image": core_image_method_index(root),
        "pillow_rs::Draw": core_draw_method_index(root),
    }
    pending_method_calls = [
        {
            "from": seed["from"],
            "call": seed["call"],
            "site_path": seed["site_path"],
            "core_type": seed["core_type"],
            "core_owner": seed["core_owner"],
            "relation": "calls_core_rust_method",
            "require_public": True,
        }
        for seed in receiver_method_seeds
    ]
    visited_core_methods: set[str] = set()
    while pending_method_calls:
        pending = pending_method_calls.pop()
        call = pending["call"]
        candidates = core_methods_by_type[pending["core_type"]].get(call["symbol"], [])
        if pending["require_public"]:
            candidates = [candidate for candidate in candidates if candidate["public"]]
        if len(candidates) != 1:
            unresolved_pyo3_receiver_method_calls += int(pending["require_public"])
            findings.append(
                {
                    "flag": "core_receiver_method_route_unresolved",
                    "subject": f"{pending['from']}:{call['symbol']}",
                    "evidence": [
                        {"path": pending["site_path"], "line": call["line"]},
                        *[
                            {"path": candidate["path"], "line": candidate["line"]}
                            for candidate in candidates
                        ],
                    ],
                    "reason_for_review": (
                        "The direct receiver call has no unique matching inherent core "
                        "method definition (or more than one definition is present)."
                    ),
                }
            )
            continue
        method = candidates[0]
        method_id = (
            f"core-{pending['core_owner'].lower()}-method:"
            f"{method['path']}:{method['line']}:{method['symbol']}"
        )
        nodes.setdefault(
            method_id,
            {
                "id": method_id,
                "kind": f"core_{pending['core_owner'].lower()}_method",
                "owner": pending["core_owner"],
                "symbol": method["symbol"],
                "path": method["path"],
                "line": method["line"],
                "visibility": "public" if method["public"] else "private",
                "runtime_execution_proven": False,
            },
        )
        edges.append(
            {
                "from": pending["from"],
                "relation": pending["relation"],
                "to": method_id,
                "site": {"path": pending["site_path"], "line": call["line"]},
                "definition_site": {"path": method["path"], "line": method["line"]},
                "call_expression": call["expression"],
                "evidence_kind": "direct_receiver_call_plus_unique_inherent_method",
                "status": "static_direct_method_route",
                "runtime_execution_proven": False,
                **(
                    {
                        "alias_definition_site": {
                            "path": pending["site_path"],
                            "line": call["receiver_alias"]["line"],
                            "expression": call["receiver_alias"]["expression"],
                        }
                    }
                    if call.get("receiver_alias")
                    else {}
                ),
            }
        )
        if pending["require_public"]:
            mapped_pyo3_receiver_method_calls += 1
        if method_id in visited_core_methods:
            continue
        visited_core_methods.add(method_id)
        for reference in method["pipeline_references"]:
            if reference["variant"] not in variant_names:
                findings.append(
                    {
                        "flag": "core_pipeline_variant_unresolved",
                        "subject": f"{method_id}:{reference['variant']}",
                        "evidence": [
                            {"path": reference["path"], "line": reference["line"]},
                            {"path": PIPELINE_PATH.as_posix()},
                        ],
                        "reason_for_review": (
                            "A directly referenced PipelineOp variant has no parsed "
                            "declaration in the PipelineOp enum."
                        ),
                    }
                )
                continue
            core_method_pipeline_routes += 1
            edges.append(
                {
                    "from": method_id,
                    "relation": "references_pipeline_variant",
                    "to": f"pipeline:{reference['variant']}",
                    "site": {"path": reference["path"], "line": reference["line"]},
                    "evidence_kind": "direct_pipelineop_variant_reference",
                    "status": "static_core_method_route",
                    "runtime_execution_proven": False,
                }
            )
        for helper_call in method["direct_self_calls"]:
            if helper_call["symbol"] == "clone":
                # Image and Draw use Clone's trait method; it is not an inherent
                # operation-family helper and does not lead to backend dispatch.
                continue
            pending_method_calls.append(
                {
                    "from": method_id,
                    "call": helper_call,
                    "site_path": method["path"],
                    "core_type": pending["core_type"],
                    "core_owner": pending["core_owner"],
                    "relation": "calls_core_rust_method",
                    "require_public": False,
                }
            )
    mapping_by_variant: dict[str, list[dict[str, Any]]] = {}
    for mapping in mappings:
        mapping_by_variant.setdefault(mapping["variant"], []).append(mapping)
    for variant in variants:
        node_id = f"pipeline:{variant['name']}"
        nodes[node_id] = {"id": node_id, "kind": "pipeline_variant", **variant}
        targets = mapping_by_variant.get(variant["name"], [])
        if not targets:
            findings.append(
                {
                    "flag": "unmapped_pipeline_variant",
                    "subject": node_id,
                    "evidence": [variant],
                    "reason_for_review": "No variant_key arm was parsed for this pipeline variant.",
                }
            )
        elif len(targets) > 1:
            findings.append(
                {
                    "flag": "ambiguous_pipeline_key_mapping",
                    "subject": node_id,
                    "evidence": targets,
                    "reason_for_review": "More than one registry key mapping was found.",
                }
            )
        for mapping in targets:
            key_id = f"registry-key:{mapping['key']}"
            nodes.setdefault(
                key_id,
                {
                    "id": key_id,
                    "kind": "registry_key",
                    "name": mapping["key"],
                    "path": REGISTRY_PATH.as_posix(),
                    "line": mapping["line"],
                },
            )
            edges.append(
                {
                    "from": node_id,
                    "relation": "maps_to_registry_key",
                    "to": key_id,
                    "site": {"path": REGISTRY_PATH.as_posix(), "line": mapping["line"]},
                    "evidence_kind": "variant_key_match_arm",
                    "status": "static_source_mapping",
                }
            )

    pipeline_variant_names = {variant["name"] for variant in variants}
    core_pipeline_variant_routes = 0
    for core_node in [
        node for node in nodes.values() if node.get("kind") == "core_public_api_route"
    ]:
        for reference in direct_pipeline_variant_references(
            root, core_node["path"], core_node["symbol"]
        ):
            if reference["variant"] not in pipeline_variant_names:
                findings.append(
                    {
                        "flag": "core_pipeline_variant_unresolved",
                        "subject": f"{core_node['id']}:{reference['variant']}",
                        "evidence": [
                            {"path": reference["path"], "line": reference["line"]},
                            {"path": PIPELINE_PATH.as_posix()},
                        ],
                        "reason_for_review": (
                            "A directly referenced PipelineOp variant has no parsed "
                            "declaration in the PipelineOp enum."
                        ),
                    }
                )
                continue
            core_pipeline_variant_routes += 1
            edges.append(
                {
                    "from": core_node["id"],
                    "relation": "references_pipeline_variant",
                    "to": f"pipeline:{reference['variant']}",
                    "site": {"path": reference["path"], "line": reference["line"]},
                    "evidence_kind": "direct_pipelineop_variant_reference",
                    "status": "static_core_source_route",
                    "runtime_execution_proven": False,
                }
            )

    core_function_definition_cache: dict[str, dict[str, list[dict[str, Any]]]] = {}
    core_function_helper_routes = 0
    core_function_helper_pipeline_routes = 0
    visited_core_function_nodes: set[str] = set()
    pending_core_function_calls: list[dict[str, Any]] = []
    for core_node in [
        node for node in nodes.values() if node.get("kind") == "core_public_api_route"
    ]:
        path = core_node["path"]
        if path not in core_function_definition_cache:
            source = (root / path).read_text(encoding="utf-8")
            core_function_definition_cache[path] = top_level_function_definitions(source, path)
        root_definitions = core_function_definition_cache[path].get(core_node["symbol"], [])
        if len(root_definitions) != 1:
            continue
        for call in root_definitions[0]["direct_function_calls"]:
            pending_core_function_calls.append(
                {
                    "from": core_node["id"],
                    "call": call,
                    "path": path,
                }
            )
    while pending_core_function_calls:
        pending = pending_core_function_calls.pop()
        call = pending["call"]
        path = pending["path"]
        definitions = core_function_definition_cache[path].get(call["symbol"], [])
        if len(definitions) != 1:
            if len(definitions) > 1:
                findings.append(
                    {
                        "flag": "core_helper_function_route_ambiguous",
                        "subject": f"{pending['from']}:{call['symbol']}",
                        "evidence": [
                            {"path": path, "line": call["line"]},
                            *[
                                {"path": path, "line": definition["line"]}
                                for definition in definitions
                            ],
                        ],
                        "reason_for_review": (
                            "A direct core function call has multiple same-module "
                            "definitions, possibly due to conditional compilation."
                        ),
                    }
                )
            continue
        definition = definitions[0]
        function_id = (
            f"core-function:{path}:{definition['line']}:{definition['symbol']}"
        )
        nodes.setdefault(
            function_id,
            {
                "id": function_id,
                "kind": "core_function",
                "symbol": definition["symbol"],
                "path": path,
                "line": definition["line"],
                "runtime_execution_proven": False,
            },
        )
        edges.append(
            {
                "from": pending["from"],
                "relation": "calls_core_helper_function",
                "to": function_id,
                "site": {"path": path, "line": call["line"]},
                "definition_site": {"path": path, "line": definition["line"]},
                "call_expression": call["expression"],
                "evidence_kind": "direct_unqualified_call_plus_unique_same_module_definition",
                "status": "static_core_helper_route",
                "runtime_execution_proven": False,
            }
        )
        core_function_helper_routes += 1
        if function_id in visited_core_function_nodes:
            continue
        visited_core_function_nodes.add(function_id)
        for reference in definition["pipeline_references"]:
            if reference["variant"] not in pipeline_variant_names:
                findings.append(
                    {
                        "flag": "core_pipeline_variant_unresolved",
                        "subject": f"{function_id}:{reference['variant']}",
                        "evidence": [
                            {"path": path, "line": reference["line"]},
                            {"path": PIPELINE_PATH.as_posix()},
                        ],
                        "reason_for_review": (
                            "A directly referenced PipelineOp variant has no parsed "
                            "declaration in the PipelineOp enum."
                        ),
                    }
                )
                continue
            core_function_helper_pipeline_routes += 1
            edges.append(
                {
                    "from": function_id,
                    "relation": "references_pipeline_variant",
                    "to": f"pipeline:{reference['variant']}",
                    "site": {"path": path, "line": reference["line"]},
                    "evidence_kind": "direct_pipelineop_variant_reference",
                    "status": "static_core_helper_route",
                    "runtime_execution_proven": False,
                }
            )
        for helper_call in definition["direct_function_calls"]:
            pending_core_function_calls.append(
                {"from": function_id, "call": helper_call, "path": path}
            )

    registration_by_key: dict[str, list[dict[str, Any]]] = {}
    for registration in registrations:
        registration_by_key.setdefault(registration["key"], []).append(registration)
    for key, key_registrations in registration_by_key.items():
        key_id = f"registry-key:{key}"
        nodes.setdefault(
            key_id,
            {
                "id": key_id,
                "kind": "registry_key",
                "name": key,
                "path": REGISTRY_PATH.as_posix(),
                "line": key_registrations[0]["line"],
            },
        )
        if len(key_registrations) > 1:
            findings.append(
                {
                    "flag": "duplicate_registry_registration",
                    "subject": key_id,
                    "evidence": [
                        {"path": item["path"], "line": item["line"]}
                        for item in key_registrations
                    ],
                    "reason_for_review": "Multiple registry insertions use this key.",
                }
            )
        for registration in key_registrations:
            registration_path = registration["path"]
            referenced_cpu_names = cpu_registration_references(
                registration["text"], cpu_imports
            )
            cpu_names = [name for name in referenced_cpu_names if name in cpu_functions]
            if not cpu_names:
                findings.append(
                    {
                        "flag": "cpu_registration_function_unresolved",
                        "subject": key_id,
                        "evidence": [{"path": registration_path, "line": registration["line"]}],
                        "reason_for_review": "The registry expression did not resolve to a function definition under pool_cpu/ops.",
                    }
                )
            for name in referenced_cpu_names:
                if name not in cpu_functions:
                    findings.append(
                        {
                            "flag": "cpu_registration_function_unresolved",
                            "subject": f"{key_id}:{name}",
                            "evidence": [{"path": registration_path, "line": registration["line"]}],
                            "reason_for_review": f"Imported CPU operation symbol {name!r} has no parsed definition under pool_cpu/ops.",
                        }
                    )
            for name in cpu_names:
                for definition in cpu_functions[name]:
                    symbol_id = f"cpu-symbol:{name}:{definition['path']}:{definition['line']}"
                    nodes.setdefault(
                        symbol_id,
                        {
                            "id": symbol_id,
                            "kind": "cpu_function",
                            "symbol": name,
                            **definition,
                        },
                    )
                    edges.append(
                        {
                            "from": key_id,
                            "relation": "registry_expression_references_cpu_function",
                            "to": symbol_id,
                            "site": {"path": registration_path, "line": registration["line"]},
                            "evidence_kind": "function_symbol_reference",
                            "status": "static_source_registration",
                        }
                    )
            shader = registration["shader"]
            if shader:
                shader_path = (SHADER_ROOT / shader).as_posix()
                shader_id = f"gpu-default-shader:{shader_path}"
                nodes.setdefault(
                    shader_id,
                    {
                        "id": shader_id,
                        "kind": "gpu_default_shader",
                        "path": shader_path,
                        "cfg": ["feature = gpu"],
                    },
                )
                edges.append(
                    {
                        "from": key_id,
                        "relation": "registers_default_gpu_shader",
                        "to": shader_id,
                        "site": {"path": registration_path, "line": registration["line"]},
                        "evidence_kind": "gpu_entry_shader_literal",
                        "status": "conditional_registration",
                        "cfg": ["feature = gpu"],
                        "runtime_execution_proven": False,
                    }
                )
            else:
                findings.append(
                    {
                        "flag": "gpu_default_shader_unresolved",
                        "subject": key_id,
                        "evidence": [{"path": registration_path, "line": registration["line"]}],
                        "reason_for_review": "No default shader literal was parsed; custom dispatch may select shader assets elsewhere.",
                    }
                )

    simd_by_key: dict[str, list[dict[str, Any]]] = {}
    registered_symbols = {entry["symbol"] for entry in simd_entries}
    for entry in simd_entries:
        registration_path = entry["path"]
        simd_by_key.setdefault(entry["key"], []).append(entry)
        key_id = f"registry-key:{entry['key']}"
        adapter_id = f"simd-adapter:{entry['symbol']}"
        definitions = simd_functions.get(entry["symbol"], [])
        generated = simd_generated.get(entry["symbol"], [])
        if not definitions and not generated:
            findings.append(
                {
                    "flag": "simd_adapter_definition_unresolved",
                    "subject": adapter_id,
                    "evidence": [{"path": registration_path, "line": entry["line"]}],
                    "reason_for_review": "The registered SIMD adapter symbol has no parsed definition under pool_simd.",
                }
            )
        for definition in definitions:
            nodes.setdefault(
                adapter_id,
                {"id": adapter_id, "kind": "simd_adapter", "symbol": entry["symbol"], **definition},
            )
            edges.append(
                {
                    "from": key_id,
                    "relation": "registers_simd_adapter",
                    "to": adapter_id,
                    "site": {"path": registration_path, "line": entry["line"]},
                    "evidence_kind": "simd_set_entry",
                    "status": "static_source_registration",
                }
            )
        for generated_at in generated:
            nodes.setdefault(
                adapter_id,
                {
                    "id": adapter_id,
                    "kind": "macro_generated_simd_adapter",
                    "symbol": entry["symbol"],
                    "path": generated_at["path"],
                    "line": generated_at["line"],
                    "generated_by": generated_at["macro"],
                },
            )
            edges.append(
                {
                    "from": key_id,
                    "relation": "registers_simd_adapter",
                    "to": adapter_id,
                    "site": {"path": registration_path, "line": entry["line"]},
                    "implementation_site": {
                        "path": generated_at["path"],
                        "line": generated_at["line"],
                    },
                    "evidence_kind": "simd_adapter_macro_invocation",
                    "macro": generated_at["macro"],
                    "status": "static_source_registration",
                }
            )
    for variant in variants:
        for mapping in mapping_by_variant.get(variant["name"], []):
            if mapping["key"] not in registration_by_key:
                findings.append(
                    {
                        "flag": "mapped_key_without_registry_entry",
                        "subject": f"registry-key:{mapping['key']}",
                        "evidence": [{"path": REGISTRY_PATH.as_posix(), "line": mapping["line"]}],
                        "reason_for_review": "A PipelineOp variant maps to a key with no parsed m.insert/draw_entry registration.",
                    }
                )
            if mapping["key"] not in simd_by_key:
                # Missing SIMD is normal for some operations; no failure claim is made.
                pass

    shader_refs = {node["path"] for node in nodes.values() if node["kind"] == "gpu_default_shader"}
    shader_refs.update(
        (SHADER_ROOT / match).as_posix()
        for path in sorted((root / "pillow-rs/src/compute/pool_gpu").rglob("*.rs"))
        for match in re.findall(r'"([^"\n]+\.wgsl)"', path.read_text(encoding="utf-8"))
    )
    for shader_path in sorted((root / SHADER_ROOT).rglob("*.wgsl")):
        relative = shader_path.relative_to(root).as_posix()
        if relative not in shader_refs:
            findings.append(
                {
                    "flag": "unreferenced_shader_asset_candidate",
                    "subject": relative,
                    "evidence": [{"path": relative}],
                    "reason_for_review": "No literal default registration or Rust shader-name reference was found; dynamic and generated selection still require review.",
                }
            )

    for key in sorted(set(mapping["key"] for mapping in mappings) - set(registration_by_key)):
        if not any(item["subject"] == f"registry-key:{key}" for item in findings):
            findings.append(
                {
                    "flag": "mapped_key_without_registry_entry",
                    "subject": f"registry-key:{key}",
                    "evidence": [
                        {"path": REGISTRY_PATH.as_posix(), "line": item["line"]}
                        for item in mappings
                        if item["key"] == key
                    ],
                    "reason_for_review": "The key appears in variant_key but not in a parsed registry entry.",
                }
            )

    adjacency: dict[str, list[str]] = {}
    possible_route_adjacency: dict[str, list[tuple[str, bool]]] = {}
    direct_graph_nodes_by_path: dict[str, set[str]] = {}
    direct_graph_edge_count_by_path: dict[str, int] = {}
    for edge in edges:
        is_dynamic_candidate = (
            edge.get("status", "").startswith("dynamic_python_callback_candidate")
        )
        possible_route_adjacency.setdefault(edge["from"], []).append(
            (edge["to"], is_dynamic_candidate)
        )
        if is_dynamic_candidate:
            continue
        adjacency.setdefault(edge["from"], []).append(edge["to"])
        edge_paths = set()
        for node_id in (edge["from"], edge["to"]):
            node = nodes.get(node_id)
            if node and node.get("path"):
                edge_paths.add(node["path"])
                direct_graph_nodes_by_path.setdefault(node["path"], set()).add(node_id)
        for path in edge_paths:
            direct_graph_edge_count_by_path[path] = (
                direct_graph_edge_count_by_path.get(path, 0) + 1
            )
    operation_nodes_by_path: dict[str, set[str]] = {}
    operation_ids_by_path: dict[str, set[str]] = {}
    possible_operation_nodes_by_path: dict[str, set[str]] = {}
    possible_operation_ids_by_path: dict[str, set[str]] = {}
    operation_nodes = {
        node_id: node["operation_id"]
        for node_id, node in nodes.items()
        if node.get("kind") == "manifest_public_operation"
    }
    for operation_node_id, operation_id in operation_nodes.items():
        reachable: set[str] = set()
        pending = [operation_node_id]
        while pending:
            current = pending.pop()
            if current in reachable:
                continue
            reachable.add(current)
            pending.extend(adjacency.get(current, []))

        # Preserve candidate callback routes separately from static routes. A
        # literal PyO3 method name narrows the possible Python override set, but
        # does not prove which runtime object receives the callback.
        route_states = {(operation_node_id, False)}
        pending_routes = [(operation_node_id, False)]
        while pending_routes:
            current, used_dynamic_candidate = pending_routes.pop()
            for target, is_dynamic_candidate in possible_route_adjacency.get(
                current, []
            ):
                route_state = (
                    target,
                    used_dynamic_candidate or is_dynamic_candidate,
                )
                if route_state in route_states:
                    continue
                route_states.add(route_state)
                pending_routes.append(route_state)

        for node_id, used_dynamic_candidate in route_states:
            if not used_dynamic_candidate:
                continue
            node = nodes.get(node_id)
            if node and node.get("path"):
                path = node["path"]
                possible_operation_nodes_by_path.setdefault(path, set()).add(node_id)
                possible_operation_ids_by_path.setdefault(path, set()).add(operation_id)

        for node_id in reachable:
            node = nodes.get(node_id)
            if node and node.get("path"):
                path = node["path"]
                operation_nodes_by_path.setdefault(path, set()).add(node_id)
                operation_ids_by_path.setdefault(path, set()).add(operation_id)
    for source_file in rust_modules["source_files"]:
        path = source_file["path"]
        node_ids = sorted(direct_graph_nodes_by_path.get(path, set()))
        route_node_ids = sorted(operation_nodes_by_path.get(path, set()))
        public_operation_ids = sorted(operation_ids_by_path.get(path, set()))
        possible_route_node_ids = sorted(
            possible_operation_nodes_by_path.get(path, set())
        )
        possible_public_operation_ids = sorted(
            possible_operation_ids_by_path.get(path, set())
        )
        if public_operation_ids:
            status = "reached_from_public_operation_by_static_edges"
        elif possible_public_operation_ids:
            status = "possible_route_via_dynamic_python_callback_candidate"
        elif node_ids:
            status = "static_graph_edge_without_public_operation_route"
        else:
            status = "no_direct_static_edge"
        source_file["operation_crosswalk"] = {
            "status": status,
            "node_ids": node_ids,
            "route_node_ids": route_node_ids,
            "public_operation_ids": public_operation_ids,
            "possible_route_node_ids": possible_route_node_ids,
            "possible_public_operation_ids": possible_public_operation_ids,
            "edge_count": direct_graph_edge_count_by_path.get(path, 0),
        }
        if not public_operation_ids:
            if possible_public_operation_ids:
                reason_for_review = (
                    "A candidate route from a manifest public operation reaches this file only "
                    "through a runtime Python callback whose target is not statically proven. "
                    "This is a review-only candidate, not proof of execution and not an unused-code finding."
                )
            else:
                reason_for_review = (
                    "No complete directed route from a manifest public operation reaches this file. "
                    "It may still have registry edges, shared support callers, conditional/test "
                    "reachability, or a dynamic/macro-generated route; this is not an unused-code finding."
                )
            source_file["operation_crosswalk"]["reason_for_review"] = reason_for_review
        source_file["family_id"] = rust_family_for_path(path)
        source_file["family_layout"] = structure_role(path)
        if source_file["family_layout"]["assessment"] in {"review_candidate", "unclassified"}:
            findings.append(
                {
                    "flag": "rust_source_family_layout_review",
                    "subject": path,
                    "evidence": [{"path": path}],
                    "reason_for_review": (
                        source_file["family_layout"]["reason"]
                        + " This is a path-role review only, not an unused-code finding."
                    ),
                }
            )
    rust_modules["summary"]["source_files_reached_from_public_operations"] = sum(
        item["operation_crosswalk"]["status"]
        == "reached_from_public_operation_by_static_edges"
        for item in rust_modules["source_files"]
    )
    rust_modules["summary"]["source_files_with_static_edges_without_public_operation_route"] = sum(
        item["operation_crosswalk"]["status"]
        in {
            "static_graph_edge_without_public_operation_route",
            "possible_route_via_dynamic_python_callback_candidate",
        }
        and bool(item["operation_crosswalk"]["node_ids"])
        for item in rust_modules["source_files"]
    )
    rust_modules["summary"]["source_files_without_static_operation_graph_edge"] = sum(
        item["operation_crosswalk"]["status"] == "no_direct_static_edge"
        for item in rust_modules["source_files"]
    )
    rust_modules["summary"]["source_files_with_family_layout_review"] = sum(
        item["family_layout"]["assessment"] in {"review_candidate", "unclassified"}
        for item in rust_modules["source_files"]
    )
    rust_modules["summary"]["source_files_with_possible_dynamic_callback_route"] = sum(
        item["operation_crosswalk"]["status"]
        == "possible_route_via_dynamic_python_callback_candidate"
        for item in rust_modules["source_files"]
    )

    operation_graph_review_queue = [
        {
            "path": source_file["path"],
            "static_role": source_file.get("static_role"),
            "family_id": source_file["family_id"],
            "family_layout": source_file["family_layout"],
            "operation_crosswalk": source_file["operation_crosswalk"],
            "review_only": True,
        }
        for source_file in rust_modules["source_files"]
        if source_file["operation_crosswalk"]["status"]
        != "reached_from_public_operation_by_static_edges"
    ]

    operation_paths = {
        node["operation_id"]: node["public_target_path"]
        for node in nodes.values()
        if node.get("kind") == "manifest_public_operation"
    }
    declared_evidence = build_declared_evidence_index(root, manifest, operation_paths)
    declared_evidence_by_operation = {
        item["operation_id"]: item for item in declared_evidence["operations"]
    }
    for operation_id, public_path in operation_paths.items():
        node = nodes[f"public-operation:{operation_id}"]
        evidence = declared_evidence_by_operation[operation_id]
        node["declared_evidence"] = {
            "input_only": True,
            "parity_case_count": evidence["parity_case_count"],
            "benchmark_workload_count": evidence["benchmark_workload_count"],
            "parity_target_profiles": sorted({
                profile
                for case in evidence["parity_cases"]
                for profile in case["target_profiles"]
            }),
            "benchmark_target_profiles": sorted({
                profile
                for workload in evidence["benchmark_workloads"]
                for profile in workload["target_profiles"]
            }),
        }

    edges.sort(key=lambda item: (item["from"], item["relation"], item["to"], item["site"]["line"]))
    findings.sort(key=lambda item: (item["flag"], item["subject"]))
    return {
        "schema": "pillow-rs/operation-source-crosswalk@10",
        "source": {
            **git_identity(root),
            **crosswalk_source_identity(
                root,
                (item["path"] for item in declared_evidence["input_files"]),
            ),
        },
        "inventory": {
            "manifest_path": MANIFEST_PATH.as_posix(),
            "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
            "manifest_operation_count": public_operation_count,
            "parity_input_file_count": declared_evidence["summary"]["parity_input_files"],
            "benchmark_input_file_count": declared_evidence["summary"]["benchmark_input_files"],
        },
        "scope": {
            "denominator": "migration-parity operations, declared parity/benchmark inputs, and PipelineOp variants",
            "host_architecture": platform.machine(),
            "requested_cargo_features": [],
            "runtime_execution_proven": False,
            "fixture_selection_execution_proven": False,
            "rust_module_reachability_proven": False,
            "core_function_call_graph_proven": False,
            "core_receiver_method_call_graph_complete": False,
            "core_free_function_call_graph_complete": False,
        },
        "summary": {
            "pipeline_variants": len(variants),
            "variant_key_edges": len(mappings),
            "registry_keys": len(registration_by_key),
            "simd_registrations": len(simd_entries),
            "public_operations": public_operation_count,
            "public_operations_with_python_bindings": public_operation_count - unmapped_public_operations,
            "unmapped_public_operation_bindings": unmapped_public_operations,
            "python_binding_native_calls": python_native_call_count,
            "python_binding_native_calls_mapped_to_registered_pyo3": mapped_python_native_calls,
            "python_binding_native_calls_unresolved": unresolved_python_native_calls,
            "python_binding_native_calls_with_registration_review": unregistered_python_native_calls,
            "nodes": len(nodes),
            "edges": len(edges),
            "findings": len(findings),
            "unmapped_pipeline_variants": sum(
                finding["flag"] == "unmapped_pipeline_variant" for finding in findings
            ),
            "rust_source_files": rust_modules["summary"]["rust_source_files"],
            "rust_crate_roots": rust_modules["summary"]["crate_roots"],
            "rust_module_edges": rust_modules["summary"]["resolved_module_edges"],
            "rust_module_edges_unresolved": rust_modules["summary"]["unresolved_module_edges"],
            "rust_source_unreached_candidates": rust_modules["summary"]["unreached_source_candidates"],
            "rust_source_files_reached_from_public_operations": rust_modules["summary"]["source_files_reached_from_public_operations"],
            "rust_source_files_with_static_edges_without_public_operation_route": rust_modules["summary"]["source_files_with_static_edges_without_public_operation_route"],
            "rust_source_files_with_possible_dynamic_callback_route": rust_modules["summary"]["source_files_with_possible_dynamic_callback_route"],
            "rust_source_files_without_static_operation_graph_edge": rust_modules["summary"]["source_files_without_static_operation_graph_edge"],
            "operation_graph_review_candidates": len(operation_graph_review_queue),
            "declared_parity_cases": declared_evidence["summary"]["parity_cases"],
            "operations_with_parity_cases": declared_evidence["summary"]["operations_with_parity_cases"],
            "operations_without_parity_cases": declared_evidence["summary"]["operations_without_parity_cases"],
            "declared_benchmark_workloads": declared_evidence["summary"]["benchmark_workloads"],
            "operations_with_benchmark_workloads": declared_evidence["summary"]["operations_with_benchmark_workloads"],
            "operations_without_benchmark_workloads": declared_evidence["summary"]["operations_without_benchmark_workloads"],
            "unmapped_fixture_operation_references": declared_evidence["summary"]["unmapped_fixture_operation_references"],
            "unresolved_benchmark_parity_case_references": declared_evidence["summary"]["unresolved_benchmark_parity_case_references"],
            "unmapped_requirement_references": declared_evidence["summary"]["unmapped_requirement_references"],
            "unsupported_benchmark_input_kinds": declared_evidence["summary"]["unsupported_benchmark_input_kinds"],
            "rust_source_files_with_family_layout_review": rust_modules["summary"]["source_files_with_family_layout_review"],
            "pyo3_core_public_api_calls": pyo3_core_public_call_count,
            "pyo3_core_public_api_calls_mapped_to_core_routes": mapped_pyo3_core_public_calls,
            "pyo3_core_public_api_calls_unresolved": unresolved_pyo3_core_public_calls,
            "core_public_api_pipeline_variant_routes": core_pipeline_variant_routes,
            "core_function_helpers_reached_from_public_operations": len(visited_core_function_nodes),
            "core_function_helper_routes": core_function_helper_routes,
            "core_function_helper_pipeline_variant_routes": core_function_helper_pipeline_routes,
            "pyo3_direct_core_receiver_method_calls": pyo3_receiver_method_call_count,
            "pyo3_direct_core_receiver_method_calls_mapped": mapped_pyo3_receiver_method_calls,
            "pyo3_direct_core_receiver_method_calls_unresolved": unresolved_pyo3_receiver_method_calls,
            "core_methods_reached_from_pyo3": len(visited_core_methods),
            "core_method_pipeline_variant_routes": core_method_pipeline_routes,
        },
        "declared_evidence": declared_evidence,
        "rust_module_inventory": rust_modules,
        "review_queues": {
            "operation_graph_source_without_public_route": operation_graph_review_queue,
            "operation_graph_possible_dynamic_callback_routes": [
                item for item in operation_graph_review_queue
                if item["operation_crosswalk"]["status"]
                == "possible_route_via_dynamic_python_callback_candidate"
            ],
            **declared_evidence["review_queues"],
        },
        "nodes": sorted(nodes.values(), key=lambda item: item["id"]),
        "edges": edges,
        "findings": findings,
        "limits": [
            "A registry edge proves source registration, not runtime admission or execution.",
            "GPU shader edges identify a conditional default shader; runtime shader selection may be contextual.",
            "Static findings are investigation candidates and are never deletion instructions.",
            "Manifest operations resolve to Python definitions by AST alias analysis. Explicit calls through _core and Rust Image handles map to PyO3 declarations; direct crate-root function calls map through literal Rust public re-exports or root function definitions to their core module. Direct PipelineOp references in those function bodies connect to the registered pipeline route. PyImage.inner and PyDraw.draw calls are linked only when their field types resolve to supported pillow_rs types and exactly one matching inherent method exists; clone-derived local aliases are followed only when their initializer directly clones that field. Direct self-method calls inside reached inherent methods are followed recursively where unique. Core public functions also follow unqualified same-module calls only when one function definition matches. These are partial static method and helper graphs, not complete call graphs; local name shadowing and cfg effects are not compiler-resolved. Runtime-dependent dispatch, PipelineOp admission, and backend execution remain unproven.",
            "The Rust module inventory records statically discovered Cargo roots, module declarations, cfg attributes, and literal includes. It does not evaluate cfg or prove runtime reachability; unresolved and unreached source is review-only.",
            *declared_evidence["limits"],
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="pillow-rs checkout")
    parser.add_argument("--output", type=Path, help="write JSON here; default is stdout")
    args = parser.parse_args(argv)
    report = build_crosswalk(args.root.resolve())
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
