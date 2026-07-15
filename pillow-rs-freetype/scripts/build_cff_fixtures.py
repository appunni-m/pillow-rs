#!/usr/bin/env python3
"""Build compact CFF/OpenType fixtures for public metadata paths."""

from __future__ import annotations

from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "cff"
UNITS_PER_EM = 1000
FIXED_HEAD_TIME = 0
GLYPH_ORDER = [".notdef", "A"]
METRICS = {".notdef": (600, 0), "A": (600, 40)}
CUBIC_GLYPH_ORDER = [".notdef", "A", "cubic_c2_x_flatness", "cubic_c2_y_flatness"]
NAMES = {
    "familyName": "Hybrid OTTO Coverage",
    "styleName": "Regular",
    "uniqueFontIdentifier": "Hybrid OTTO Coverage Regular",
    "fullName": "Hybrid OTTO Coverage Regular",
    "psName": "HybridOTTOCoverage-Regular",
}


def t2_charstring(rectangle: bool = False, cubic: str | None = None):
    pen = T2CharStringPen(600, None)
    if cubic == "arched":
        pen.moveTo((128, 0))
        pen.curveTo((240, 900), (720, 900), (832, 0))
        pen.moveTo((128, 0))
        pen.curveTo((300, 1120), (660, 1120), (832, 0))
    elif cubic == "c2_x":
        # Exercises the third `split_sdf_cubic` flatness term via public SDF.
        pen.moveTo((0, 0))
        pen.curveTo((100, 33), (250, 66), (300, 100))
    elif cubic == "c2_y":
        # Exercises the fourth `split_sdf_cubic` flatness term via public SDF.
        pen.moveTo((0, 0))
        pen.curveTo((100, 0), (200, 80), (300, 0))
    elif rectangle:
        pen.moveTo((80, 0))
        pen.lineTo((520, 0))
        pen.lineTo((520, 700))
        pen.lineTo((80, 700))
        pen.closePath()
    return pen.getCharString()


def glyf_glyph(rectangle: bool = False):
    pen = TTGlyphPen(GLYPH_ORDER)
    if rectangle:
        pen.moveTo((80, 0))
        pen.lineTo((520, 0))
        pen.lineTo((520, 700))
        pen.lineTo((80, 700))
        pen.closePath()
    return pen.glyph()


def build_cff(path: Path) -> None:
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupHorizontalMetrics(METRICS)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(NAMES)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupCFF(
        NAMES["psName"],
        {
            "FullName": NAMES["fullName"],
            "FamilyName": NAMES["familyName"],
            "Weight": NAMES["styleName"],
        },
        {".notdef": t2_charstring(), "A": t2_charstring(rectangle=True)},
        {},
    )
    builder.setupMaxp()
    builder.save(path)


def build_cubic_cff(path: Path) -> None:
    names = {
        "familyName": "Pure CFF Cubic Coverage",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Cubic Coverage Regular",
        "fullName": "Pure CFF Cubic Coverage Regular",
        "psName": "PureCFFCubicCoverage-Regular",
    }
    metrics = {
        ".notdef": (900, 0),
        "A": (900, 128),
        "cubic_c2_x_flatness": (420, 0),
        "cubic_c2_y_flatness": (420, 0),
    }
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(CUBIC_GLYPH_ORDER)
    builder.setupCharacterMap(
        {
            0x41: "A",
            0x42: "cubic_c2_x_flatness",
            0x43: "cubic_c2_y_flatness",
        }
    )
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=1200, descent=-200)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=1200,
        sTypoDescender=-200,
        usWinAscent=1200,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {
            ".notdef": t2_charstring(),
            "A": t2_charstring(cubic="arched"),
            "cubic_c2_x_flatness": t2_charstring(cubic="c2_x"),
            "cubic_c2_y_flatness": t2_charstring(cubic="c2_y"),
        },
        {},
    )
    builder.setupMaxp()
    builder.save(path)


def build_matching_glyf(path: Path) -> None:
    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupGlyf({".notdef": glyf_glyph(), "A": glyf_glyph(rectangle=True)})
    builder.setupHorizontalMetrics(METRICS)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(NAMES)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    builder.save(path)


def write_hybrid_otto_face_info() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "hybrid-otto-face-info.otf"
    with TemporaryDirectory() as tmp:
        cff_path = Path(tmp) / "cff.otf"
        glyf_path = Path(tmp) / "glyf.ttf"
        build_cff(cff_path)
        build_matching_glyf(glyf_path)

        cff_font = TTFont(cff_path, recalcTimestamp=False)
        glyf_font = TTFont(glyf_path, recalcTimestamp=False)
        # The current Rust parser accepts OTTO SFNT wrappers but still reads
        # TrueType outline tables for metadata-only public paths.  Keeping a
        # valid CFF table lets the native FreeType oracle open the same face.
        cff_font["glyf"] = glyf_font["glyf"]
        cff_font["loca"] = glyf_font["loca"]
        cff_font["maxp"] = glyf_font["maxp"]
        cff_font["head"].created = FIXED_HEAD_TIME
        cff_font["head"].modified = FIXED_HEAD_TIME
        cff_font.sfntVersion = "OTTO"
        cff_font.recalcTimestamp = False
        if out.exists() or out.is_symlink():
            out.unlink()
        cff_font.save(out, reorderTables=True)


def write_pure_cff_cubic() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-cubic.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cubic_cff(out)


def main() -> None:
    write_hybrid_otto_face_info()
    write_pure_cff_cubic()


if __name__ == "__main__":
    main()
