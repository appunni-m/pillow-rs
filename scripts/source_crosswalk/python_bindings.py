"""Resolve public PIL paths through Python source aliases without importing them."""

from __future__ import annotations

import ast
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class Binding:
    kind: str
    module: str
    name: str
    path: str
    line: int
    expression: ast.expr | None = None
    members: dict[str, Binding] = field(default_factory=dict)
    node: ast.AST | None = None


@dataclass(frozen=True)
class ModuleRef:
    module: str
    path: str = ""
    line: int = 0


@dataclass(frozen=True)
class SymbolRef:
    module: str
    name: str
    path: str
    line: int


class PythonBindingIndex:
    """A small AST index for the checked-in PIL facade and pillow_rs modules."""

    def __init__(self, python_root: Path, repo_root: Path):
        self.python_root = python_root
        self.repo_root = repo_root
        self.files: dict[str, tuple[Path, ast.Module]] = {}
        self.bindings: dict[str, dict[str, Binding | ModuleRef | SymbolRef]] = {}
        for package in ("PIL", "pillow_rs"):
            for path in sorted((python_root / package).rglob("*.py")):
                relative = path.relative_to(python_root).with_suffix("")
                parts = list(relative.parts)
                if parts[-1] == "__init__":
                    parts.pop()
                module = ".".join(parts)
                tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
                self.files[module] = (path, tree)
        for module, (path, tree) in self.files.items():
            self.bindings[module] = self._index_module(module, path, tree)

    def _relative(self, path: Path) -> str:
        return path.relative_to(self.repo_root).as_posix()

    def _module_path(self, module: str) -> str:
        path = self.files.get(module)
        return self._relative(path[0]) if path else ""

    def _import_base(self, module: str, path: Path, node: ast.ImportFrom) -> str:
        if node.level == 0:
            return node.module or ""
        if path.name == "__init__.py":
            package = module
        else:
            package = module.rpartition(".")[0]
        parts = package.split(".") if package else []
        keep = max(0, len(parts) - node.level + 1)
        prefix = parts[:keep]
        if node.module:
            prefix.extend(node.module.split("."))
        return ".".join(prefix)

    def _index_class(self, module: str, path: Path, node: ast.ClassDef) -> Binding:
        binding = Binding(
            "python_class", module, node.name, self._relative(path), node.lineno, node=node
        )
        for child in node.body:
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                kind = "python_class" if isinstance(child, ast.ClassDef) else "python_method"
                nested = Binding(
                    kind, module, child.name, self._relative(path), child.lineno, node=child
                )
                if isinstance(child, ast.ClassDef):
                    nested.members = self._index_class_members(module, path, child)
                binding.members[child.name] = nested
            elif isinstance(child, (ast.Assign, ast.AnnAssign)):
                value = child.value
                if value is None:
                    continue
                line = child.lineno
                for name in assigned_names(child):
                    binding.members[name] = Binding(
                        "python_class_assignment", module, name, self._relative(path), line,
                        expression=value, node=child,
                    )
        init = next(
            (
                child
                for child in node.body
                if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef))
                and child.name == "__init__"
            ),
            None,
        )
        if init is not None:
            for item in ast.walk(init):
                if isinstance(item, ast.Assign):
                    targets = item.targets
                elif isinstance(item, ast.AnnAssign):
                    targets = [item.target]
                else:
                    continue
                for target in targets:
                    if isinstance(target, ast.Attribute) and isinstance(target.value, ast.Name) and target.value.id == "self":
                        binding.members.setdefault(
                            target.attr,
                            Binding(
                                "python_instance_field", module, target.attr,
                                self._relative(path), item.lineno, node=item,
                            ),
                        )
        return binding

    def _index_class_members(self, module: str, path: Path, node: ast.ClassDef) -> dict[str, Binding]:
        return self._index_class(module, path, node).members

    def _index_module(
        self, module: str, path: Path, tree: ast.Module
    ) -> dict[str, Binding | ModuleRef | SymbolRef]:
        bindings: dict[str, Binding | ModuleRef | SymbolRef] = {}
        for node in tree.body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                bindings[node.name] = Binding(
                    "python_function", module, node.name, self._relative(path), node.lineno,
                    node=node,
                )
            elif isinstance(node, ast.ClassDef):
                bindings[node.name] = self._index_class(module, path, node)
            elif isinstance(node, ast.ImportFrom):
                base = self._import_base(module, path, node)
                for imported in node.names:
                    name = imported.asname or imported.name
                    module_name = ".".join(part for part in (base, imported.name) if part)
                    if module_name in self.files or imported.name == "_core":
                        bindings[name] = ModuleRef(module_name, self._relative(path), node.lineno)
                    else:
                        bindings[name] = SymbolRef(base, imported.name, self._relative(path), node.lineno)
            elif isinstance(node, ast.Import):
                for imported in node.names:
                    name = imported.asname or imported.name.split(".")[0]
                    module_name = imported.name if imported.asname else imported.name.split(".")[0]
                    bindings[name] = ModuleRef(module_name)
            elif isinstance(node, (ast.Assign, ast.AnnAssign)):
                value = node.value
                if value is None:
                    continue
                for name in assigned_names(node):
                    bindings[name] = Binding(
                        "python_assignment", module, name, self._relative(path), node.lineno,
                        expression=value, node=node,
                    )
        return bindings

    def _resolve_symbol(
        self,
        reference: SymbolRef,
        trail: list[dict[str, Any]],
        seen: set[tuple[str, str]],
    ) -> Binding | ModuleRef | SymbolRef | None:
        key = (reference.module, reference.name)
        if key in seen:
            return None
        seen.add(key)
        trail.append(
            {
                "module": reference.module,
                "name": reference.name,
                "path": reference.path,
                "line": reference.line,
                "via": "python_import",
            }
        )
        module_name = reference.module
        joined = ".".join(part for part in (module_name, reference.name) if part)
        if joined in self.files:
            return ModuleRef(joined)
        module_bindings = self.bindings.get(module_name)
        if module_bindings is None:
            return Binding(
                "native_extension_symbol", module_name, reference.name,
                reference.path, reference.line,
            )
        target = module_bindings.get(reference.name)
        if target is None:
            return Binding(
                "external_python_symbol", module_name, reference.name,
                reference.path, reference.line,
            )
        return self._resolve(target, trail, seen)

    def _resolve(
        self,
        value: Binding | ModuleRef | SymbolRef,
        trail: list[dict[str, Any]],
        seen: set[tuple[str, str]],
    ) -> Binding | ModuleRef | SymbolRef | None:
        if isinstance(value, (ModuleRef, Binding)) and isinstance(value, ModuleRef):
            return value
        if isinstance(value, SymbolRef):
            return self._resolve_symbol(value, trail, seen)
        if value.kind not in {"python_assignment", "python_class_assignment"}:
            return value
        trail.append(
            {
                "module": value.module,
                "name": value.name,
                "path": value.path,
                "line": value.line,
                "via": "python_assignment",
            }
        )
        resolved = self._resolve_expression(value.module, value.expression, trail, seen)
        return resolved if resolved is not None else value

    def _resolve_expression(
        self,
        module: str,
        expression: ast.expr | None,
        trail: list[dict[str, Any]],
        seen: set[tuple[str, str]],
    ) -> Binding | ModuleRef | SymbolRef | None:
        if isinstance(expression, ast.Name):
            binding = self.bindings.get(module, {}).get(expression.id)
            if binding is None:
                return None
            return self._resolve(binding, trail, seen)
        if isinstance(expression, ast.Attribute):
            base = self._resolve_expression(module, expression.value, trail, seen)
            return self._get_attribute(base, expression.attr, trail, seen)
        return None

    def _get_attribute(
        self,
        base: Binding | ModuleRef | SymbolRef | None,
        name: str,
        trail: list[dict[str, Any]],
        seen: set[tuple[str, str]],
    ) -> Binding | ModuleRef | SymbolRef | None:
        if isinstance(base, SymbolRef):
            base = self._resolve_symbol(base, trail, seen)
        if isinstance(base, ModuleRef):
            child_module = ".".join(part for part in (base.module, name) if part)
            if child_module in self.files:
                child_path = self._module_path(child_module)
                trail.append(
                    {
                        "module": base.module,
                        "name": name,
                        "path": child_path,
                        "line": 1,
                        "via": "python_submodule",
                    }
                )
                return ModuleRef(child_module, child_path, 1)
            target = self.bindings.get(base.module, {}).get(name)
            if target is not None:
                if isinstance(target, Binding):
                    target_path, target_line = target.path, target.line
                elif isinstance(target, (ModuleRef, SymbolRef)):
                    target_path, target_line = target.path, target.line
                else:
                    target_path, target_line = "", 0
                trail.append(
                    {
                        "module": base.module,
                        "name": name,
                        "path": target_path,
                        "line": target_line,
                        "via": "python_module_binding",
                    }
                )
                return self._resolve(target, trail, seen)
            if base.module.endswith("._core"):
                return Binding("native_extension_symbol", base.module, name, base.path, base.line)
            return None
        if isinstance(base, Binding):
            target = base.members.get(name)
            if target is not None:
                trail.append(
                    {
                        "module": base.module,
                        "name": name,
                        "path": target.path,
                        "line": target.line,
                        "via": "python_class_member",
                    }
                )
                return self._resolve(target, trail, seen)
            if base.kind == "python_class" and isinstance(base.node, ast.ClassDef):
                for inherited in base.node.bases:
                    parent = self._resolve_expression(base.module, inherited, trail, seen)
                    if isinstance(parent, Binding):
                        inherited_member = self._get_attribute(parent, name, trail, seen)
                        if inherited_member is not None:
                            return inherited_member
            if base.kind in {"native_extension_symbol", "external_python_symbol"}:
                return Binding("native_extension_method", base.module, name, base.path, base.line)
        return None

    def resolve_public_path(
        self, public_path: str
    ) -> tuple[Binding | ModuleRef | SymbolRef | None, list[dict[str, Any]]]:
        parts = public_path.split(".")
        if not parts or parts[0] != "PIL" or "PIL" not in self.files:
            return None, []
        trail: list[dict[str, Any]] = []
        value: Binding | ModuleRef | SymbolRef | None = ModuleRef("PIL")
        seen: set[tuple[str, str]] = set()
        for part in parts[1:]:
            value = self._get_attribute(value, part, trail, seen)
            if value is None:
                break
        if isinstance(value, SymbolRef):
            value = self._resolve_symbol(value, trail, seen)
        return value, trail

    def node(self, value: Binding) -> dict[str, Any]:
        return {
            "kind": value.kind,
            "symbol": value.name,
            "module": value.module,
            "path": value.path,
            "line": value.line,
        }


