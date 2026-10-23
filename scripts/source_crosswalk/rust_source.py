"""Conservative Rust source scanners for the operation crosswalk."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Iterator

PIPELINE_PATH = Path("pillow-rs/src/pipeline.rs")
REGISTRY_PATH = Path("pillow-rs/src/compute/registry.rs")
REGISTRATION_ROOT = Path("pillow-rs/src/compute/registry/registration")
CPU_ROOT = Path("pillow-rs/src/compute/pool_cpu/ops")
SIMD_ROOT = Path("pillow-rs/src/compute/pool_simd")
SHADER_ROOT = Path("pillow-rs/src/compute/pool_gpu/shaders")
RAW_STRING = re.compile(r"(?:br|r)(#*)\"")
CHAR_LITERAL = re.compile(r"(?:b)?'(?:\\.|[^'\\\n])'")

def code_only(source: str) -> str:
    """Mask Rust comments and literals while retaining offsets and newlines."""
    chars = list(source)
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end < 0 else end
            for offset in range(index, end):
                chars[offset] = " "
            index = end
            continue
        if source.startswith("/*", index):
            start = index
            depth = 1
            index += 2
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            for offset in range(start, index):
                if chars[offset] != "\n":
                    chars[offset] = " "
            continue
        raw = (
            RAW_STRING.match(source, index)
            if source[index] in "rb"
            else None
        )
        if raw:
            start = index
            terminator = '"' + raw.group(1)
            index = raw.end()
            end = source.find(terminator, index)
            index = length if end < 0 else end + len(terminator)
            for offset in range(start, index):
                if chars[offset] != "\n":
                    chars[offset] = " "
            continue
        if source[index] == '"' or (
            source[index] == "b" and index + 1 < length and source[index + 1] == '"'
        ):
            start = index
            if source[index] == "b":
                index += 1
            index += 1
            escaped = False
            while index < length:
                char = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            for offset in range(start, index):
                if chars[offset] != "\n":
                    chars[offset] = " "
            continue
        char_literal = (
            CHAR_LITERAL.match(source, index)
            if source[index] == "'" or (source[index] == "b" and source[index : index + 2] == "b'")
            else None
        )
        if char_literal:
            start = index
            index = char_literal.end()
            for offset in range(start, index):
                chars[offset] = " "
            continue
        index += 1
    return "".join(chars)


def matching_delimiter(source: str, opening: int, left: str, right: str) -> int:
    """Return the matching delimiter, ignoring Rust comments and literals."""
    if opening >= len(source) or source[opening] != left:
        raise ValueError(f"expected {left!r} at offset {opening}")
    index = opening
    depth = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index)
            index = length if end < 0 else end
            continue
        if source.startswith("/*", index):
            nesting = 1
            index += 2
            while index < length and nesting:
                if source.startswith("/*", index):
                    nesting += 1
                    index += 2
                elif source.startswith("*/", index):
                    nesting -= 1
                    index += 2
                else:
                    index += 1
            continue
        raw = (
            RAW_STRING.match(source, index)
            if source[index] in "rb"
            else None
        )
        if raw:
            terminator = '"' + raw.group(1)
            index = raw.end()
            end = source.find(terminator, index)
            if end < 0:
                raise ValueError("unterminated Rust raw string")
            index = end + len(terminator)
            continue
        if source[index] == '"' or (
            source[index] == "b" and index + 1 < length and source[index + 1] == '"'
        ):
            if source[index] == "b":
                index += 1
            index += 1
            escaped = False
            while index < length:
                char = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            continue
        char_literal = (
            CHAR_LITERAL.match(source, index)
            if source[index] == "'" or (source[index] == "b" and source[index : index + 2] == "b'")
            else None
        )
        if char_literal:
            index = char_literal.end()
            continue
        if source[index] == left:
            depth += 1
        elif source[index] == right:
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise ValueError(f"unclosed {left!r} starting at offset {opening}")


def item_body(source: str, marker: str) -> tuple[str, int]:
    masked = code_only(source)
    match = re.search(marker, masked)
    if not match:
        raise ValueError(f"Rust source is missing expected item {marker!r}")
    opening = masked.find("{", match.end())
    if opening < 0:
        raise ValueError(f"Rust item {marker!r} has no body")
    closing = matching_delimiter(source, opening, "{", "}")
    return source[opening + 1 : closing], opening + 1


def line_number(source: str, offset: int) -> int:
    return source.count("\n", 0, offset) + 1


def enum_variants(source: str) -> list[dict[str, Any]]:
    body, start = item_body(source, r"\bpub\s+enum\s+PipelineOp\b")
    masked = code_only(body)
    variants = []
    depth = 0
    line_start = 0
    for index, char in enumerate(masked):
        if index == line_start and depth == 0:
            line_end = masked.find("\n", index)
            line_end = len(masked) if line_end < 0 else line_end
            line = masked[index:line_end]
            match = re.match(r"\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\{|,|\()", line)
            if match:
                variants.append(
                    {
                        "name": match.group(1),
                        "path": PIPELINE_PATH.as_posix(),
                        "line": line_number(source, start + index),
                    }
                )
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        if char == "\n":
            line_start = index + 1
    return variants


def variant_key_edges(source: str) -> list[dict[str, Any]]:
    body, start = item_body(source, r"\bfn\s+variant_key\s*\(")
    edges = []
    pending = ""
    offset = 0
    for line in body.splitlines(keepends=True):
        visible = code_only(line)
        if not visible.strip():
            offset += len(line)
            continue
        pending += visible
        if "=>" in visible:
            left, right = pending.split("=>", 1)
            key = re.search(r'"([A-Za-z0-9_]+)"', line)
            if key:
                for variant in re.findall(r"\bPipelineOp::([A-Za-z_][A-Za-z0-9_]*)", left):
                    edges.append(
                        {
                            "variant": variant,
                            "key": key.group(1),
                            "line": line_number(source, start + offset),
                        }
                    )
            pending = ""
        offset += len(line)
    return edges


def invocations(source: str, pattern: str) -> Iterator[tuple[str, int]]:
    masked = code_only(source)
    for match in re.finditer(pattern, masked):
        opening = masked.find("(", match.start(), match.end() + 1)
        if opening < 0:
            opening = masked.find("(", match.end())
        if opening < 0:
            continue
        closing = matching_delimiter(source, opening, "(", ")")
        yield source[match.start() : closing + 1], match.start()


def function_index(root: Path, repo_root: Path) -> dict[str, list[dict[str, Any]]]:
    result: dict[str, list[dict[str, Any]]] = {}
    for path in sorted(root.rglob("*.rs")):
        source = path.read_text(encoding="utf-8")
        masked = code_only(source)
        for match in re.finditer(
            r"\b(?:(?:pub)(?:\([^)]*\))?\s+)?(?:(?:unsafe|async)\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(",
            masked,
        ):
            name = match.group(1)
            line = line_number(source, match.start())
            preceding = source.splitlines()[: line - 1]
            attributes = []
            for prior in reversed(preceding[-6:]):
                stripped = prior.strip()
                if not stripped or stripped.startswith("///"):
                    continue
                if stripped.startswith("#["):
                    if "cfg" in stripped or "target_feature" in stripped:
                        attributes.append(stripped)
                    continue
                break
            result.setdefault(name, []).append(
                {
                    "path": path.relative_to(repo_root).as_posix(),
                    "line": line,
                    "attributes": list(reversed(attributes)),
                }
            )
    return result


def macro_generated_functions(root: Path, repo_root: Path) -> dict[str, list[dict[str, Any]]]:
    """Find explicit macro invocations whose first argument names a SIMD fn."""
    result: dict[str, list[dict[str, Any]]] = {}
    pattern = re.compile(
        r"\b([A-Za-z_][A-Za-z0-9_]*)\s*!\s*\(\s*(simd_[A-Za-z_][A-Za-z0-9_]*)\s*,"
    )
    for path in sorted(root.rglob("*.rs")):
        source = path.read_text(encoding="utf-8")
        masked = code_only(source)
        for match in pattern.finditer(masked):
            result.setdefault(match.group(2), []).append(
                {
                    "path": path.relative_to(repo_root).as_posix(),
                    "line": line_number(source, match.start()),
                    "macro": match.group(1),
                }
            )
    return result


def parse_registry(source: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    body, start = item_body(source, r"\bfn\s+register_all\s*\(")
    registrations = []
    draw_macro, _ = item_body(body, r"\bmacro_rules!\s+draw_entry\b")
    draw_shader_match = re.search(r'"([^"\n]+\.wgsl)"', draw_macro)
    draw_shader = draw_shader_match.group(1) if draw_shader_match else None

    for call, offset in invocations(body, r"\bm\.insert\s*\("):
        key_match = re.search(r'\(\s*"([A-Za-z0-9_]+)"', call)
        if not key_match:
            continue
        shaders = re.findall(r'"([^"\n]+\.wgsl)"', call)
        registrations.append(
            {
                "key": key_match.group(1),
                "line": line_number(source, start + offset),
                "text": call,
                "shader": shaders[-1] if shaders else None,
            }
        )
    for call, offset in invocations(body, r"\bdraw_entry\s*!\s*"):
        key_match = re.search(r'\(\s*"([A-Za-z0-9_]+)"', call)
        if key_match:
            registrations.append(
                {
                    "key": key_match.group(1),
                    "line": line_number(source, start + offset),
                    "text": call,
                    "shader": draw_shader,
                }
            )

    simd_entries = []
    for match in re.finditer(
        r'\bsimd_set\s*\(\s*m\s*,\s*"([A-Za-z0-9_]+)"\s*,\s*adapters::([A-Za-z_][A-Za-z0-9_]*)\s*\)',
        body,
    ):
        simd_entries.append(
            {
                "key": match.group(1),
                "symbol": match.group(2),
                "line": line_number(source, start + match.start()),
            }
        )
    return registrations, simd_entries


def parse_registry_modules(
    sources: dict[str, str],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Index operation registrations split across registry initializer modules."""
    module_path = (REGISTRATION_ROOT / "mod.rs").as_posix()
    module_source = sources[module_path]
    draw_macro, _ = item_body(module_source, r"\bmacro_rules!\s+draw_entry\b")
    draw_shader_match = re.search(r'"([^"\n]+\.wgsl)"', draw_macro)
    draw_shader = draw_shader_match.group(1) if draw_shader_match else None

    registrations = []
    simd_entries = []
    for path, source in sorted(sources.items()):
        for call, offset in invocations(source, r"\bm\.insert\s*\("):
            key_match = re.search(r'\(\s*"([A-Za-z0-9_]+)"', call)
            if not key_match:
                continue
            shaders = re.findall(r'"([^"\n]+\.wgsl)"', call)
            registrations.append(
                {
                    "key": key_match.group(1),
                    "path": path,
                    "line": line_number(source, offset),
                    "text": call,
                    "shader": shaders[-1] if shaders else None,
                }
            )
        for call, offset in invocations(source, r"\bdraw_entry\s*!\s*"):
            key_match = re.search(r'\(\s*[^,]+,\s*"([A-Za-z0-9_]+)"', call)
            if key_match:
                registrations.append(
                    {
                        "key": key_match.group(1),
                        "path": path,
                        "line": line_number(source, offset),
                        "text": call,
                        "shader": draw_shader,
                    }
                )
        for call, offset in invocations(source, r"\bsimd_set\s*\("):
            match = re.search(
                r'\(\s*m\s*,\s*"([A-Za-z0-9_]+)"\s*,\s*adapters::([A-Za-z_][A-Za-z0-9_]*)\s*\)',
                call,
            )
            if not match:
                continue
            simd_entries.append(
                {
                    "key": match.group(1),
                    "symbol": match.group(2),
                    "path": path,
                    "line": line_number(source, offset),
                }
            )
    return registrations, simd_entries


