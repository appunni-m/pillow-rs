#!/usr/bin/env python3
"""Build shared vertical-metrics fixtures for public API inputs."""

from __future__ import annotations

import os
from pathlib import Path

from fontTools.ttLib import TTFont, newTable


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
BASE_FONT = FIXTURE_ROOT / "input" / "fonts" / "DejaVuSans.ttf"
GENERATED = (
    FIXTURE_ROOT
    / "input"
    / "fonts"
    / "generated"
    / "vertical"
    / "cjk-vertical-metrics.ttf"
)
LINKS = [
    FIXTURE_ROOT / "input" / "fonts" / "vertical" / "cjk-vertical-metrics.ttf",
    FIXTURE_ROOT / "fonts" / "vertical" / "cjk-vertical-metrics.ttf",
]


def build_font() -> None:
    GENERATED.parent.mkdir(parents=True, exist_ok=True)
    font = TTFont(BASE_FONT)
    glyph_order = font.getGlyphOrder()

    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 1000
    vhea.descent = -1000
    vhea.lineGap = 0
    vhea.advanceHeightMax = 2048
    vhea.minTopSideBearing = 0
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 2048
    vhea.caretSlopeRise = 0
    vhea.caretSlopeRun = 1
    vhea.caretOffset = 0
    vhea.reserved1 = 0
    vhea.reserved2 = 0
    vhea.reserved3 = 0
    vhea.reserved4 = 0
    vhea.metricDataFormat = 0
    vhea.numberOfVMetrics = len(glyph_order)
    font["vhea"] = vhea

    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (2048, 0) for name in glyph_order}
    font["vmtx"] = vmtx
    font.save(GENERATED)


def relink(link: Path) -> None:
    link.parent.mkdir(parents=True, exist_ok=True)
    if link.exists() or link.is_symlink():
        link.unlink()
    link.symlink_to(os.path.relpath(GENERATED, link.parent))


def main() -> None:
    build_font()
    for link in LINKS:
        relink(link)
    print(f"wrote {GENERATED.relative_to(ROOT)}")
    print(f"ensured {len(LINKS)} vertical fixture asset paths")


if __name__ == "__main__":
    main()
