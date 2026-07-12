#!/usr/bin/env python3
"""Build compact autohint fonts that exercise script-selection coverage."""

from __future__ import annotations

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphCoordinates
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "autohint"

UNITS_PER_EM = 1000

SCRIPT_PROBES: list[tuple[str, int]] = [
    ("adlm", 0x1E90C),
    ("arab", 0x0627),
    ("armn", 0x0531),
    ("avst", 0x10B00),
    ("bamu", 0xA6A7),
    ("beng", 0x0987),
    ("buhd", 0x1750),
    ("cakm", 0x11103),
    ("cans", 0x15DC),
    ("cari", 0x102A7),
    ("cher", 0x13C6),
    ("copt", 0x2C8C),
    ("cprt", 0x1080D),
    ("cyrl", 0x0411),
    ("deva", 0x0908),
    ("dsrt", 0x10402),
    ("ethi", 0x1200),
    ("geor", 0x10D2),
    ("geok", 0x10B1),
    ("glag", 0x2C05),
    ("goth", 0x10332),
    ("grek", 0x0393),
    ("gujr", 0x0AA4),
    ("guru", 0x0A07),
    ("hebr", 0x05D1),
    ("kali", 0xA905),
    ("khmr", 0x1781),
    ("khms", 0x19E0),
    ("knda", 0x0C87),
    ("lao", 0x0EB2),
    ("latb", 0x2080),
    ("latp", 0x2070),
    ("latn", 0x006F),
    ("limb", 0x1900),
    ("lisu", 0xA4E1),
    ("mlym", 0x0D12),
    ("medf", 0x16E40),
    ("mong", 0x1833),
    ("mymr", 0x1001),
    ("nkoo", 0x07D0),
    ("olck", 0x1C5B),
    ("orkh", 0x10C17),
    ("orya", 0x0B13),
    ("osge", 0x104BE),
    ("osma", 0x10486),
    ("rohg", 0x10D03),
    ("saur", 0xA89C),
    ("shaw", 0x10455),
    ("sinh", 0x0D89),
    ("sund", 0x1B8B),
    ("sylo", 0xA807),
    ("taml", 0x0B89),
    ("tavt", 0xAA86),
    ("telu", 0x0C07),
    ("tfng", 0x2D54),
    ("thai", 0x0E1A),
    ("tibt", 0x0F40),
    ("vaii", 0xA5CD),
    ("hani", 0x4ED6),
]

# ASCII digits are global autohinter metrics probes, not script probes.  Keep
# them after the script glyphs so existing fixture glyph indices remain stable.
DIGIT_WIDTH_PROBES: list[tuple[str, int, int]] = [
    ("digit_zero_wide", 0x0030, 620),
    ("digit_one_narrow", 0x0031, 520),
]

STANDARD_CHARS: dict[str, int] = {
    "adlm": 0x1E90C,
    "arab": 0x0644,
    "armn": 0x057D,
    "avst": 0x10B1A,
    "bamu": 0xA6C1,
    "beng": 0x09E6,
    "buhd": 0x174B,
    "cakm": 0x11124,
    "cans": 0x144C,
    "cari": 0x102AB,
    "cher": 0x13A4,
    "copt": 0x2C9E,
    "cprt": 0x10805,
    "cyrl": 0x043E,
    "deva": 0x0920,
    "dsrt": 0x10404,
    "ethi": 0x12D0,
    "geok": 0x10B6,
    "geor": 0x10D8,
    "glag": 0x2C15,
    "goth": 0x10334,
    "grek": 0x03BF,
    "gujr": 0x0A9F,
    "guru": 0x0A20,
    "hani": 0x7530,
    "hebr": 0x05DD,
    "kali": 0xA90D,
    "khmr": 0x17E0,
    "khms": 0x19E1,
    "knda": 0x0CE6,
    "lao": 0x0ED0,
    "latb": 0x2092,
    "latn": 0x006F,
    "latp": 0x1D52,
    "limb": 0x006F,
    "lisu": 0xA4F3,
    "medf": 0x16E61,
    "mlym": 0x0D20,
    "mong": 0x1842,
    "mymr": 0x101D,
    "nkoo": 0x07CB,
    "olck": 0x1C5B,
    "orkh": 0x10C17,
    "orya": 0x006F,
    "osge": 0x104C2,
    "osma": 0x10486,
    "rohg": 0x10D30,
    "saur": 0xA89D,
    "shaw": 0x10474,
    "sinh": 0x0DA7,
    "sund": 0x1BB0,
    "sylo": 0x006F,
    "taml": 0x0BE6,
    "tavt": 0xAA92,
    "telu": 0x0C66,
    "tfng": 0x2D54,
    "thai": 0x0E32,
    "tibt": 0x006F,
    "vaii": 0xA613,
}