def direct_self_method_call_lines(binding: Binding, method_name: str) -> list[int]:
    """Find direct `self.method(...)` calls in one indexed Python method."""
    if not isinstance(binding.node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return []
    return [
        node.lineno
        for node in ast.walk(binding.node)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == method_name
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "self"
    ]


def direct_class_base_names(binding: Binding) -> set[str]:
    """Return simple base class names from the checked-in class AST."""
    if not isinstance(binding.node, ast.ClassDef):
        return set()
    return {
        base.id if isinstance(base, ast.Name) else base.attr
        for base in binding.node.bases
        if isinstance(base, (ast.Name, ast.Attribute))
    }


def python_method_owner(
    index: PythonBindingIndex, method: Binding
) -> tuple[str, Binding] | None:
    """Find the class that directly declares an indexed method node."""
    for name, candidate in index.bindings.get(method.module, {}).items():
        if not isinstance(candidate, Binding) or candidate.kind != "python_class":
            continue
        member = candidate.members.get(method.name)
        if member is not None and member.node is method.node:
            return name, candidate
    return None


def assigned_names(node: ast.Assign | ast.AnnAssign) -> list[str]:
    targets = node.targets if isinstance(node, ast.Assign) else [node.target]
    names = []
    for target in targets:
        if isinstance(target, ast.Name):
            names.append(target.id)
    return names
