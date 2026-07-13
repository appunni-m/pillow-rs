#!/usr/bin/env python3
"""Build compact render-path coverage fixtures."""

from __future__ import annotations

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "glyf"

UNITS_PER_EM = 1024
GLYPH_ORDER = [".notdef", "horizontal_dropout_guard"]


def empty_glyph():
    return TTGlyphPen(None).glyph()


def horizontal_dropout_guard_glyph():
    pen = TTGlyphPen(None)

    # At 16 ppem with 1024 UPEM, design units are 26.6 coordinates.
    # The vertical lines set row 1 before the horizontal mono dropout pass.
    for x in (48, 112):
        pen.moveTo((x, 0))
        pen.lineTo((x, 192))
        pen.closePath()

    pen.moveTo((0, 64))
    pen.lineTo((192, 64))
    pen.lineTo((192, 80))
    pen.lineTo((0, 80))
    pen.closePath()

    return pen.glyph()


def build_render_coverage() -> None:
    glyphs = {
        ".notdef": empty_glyph(),
        "horizontal_dropout_guard": horizontal_dropout_guard_glyph(),
    }
    metrics = {
        ".notdef": (256, 0),
        "horizontal_dropout_guard": (256, 0),
    }

    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0xE100: "horizontal_dropout_guard"})
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=256, descent=0)
    builder.setupNameTable(
        {
            "familyName": "Render Coverage",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Render Coverage Regular",
            "fullName": "Render Coverage Regular",
            "psName": "RenderCoverage-Regular",
            "version": "Version 1.0",
        }
    )
    builder.setupOS2(
        sTypoAscender=256,
        sTypoDescender=0,
        usWinAscent=256,
        usWinDescent=0,
    )
    builder.setupPost()
    builder.setupMaxp()

    head = builder.font["head"]
    head.created = 0
    head.modified = 0
    builder.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    builder.save(OUT_DIR / "render-coverage.ttf")


def main() -> None:
    build_render_coverage()


if __name__ == "__main__":
    main()
