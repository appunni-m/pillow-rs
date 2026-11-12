"""Map Python wrapper callsites to checked-in PyO3 declarations."""

from __future__ import annotations

import ast
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .python_bindings import Binding, ModuleRef, PythonBindingIndex, SymbolRef
from .rust_calls import (
    direct_public_api_calls,
    direct_receiver_aliases,
    direct_receiver_method_calls,
    direct_receiver_method_calls_in_body,
    direct_literal_python_method_callbacks,
    function_body_at,
)
from .rust_source import code_only, line_number, matching_delimiter


@dataclass(frozen=True)
class PyO3Declaration:
    kind: str
    owner: str | None
    symbol: str
    exposed_name: str
    path: str
    line: int
    registered: bool
    direct_core_calls: tuple[dict[str, Any], ...] = ()
    direct_receiver_calls: tuple[dict[str, Any], ...] = ()
    receiver_field_type: dict[str, Any] | None = None
    dynamic_python_callbacks: tuple[dict[str, Any], ...] = ()


@dataclass(frozen=True)
class PythonNativeCall:
    kind: str
    owner: str | None
    symbol: str
    path: str
    line: int
    expression: str
    dynamic_callback_candidate: bool = False


class PyO3Index:
    """Index PyO3 function registration and image methods without compiling."""

    def __init__(self, source_root: Path, repo_root: Path):
        self.source_root = source_root
        self.repo_root = repo_root
        self.declarations: dict[tuple[str, str | None, str], PyO3Declaration] = {}
        sources = [
            (path, path.read_text(encoding="utf-8"))
            for path in sorted(source_root.rglob("*.rs"))
        ]
        registered_classes: set[str] = set()
        receiver_fields: dict[tuple[str, str], list[dict[str, Any]]] = {}
        for _, source in sources:
            masked = code_only(source)
            registered_classes.update(
                re.findall(
                    r"\badd_class\s*::<\s*([A-Za-z_][A-Za-z0-9_]*)\s*>\s*\(",
                    masked,
                )
            )
            for owner, field in (("PyImage", "inner"), ("PyDraw", "draw")):
                if re.search(rf"\bstruct\s+{owner}\s*\{{", masked):
                    receiver_fields.setdefault((owner, field), []).append(
                        struct_field_type(source, owner, field)
                    )
        receiver_field_types = {
            key: values[0]
            if len(values) == 1
            else {"field": key[1], "resolution": "ambiguous_struct"}
            for key, values in receiver_fields.items()
        }
        for path, source in sources:
            self._index_source(
                path,
                source,
                registered_classes=registered_classes,
                receiver_field_types=receiver_field_types,
            )

    def _relative(self, path: Path) -> str:
        return path.relative_to(self.repo_root).as_posix()

    def _index_source(
        self,
        path: Path,
        source: str,
        *,
        registered_classes: set[str] | None = None,
        receiver_field_types: dict[tuple[str, str], dict[str, Any]] | None = None,
    ) -> None:
        masked = code_only(source)
        registered_functions = set(
            re.findall(r"\bwrap_pyfunction!\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*,", masked)
        )
        if registered_classes is None:
            registered_classes = set(
                re.findall(
                    r"\badd_class\s*::<\s*([A-Za-z_][A-Za-z0-9_]*)\s*>\s*\(",
                    masked,
                )
            )

        for attribute in re.finditer(r"#\s*\[\s*pyfunction(?:\s*\([^]]*\))?\s*\]", masked):
            function = re.search(
                r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\(",
                masked[attribute.end() :],
            )
            if function is None:
                continue
            symbol = function.group(1)
            start = attribute.start()
            exposed = self._renamed_exposure(
                source[attribute.start() : attribute.end()], symbol
            )
            declaration = PyO3Declaration(
                "pyfunction", None, symbol, exposed, self._relative(path),
                line_number(source, start), symbol in registered_functions,
                tuple(direct_public_api_calls(source, attribute.end() + function.start())),
            )
            key = (declaration.kind, declaration.owner, declaration.exposed_name)
            self.declarations[key] = declaration

        for attribute in re.finditer(r"#\s*\[\s*pymethods\s*\]", masked):
            impl = re.search(
                r"\bimpl\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", masked[attribute.end() :]
            )
            if impl is None:
                continue
            owner = impl.group(1)
            receiver_field = {"PyImage": "inner", "PyDraw": "draw"}.get(owner)
            receiver_field_type = None
            if receiver_field is not None:
                field_key = (owner, receiver_field)
                if receiver_field_types and field_key in receiver_field_types:
                    receiver_field_type = receiver_field_types[field_key]
                else:
                    receiver_field_type = struct_field_type(
                        source, owner, receiver_field
                    )
            opening = attribute.end() + impl.end() - 1
            closing = matching_delimiter(source, opening, "{", "}")
            body = source[opening + 1 : closing]
            body_masked = code_only(body)
            for method in re.finditer(
                r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^;{}]*>)?\s*\(",
                body_masked,
            ):
                symbol = method.group(1)
                prefix = _method_attributes(body, method.start())
                exposed = self._renamed_exposure(prefix, symbol)
                declaration = PyO3Declaration(
                    "pymethod", owner, symbol, exposed, self._relative(path),
                    line_number(source, opening + 1 + method.start()), owner in registered_classes,
                    tuple(direct_public_api_calls(
                        source, opening + 1 + method.start(), masked_source=masked
                    )),
                    tuple(
                        self._direct_receiver_calls(
                            source,
                            masked,
                            opening + 1 + method.start(),
                            f"self.{receiver_field}",
                            include_cloned_aliases=owner == "PyImage",
                            pyo3_owner=owner,
                            receiver_field=receiver_field,
                        )
                        if receiver_field is not None
                        else ()
                    ),
                    receiver_field_type,
                    tuple(
                        direct_literal_python_method_callbacks(
                            source,
                            opening + 1 + method.start(),
                            masked_source=masked,
                        )
                        if owner == "PyImage"
                        else ()
                    ),
                )
                self.declarations[(declaration.kind, owner, declaration.exposed_name)] = declaration

    @staticmethod
    def _direct_receiver_calls(
        source: str,
        masked: str,
        function_start: int,
        receiver: str,
        *,
        include_cloned_aliases: bool,
        pyo3_owner: str,
        receiver_field: str,
    ) -> list[dict[str, Any]]:
        calls = direct_receiver_method_calls(
            source, function_start, receiver, masked_source=masked
        )
        # Clone is a standard derived/implemented trait call. It creates the
        # value whose alias is followed below, but it is not an operation-family
        # method or backend route.
        calls = [call for call in calls if call["symbol"] != "clone"]
        if include_cloned_aliases:
            for alias in direct_receiver_aliases(
                source, function_start, receiver, masked_source=masked
            ):
                for call in direct_receiver_method_calls(
                    source, function_start, alias["name"], masked_source=masked
                ):
                    call["receiver_alias"] = alias
                    calls.append(call)
        calls.extend(
            _pyo3_receiver_field_calls(
                source,
                masked,
                function_start,
                pyo3_owner,
                receiver_field,
            )
        )
        deduplicated: dict[tuple[str, int, str], dict[str, Any]] = {}
        for call in calls:
            deduplicated[(call["symbol"], call["line"], call["expression"])] = call
        return list(deduplicated.values())

    @staticmethod
    def _renamed_exposure(attributes: str, fallback: str) -> str:
        rename = re.search(r"\bname\s*=\s*\"([^\"]+)\"", attributes)
        return rename.group(1) if rename else fallback

    def resolve(
        self, kind: str, symbol: str, owner: str | None = None
    ) -> PyO3Declaration | None:
        return self.declarations.get((kind, owner, symbol))


