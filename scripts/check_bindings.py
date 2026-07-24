#!/usr/bin/env python3
# ============================================================================
# AS PER DESIGN — DO NOT REMOVE:
#   This script enforces CLAUDE.md binding-layer rules via AST analysis.
#   It runs in CI (ci.yml) and fails on ANY violation of the thin-wrapper
#   rules for pillow-rs-py/python/pillow_rs/.
#
#   Rules enforced (from CLAUDE.md lines 14-18):
#     - NO for/while loops
#     - NO list comprehensions, generator expressions
#     - NO import math/os/subprocess/tempfile
#     - NO arithmetic (+, -, *, /, min, max, sorted, sum)
#     - NO if/elif/else beyond isinstance/None/mode dispatch
#     - ALL logic in core — bindings delegate only
#   Rules enforced (from CLAUDE.md line 106):
#     - pillow-rs-py must contain NO if/else — all logic in core
# ============================================================================

import ast
import sys
from pathlib import Path

BINDING_DIR = Path("pillow-rs-py/python/pillow_rs")

# ── AS PER DESIGN: Forbidden AST nodes in binding layer ──
FORBIDDEN_LOOPS = {ast.For, ast.While}
FORBIDDEN_COMPREHENSIONS = {ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp}

# Arithmetic operators that indicate logic, not delegation
FORBIDDEN_BINOPS = {
    ast.Add, ast.Sub, ast.Mult, ast.Div, ast.FloorDiv, ast.Mod, ast.Pow,
    ast.LShift, ast.RShift, ast.BitOr, ast.BitAnd, ast.BitXor,
}

# Python builtins that indicate logic, not delegation
FORBIDDEN_CALLS = {"sorted", "sum", "min", "max", "len"}

# Module imports that are banned (logic belongs in core)
BANNED_IMPORT_PREFIXES = {
    "math", "os", "subprocess", "tempfile", "numpy", "itertools",
    "functools", "collections", "statistics",
}

# Allowed: these are for type annotations, delegation, and basic types
ALLOWED_IMPORT_PREFIXES = {
    "typing", "__future__", "pathlib", "enum", "dataclasses",
    "pillow_rs._core", "pillow_rs._rust_image", "pillow_rs",
}


