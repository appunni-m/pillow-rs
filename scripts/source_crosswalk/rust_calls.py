"""Conservative direct-call and public-reexport links into the Rust core."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from .rust_source import code_only, line_number, matching_delimiter

PYLLOW_RS_CALL = re.compile(r"\bpillow_rs::([A-Za-z_][A-Za-z0-9_]*)\s*\(")
PUBLIC_REEXPORT = re.compile(
    r"(?m)^\s*pub\s+use\s+crate::((?:[A-Za-z_][A-Za-z0-9_]*::)*)"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*;"
)
PUBLIC_FUNCTION = re.compile(
    r"(?m)^\s*pub\s+(?:(?:async|unsafe)\s+)*fn\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\("
)
PIPELINE_VARIANT_REFERENCE = re.compile(
    r"\bPipelineOp::([A-Z][A-Za-z0-9_]*)\b"
)
TOP_LEVEL_FUNCTION = re.compile(
    r"(?m)^\s*(?:(?:pub)(?:\([^)]*\))?\s+)?"
    r"(?:(?:async|unsafe|const)\s+)*fn\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\("
)
DIRECT_FUNCTION_CALL = re.compile(r"(?<![\w.:])([A-Za-z_][A-Za-z0-9_]*)\s*\(")
IMPL_METHOD = re.compile(
    r"(?m)^\s*(?:(?:pub(?:\([^)]*\))?|(?:async|unsafe|const))\s+)*fn\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\("
)
SIMD_ADAPTER_ROOT = "pillow-rs/src/compute/pool_simd/ops/adapters/"
SIMD_LEGACY_ROOT_FAMILIES = {
    "box_blur_luma.rs": "box_blur",
    "box_blur_rgba_avx2.rs": "box_blur",
    "box_blur_rgba_neon.rs": "box_blur",
    "gaussian_rgba_uniform.rs": "gaussian_blur",
    "native_put_pixel.rs": "put_pixel",
    "native_thumbnail.rs": "thumbnail",
    "native_transpose_avx2.rs": "transpose",
    "resize_i32.rs": "resize",
}
SIMD_LEGACY_FAMILY_DIRECTORIES = {
    "native_thumbnail": "thumbnail",
    "resize_i32": "resize",
}


def _top_level_matches(source: str, pattern: re.Pattern[str]):
    """Yield matches declared at crate scope, excluding inline modules and blocks."""
    masked = code_only(source)
    matches = iter(pattern.finditer(masked))
    pending = next(matches, None)
    depth = 0
    for offset, char in enumerate(masked):
        while pending is not None and pending.start() == offset:
            if depth == 0:
                yield pending
            pending = next(matches, None)
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1


def function_body_at(
    source: str, function_start: int, masked: str | None = None
) -> tuple[str, int] | None:
    """Return one function body and its source offset, if the item has a body."""
    masked = masked if masked is not None else code_only(source)
    signature = re.match(
        r"fn\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^;{}]*>)?\s*\(",
        masked[function_start:],
    )
    if signature is None:
        return None
    opening_paren = function_start + signature.end() - 1
    try:
        closing_paren = matching_delimiter(source, opening_paren, "(", ")")
    except ValueError:
        return None
    body_open = None
    paren_depth = bracket_depth = brace_depth = angle_depth = 0
    index = closing_paren + 1
    while index < len(masked):
        char = masked[index]
        if char == "{" and not (paren_depth or bracket_depth or brace_depth or angle_depth):
            body_open = index
            break
        if char == ";" and not (paren_depth or bracket_depth or brace_depth or angle_depth):
            return None
        if char == "(":
            paren_depth += 1
        elif char == ")":
            paren_depth = max(0, paren_depth - 1)
        elif char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth = max(0, bracket_depth - 1)
        elif char == "{" and (paren_depth or bracket_depth or angle_depth):
            brace_depth += 1
        elif char == "}" and brace_depth:
            brace_depth -= 1
        elif char == "<" and not (brace_depth or bracket_depth) and masked[index + 1 : index + 2] != "=":
            angle_depth += 1
        elif (
            char == ">"
            and angle_depth
            and masked[index - 1 : index] != "-"
            and masked[index + 1 : index + 2] != "="
        ):
            angle_depth -= 1
        index += 1
    if body_open is None:
        return None
    try:
        body_close = matching_delimiter(source, body_open, "{", "}")
    except ValueError:
        return None
    return source[body_open + 1 : body_close], body_open + 1


def direct_public_api_calls(
    source: str, function_start: int, *, masked_source: str | None = None
) -> list[dict[str, Any]]:
    """Find calls spelled directly through the public `pillow_rs::name` path."""
    function = function_body_at(source, function_start, masked_source)
    if function is None:
        return []
    body, body_offset = function
    masked = code_only(body)
    calls = []
    for match in PYLLOW_RS_CALL.finditer(masked):
        calls.append(
            {
                "symbol": match.group(1),
                "line": line_number(source, body_offset + match.start()),
                "expression": body[match.start() : match.end()].strip(),
            }
        )
    return calls


def direct_receiver_method_calls(
    source: str,
    function_start: int,
    receiver: str,
    *,
    masked_source: str | None = None,
) -> list[dict[str, Any]]:
    """Find direct calls on a spelled receiver such as `self.inner.method(...)`."""
    function = function_body_at(source, function_start, masked_source)
    if function is None:
        return []
    body, body_offset = function
    return direct_receiver_method_calls_in_body(
        source, body, body_offset, receiver
    )


def direct_receiver_aliases(
    source: str,
    function_start: int,
    receiver: str,
    *,
    masked_source: str | None = None,
) -> list[dict[str, Any]]:
    """Find local aliases initialized directly from `receiver.clone()`."""
    function = function_body_at(source, function_start, masked_source)
    if function is None:
        return []
    body, body_offset = function
    receiver_parts = receiver.split(".")
    receiver_pattern = r"\s*\.\s*".join(re.escape(part) for part in receiver_parts)
    alias_pattern = re.compile(
        rf"\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"
        rf"{receiver_pattern}\s*\.\s*clone\s*\(\s*\)"
    )
    masked_body = code_only(body)
    return [
        {
            "name": match.group(1),
            "line": line_number(source, body_offset + match.start()),
            "expression": body[match.start() : match.end()].strip(),
        }
        for match in alias_pattern.finditer(masked_body)
    ]


def direct_literal_python_method_callbacks(
    source: str,
    function_start: int,
    *,
    masked_source: str | None = None,
) -> list[dict[str, Any]]:
    """Find PyO3 `call_methodN("name", ...)` dynamic callback sites."""
    function = function_body_at(source, function_start, masked_source)
    if function is None:
        return []
    body, body_offset = function
    masked_body = code_only(body)
    call_pattern = re.compile(
        r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*"
        r"(call_method[0-9]*)\s*\("
    )
    callbacks = []
    for match in call_pattern.finditer(masked_body):
        opening = masked_body.find("(", match.start(), match.end())
        try:
            closing = matching_delimiter(body, opening, "(", ")")
        except ValueError:
            continue
        arguments = body[opening + 1 : closing]
        literal = re.match(r'\s*"([A-Za-z_][A-Za-z0-9_]*)"\s*,', arguments)
        if literal is None:
            continue
        callbacks.append(
            {
                "receiver": match.group(1),
                "call_method": match.group(2),
                "method_name": literal.group(1),
                "line": line_number(source, body_offset + match.start()),
                "expression": body[match.start() : closing + 1].strip(),
            }
        )
    return callbacks


def direct_receiver_method_calls_in_body(
    source: str, body: str, body_offset: int, receiver: str
) -> list[dict[str, Any]]:
    """Find direct receiver calls inside a previously parsed function body."""
    receiver_parts = receiver.split(".")
    receiver_pattern = r"\s*\.\s*".join(re.escape(part) for part in receiver_parts)
    call_pattern = re.compile(
        rf"(?<![\w.]){receiver_pattern}\s*\.\s*"
        rf"([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    masked = code_only(body)
    return [
        {
            "symbol": match.group(1),
            "receiver": receiver,
            "line": line_number(source, body_offset + match.start()),
            "expression": body[match.start() : match.end()].strip(),
        }
        for match in call_pattern.finditer(masked)
    ]


def inherent_method_definitions(
    source: str, source_path: str, owner: str
) -> list[dict[str, Any]]:
    """Index direct methods in top-level inherent impl blocks for one Rust type."""
    masked = code_only(source)
    impl_pattern = re.compile(rf"\bimpl\s+{re.escape(owner)}\s*\{{")
    definitions: list[dict[str, Any]] = []
    for impl_match in _top_level_matches(source, impl_pattern):
        opening = masked.find("{", impl_match.start(), impl_match.end())
        try:
            closing = matching_delimiter(source, opening, "{", "}")
        except ValueError:
            continue
        body_start = opening + 1
        body = source[body_start:closing]
        body_masked = masked[body_start:closing]
        for method_match in _top_level_matches(body, IMPL_METHOD):
            method_fn = body_masked.find("fn", method_match.start(), method_match.end())
            if method_fn < 0:
                continue
            function_start = body_start + method_fn
            signature = re.match(
                r"fn\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\(",
                masked[function_start:],
            )
            if signature is None:
                continue
            visibility_prefix = body_masked[method_match.start():method_fn]
            function_body = function_body_at(source, function_start, masked)
            if function_body is None:
                continue
            body_source, body_offset = function_body
            definitions.append(
                {
                    "owner": owner,
                    "symbol": signature.group(1),
                    "path": source_path,
                    "line": line_number(source, function_start),
                    "function_start": function_start,
                    "public": bool(re.search(r"\bpub(?:\([^)]*\))?\b", visibility_prefix)),
                    "body": body_source,
                    "body_offset": body_offset,
                    "direct_self_calls": direct_receiver_method_calls_in_body(
                        source, body_source, body_offset, "self"
                    ),
                    "pipeline_references": direct_pipeline_variant_references_in_body(
                        body_source, body_offset, source, source_path, signature.group(1)
                    ),
                }
            )
    return definitions


def core_image_method_index(root: Path) -> dict[str, list[dict[str, Any]]]:
    """Index inherent `Image` methods in the core crate, including private helpers."""
    source_root = root / "pillow-rs/src"
    paths = [source_root / "image.rs", *sorted((source_root / "ops").rglob("*.rs"))]
    methods: dict[str, list[dict[str, Any]]] = {}
    for path in paths:
        if not path.is_file():
            continue
        source = path.read_text(encoding="utf-8")
        relative = path.relative_to(root).as_posix()
        for method in inherent_method_definitions(source, relative, "Image"):
            methods.setdefault(method["symbol"], []).append(method)
    return methods


def core_draw_method_index(root: Path) -> dict[str, list[dict[str, Any]]]:
    """Index inherent `Draw` methods in the core crate."""
    path = root / "pillow-rs/src/draw/mod.rs"
    methods: dict[str, list[dict[str, Any]]] = {}
    if not path.is_file():
        return methods
    source = path.read_text(encoding="utf-8")
    relative = path.relative_to(root).as_posix()
    for method in inherent_method_definitions(source, relative, "Draw"):
        methods.setdefault(method["symbol"], []).append(method)
    return methods


def crate_root_api_routes(
    source: str,
    module_edges: list[dict[str, Any]],
    *,
    crate_root: str = "pillow-rs/src/lib.rs",
    export_path: str = "pillow-rs/src/lib.rs",
) -> dict[str, list[dict[str, Any]]]:
    """Resolve direct `pub use crate::module::item [as public_name]` exports."""
    edges_by_parent: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for edge in module_edges:
        if edge.get("resolution") == "resolved" and edge.get("to"):
            edges_by_parent.setdefault((edge["from"], edge["module"]), []).append(edge)

    resolved: dict[str, list[dict[str, Any]]] = {}
    for match in _top_level_matches(source, PUBLIC_REEXPORT):
        modules = [item for item in match.group(1).split("::") if item]
        item = match.group(2)
        public_name = match.group(3) or item
        current = crate_root
        status = "resolved"
        for module in modules:
            candidates = edges_by_parent.get((current, module), [])
            if len(candidates) != 1:
                status = "missing_module_edge" if not candidates else "ambiguous_module_edge"
                break
            current = candidates[0]["to"]
        row = {
            "kind": "public_reexport",
            "public_name": public_name,
            "item": item,
            "module_path": "crate::" + "::".join(modules) if modules else "crate",
            "export_path": export_path,
            "export_line": line_number(source, match.start()),
            "implementation_path": current if status == "resolved" else None,
            "resolution": status,
        }
        resolved.setdefault(public_name, []).append(row)
    for match in _top_level_matches(source, PUBLIC_FUNCTION):
        public_name = match.group(1)
        resolved.setdefault(public_name, []).append(
            {
                "kind": "public_function_definition",
                "public_name": public_name,
                "item": public_name,
                "module_path": "crate",
                "export_path": export_path,
                "export_line": line_number(source, match.start()),
                "implementation_path": crate_root,
                "resolution": "resolved",
            }
        )
    return resolved


def resolve_public_api_call(
    call: dict[str, Any], reexports: dict[str, list[dict[str, Any]]]
) -> tuple[dict[str, Any] | None, str]:
    """Resolve a direct PyO3 crate-root call to one static `pub use` route."""
    candidates = reexports.get(call["symbol"], [])
    if len(candidates) != 1:
        return None, "missing_reexport" if not candidates else "ambiguous_reexport"
    candidate = candidates[0]
    if candidate["resolution"] != "resolved":
        return None, candidate["resolution"]
    return candidate, "resolved"


def direct_pipeline_variant_references(
    root: Path, source_path: str, function_symbol: str
) -> list[dict[str, Any]]:
    """Find variants constructed directly by a public core function body."""
    path = root / source_path
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    masked = code_only(source)
    function_pattern = re.compile(
        rf"(?m)^\s*(?:(?:pub)(?:\([^)]*\))?\s+)?"
        rf"(?:(?:async|unsafe|const)\s+)*fn\s+{re.escape(function_symbol)}"
        r"(?:\s*<[^;{}]*>)?\s*\("
    )
    references = []
    for function_match in _top_level_matches(source, function_pattern):
        function_start = masked.find("fn", function_match.start(), function_match.end())
        references.extend(
            direct_pipeline_variant_references_at(
                source, source_path, function_start, function_symbol
            )
        )
    return references


def direct_pipeline_variant_references_at(
    source: str,
    source_path: str,
    function_start: int,
    function_symbol: str,
) -> list[dict[str, Any]]:
    """Find literal pipeline-variant references in one known function or method."""
    function = function_body_at(source, function_start)
    if function is None:
        return []
    body, body_offset = function
    return [
        {
            "variant": match.group(1),
            "function_symbol": function_symbol,
            "path": source_path,
            "line": line_number(source, body_offset + match.start()),
        }
        for match in PIPELINE_VARIANT_REFERENCE.finditer(code_only(body))
    ]


def direct_pipeline_variant_references_in_body(
    body: str, body_offset: int, source: str, source_path: str, function_symbol: str
) -> list[dict[str, Any]]:
    """Find literal pipeline-variant references in an already parsed body."""
    return [
        {
            "variant": match.group(1),
            "function_symbol": function_symbol,
            "path": source_path,
            "line": line_number(source, body_offset + match.start()),
        }
        for match in PIPELINE_VARIANT_REFERENCE.finditer(code_only(body))
    ]


def top_level_function_definitions(
    source: str, source_path: str
) -> dict[str, list[dict[str, Any]]]:
    """Index top-level functions and their direct same-module function calls."""
    masked = code_only(source)
    definitions: dict[str, list[dict[str, Any]]] = {}
    for match in _top_level_matches(source, TOP_LEVEL_FUNCTION):
        function_start = masked.find("fn", match.start(), match.end())
        if function_start < 0:
            continue
        symbol = match.group(1)
        function = function_body_at(source, function_start, masked)
        if function is None:
            continue
        body, body_offset = function
        calls = [
            {
                "symbol": call.group(1),
                "line": line_number(source, body_offset + call.start()),
                "expression": body[call.start() : call.end()].strip(),
            }
            for call in DIRECT_FUNCTION_CALL.finditer(code_only(body))
        ]
        definitions.setdefault(symbol, []).append(
            {
                "owner": None,
                "symbol": symbol,
                "path": source_path,
                "line": line_number(source, function_start),
                "function_start": function_start,
                "body": body,
                "body_offset": body_offset,
                "direct_function_calls": calls,
                "pipeline_references": direct_pipeline_variant_references_in_body(
                    body, body_offset, source, source_path, symbol
                ),
            }
        )
    return definitions


def core_public_api_node(
    reexport: dict[str, Any], source_identity: str
) -> dict[str, Any]:
    """Build a node for a public crate API call and its owning semantic module."""
    node_id = (
        f"core-public-api:{reexport['public_name']}:{reexport['implementation_path']}"
        f":{reexport['item']}"
    )
    return {
        "id": node_id,
        "kind": "core_public_api_route",
        "route_kind": reexport["kind"],
        "symbol": reexport["item"],
        "public_name": reexport["public_name"],
        "module_path": reexport["module_path"],
        "path": reexport["implementation_path"],
        "export_path": reexport["export_path"],
        "export_line": reexport["export_line"],
        "evidence_kind": source_identity,
        "runtime_execution_proven": False,
    }


def structure_role(path: str, content: str = "") -> dict[str, Any]:
    """Reuse the repository's single path authority for each Rust source file."""
    try:
        from implementation_structure.audit import classify_path
    except ModuleNotFoundError:
        from scripts.implementation_structure.audit import classify_path

    assessment = classify_path(path, content, exhaustive=True)
    return {
        "assessment": assessment["assessment"],
        "rule": assessment["rule"],
        "expected_shape": assessment.get("expected_shape"),
        "reason": assessment["reason"],
    }