def _method_attributes(body: str, method_start: int) -> str:
    """Return the contiguous Rust attributes immediately before a method."""
    match = re.search(r"((?:\s*#\s*\[[^]]*\]\s*)*)$", body[:method_start])
    return match.group(1) if match else ""


def _pyo3_receiver_field_calls(
    source: str,
    masked_source: str,
    function_start: int,
    owner: str,
    field: str,
) -> list[dict[str, Any]]:
    """Trace direct core calls through a PyO3 method's own wrapper field.

    PyO3 methods commonly name their `Py<Self>` receiver `slf`, then access
    the wrapped core value as `slf.bind(py).borrow().inner`. A clone may be
    called later through a local alias, so record both forms as static source
    edges. This is caller evidence only; it does not prove that the method is
    reached at runtime.
    """
    signature = re.match(
        r"fn\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^;{}]*>)?\s*\(",
        masked_source[function_start:],
    )
    if signature is None:
        return []
    opening = function_start + signature.end() - 1
    try:
        closing = matching_delimiter(source, opening, "(", ")")
    except ValueError:
        return []

    # Pyo3's instance receiver is the first parameter. Stop at the first
    # top-level comma so commas inside PyRef/Bound types do not split it.
    parameters = masked_source[opening + 1 : closing]
    angle = paren = bracket = brace = 0
    first_end = len(parameters)
    for index, char in enumerate(parameters):
        if char == "," and not (angle or paren or bracket or brace):
            first_end = index
            break
        if char == "<":
            angle += 1
        elif char == ">" and angle:
            angle -= 1
        elif char == "(":
            paren += 1
        elif char == ")":
            paren = max(0, paren - 1)
        elif char == "[":
            bracket += 1
        elif char == "]":
            bracket = max(0, bracket - 1)
        elif char == "{":
            brace += 1
        elif char == "}":
            brace = max(0, brace - 1)
    first = parameters[:first_end].strip()
    receiver_match = re.match(
        r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*(?P<type>.+)", first
    )
    if receiver_match is None:
        return []
    receiver_name = receiver_match.group("name")
    receiver_type = receiver_match.group("type")
    if not re.search(rf"\b(?:Self|{re.escape(owner)})\b", receiver_type):
        return []

    body_result = function_body_at(source, function_start, masked_source)
    if body_result is None:
        return []
    body, body_offset = body_result
    masked_body = code_only(body)
    name = re.escape(receiver_name)
    member = re.escape(field)
    receiver_access = (
        rf"{name}\s*\.\s*bind\s*\(\s*[A-Za-z_][A-Za-z0-9_]*\s*\)"
        rf"\s*\.\s*borrow(?:_mut)?\s*\(\s*\)\s*\.\s*{member}"
        rf"|{name}\s*\.\s*borrow(?:_mut)?\s*\(\s*"
        rf"[A-Za-z_][A-Za-z0-9_]*\s*\)\s*\.\s*{member}"
        rf"|{name}\s*\.\s*{member}"
    )
    field_calls: list[dict[str, Any]] = []
    call_pattern = re.compile(
        rf"(?<![\w.])(?:{receiver_access})\s*\.\s*"
        rf"(?P<symbol>[A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    for match in call_pattern.finditer(masked_body):
        symbol = match.group("symbol")
        if symbol == "clone":
            continue
        field_calls.append(
            {
                "symbol": symbol,
                "receiver": f"{receiver_name}.{field}",
                "line": line_number(source, body_offset + match.start()),
                "expression": body[match.start() : match.end()].strip(),
                "evidence_kind": "static_pyo3_receiver_field_call",
            }
        )

    alias_pattern = re.compile(
        rf"\blet\s+(?:mut\s+)?(?P<alias>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*"
        rf"(?:{receiver_access})\s*\.\s*clone\s*\(\s*\)"
    )
    for alias in alias_pattern.finditer(masked_body):
        alias_metadata = {
            "name": alias.group("alias"),
            "line": line_number(source, body_offset + alias.start()),
            "expression": body[alias.start() : alias.end()].strip(),
        }
        for call in direct_receiver_method_calls_in_body(
            source, body, body_offset, alias.group("alias")
        ):
            if call["symbol"] == "clone":
                continue
            call["receiver_alias"] = alias_metadata
            call["evidence_kind"] = "static_pyo3_receiver_field_clone"
            field_calls.append(call)
    return field_calls


def struct_field_type(source: str, struct_name: str, field_name: str) -> dict[str, Any]:
    """Resolve a simple field type through an explicit `pillow_rs` type import."""
    masked = code_only(source)
    declaration = re.search(
        rf"\bstruct\s+{re.escape(struct_name)}\s*\{{", masked
    )
    if declaration is None:
        return {"field": field_name, "resolution": "missing_struct"}
    opening = masked.find("{", declaration.start(), declaration.end())
    try:
        closing = matching_delimiter(source, opening, "{", "}")
    except ValueError:
        return {"field": field_name, "resolution": "unclosed_struct"}
    body = source[opening + 1 : closing]
    field_types = re.findall(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?{re.escape(field_name)}\s*:\s*([^,\n]+)",
        code_only(body),
    )
    if len(field_types) != 1:
        return {
            "field": field_name,
            "resolution": "missing_field" if not field_types else "ambiguous_field",
        }
    field_type = field_types[0].strip()
    imports = {
        match.group(2) or match.group(1): f"pillow_rs::{match.group(1)}"
        for match in re.finditer(
            r"(?m)^\s*use\s+pillow_rs::([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*;",
            masked,
        )
    }
    resolved_type = field_type if field_type.startswith("pillow_rs::") else imports.get(field_type)
    return {
        "field": field_name,
        "field_type": field_type,
        "resolved_type": resolved_type,
        "resolution": "resolved" if resolved_type else "unresolved_type",
    }

def python_native_calls(
    binding: Binding, index: PythonBindingIndex
) -> list[PythonNativeCall]:
    """Return explicit calls through `_core`, native-class aliases, and Rust handles."""
    if not isinstance(binding.node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return []

    module_bindings = index.bindings.get(binding.module, {})
    core_names = {
        name
        for name, value in module_bindings.items()
        if isinstance(value, ModuleRef) and value.module.endswith("._core")
    }
    module_source = index.files.get(binding.module)
    if module_source is not None:
        module_tree = module_source[1]
        importer_names = {
            alias.asname or alias.name
            for node in module_tree.body
            if isinstance(node, ast.ImportFrom) and node.module == "importlib"
            for alias in node.names
            if alias.name == "import_module"
        }
        for node in ast.walk(module_tree):
            if isinstance(node, ast.Assign):
                assigned = {
                    name.id
                    for target in node.targets
                    for name in ast.walk(target)
                    if isinstance(name, ast.Name)
                }
                value = node.value
            elif isinstance(node, ast.AnnAssign):
                assigned = {
                    name.id
                    for name in ast.walk(node.target)
                    if isinstance(name, ast.Name)
                }
                value = node.value
            else:
                continue
            if (
                "_core" in assigned
                and isinstance(value, ast.Call)
                and isinstance(value.func, ast.Name)
                and value.func.id in importer_names
                and value.args
                and isinstance(value.args[0], ast.Constant)
                and isinstance(value.args[0].value, str)
                and value.args[0].value in {"._core", "._core_parallel"}
            ):
                core_names.add("_core")
    for node in ast.walk(binding.node):
        if isinstance(node, ast.ImportFrom):
            core_import = any(item.name == "_core" for item in node.names)
            if core_import:
                core_names.update(
                    (item.asname or item.name)
                    for item in node.names
                    if item.name == "_core"
                )

    calls: dict[tuple[str, str, int], PythonNativeCall] = {}
    handle_owners = {
        "_rust_image": "PyImage",
        "_draw": "PyDraw",
        "_rust_font": "PyFont",
        "_rust_pilfont": "PyPilFont",
        "_rust_outline": "PyOutline",
    }
    native_class_owners = {
        "Image": "PyImage",
        "ImageDraw": "PyDraw",
        "Outline": "PyOutline",
    }
    # Bindings commonly import `_core.Image` under an internal alias such as
    # `RustImage`; calls on that alias are still direct PyO3 receiver calls.
    for name, value in module_bindings.items():
        if isinstance(value, SymbolRef) and value.module.endswith("._core"):
            owner = native_class_owners.get(value.name)
            if owner is not None:
                handle_owners[name] = owner
    callback_image_parameter = None
    if (
        binding.kind == "python_method"
        and binding.name == "_apply"
        and isinstance(binding.node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and len(binding.node.args.args) > 1
        and binding.node.args.args[1].arg == "rust_image"
    ):
        # PyImage.filter passes its wrapped image as the first callback
        # argument. This identifies a possible callback-body call route, not
        # proof that a particular runtime filter object receives the callback.
        callback_image_parameter = "rust_image"
        handle_owners[callback_image_parameter] = "PyImage"
    for node in ast.walk(binding.node):
        if not isinstance(node, ast.Call):
            continue
        path = attribute_path(node.func)
        if not path:
            continue
        kind = ""
        owner = None
        symbol = ""
        if len(path) == 2 and path[0] in core_names:
            kind, symbol = "pyfunction", path[-1]
        elif len(path) >= 2 and path[-2] in handle_owners:
            kind, owner, symbol = "pymethod", handle_owners[path[-2]], path[-1]
        if not kind:
            continue
        call = PythonNativeCall(
            kind, owner, symbol, binding.path, getattr(node, "lineno", binding.line),
            ".".join(path), path[0] == callback_image_parameter,
        )
        calls[(kind, owner or "", symbol, call.line)] = call
    return sorted(
        calls.values(), key=lambda item: (item.line, item.kind, item.owner or "", item.symbol)
    )


def attribute_path(node: ast.expr) -> list[str]:
    parts: list[str] = []
    current: ast.expr = node
    while isinstance(current, ast.Attribute):
        parts.append(current.attr)
        current = current.value
    if isinstance(current, ast.Name):
        parts.append(current.id)
    else:
        return []
    return list(reversed(parts))


def declaration_node(declaration: PyO3Declaration) -> dict[str, Any]:
    return {
        "kind": declaration.kind,
        "owner": declaration.owner,
        "symbol": declaration.symbol,
        "exposed_name": declaration.exposed_name,
        "path": declaration.path,
        "line": declaration.line,
        "registered": declaration.registered,
        "direct_core_calls": list(declaration.direct_core_calls),
        "direct_receiver_calls": list(declaration.direct_receiver_calls),
        "receiver_field_type": declaration.receiver_field_type,
        "dynamic_python_callbacks": list(declaration.dynamic_python_callbacks),
    }


def py_source_files(repo_root: Path) -> Path:
    return repo_root / "pillow-rs-py/src"
