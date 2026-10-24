#!/usr/bin/env python3
"""Enforce the pillow-rs public API boundary.

The core crate's external API must be defined explicitly in
`pillow-rs/src/lib.rs`. Implementation modules may exist, but they must not be
public module delegations. Binding crates must call exact root names from
`pillow_rs`, not deep implementation paths.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LIB_RS = ROOT / "pillow-rs" / "src" / "lib.rs"
BINDING_FILES = [
    ROOT / "pillow-rs-py" / "src" / "lib.rs",
    ROOT / "pillow-rs-js" / "src" / "lib.rs",
]
RUST_GLOBS = [
    "pillow-rs/src/**/*.rs",
    "pillow-rs-py/src/**/*.rs",
    "pillow-rs-js/src/**/*.rs",
]
FONTDONE_ADAPTER = ROOT / "pillow-rs" / "src" / "font" / "imagingft.rs"
FONTDONE_ADAPTER_DIR = FONTDONE_ADAPTER.with_suffix("")
WILDCARD_BASELINE_PATH = ROOT / "scripts" / "public_api_boundary_baseline.json"

DEEP_MODULES = {
    "checked_dims",
    "color",
    "compute",
    "draw",
    "error",
    "font",
    "format",
    "image",
    "image_utils",
    "infallible",
    "ops",
    "par",
    "pipeline",
    "pixel_format",
}

PUBLIC_MOD_RE = re.compile(r"^\s*pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", re.MULTILINE)
PUB_USE_RE = re.compile(r"^\s*pub\s+use\s+([^;]+);", re.MULTILINE)
WILDCARD_USE_RE = re.compile(r"^\s*(?:pub\s+)?use\s+[^;]*::\*\s*;", re.MULTILINE)
GROUPED_USE_RE = re.compile(r"^\s*pub\s+use\s+[^;]*\{[^;]*\}\s*;", re.MULTILINE)
ROOT_DEEP_USE_RE = re.compile(
    r"\bpillow_rs::("
    + "|".join(sorted(DEEP_MODULES))
    + r")(?:::|\b)"
)
FONTDONE_USE_RE = re.compile(r"\bfontdone::")
ROOT_FT_EXPORT_RE = re.compile(r"^\s*pub\s+(?:use|fn|struct|enum|type|const)\s+.*\bFT_[A-Za-z0-9_]*", re.MULTILINE)


def rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def collect_rust_files() -> list[Path]:
    files: list[Path] = []
    for pattern in RUST_GLOBS:
        files.extend(ROOT.glob(pattern))
    return sorted(set(files))


def is_fontdone_adapter_source(path: Path) -> bool:
    """Treat the adapter root and its private implementation modules as one boundary."""
    return path == FONTDONE_ADAPTER or FONTDONE_ADAPTER_DIR in path.parents


def code_before_line_comment(line: str) -> str:
    """Exclude line-comment text from source-boundary checks."""
    return line.split("//", 1)[0]


def load_wildcard_baseline() -> tuple[dict[str, object], Counter[str]]:
    """Load the reviewed per-file ceilings for existing wildcard imports."""
    data = json.loads(WILDCARD_BASELINE_PATH.read_text(encoding="utf-8"))
    if data.get("format_version") != 1:
        raise ValueError(f"unsupported wildcard baseline: {WILDCARD_BASELINE_PATH}")
    for field in ("owner", "reduction_task", "reason"):
        if not isinstance(data.get(field), str) or not data[field].strip():
            raise ValueError(f"wildcard baseline is missing {field!r}")
    raw_counts = data.get("wildcard_use_counts")
    if not isinstance(raw_counts, dict):
        raise ValueError("wildcard baseline is missing wildcard_use_counts")
    counts: Counter[str] = Counter()
    for path, count in raw_counts.items():
        if (
            not isinstance(path, str)
            or not path.endswith(".rs")
            or not isinstance(count, int)
            or count <= 0
        ):
            raise ValueError(f"invalid wildcard baseline entry: {path!r}={count!r}")
        counts[path] = count
    return data, counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--update-wildcard-baseline",
        action="store_true",
        help="record reviewed reductions without permitting per-file growth",
    )
    args = parser.parse_args()

    errors: list[str] = []
    wildcard_counts: Counter[str] = Counter()
    wildcard_examples: dict[str, list[tuple[int, str]]] = {}
    try:
        baseline_metadata, baseline = load_wildcard_baseline()
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(
            f"could not read public API wildcard baseline: {error}",
            file=sys.stderr,
        )
        return 2

    lib_text = LIB_RS.read_text()

    for match in PUBLIC_MOD_RE.finditer(lib_text):
        errors.append(
            f"{rel(LIB_RS)}:{line_number(lib_text, match.start())}: "
            f"public module delegation is forbidden: pub mod {match.group(1)};"
        )

    for match in PUB_USE_RE.finditer(lib_text):
        export = match.group(1).strip()
        if "*" in export:
            errors.append(
                f"{rel(LIB_RS)}:{line_number(lib_text, match.start())}: "
                "wildcard public export is forbidden"
            )
        if "{" in export or "}" in export:
            errors.append(
                f"{rel(LIB_RS)}:{line_number(lib_text, match.start())}: "
                "grouped public export is forbidden; name each item on its own line"
            )
        if not re.search(r"::[A-Z_a-z][A-Za-z0-9_]*(?:\s+as\s+[A-Z_a-z][A-Za-z0-9_]*)?$", export):
            errors.append(
                f"{rel(LIB_RS)}:{line_number(lib_text, match.start())}: "
                f"public export must end in one exact item name: pub use {export};"
            )

    for path in collect_rust_files():
        text = path.read_text()
        if path != LIB_RS:
            for match in PUBLIC_MOD_RE.finditer(text):
                errors.append(
                    f"{rel(path)}:{line_number(text, match.start())}: "
                    "non-root modules must be private or pub(crate), not pub mod"
                )
        for match in WILDCARD_USE_RE.finditer(text):
            relative_path = rel(path)
            wildcard_counts[relative_path] += 1
            wildcard_examples.setdefault(relative_path, []).append(
                (line_number(text, match.start()), match.group(0).strip())
            )
        if not is_fontdone_adapter_source(path):
            line_offset = 0
            for line in text.splitlines(keepends=True):
                code = code_before_line_comment(line)
                if FONTDONE_USE_RE.search(code):
                    errors.append(
                        f"{rel(path)}:{line_number(text, line_offset)}: "
                        "fontdone access must stay isolated to the imagingft adapter"
                    )
                line_offset += len(line)

    for match in ROOT_FT_EXPORT_RE.finditer(lib_text):
        errors.append(
            f"{rel(LIB_RS)}:{line_number(lib_text, match.start())}: "
            "raw FreeType-shaped FT_* symbols must not be part of pillow-rs root API"
        )

    for path in BINDING_FILES:
        text = path.read_text()
        for match in ROOT_DEEP_USE_RE.finditer(text):
            errors.append(
                f"{rel(path)}:{line_number(text, match.start())}: "
                f"binding crate must use explicit root API, not pillow_rs::{match.group(1)}"
            )

    wildcard_growth: list[str] = []
    for path, current_count in sorted(wildcard_counts.items()):
        baseline_count = baseline[path]
        if current_count <= baseline_count:
            continue
        wildcard_growth.append(
            f"{path}: wildcard use/export grew from {baseline_count} to {current_count} "
            "relative to the reviewed baseline"
        )
        for line, statement in wildcard_examples[path][baseline_count : baseline_count + 3]:
            wildcard_growth.append(f"{path}:{line}: new wildcard use/export: {statement}")

    print(
        "wildcard use/export ratchet: "
        f"{sum(wildcard_counts.values())} current / {sum(baseline.values())} baseline "
        f"across {len(wildcard_counts)} files"
    )

    if args.update_wildcard_baseline:
        if errors or wildcard_growth:
            print("baseline update refused; resolve boundary errors and growth first:", file=sys.stderr)
            for error in errors + wildcard_growth:
                print(f"  {error}", file=sys.stderr)
            return 1
        baseline_metadata["wildcard_use_counts"] = dict(sorted(wildcard_counts.items()))
        WILDCARD_BASELINE_PATH.write_text(
            json.dumps(baseline_metadata, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        print(f"updated reviewed wildcard baseline: {WILDCARD_BASELINE_PATH}")
        return 0

    errors.extend(wildcard_growth)

    if errors:
        print("public API boundary violations:")
        for error in errors:
            print(f"  {error}")
        return 1

    print("public API boundary check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