def empty_glyph():
    return TTGlyphPen(None).glyph()


def rectangle_glyph(left: int, bottom: int, right: int, top: int):
    pen = TTGlyphPen(None)
    pen.moveTo((left, bottom))
    pen.lineTo((left, top))
    pen.lineTo((right, top))
    pen.lineTo((right, bottom))
    pen.closePath()
    return pen.glyph()


def rectangles_glyph(rects: list[tuple[int, int, int, int]]):
    pen = TTGlyphPen(None)
    for left, bottom, right, top in rects:
        pen.moveTo((left, bottom))
        pen.lineTo((left, top))
        pen.lineTo((right, top))
        pen.lineTo((right, bottom))
        pen.closePath()
    return pen.glyph()


def ring_glyph(
    left: int,
    bottom: int,
    right: int,
    top: int,
    inset_left: int,
    inset_bottom: int,
    inset_right: int,
    inset_top: int,
):
    pen = TTGlyphPen(None)
    mid_x = (left + right) // 2
    mid_y = (bottom + top) // 2
    pen.moveTo((mid_x, bottom))
    pen.qCurveTo((right, bottom), (right, mid_y))
    pen.qCurveTo((right, top), (mid_x, top))
    pen.qCurveTo((left, top), (left, mid_y))
    pen.qCurveTo((left, bottom), (mid_x, bottom))
    pen.closePath()

    inset_mid_x = (inset_left + inset_right) // 2
    inset_mid_y = (inset_bottom + inset_top) // 2
    pen.moveTo((inset_mid_x, inset_bottom))
    pen.qCurveTo((inset_left, inset_bottom), (inset_left, inset_mid_y))
    pen.qCurveTo((inset_left, inset_top), (inset_mid_x, inset_top))
    pen.qCurveTo((inset_right, inset_top), (inset_right, inset_mid_y))
    pen.qCurveTo((inset_right, inset_bottom), (inset_mid_x, inset_bottom))
    pen.closePath()
    return pen.glyph()


def one_point_contour_glyph(points: list[tuple[int, int]]):
    glyph = Glyph()
    glyph.numberOfContours = len(points)
    glyph.coordinates = GlyphCoordinates(points)
    glyph.endPtsOfContours = list(range(len(points)))
    glyph.flags = bytearray([1] * len(points))
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = min(x for x, _ in points)
    glyph.xMax = max(x for x, _ in points)
    glyph.yMin = min(y for _, y in points)
    glyph.yMax = max(y for _, y in points)
    return glyph


def glyph_name(tag: str) -> str:
    return f"script_{tag}"


def build_script_coverage() -> None:
    glyph_order = [".notdef", "space"]
    glyph_order.extend(glyph_name(tag) for tag, _ in SCRIPT_PROBES)
    glyph_order.extend(name for name, _, _ in DIGIT_WIDTH_PROBES)

    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
    }
    cmap = {0x20: "space"}

    for index, (tag, codepoint) in enumerate(SCRIPT_PROBES):
        name = glyph_name(tag)
        width = 500 + (index % 5) * 20
        top = 480 + (index % 7) * 24
        left = 70 + (index % 3) * 10
        glyphs[name] = rectangle_glyph(left, 0, left + width, top)
        metrics[name] = (700, left)
        cmap[codepoint] = name
        standard = STANDARD_CHARS.get(tag)
        if standard is not None:
            cmap.setdefault(standard, name)

    for name, codepoint, advance in DIGIT_WIDTH_PROBES:
        glyphs[name] = rectangle_glyph(100, 0, 440, 560)
        metrics[name] = (advance, 100)
        cmap[codepoint] = name

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Script Coverage",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Script Coverage Regular",
            "fullName": "Autohint Script Coverage Regular",
            "psName": "AutohintScriptCoverage-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "script-coverage.ttf")


