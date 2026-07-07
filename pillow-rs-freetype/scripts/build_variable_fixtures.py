#!/usr/bin/env python3
"""Build shared variable-font fixtures for public API inputs."""

from __future__ import annotations

import os
import shutil
from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
GENERATED = FIXTURE_ROOT / "input" / "fonts" / "generated" / "variable" / "ubuntu-sans-variable.ttf"

SOURCE_CANDIDATES = [
    Path("/usr/share/fonts/truetype/ubuntu/UbuntuSans[wdth,wght].ttf"),
    Path("/usr/share/fonts/truetype/ubuntu/Ubuntu[wdth,wght].ttf"),
]

LINKS = [
    FIXTURE_ROOT / "fonts" / "variable" / "inter-var.ttf",
    FIXTURE_ROOT / "input" / "fonts" / "variable" / "named-instances.ttf",
]


def source_font() -> Path:
    for path in SOURCE_CANDIDATES:
        if path.is_file():
            font = TTFont(path, lazy=True)
            if "fvar" in font and font["fvar"].instances:
                return path
    candidates = ", ".join(str(path) for path in SOURCE_CANDIDATES)
    raise SystemExit(f"no usable variable font fixture source found; checked: {candidates}")


def relink(link: Path, target: Path) -> None:
    link.parent.mkdir(parents=True, exist_ok=True)
    if link.exists() or link.is_symlink():
        link.unlink()
    link.symlink_to(os.path.relpath(target, link.parent))


def main() -> None:
    source = source_font()
    GENERATED.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, GENERATED)
    for link in LINKS:
        relink(link, GENERATED)
    print(f"wrote {GENERATED.relative_to(ROOT)} from {source}")
    print(f"ensured {len(LINKS)} variable fixture asset paths")


if __name__ == "__main__":
    main()
