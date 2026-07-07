#!/usr/bin/env python3
"""Build deterministic face-property fixture fonts for public API inputs."""

from __future__ import annotations

import os
from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
FONT_ROOT = FIXTURE_ROOT / "input" / "fonts"
AUTOHINT_FONT_ROOT = FIXTURE_ROOT / "input" / "fonts_autohint"
GENERATED_DIR = FONT_ROOT / "generated" / "face-properties"

DEJAVU = FONT_ROOT / "DejaVuSans.ttf"
MONO = AUTOHINT_FONT_ROOT / "DejaVuSansMono.ttf"
NO_KERN = FONT_ROOT / "generated" / "kerning" / "no-kern-table.ttf"


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


def build_no_post_font() -> Path:
    GENERATED_DIR.mkdir(parents=True, exist_ok=True)
    font = TTFont(DEJAVU)
    if "post" in font:
        del font["post"]
    target = GENERATED_DIR / "no-post-names.ttf"
    font.save(target)
    return target


def main() -> None:
    no_post = build_no_post_font()
    ensure_link("fonts/control/no-kerning.ttf", NO_KERN)
    ensure_link("fonts/metrics/fixed-width.ttf", MONO)
    ensure_link("input/fonts/glyph-names/post-glyph-names.otf", DEJAVU)
    ensure_link("input/fonts/no-glyph-names/no-post-names.ttf", no_post)
    print(f"wrote {no_post.relative_to(ROOT)} without a post table")
    print("ensured 4 face-property fixture asset paths")


if __name__ == "__main__":
    main()
