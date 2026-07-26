#!/usr/bin/env python3
"""Enforce the pillow-rs public API boundary.

The core crate's external API must be defined explicitly in
`pillow-rs/src/lib.rs`. Implementation modules may exist, but they must not be
public module delegations. Binding crates must call exact root names from
`pillow_rs`, not deep implementation paths.
"""

from __future__ import annotations

import re
import sys
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


def main() -> int:
    errors: list[str] = []

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
            errors.append(
                f"{rel(path)}:{line_number(text, match.start())}: "
                "wildcard use/export is forbidden"
            )
        if path != FONTDONE_ADAPTER:
            for match in FONTDONE_USE_RE.finditer(text):
                errors.append(
                    f"{rel(path)}:{line_number(text, match.start())}: "
                    "fontdone access must stay isolated to pillow-rs/src/font/imagingft.rs"
                )

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

    if errors:
        print("public API boundary violations:")
        for error in errors:
            print(f"  {error}")
        return 1

    print("public API boundary check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
