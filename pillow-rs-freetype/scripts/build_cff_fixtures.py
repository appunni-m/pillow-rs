#!/usr/bin/env python3
"""Build compact CFF/OpenType fixtures for public metadata paths."""

from __future__ import annotations

from array import array
from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.fontBuilder import FontBuilder
from fontTools.cffLib import TopDict
from fontTools.misc.psCharStrings import T2CharString
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "cff"
UNITS_PER_EM = 1000
FIXED_HEAD_TIME = 0
GLYPH_ORDER = [".notdef", "A"]
METRICS = {".notdef": (600, 0), "A": (600, 40)}
CUBIC_GLYPH_ORDER = [
    ".notdef",
    "A",
    "cubic_c2_x_flatness",
    "cubic_c2_y_flatness",
    "vertical_lines",
    "relative_lines",
    "vh_curve",
    "hv_curve_no_last_delta",
    "vh_curve_no_last_delta",
    "hmoveto_default_width",
    "vmoveto_default_width",
    "rmoveto_default_width",
    "endchar_default_width",
    "hvcurveto_initial_width",
    "fixed_hmoveto",
    "rlineto_initial_width",
    "rrcurveto_initial_width",
    "hlineto_missing_args",
    "hmoveto_missing_args",
    "vmoveto_missing_args",
    "rmoveto_missing_args",
    "hvcurveto_missing_args",
    "hvcurveto_trailing_args",
    "type2_escape_unsupported",
    "type2_op_unsupported",
    "type2_shortint_overflow",
    "rlineto_missing_args",
    "rrcurveto_missing_args",
    "type2_positive_overflow",
    "type2_negative_overflow",
    "type2_shortint_hmoveto",
    "type2_no_endchar_eof",
    "rlineto_secondary_malformed",
    "rrcurveto_secondary_malformed",
    "tiny_cubic_y_span",
    "flat_cubic_y_span",
    "moveto_endchar_empty_contour",
    "repeated_moveto_empty_contours",
    "explicit_close_point",
    "same_x_open_contour",
]
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
    elif cubic == "tiny_y":
        # At 24 ppem this remains below one scanline after scaling, which
        # reaches FreeType black rasterizer's Bezier_Up early span rejection.
        pen.moveTo((0, 0))
        pen.curveTo((100, 4), (200, 8), (300, 12))
    elif rectangle:
        pen.moveTo((80, 0))
        pen.lineTo((520, 0))
        pen.lineTo((520, 700))
        pen.lineTo((80, 700))
        pen.closePath()
    return pen.getCharString()


def t2_program_charstring(program: list[object]) -> T2CharString:
    return T2CharString(program=program, private=None, globalSubrs=[])


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


