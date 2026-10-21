"""Conservative inventory of first-party Rust crate roots and module paths."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Iterable

from .rust_source import code_only, line_number, matching_delimiter

WORKSPACE_RUST_PACKAGES = ("pillow-rs", "pillow-rs-py", "pillow-rs-js")
SOURCE_AREAS = ("src", "examples", "tests", "benches")
MODULE_ITEM = re.compile(
    r"\b(?:(?:pub)(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([;{])"
)
INCLUDE_CALL = re.compile(r"\b(include|include_str|include_bytes)\s*!")
REEXPORT = re.compile(r"\bpub(?:\([^)]*\))?\s+use\b")
LITERAL_PATH = re.compile(r'^\s*(?:b)?\s*"((?:\\.|[^"\\])*)"\s*$')
RAW_LITERAL_PATH = re.compile(r'^\s*(?:br|r)(#*)"(.*?)"\1\s*$', re.DOTALL)
PATH_ATTRIBUTE = re.compile(r'\bpath\s*=\s*"((?:\\.|[^"\\])*)"')


def _attribute_blocks(source: str, before: int) -> list[str]:
    """Return immediately preceding outer attributes, preserving cfg/path text."""
    masked = code_only(source)
    blocks: list[tuple[int, int]] = []
    for match in re.finditer(r"#\s*\[", masked[:before]):
        opening = masked.find("[", match.start(), match.end())
        try:
            closing = matching_delimiter(source, opening, "[", "]")
        except ValueError:
            continue
        if closing < before:
            blocks.append((match.start(), closing + 1))

    selected: list[tuple[int, int]] = []
    cursor = before
    for start, end in reversed(blocks):
        if end > cursor:
            continue
        if masked[end:cursor].strip():
            break
        selected.append((start, end))
        cursor = start
    selected.reverse()
    return [source[start:end] for start, end in selected]


def _path_from_attributes(attributes: Iterable[str]) -> tuple[str | None, bool]:
    for attribute in attributes:
        match = PATH_ATTRIBUTE.search(attribute)
        if match:
            return match.group(1), "cfg_attr" in attribute
    return None, False


def _module_declarations(source: str) -> list[dict[str, Any]]:
    """Find external/inline module items and their lexical inline scopes."""
    masked = code_only(source)
    declarations: list[dict[str, Any]] = []
    inline_ranges: list[dict[str, Any]] = []
    for match in MODULE_ITEM.finditer(masked):
        name = match.group(1)
        delimiter = match.group(2)
        attributes = _attribute_blocks(source, match.start())
        declaration = {
            "name": name,
            "kind": "external" if delimiter == ";" else "inline",
            "offset": match.start(),
            "line": line_number(source, match.start()),
            "attributes": attributes,
        }
        if delimiter == "{":
            opening = masked.find("{", match.end() - 1)
            try:
                closing = matching_delimiter(source, opening, "{", "}")
            except ValueError:
                declaration["parse_error"] = "unclosed inline module body"
                closing = len(source)
            declaration["body_end"] = closing
            inline_ranges.append(declaration)
        declarations.append(declaration)

    for declaration in declarations:
        enclosing = [
            item for item in inline_ranges
            if item["offset"] < declaration["offset"] < item.get("body_end", -1)
        ]
        declaration["inline_scope"] = [
            item["name"] for item in sorted(enclosing, key=lambda item: item["offset"])
        ]
        declaration["inherited_attributes"] = [
            attribute
            for item in sorted(enclosing, key=lambda item: item["offset"])
            for attribute in item["attributes"]
        ]
    return declarations


def _module_directory(path: Path, is_crate_root: bool) -> Path:
    if is_crate_root or path.name == "mod.rs":
        return path.parent
    return path.parent / path.stem


def _resolve_module(
    base: Path, name: str, path_override: str | None
) -> tuple[Path | None, list[Path], str]:
    if path_override is not None:
        candidate = Path(path_override)
        candidate = candidate if candidate.is_absolute() else base / candidate
        candidate = candidate.resolve()
        return (candidate if candidate.is_file() else None), [candidate], (
            "resolved" if candidate.is_file() else "missing_explicit_path"
        )
    candidates = [base / f"{name}.rs", base / name / "mod.rs"]
    existing = [candidate.resolve() for candidate in candidates if candidate.is_file()]
    if len(existing) == 1:
        return existing[0], [candidate.resolve() for candidate in candidates], "resolved"
    if len(existing) > 1:
        return None, [candidate.resolve() for candidate in candidates], "ambiguous"
    return None, [candidate.resolve() for candidate in candidates], "missing_conventional_path"


def _source_roots(root: Path, package: str) -> tuple[set[Path], set[Path]]:
    package_root = root / package
    sources: set[Path] = set()
    roots: set[Path] = set()
    for area in SOURCE_AREAS:
        directory = package_root / area
        if not directory.is_dir():
            continue
        sources.update(path.resolve() for path in directory.rglob("*.rs") if path.is_file())
        if area == "src":
            for name in ("lib.rs", "main.rs"):
                candidate = directory / name
                if candidate.is_file():
                    roots.add(candidate.resolve())
            bin_dir = directory / "bin"
            if bin_dir.is_dir():
                for candidate in bin_dir.rglob("*.rs"):
                    if candidate.is_file() and (
                        candidate.parent == bin_dir or candidate.name == "main.rs"
                    ):
                        roots.add(candidate.resolve())
        else:
            for candidate in directory.glob("*.rs"):
                if candidate.is_file():
                    roots.add(candidate.resolve())
            for candidate in directory.rglob("main.rs"):
                if candidate.is_file():
                    roots.add(candidate.resolve())

    build_script = package_root / "build.rs"
    if build_script.is_file():
        roots.add(build_script.resolve())
        sources.add(build_script.resolve())
    return roots, sources


def _literal_argument(argument: str) -> str | None:
    match = LITERAL_PATH.match(argument)
    if match:
        return bytes(match.group(1), "utf-8").decode("unicode_escape")
    match = RAW_LITERAL_PATH.match(argument)
    return match.group(2) if match else None


def _include_references(
    source: str, path: Path, repo_root: Path
) -> tuple[list[dict[str, Any]], set[Path]]:
    masked = code_only(source)
    edges: list[dict[str, Any]] = []
    referenced: set[Path] = set()
    for match in INCLUDE_CALL.finditer(masked):
        opening = masked.find("(", match.end())
        if opening < 0:
            continue
        try:
            closing = matching_delimiter(source, opening, "(", ")")
        except ValueError:
            continue
        argument = source[opening + 1 : closing]
        literal = _literal_argument(argument)
        target = (path.parent / literal).resolve() if literal is not None else None
        exists = target is not None and target.is_file()
        if exists:
            referenced.add(target)
        edge: dict[str, Any] = {
            "from": path.relative_to(repo_root).as_posix(),
            "kind": match.group(1),
            "line": line_number(source, match.start()),
            "literal_path": literal,
            "status": "resolved" if exists else (
                "missing_literal_path" if literal is not None else "dynamic_path_review"
            ),
        }
        if target is not None:
            edge["to"] = target.relative_to(repo_root).as_posix() if exists else str(target)
        edges.append(edge)
    return edges, referenced


def _reexport_declarations(source: str, path: Path, repo_root: Path) -> list[dict[str, Any]]:
    masked = code_only(source)
    result = []
    for match in REEXPORT.finditer(masked):
        end = masked.find(";", match.end())
        if end < 0:
            continue
        declaration = re.sub(r"\s+", " ", source[match.start() : end + 1]).strip()
        result.append(
            {
                "from": path.relative_to(repo_root).as_posix(),
                "line": line_number(source, match.start()),
                "declaration": declaration,
            }
        )
    return result


def build_rust_module_inventory(
    root: Path,
    packages: Iterable[str] = WORKSPACE_RUST_PACKAGES,
) -> dict[str, Any]:
    """Build a static Rust source inventory without equating missing edges with dead code."""
    repo_root = root.resolve()
    packages = tuple(packages)
    all_sources: set[Path] = set()
    all_roots: set[Path] = set()
    package_for_path: dict[Path, str] = {}
    for package in packages:
        roots, sources = _source_roots(repo_root, package)
        all_roots.update(roots)
        all_sources.update(sources)
        package_for_path.update({path: package for path in sources})

    queue: list[tuple[Path, bool, tuple[str, ...]]] = [
        (path, True, ()) for path in sorted(all_roots)
    ]
    visited_contexts: set[tuple[Path, bool, tuple[str, ...]]] = set()
    reached: set[Path] = set(all_roots)
    module_edges: list[dict[str, Any]] = []
    include_edges: list[dict[str, Any]] = []
    reexports: list[dict[str, Any]] = []
    unresolved: list[dict[str, Any]] = []

    while queue:
        current, is_root, inherited_conditions = queue.pop(0)
        context = (current, is_root, inherited_conditions)
        if context in visited_contexts or not current.is_file():
            continue
        visited_contexts.add(context)
        source = current.read_text(encoding="utf-8")
        reexports.extend(_reexport_declarations(source, current, repo_root))
        declarations = _module_declarations(source)
        base = _module_directory(current, is_root)
        for declaration in declarations:
            if declaration["kind"] != "external":
                continue
            attributes = (
                list(inherited_conditions)
                + declaration["inherited_attributes"]
                + declaration["attributes"]
            )
            path_override, conditional_path = _path_from_attributes(
                declaration["attributes"]
            )
            inline_scope = Path(*declaration["inline_scope"]) if declaration["inline_scope"] else Path()
            scoped_module_base = base / inline_scope
            if path_override is None:
                scoped_path_base = scoped_module_base
            else:
                # Rust #[path] is based on the file containing the declaration;
                # an inline module contributes its own source-path component.
                scoped_path_base = current.parent / inline_scope
            target, candidates, status = _resolve_module(
                scoped_module_base if path_override is None else scoped_path_base,
                declaration["name"],
                path_override,
            )
            cfg_attributes = [
                attribute for attribute in attributes
                if re.search(r"#\s*\[\s*(?:cfg|cfg_attr)\b", attribute)
            ]
            edge = {
                "from": current.relative_to(repo_root).as_posix(),
                "module": declaration["name"],
                "line": declaration["line"],
                "inline_scope": declaration["inline_scope"],
                "cfg_attributes": cfg_attributes,
                "conditional": bool(cfg_attributes or conditional_path),
                "resolution": status,
                "candidate_paths": [
                    _display_path(candidate, repo_root)
                    for candidate in candidates
                ],
            }
            if target is not None:
                edge["to"] = _display_path(target, repo_root)
                module_edges.append(edge)
                reached.add(target)
                queue.append((target, False, tuple(cfg_attributes)))
            else:
                edge["to"] = None
                unresolved.append(edge)

        includes, included_paths = _include_references(source, current, repo_root)
        include_edges.extend(includes)
        reached.update(included_paths)

    roots = [
        {
            "path": path.relative_to(repo_root).as_posix(),
            "package": package_for_path.get(path),
            "target_kind": (
                "build_script" if path.name == "build.rs"
                else "library" if path.name == "lib.rs"
                else "binary_or_example_or_test"
            ),
        }
        for path in sorted(all_roots)
    ]
    incoming_modules: dict[str, list[dict[str, Any]]] = {}
    for edge in module_edges:
        incoming_modules.setdefault(edge["to"], []).append(edge)
    incoming_includes: dict[str, list[dict[str, Any]]] = {}
    for edge in include_edges:
        target = edge.get("to")
        if target and not target.startswith("/"):
            incoming_includes.setdefault(target, []).append(edge)

    source_files = []
    for path in sorted(all_sources):
        relative = path.relative_to(repo_root).as_posix()
        declarations = incoming_modules.get(relative, [])
        includes = incoming_includes.get(relative, [])
        role = (
            "crate_target_root" if path in all_roots
            else "declared_module" if declarations
            else "included_source" if includes
            else "unreached_review_candidate"
        )
        source_files.append(
            {
                "path": relative,
                "package": package_for_path.get(path),
                "static_role": role,
                "declared_conditionally": any(item["conditional"] for item in declarations),
                "declared_by": [
                    {
                        "path": item["from"],
                        "line": item["line"],
                        "module": item["module"],
                        "conditional": item["conditional"],
                        "cfg_attributes": item["cfg_attributes"],
                    }
                    for item in declarations
                ],
                "included_by": [
                    {"path": item["from"], "line": item["line"], "kind": item["kind"]}
                    for item in includes
                ],
                "operation_crosswalk": {
                    "status": "not_assessed_by_module_inventory",
                    "node_ids": [],
                    "edge_count": 0,
                },
            }
        )
    unreached = []
    for path in sorted(all_sources - reached):
        unreached.append(
            {
                "path": path.relative_to(repo_root).as_posix(),
                "package": package_for_path.get(path),
                "flag": "rust_source_without_discovered_root_or_module_edge",
                "reason_for_review": (
                    "No selected Cargo target root, statically resolved module declaration, "
                    "or literal include reference reaches this file. Macro-generated, "
                    "build-generated, and target-specific references may still exist."
                ),
            }
        )

    module_edges.sort(key=lambda item: (item["from"], item["line"], item["module"]))
    include_edges.sort(key=lambda item: (item["from"], item["line"], item["kind"]))
    reexports.sort(key=lambda item: (item["from"], item["line"], item["declaration"]))
    unresolved.sort(key=lambda item: (item["from"], item["line"], item["module"]))
    return {
        "scope": {
            "packages": list(packages),
            "areas": list(SOURCE_AREAS) + ["build.rs"],
            "runtime_reachability_proven": False,
            "cfg_evaluated": False,
        },
        "summary": {
            "rust_source_files": len(all_sources),
            "crate_roots": len(all_roots),
            "resolved_module_edges": len(module_edges),
            "unresolved_module_edges": len(unresolved),
            "include_references": len(include_edges),
            "reexports": len(reexports),
            "unreached_source_candidates": len(unreached),
        },
        "roots": roots,
        "source_files": source_files,
        "module_edges": module_edges,
        "unresolved_module_edges": unresolved,
        "include_references": include_edges,
        "reexports": reexports,
        "unreached_source_candidates": unreached,
        "limits": [
            "Module and include edges are static source evidence; cfg attributes are recorded but not evaluated.",
            "Unreached files and unresolved declarations are investigation candidates, not proof of unreachable or unused code.",
            "Macro-generated paths, build output, proc-macro expansion, and runtime loading are not resolved.",
        ],
    }


def _display_path(path: Path, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return str(path)