def build_cjk_empty_standard() -> None:
    glyph_order = [".notdef", "space", "hani_empty"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_empty": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_empty": (700, 0),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_empty",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Empty Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Empty Standard Regular",
            "fullName": "Autohint CJK Empty Standard Regular",
            "psName": "AutohintCJKEmptyStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-empty-standard.ttf")


def build_cjk_blue_edge_cases() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "blue_empty",
        "blue_degenerate",
        "top_flat",
        "bottom_fill",
        "bottom_flat",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        "blue_empty": empty_glyph(),
        "blue_degenerate": one_point_contour_glyph([(160, 40), (260, 120), (360, 200)]),
        "top_flat": rectangle_glyph(110, 20, 580, 220),
        "bottom_fill": rectangle_glyph(120, 0, 560, 360),
        "bottom_flat": rectangle_glyph(120, -80, 560, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "blue_empty": (700, 0),
        "blue_degenerate": (700, 160),
        "top_flat": (700, 110),
        "bottom_fill": (700, 120),
        "bottom_flat": (700, 120),
    }
    cmap = {
        0x20: "space",
        0x4E2A: "bottom_fill",
        0x4E3B: "bottom_flat",
        0x4ED6: "blue_empty",
        0x4EEC: "blue_degenerate",
        0x519B: "top_flat",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Blue Edge Cases",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Blue Edge Cases Regular",
            "fullName": "Autohint CJK Blue Edge Cases Regular",
            "psName": "AutohintCJKBlueEdgeCases-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-blue-edge-cases.ttf")


def build_cjk_tiny_stem() -> None:
    glyph_order = [".notdef", "space", "hani_tiny_stem"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_tiny_stem": rectangle_glyph(100, 0, 120, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_tiny_stem": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_tiny_stem",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Tiny Stem",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Tiny Stem Regular",
            "fullName": "Autohint CJK Tiny Stem Regular",
            "psName": "AutohintCJKTinyStem-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-tiny-stem.ttf")


def build_cjk_snap_below_standard() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_snap_below"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_snap_below": rectangle_glyph(100, 0, 190, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_snap_below": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_snap_below",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Snap Below Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Snap Below Standard Regular",
            "fullName": "Autohint CJK Snap Below Standard Regular",
            "psName": "AutohintCJKSnapBelowStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-snap-below-standard.ttf")


def build_cjk_round_stem_light() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_round_ring"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_round_ring": ring_glyph(80, 20, 520, 460, 180, 120, 420, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_round_ring": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x51A2: "hani_round_ring",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Round Stem Light",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Round Stem Light Regular",
            "fullName": "Autohint CJK Round Stem Light Regular",
            "psName": "AutohintCJKRoundStemLight-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-round-stem-light.ttf")


def build_cjk_duplicate_edge() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_duplicate_edge"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_duplicate_edge": rectangles_glyph(
            [
                (40, 20, 80, 220),
                (40, 260, 320, 460),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_duplicate_edge": (700, 40),
    }
    cmap = {
        0x20: "space",
        0x519E: "hani_duplicate_edge",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Duplicate Edge",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Duplicate Edge Regular",
            "fullName": "Autohint CJK Duplicate Edge Regular",
            "psName": "AutohintCJKDuplicateEdge-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-duplicate-edge.ttf")


def main() -> None:
    build_script_coverage()
    build_cjk_empty_standard()
    build_cjk_blue_edge_cases()
    build_cjk_tiny_stem()
    build_cjk_snap_below_standard()
    build_cjk_round_stem_light()
    build_cjk_duplicate_edge()


if __name__ == "__main__":
    main()
