#!/usr/bin/env python3
"""Build deterministic kerning fixture fonts for public API inputs."""

from __future__ import annotations

import os
from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
BASE_FONT = FIXTURE_ROOT / "input" / "fonts" / "LiberationSerif-Regular.ttf"
GENERATED_DIR = FIXTURE_ROOT / "input" / "fonts" / "generated" / "kerning"


def save_font(name: str, remove_tables: set[str]) -> Path:
    GENERATED_DIR.mkdir(parents=True, exist_ok=True)
    font = TTFont(BASE_FONT)
    for table in remove_tables:
        if table in font:
            del font[table]
    target = GENERATED_DIR / name
    font.save(target)
    return target


def ensure_link(asset_path: str, source: Path) -> None:
    target = FIXTURE_ROOT / asset_path
    target.parent.mkdir(parents=True, exist_ok=True)
    relative_source = os.path.relpath(source, target.parent)
    if target.is_symlink():
        if os.readlink(target) != relative_source:
            target.unlink()
            target.symlink_to(relative_source)
        return
    if target.exists():
        target.unlink()
    target.symlink_to(relative_source)


def main() -> None:
    legacy = save_font("legacy-av-kern.ttf", {"GPOS"})
    no_kern = save_font("no-kern-table.ttf", {"kern", "GPOS"})
    gpos_only = save_font("gpos-only-av.ttf", {"kern"})

    ensure_link("input/fonts/kerning/legacy-av-kern.ttf", legacy)
    ensure_link("input/fonts/kerning/no-kern-table.ttf", no_kern)
    ensure_link("input/fonts/kerning/gpos-only-av.ttf", gpos_only)
    ensure_link("fonts/kerning/kern-pair-av.ttf", legacy)

    print(f"wrote {legacy.relative_to(ROOT)} with legacy kern table")
    print(f"wrote {no_kern.relative_to(ROOT)} without kern or GPOS tables")
    print(f"wrote {gpos_only.relative_to(ROOT)} with GPOS only")
    print("ensured 4 kerning fixture asset paths")


if __name__ == "__main__":
    main()