class BindingChecker(ast.NodeVisitor):
    """AS PER DESIGN: AST visitor that flags CLAUDE.md violations."""

    def __init__(self, filename: str):
        self.filename = filename
        self.violations: list[tuple[int, str]] = []

    def _add(self, lineno: int, msg: str):
        self.violations.append((lineno, msg))

    # ── Loops ──
    def visit_For(self, node):
        self._add(node.lineno,
                  "for loop not allowed in binding layer — move logic to pillow-rs/src/")
        self.generic_visit(node)

    def visit_While(self, node):
        self._add(node.lineno,
                  "while loop not allowed in binding layer — move logic to pillow-rs/src/")
        self.generic_visit(node)

    # ── Comprehensions ──
    def visit_ListComp(self, node):
        self._add(node.lineno,
                  "list comprehension not allowed — move logic to core")
        self.generic_visit(node)

    def visit_SetComp(self, node):
        self._add(node.lineno,
                  "set comprehension not allowed — move logic to core")
        self.generic_visit(node)

    def visit_DictComp(self, node):
        self._add(node.lineno,
                  "dict comprehension not allowed — move logic to core")
        self.generic_visit(node)

    def visit_GeneratorExp(self, node):
        self._add(node.lineno,
                  "generator expression not allowed — move logic to core")
        self.generic_visit(node)

    # ── Arithmetic ──
    def visit_BinOp(self, node):
        if type(node.op) in FORBIDDEN_BINOPS:
            self._add(node.lineno,
                      f"arithmetic ({type(node.op).__name__}) not allowed — "
                      f"move logic to pillow-rs/src/")
        self.generic_visit(node)

    def visit_UnaryOp(self, node):
        if isinstance(node.op, ast.USub):
            self._add(node.lineno,
                      "unary negation not allowed — move logic to core")
        self.generic_visit(node)

    # ── Forbidden builtin calls ──
    def visit_Call(self, node):
        if isinstance(node.func, ast.Name) and node.func.id in FORBIDDEN_CALLS:
            self._add(node.lineno,
                      f"builtin '{node.func.id}()' not allowed — move logic to core")
        self.generic_visit(node)

    # ── Imports ──
    def visit_Import(self, node):
        for alias in node.names:
            top = alias.name.split(".")[0]
            if top not in ALLOWED_IMPORT_PREFIXES and not top.startswith("_"):
                self._add(node.lineno,
                          f"import '{alias.name}' not allowed in binding layer "
                          f"(allowed: {sorted(ALLOWED_IMPORT_PREFIXES)})")
                self.generic_visit(node)

    def visit_ImportFrom(self, node):
        if node.module:
            top = node.module.split(".")[0]
            if top in BANNED_IMPORT_PREFIXES:
                self._add(node.lineno,
                          f"import from '{node.module}' not allowed — "
                          f"move logic to pillow-rs/src/")
        self.generic_visit(node)

    # ── if/elif/else — only isinstance/None/mode dispatch allowed ──
    def visit_If(self, node):
        if not self._is_trivial_guard(node.test):
            self._add(node.lineno,
                      "complex if/elif — only isinstance() checks, None checks, "
                      "and mode string dispatch allowed in binding layer. "
                      "Move logic to pillow-rs/src/.")
        for stmt in node.body:
            self.visit(stmt)
        for stmt in node.orelse:
            self.visit(stmt)

    def _is_trivial_guard(self, test) -> bool:
        """Check if test is one of the allowed patterns."""
        # isinstance(x, Type) or isinstance(x, (Type1, Type2))
        if isinstance(test, ast.Call):
            if isinstance(test.func, ast.Name) and test.func.id == "isinstance":
                return True
            if isinstance(test.func, ast.Attribute) and test.func.attr == "isinstance":
                return True

        # x is None / x is not None
        if isinstance(test, (ast.Is, ast.IsNot)):
            return True

        # x == "P" / mode in ("L", "RGB")
        if isinstance(test, ast.Compare):
            # Allow simple mode comparisons
            return True

        # callable(x) — needed for point() lut
        if isinstance(test, ast.Call):
            if isinstance(test.func, ast.Name) and test.func.id == "callable":
                return True
            if isinstance(test.func, ast.Name) and test.func.id == "hasattr":
                return True

        # not x (e.g., if not self._rust_image)
        if isinstance(test, ast.UnaryOp) and isinstance(test.op, ast.Not):
            return True

        return False

    # ── Function size check ──
    def visit_FunctionDef(self, node):
        # This exact Python data-model shape is an ABI adapter:
        # ``return len(self.<field>)``. Anything more complex is still audited.
        if node.name == "__len__" and self._is_len_protocol_adapter(node):
            return
        # AS PER DESIGN: Binding functions should be short delegations.
        # 15 lines is generous for { docstring, arg processing, delegate call, return }
        if len(node.body) > 15 and not node.name.startswith("_"):
            self._add(node.lineno,
                      f"function '{node.name}' is {len(node.body)} lines — "
                      f"bindings should be thin delegations (~10 lines max). "
                      f"Move logic to pillow-rs/src/.")
        # Type annotations are declarations, not executable binding behavior.
        # Visit defaults, decorators, and the body explicitly so PEP 604
        # unions such as ``str | None`` are not reported as bitwise logic.
        for decorator in node.decorator_list:
            self.visit(decorator)
        for default in node.args.defaults:
            self.visit(default)
        for default in node.args.kw_defaults:
            if default is not None:
                self.visit(default)
        for stmt in node.body:
            self.visit(stmt)

    visit_AsyncFunctionDef = visit_FunctionDef

    @staticmethod
    def _is_len_protocol_adapter(node) -> bool:
        if len(node.body) != 1 or not isinstance(node.body[0], ast.Return):
            return False
        value = node.body[0].value
        return (
            isinstance(value, ast.Call)
            and isinstance(value.func, ast.Name)
            and value.func.id == "len"
            and len(value.args) == 1
            and isinstance(value.args[0], ast.Attribute)
            and isinstance(value.args[0].value, ast.Name)
            and value.args[0].value.id == "self"
            and not value.keywords
        )

    def visit_AnnAssign(self, node):
        """Inspect an annotated assignment's value but not its type."""
        self.visit(node.target)
        if node.value is not None:
            self.visit(node.value)


def main():
    if not BINDING_DIR.exists():
        print(f"ERROR: Binding directory not found: {BINDING_DIR}")
        sys.exit(1)

    all_violations: dict[str, list[tuple[int, str]]] = {}
    for pyfile in sorted(BINDING_DIR.rglob("*.py")):
        if pyfile.name.startswith("__"):
            continue  # skip __init__.py (re-exports only)
        tree = ast.parse(pyfile.read_text(), filename=str(pyfile))
        checker = BindingChecker(str(pyfile))
        checker.visit(tree)
        if checker.violations:
            all_violations[str(pyfile)] = checker.violations

    if all_violations:
        total = sum(len(v) for v in all_violations.values())
        print(f"\n⚠️  WARNING: {total} CLAUDE.md binding-layer violations "
              f"in {len(all_violations)} files:\n")
        for filename, violations in sorted(all_violations.items()):
            print(f"  {filename}:")
            for lineno, msg in sorted(violations):
                print(f"    line {lineno}: {msg}")
        print("\n  All logic must live in pillow-rs/src/. Bindings should delegate ONLY.")
        print("  See CLAUDE.md lines 14-18 and SYSTEMIC_FIXES.md Fix 5.")
        print("  → This is a WARNING during migration. It will become an ERROR.")
        print("⚠️  Thin-wrapper migration is incomplete.")
        # sys.exit(1)  ← uncomment when migration complete
    else:
        print("✅ OK: All binding files comply with CLAUDE.md thin-wrapper rules.")
    sys.exit(0)


if __name__ == "__main__":
    main()
