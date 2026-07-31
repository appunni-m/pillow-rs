#!/usr/bin/env python3
"""Validate or refresh the generated source tree in docs/REPO_MAP.md."""

from __future__ import annotations

import argparse
import difflib
import subprocess
import sys
from pathlib import Path


BEGIN = "<!-- BEGIN GENERATED CODE TREE -->"
END = "<!-- END GENERATED CODE TREE -->"
DOC_PATH = Path("docs/REPO_MAP.md")

ROOT_FILES = {
    ".github/workflows/ci.yml",
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "CONTRIBUTING.md",
    "Makefile",
    "README.md",
    "deny.toml",
    "rust-toolchain.toml",
    "rustfmt.toml",
}

INCLUDED_PREFIXES = (
    ".claude/skills/",
    "pillow-rs/src/",
    "pillow-rs-js/src/",
    "pillow-rs-py/python/",
    "pillow-rs-py/src/",
    "scripts/",
    "tests/",
)

INCLUDED_EXACT = {
    "docs/REPO_MAP.md",
    "pillow-rs/Cargo.toml",
    "pillow-rs-js/Cargo.toml",
    "pillow-rs-js/package.json",
    "pillow-rs-js/package-lock.json",
    "pillow-rs-py/Cargo.toml",
    "pillow-rs-py/pyproject.toml",
    "pillow-rs/tests/fixtures/manifest.yaml",
}

EXCLUDED_PREFIXES = (
    "tests/fixtures/",
)

EXCLUDED_SUFFIXES = (
    ".bmp",
    ".gif",
    ".ico",
    ".jpg",
    ".jpeg",
    ".png",
    ".tif",
    ".tiff",
    ".webp",
)


def repo_root() -> Path:
    """Return the git repository root."""
    output = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"],
        text=True,
    )
    return Path(output.strip())


def tracked_files(root: Path) -> list[str]:
    """Return tracked files from git, sorted lexicographically."""
    output = subprocess.check_output(["git", "ls-files"], cwd=root, text=True)
    return sorted(line for line in output.splitlines() if line)


def should_include(path: str) -> bool:
    """Return whether a tracked file belongs in the maintained source tree."""
    if path in ROOT_FILES or path in INCLUDED_EXACT:
        return True
    if path.startswith(EXCLUDED_PREFIXES):
        return False
    if path.endswith(EXCLUDED_SUFFIXES):
        return False
    return path.startswith(INCLUDED_PREFIXES)


def build_tree(paths: list[str]) -> dict[str, object]:
    """Build a nested dictionary tree from slash-separated paths."""
    root: dict[str, object] = {}
    for path in paths:
        node = root
        parts = path.split("/")
        for part in parts[:-1]:
            child = node.setdefault(part, {})
            if not isinstance(child, dict):
                raise ValueError(f"path conflict at {path}")
            node = child
        node[parts[-1]] = None
    return root


def render_tree(node: dict[str, object], prefix: str = "") -> list[str]:
    """Render a nested tree using ASCII connectors."""
    lines: list[str] = []
    names = sorted(node)
    for index, name in enumerate(names):
        value = node[name]
        is_last = index == len(names) - 1
        connector = "`-- " if is_last else "|-- "
        if isinstance(value, dict):
            lines.append(f"{prefix}{connector}{name}/")
            extension = "    " if is_last else "|   "
            lines.extend(render_tree(value, prefix + extension))
        else:
            lines.append(f"{prefix}{connector}{name}")
    return lines


def generated_tree(root: Path) -> str:
    """Generate the tree block contents."""
    paths = [path for path in tracked_files(root) if should_include(path)]
    tree = build_tree(paths)
    lines = ["."]
    lines.extend(render_tree(tree))
    return "\n".join(lines)


def generated_block(root: Path) -> str:
    """Generate the full marked markdown block."""
    return f"{BEGIN}\n```text\n{generated_tree(root)}\n```\n{END}"


def replace_block(document: str, replacement: str) -> str:
    """Replace the generated block in the document."""
    start = document.find(BEGIN)
    end = document.find(END)
    if start == -1 or end == -1 or end < start:
        raise ValueError(f"{DOC_PATH} is missing generated tree markers")
    end += len(END)
    return f"{document[:start]}{replacement}{document[end:]}"


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Validate or refresh docs/REPO_MAP.md generated source tree.",
    )
    parser.add_argument(
        "--write",
        action="store_true",
        help="rewrite docs/REPO_MAP.md with the current generated tree",
    )
    return parser.parse_args()


def main() -> int:
    """Run the repo map validator."""
    args = parse_args()
    root = repo_root()
    path = root / DOC_PATH
    document = path.read_text()
    expected = replace_block(document, generated_block(root))

    if args.write:
        path.write_text(expected)
        print(f"updated {DOC_PATH}")
        return 0

    if document == expected:
        print(f"{DOC_PATH} is current")
        return 0

    diff = difflib.unified_diff(
        document.splitlines(),
        expected.splitlines(),
        fromfile=str(DOC_PATH),
        tofile=f"{DOC_PATH} (expected)",
        lineterm="",
    )
    print("\n".join(diff), file=sys.stderr)
    print("Run `make repo-map-update` to refresh the generated tree.", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