def cpu_registration_imports(source: str) -> dict[str, str]:
    """Map local names imported from pool_cpu operation modules to their symbols."""
    register_all = re.search(r"\bfn\s+register_all\s*\(", code_only(source))
    body = item_body(source, r"\bfn\s+register_all\s*\(")[0] if register_all else source
    masked = code_only(body)
    imported: dict[str, str] = {}
    use_pattern = re.compile(
        r"\buse\s+crate::compute::pool_cpu::ops::([^;]+);"
    )
    grouped = re.compile(r"(?:[A-Za-z_][A-Za-z0-9_]*::)*\{([^{}]*)\}")
    item_pattern = re.compile(
        r"\s*([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*"
    )
    for use_match in use_pattern.finditer(masked):
        tree_start, tree_end = use_match.span(1)
        tree = masked[tree_start:tree_end]
        group = grouped.fullmatch(tree.strip())
        # A family operation can be imported directly from its focused module
        # (for example, `ops::grayscale::op_grayscale`) instead of from a
        # multi-family grouped import (`ops::imageops::{...}`). In the direct
        # form, only the final path segment names the imported symbol.
        items = (
            group.group(1).split(",")
            if group
            else [tree.strip().rsplit("::", maxsplit=1)[-1]]
        )
        for item in items:
            match = item_pattern.fullmatch(item)
            if match:
                symbol = match.group(1)
                local_name = match.group(2) or symbol
                imported[local_name] = symbol
    return imported


def cpu_registration_references(
    registration: str, imported_symbols: dict[str, str]
) -> list[str]:
    """Return referenced CPU symbols, excluding macro and method names.

    Registration closures may call a CPU implementation through either a local
    `use` binding or its fully qualified `crate::compute::pool_cpu::ops` path.
    Resolve both forms so moving an operation into a focused module does not
    hide the registry edge from the source crosswalk.
    """
    masked = code_only(registration)
    references = []
    for local_name, symbol in imported_symbols.items():
        for match in re.finditer(rf"\b{re.escape(local_name)}\b", masked):
            suffix = masked[match.end() :]
            prefix = masked[: match.start()].rstrip()
            if re.match(r"\s*!", suffix) or prefix.endswith("."):
                continue
            references.append((match.start(), symbol))
    qualified_path = re.compile(
        r"\bcrate::compute::pool_cpu::ops::"
        r"(?:[A-Za-z_][A-Za-z0-9_]*::)+"
        r"(?P<symbol>[A-Za-z_][A-Za-z0-9_]*)\b"
    )
    references.extend(
        (match.start(), match.group("symbol"))
        for match in qualified_path.finditer(masked)
    )
    return list(dict.fromkeys(symbol for _, symbol in sorted(references)))
