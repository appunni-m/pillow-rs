#!/usr/bin/env python3
"""Build compact render-path coverage fixtures."""

from __future__ import annotations

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "glyf"

UNITS_PER_EM = 1024
GLYPH_ORDER = [
    ".notdef",
    "horizontal_dropout_guard",
    "vertical_dropout_guard",
    "conic_bbox_extrema",
    "subpixel_short_box",
    "folded_profile_dropout",
]


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


def vertical_dropout_guard_glyph():
    pen = TTGlyphPen(None)

    # Mirror the horizontal guard so the normal mono profile sweep sees the
    # thin vertical dropout after already-set pixels in the same scan row.
    for y in (48, 112):
        pen.moveTo((0, y))
        pen.lineTo((192, y))
        pen.closePath()

    pen.moveTo((64, 0))
    pen.lineTo((80, 0))
    pen.lineTo((80, 192))
    pen.lineTo((64, 192))
    pen.closePath()

    return pen.glyph()


def conic_bbox_extrema_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((128, 0))
    pen.qCurveTo((-128, 768), (512, 0))
    pen.closePath()

    return pen.glyph()


def subpixel_short_box_glyph():
    pen = TTGlyphPen(None)

    # At 16 ppem this is one 26.6 unit tall: enough to create mono profile
    # state, but below the first drawable scan center in the normal sweep.
    pen.moveTo((0, 0))
    pen.lineTo((128, 0))
    pen.lineTo((128, 1))
    pen.lineTo((0, 1))
    pen.closePath()

    return pen.glyph()


def folded_profile_dropout_glyph():
    pen = TTGlyphPen(None)

    # Four one-row vertical profiles in one folded contour make the mono
    # dropout pass compare same-contour profiles that are not adjacent in order.
    pen.moveTo((0, 32))
    pen.lineTo((0, 48))
    pen.lineTo((48, 48))
    pen.lineTo((48, 32))
    pen.lineTo((32, 32))
    pen.lineTo((32, 48))
    pen.lineTo((16, 48))
    pen.lineTo((16, 32))
    pen.closePath()

    return pen.glyph()


def build_render_coverage() -> None:
    glyphs = {
        ".notdef": empty_glyph(),
        "horizontal_dropout_guard": horizontal_dropout_guard_glyph(),
        "vertical_dropout_guard": vertical_dropout_guard_glyph(),
        "conic_bbox_extrema": conic_bbox_extrema_glyph(),
        "subpixel_short_box": subpixel_short_box_glyph(),
        "folded_profile_dropout": folded_profile_dropout_glyph(),
    }
    metrics = {
        ".notdef": (256, 0),
        "horizontal_dropout_guard": (256, 0),
        "vertical_dropout_guard": (256, 0),
        "conic_bbox_extrema": (640, 0),
        "subpixel_short_box": (256, 0),
        "folded_profile_dropout": (256, 0),
    }

    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap(
        {
            0xE100: "horizontal_dropout_guard",
            0xE101: "vertical_dropout_guard",
            0xE102: "conic_bbox_extrema",
            0xE103: "subpixel_short_box",
            0xE104: "folded_profile_dropout",
        }
    )
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