def build_cubic_cff(path: Path, with_vertical_metrics: bool = False) -> None:
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
        "vertical_lines": (420, 0),
        "relative_lines": (420, 0),
        "vh_curve": (420, 0),
        "hv_curve_no_last_delta": (420, 0),
        "vh_curve_no_last_delta": (420, 0),
        "hmoveto_default_width": (420, 0),
        "vmoveto_default_width": (420, 0),
        "rmoveto_default_width": (420, 0),
        "endchar_default_width": (420, 0),
        "hvcurveto_initial_width": (420, 0),
        "fixed_hmoveto": (420, 0),
        "rlineto_initial_width": (420, 0),
        "rrcurveto_initial_width": (420, 0),
        "hlineto_missing_args": (420, 0),
        "hmoveto_missing_args": (420, 0),
        "vmoveto_missing_args": (420, 0),
        "rmoveto_missing_args": (420, 0),
        "hvcurveto_missing_args": (420, 0),
        "hvcurveto_trailing_args": (420, 0),
        "type2_escape_unsupported": (420, 0),
        "type2_op_unsupported": (420, 0),
        "type2_shortint_overflow": (420, 0),
        "rlineto_missing_args": (420, 0),
        "rrcurveto_missing_args": (420, 0),
        "type2_positive_overflow": (420, 0),
        "type2_negative_overflow": (420, 0),
        "type2_shortint_hmoveto": (420, 0),
        "type2_no_endchar_eof": (420, 0),
        "rlineto_secondary_malformed": (420, 0),
        "rrcurveto_secondary_malformed": (420, 0),
        "tiny_cubic_y_span": (420, 0),
        "flat_cubic_y_span": (420, 0),
        "moveto_endchar_empty_contour": (420, 0),
        "repeated_moveto_empty_contours": (420, 0),
        "explicit_close_point": (420, 0),
        "same_x_open_contour": (420, 0),
    }
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(CUBIC_GLYPH_ORDER)
    builder.setupCharacterMap(
        {
            0x41: "A",
            0x42: "cubic_c2_x_flatness",
            0x43: "cubic_c2_y_flatness",
            0x44: "vertical_lines",
            0x45: "relative_lines",
            0x46: "vh_curve",
            0x47: "hv_curve_no_last_delta",
            0x48: "vh_curve_no_last_delta",
            0x49: "hmoveto_default_width",
            0x4A: "vmoveto_default_width",
            0x4B: "rmoveto_default_width",
            0x4C: "endchar_default_width",
            0x4D: "hvcurveto_initial_width",
            0x4E: "fixed_hmoveto",
            0x4F: "rlineto_initial_width",
            0x50: "rrcurveto_initial_width",
            0x51: "hlineto_missing_args",
            0x52: "hmoveto_missing_args",
            0x53: "vmoveto_missing_args",
            0x54: "rmoveto_missing_args",
            0x55: "hvcurveto_missing_args",
            0x56: "hvcurveto_trailing_args",
            0x57: "type2_escape_unsupported",
            0x58: "type2_op_unsupported",
            0x59: "type2_shortint_overflow",
            0x5A: "rlineto_missing_args",
            0x5B: "rrcurveto_missing_args",
            0x5C: "type2_positive_overflow",
            0x5D: "type2_negative_overflow",
            0x5E: "type2_shortint_hmoveto",
            0x5F: "type2_no_endchar_eof",
            0x60: "rlineto_secondary_malformed",
            0x61: "rrcurveto_secondary_malformed",
            0x62: "tiny_cubic_y_span",
            0x63: "flat_cubic_y_span",
            0x64: "moveto_endchar_empty_contour",
            0x65: "repeated_moveto_empty_contours",
            0x66: "explicit_close_point",
            0x67: "same_x_open_contour",
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
            "vertical_lines": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    200,
                    "vlineto",
                    100,
                    "hlineto",
                    -200,
                    "vlineto",
                    "endchar",
                ]
            ),
            "relative_lines": t2_program_charstring(
                [
                    600,
                    0,
                    60,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "vh_curve": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    80,
                    "vhcurveto",
                    "endchar",
                ]
            ),
            "hv_curve_no_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "vh_curve_no_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    "vhcurveto",
                    "endchar",
                ]
            ),
            "hmoveto_default_width": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    100,
                    100,
                    100,
                    -100,
                    -100,
                    -100,
                    "rlineto",
                    "endchar",
                ]
            ),
            "vmoveto_default_width": t2_program_charstring(
                [
                    100,
                    "vmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rmoveto_default_width": t2_program_charstring(
                [
                    0,
                    60,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "endchar_default_width": t2_program_charstring(
                [
                    "endchar",
                ]
            ),
            "hvcurveto_initial_width": t2_program_charstring(
                [
                    600,
                    100,
                    50,
                    60,
                    70,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "fixed_hmoveto": t2_program_charstring(
                [
                    600,
                    1.5,
                    "hmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rlineto_initial_width": t2_program_charstring(
                [
                    600,
                    80,
                    0,
                    0,
                    100,
                    -80,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_initial_width": t2_program_charstring(
                [
                    600,
                    60,
                    0,
                    60,
                    100,
                    120,
                    0,
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "hlineto_missing_args": t2_program_charstring(
                [
                    "hlineto",
                    "endchar",
                ]
            ),
            "hmoveto_missing_args": t2_program_charstring(
                [
                    "hmoveto",
                    "endchar",
                ]
            ),
            "vmoveto_missing_args": t2_program_charstring(
                [
                    "vmoveto",
                    "endchar",
                ]
            ),
            "rmoveto_missing_args": t2_program_charstring(
                [
                    "rmoveto",
                    "endchar",
                ]
            ),
            "hvcurveto_missing_args": t2_program_charstring(
                [
                    10,
                    20,
                    30,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "hvcurveto_trailing_args": t2_program_charstring(
                [
                    10,
                    20,
                    30,
                    40,
                    50,
                    60,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "type2_escape_unsupported": T2CharString(
                bytecode=bytes([12, 0, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_op_unsupported": T2CharString(
                bytecode=bytes([10, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_shortint_overflow": T2CharString(
                bytecode=bytes([28]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "rlineto_missing_args": t2_program_charstring(
                [
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_missing_args": t2_program_charstring(
                [
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "type2_positive_overflow": T2CharString(
                bytecode=bytes([247]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_negative_overflow": T2CharString(
                bytecode=bytes([251]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_shortint_hmoveto": T2CharString(
                bytecode=bytes([28, 0, 0, 22, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_no_endchar_eof": T2CharString(
                bytecode=bytes([139, 22]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "rlineto_secondary_malformed": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    10,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_secondary_malformed": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    10,
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "tiny_cubic_y_span": t2_charstring(cubic="tiny_y"),
            "flat_cubic_y_span": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    100,
                    0,
                    100,
                    0,
                    100,
                    0,
                    "rrcurveto",
                    0,
                    120,
                    -300,
                    0,
                    0,
                    -120,
                    "rlineto",
                    "endchar",
                ]
            ),
            "moveto_endchar_empty_contour": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    "endchar",
                ]
            ),
            "repeated_moveto_empty_contours": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    100,
                    "hmoveto",
                    "endchar",
                ]
            ),
            "explicit_close_point": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    0,
                    -100,
                    "rlineto",
                    "endchar",
                ]
            ),
            "same_x_open_contour": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    0,
                    100,
                    "rlineto",
                    "endchar",
                ]
            ),
        },
        {},
    )
    builder.setupMaxp()
    if with_vertical_metrics:
        add_vertical_metrics(builder.font)
    recalc_font_bbox = TopDict.recalcFontBBox
    try:
        # fontTools' bounds walker treats these `rlineto` and `rrcurveto`
        # programs as malformed because they intentionally begin with an odd
        # operand count and no moveto.  FreeType reaches them through real
        # public glyph loads and rejects them, so keep the raw charstrings and
        # preserve the explicit compact fixture bbox.
        TopDict.recalcFontBBox = lambda self: None
        builder.font.recalcBBoxes = False
        builder.font["head"].created = FIXED_HEAD_TIME
        builder.font["head"].modified = FIXED_HEAD_TIME
        builder.font["CFF "].cff.topDictIndex[0].FontBBox = [0, 0, 900, 1200]
        builder.save(path)
    finally:
        TopDict.recalcFontBBox = recalc_font_bbox


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


def add_vertical_metrics(font: TTFont) -> None:
    glyph_order = font.getGlyphOrder()
    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (880, 120) for name in glyph_order}
    font["vmtx"] = vmtx

    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 760
    vhea.descent = -120
    vhea.lineGap = 0
    vhea.advanceHeightMax = 880
    vhea.minTopSideBearing = 120
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 880
    vhea.caretSlopeRise = 1
    vhea.caretSlopeRun = 0
    vhea.caretOffset = 0
    vhea.reserved1 = 0
    vhea.reserved2 = 0
    vhea.reserved3 = 0
    vhea.reserved4 = 0
    vhea.metricDataFormat = 0
    vhea.numberOfVMetrics = len(glyph_order)
    font["vhea"] = vhea


def write_pure_cff_cubic_vmtx() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-cubic-vmtx.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cubic_cff(out, with_vertical_metrics=True)


def empty_program_table(tag: str):
    table = newTable(tag)
    table.program = Program()
    table.program.fromBytecode([])
    return table


def write_pure_cff_empty_tt_programs() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-empty-tt-programs.otf"
    with TemporaryDirectory() as tmp:
        cff_path = Path(tmp) / "pure-cff-cubic.otf"
        build_cubic_cff(cff_path)
        font = TTFont(cff_path, recalcTimestamp=False)
        # This deliberately odd OTTO face keeps CFF outlines while carrying
        # empty TrueType program tables.  It exercises the scaler's public CFF
        # metrics route without giving the TrueType VM any executable work.
        font["fpgm"] = empty_program_table("fpgm")
        font["prep"] = empty_program_table("prep")
        cvt = newTable("cvt ")
        cvt.values = array("h")
        font["cvt "] = cvt
        font["head"].created = FIXED_HEAD_TIME
        font["head"].modified = FIXED_HEAD_TIME
        font.recalcTimestamp = False
        if out.exists() or out.is_symlink():
            out.unlink()
        font.save(out, reorderTables=True)


def sfnt_checksum(data: bytes) -> int:
    padded = data + b"\0" * ((4 - len(data) % 4) % 4)
    return sum(int.from_bytes(padded[i : i + 4], "big") for i in range(0, len(padded), 4)) & 0xFFFFFFFF


def replace_sfnt_table(source: Path, dest: Path, tag: bytes, payload: bytes) -> None:
    data = bytearray(source.read_bytes())
    num_tables = int.from_bytes(data[4:6], "big")
    for index in range(num_tables):
        record = 12 + index * 16
        if bytes(data[record : record + 4]) != tag:
            continue
        offset = int.from_bytes(data[record + 8 : record + 12], "big")
        old_length = int.from_bytes(data[record + 12 : record + 16], "big")
        if len(payload) > old_length:
            raise ValueError(f"{tag!r} replacement is larger than source table")
        data[offset : offset + len(payload)] = payload
        data[offset + len(payload) : offset + old_length] = b"\0" * (old_length - len(payload))
        data[record + 4 : record + 8] = sfnt_checksum(payload).to_bytes(4, "big")
        data[record + 12 : record + 16] = len(payload).to_bytes(4, "big")
        if dest.exists() or dest.is_symlink():
            dest.unlink()
        dest.write_bytes(data)
        return
    raise ValueError(f"missing table {tag!r} in {source}")


def cff_index(objects: list[bytes]) -> bytes:
    if not objects:
        return b"\0\0"
    offsets = [1]
    cursor = 1
    for item in objects:
        cursor += len(item)
        offsets.append(cursor)
    return (
        len(objects).to_bytes(2, "big")
        + b"\x01"
        + bytes(offsets)
        + b"".join(objects)
    )


def malformed_cff_payload(kind: str) -> bytes:
    header = b"\x01\x00\x04\x04"
    match kind:
        case "short_header":
            return b"\x01\x00\x04"
        case "invalid_name_index_offsize":
            return header + b"\x00\x01\x00"
        case "name_index_offsets_out_of_order":
            return header + b"\x00\x01\x01\x02\x01"
        case "escaped_top_dict_op_overflow":
            return header + cff_index([]) + cff_index([b"\x0C"]) + cff_index([]) + cff_index([])
        case _:
            raise ValueError(f"unknown malformed CFF fixture kind {kind}")


def write_malformed_cff_faces() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base.otf"
        build_cubic_cff(base)
        for kind in [
            "short_header",
            "invalid_name_index_offsize",
            "name_index_offsets_out_of_order",
            "escaped_top_dict_op_overflow",
        ]:
            replace_sfnt_table(
                base,
                OUT_DIR / f"malformed-{kind.replace('_', '-')}.otf",
                b"CFF ",
                malformed_cff_payload(kind),
            )


def main() -> None:
    write_hybrid_otto_face_info()
    write_pure_cff_cubic()
    write_pure_cff_cubic_vmtx()
    write_pure_cff_empty_tt_programs()
    write_malformed_cff_faces()


if __name__ == "__main__":
    main()