def rust_family_for_path(path: str) -> str | None:
    """Return a family only when the path classifier proves its family role."""
    path = Path(path).as_posix()
    parts = path.split("/")
    role = structure_role(path)
    rule = role["rule"]

    if path.startswith("pillow-rs/src/ops/"):
        relative = parts[3:]
        if rule in {"core_operation_family", "core_operation_child"} and relative:
            family = relative[0]
            if family == "imageops" and len(relative) > 1:
                family = Path(relative[1]).stem
            return f"core:{Path(family).stem}"

    cpu_ops_prefix = "pillow-rs/src/compute/pool_cpu/ops/"
    if path.startswith(cpu_ops_prefix):
        relative = parts[len(cpu_ops_prefix.rstrip("/").split("/")):]
        if rule in {
            "cpu_family_module",
            "cpu_family_kernel",
            "cpu_filter_helper_outside_kernel_layer",
        } and relative:
            return f"cpu:{Path(relative[0]).stem}"
        # This pre-existing path encodes the operation below a multi-family
        # geometry aggregator; do not mislabel it as a geometry family.
        if path.startswith(
            "pillow-rs/src/compute/pool_cpu/ops/geometry/kernels/transpose_rgba/"
        ):
            return "cpu:transpose"

    if path.startswith(SIMD_ADAPTER_ROOT):
        relative = path[len(SIMD_ADAPTER_ROOT):].split("/")
        if len(relative) == 1:
            legacy_family = SIMD_LEGACY_ROOT_FAMILIES.get(relative[0])
            if legacy_family is not None:
                return f"simd:{legacy_family}"
        if relative and relative[0] in SIMD_LEGACY_FAMILY_DIRECTORIES:
            return f"simd:{SIMD_LEGACY_FAMILY_DIRECTORIES[relative[0]]}"
        if rule in {
            "simd_family_orchestrator",
            "simd_kernel_dispatch",
            "simd_family_kernel",
        } and relative:
            return f"simd:{Path(relative[0]).stem}"

    if path.startswith("pillow-rs/src/compute/pool_gpu/shaders/") and rule == "gpu_family_shader":
        relative = parts[5:]
        if relative:
            return f"gpu:{Path(relative[0]).stem}"
    return None
