#!/usr/bin/env python3
"""Build compact render-path coverage fixtures."""

from __future__ import annotations

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.ttProgram import Program


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
    "overlap_simple_flag",
    "overlap_compound_flag",
    "overlap_heavy_flag",
    "overlap_wide_overflow_flag",
    "sdf_zero_length_segment",
    "sdf_centerline_segment",
    "mono_left_edge_dropout",
    "mono_bottom_edge_dropout",
    "sdf_flat_horizontal_segment",
    "sdf_flat_vertical_segment",
    "sdf_large_interior",
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


def overlap_simple_flag_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((0, 0))
    pen.lineTo((192, 0))
    pen.lineTo((192, 192))
    pen.lineTo((0, 192))
    pen.closePath()

    glyph = pen.glyph()
    glyph.flags[0] |= 0x40
    return glyph


def overlap_compound_flag_glyph():
    pen = TTGlyphPen({"subpixel_short_box": None})
    pen.addComponent("subpixel_short_box", (1, 0, 0, 1, 0, 0))

    glyph = pen.glyph()
    glyph.components[0].flags |= 0x0400
    return glyph


def overlap_heavy_flag_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((0, 0))
    pen.lineTo((224, 0))
    pen.lineTo((224, 224))
    pen.lineTo((0, 224))
    pen.closePath()

    pen.moveTo((96, 32))
    pen.lineTo((320, 32))
    pen.lineTo((320, 256))
    pen.lineTo((96, 256))
    pen.closePath()

    glyph = pen.glyph()
    glyph.flags[0] |= 0x40
    return glyph


def overlap_wide_overflow_flag_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((0, 0))
    pen.lineTo((9000, 0))
    pen.lineTo((9000, 4))
    pen.lineTo((0, 4))
    pen.closePath()

    glyph = pen.glyph()
    glyph.flags[0] |= 0x40
    return glyph


def sdf_zero_length_segment_glyph():
    pen = TTGlyphPen(None)

    # Keep a zero-length line segment in a non-empty outline so public SDF
    # rendering reaches FreeType's degenerate segment path.
    pen.moveTo((0, 0))
    pen.lineTo((0, 0))
    pen.lineTo((192, 0))
    pen.lineTo((0, 192))
    pen.closePath()

    return pen.glyph()


def sdf_centerline_segment_glyph():
    pen = TTGlyphPen(None)

    # At 16 ppem this first edge lies on a pixel-center scanline after the
    # SDF renderer applies its spread translation.
    pen.moveTo((32, 32))
    pen.lineTo((160, 32))
    pen.lineTo((32, 160))
    pen.closePath()

    return pen.glyph()


def sdf_flat_horizontal_segment_glyph():
    pen = TTGlyphPen(None)

    # Keep a non-empty outline whose control box collapses vertically so the
    # SDF renderer takes its zero-rows early return on a public glyph route.
    pen.moveTo((0, 0))
    pen.lineTo((192, 0))
    pen.lineTo((96, 0))
    pen.closePath()

    return pen.glyph()


def sdf_flat_vertical_segment_glyph():
    pen = TTGlyphPen(None)

    # Mirror the horizontal degenerate SDF case: the outline is non-empty, but
    # the normal preset control box has zero width before SDF spread padding.
    pen.moveTo((0, 0))
    pen.lineTo((0, 192))
    pen.lineTo((0, 96))
    pen.closePath()

    return pen.glyph()


def sdf_large_interior_glyph():
    pen = TTGlyphPen(None)

    # Large enough at 16 ppem for the SDF renderer to emit pixels whose
    # interior signed distance exceeds the renderer spread, reaching
    # FreeType's negative-distance saturation path.
    pen.moveTo((0, 0))
    pen.lineTo((1024, 0))
    pen.lineTo((1024, 1024))
    pen.lineTo((0, 1024))
    pen.closePath()

    return pen.glyph()


def mono_left_edge_dropout_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((0, 0))
    pen.lineTo((16, 0))
    pen.lineTo((16, 192))
    pen.lineTo((0, 192))
    pen.closePath()

    return pen.glyph()


def mono_bottom_edge_dropout_glyph():
    pen = TTGlyphPen(None)

    pen.moveTo((0, 0))
    pen.lineTo((192, 0))
    pen.lineTo((192, 16))
    pen.lineTo((0, 16))
    pen.closePath()

    return pen.glyph()


def build_render_coverage_font(name: str, notdef_glyph=None, ascender: int = 256) -> None:
    glyphs = {
        ".notdef": notdef_glyph or empty_glyph(),
        "horizontal_dropout_guard": horizontal_dropout_guard_glyph(),
        "vertical_dropout_guard": vertical_dropout_guard_glyph(),
        "conic_bbox_extrema": conic_bbox_extrema_glyph(),
        "subpixel_short_box": subpixel_short_box_glyph(),
        "folded_profile_dropout": folded_profile_dropout_glyph(),
        "overlap_simple_flag": overlap_simple_flag_glyph(),
        "overlap_compound_flag": overlap_compound_flag_glyph(),
        "overlap_heavy_flag": overlap_heavy_flag_glyph(),
        "overlap_wide_overflow_flag": overlap_wide_overflow_flag_glyph(),
        "sdf_zero_length_segment": sdf_zero_length_segment_glyph(),
        "sdf_centerline_segment": sdf_centerline_segment_glyph(),
        "sdf_flat_horizontal_segment": sdf_flat_horizontal_segment_glyph(),
        "sdf_flat_vertical_segment": sdf_flat_vertical_segment_glyph(),
        "sdf_large_interior": sdf_large_interior_glyph(),
        "mono_left_edge_dropout": mono_left_edge_dropout_glyph(),
        "mono_bottom_edge_dropout": mono_bottom_edge_dropout_glyph(),
    }
    metrics = {
        ".notdef": (256, 0),
        "horizontal_dropout_guard": (256, 0),
        "vertical_dropout_guard": (256, 0),
        "conic_bbox_extrema": (640, 0),
        "subpixel_short_box": (256, 0),
        "folded_profile_dropout": (256, 0),
        "overlap_simple_flag": (256, 0),
        "overlap_compound_flag": (256, 0),
        "overlap_heavy_flag": (384, 0),
        "overlap_wide_overflow_flag": (9216, 0),
        "sdf_zero_length_segment": (256, 0),
        "sdf_centerline_segment": (256, 0),
        "sdf_flat_horizontal_segment": (256, 0),
        "sdf_flat_vertical_segment": (256, 0),
        "sdf_large_interior": (1024, 0),
        "mono_left_edge_dropout": (256, 0),
        "mono_bottom_edge_dropout": (256, 0),
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
            0xE105: "overlap_simple_flag",
            0xE106: "overlap_compound_flag",
            0xE107: "overlap_heavy_flag",
            0xE108: "overlap_wide_overflow_flag",
            0xE109: "sdf_zero_length_segment",
            0xE10A: "sdf_centerline_segment",
            0xE10B: "mono_left_edge_dropout",
            0xE10C: "mono_bottom_edge_dropout",
            0xE10D: "sdf_flat_horizontal_segment",
            0xE10E: "sdf_flat_vertical_segment",
            0xE10F: "sdf_large_interior",
        }
    )
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=ascender, descent=0)
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
        sTypoAscender=ascender,
        sTypoDescender=0,
        usWinAscent=ascender,
        usWinDescent=0,
    )
    builder.setupPost()
    builder.setupMaxp()

    head = builder.font["head"]
    head.created = 0
    head.modified = 0
    builder.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    builder.save(OUT_DIR / name)


def build_render_coverage() -> None:
    build_render_coverage_font("render-coverage.ttf")
    build_render_coverage_font(
        "render-notdef-composite.ttf",
        notdef_glyph=overlap_compound_flag_glyph(),
        ascender=513,
    )


def build_render_prep_only() -> None:
    font = TTFont(OUT_DIR / "render-coverage.ttf", recalcTimestamp=False)
    prep = newTable("prep")
    program = Program()
    # Three PUSHB[1] 0; POP pairs keep the program valid and side-effect-free
    # while making prep_len > 7 so default-load fallback autohint is bypassed.
    program.fromBytecode(bytes.fromhex("b0 00 21 b0 00 21 b0 00 21"))
    prep.program = program
    font["prep"] = prep
    font.save(OUT_DIR / "render-prep-only.ttf", reorderTables=True)


def build_render_fpgm_no_cvt() -> None:
    font = TTFont(OUT_DIR / "render-coverage.ttf", recalcTimestamp=False)
    fpgm = newTable("fpgm")
    program = Program()
    program.fromBytecode(b"")
    fpgm.program = program
    font["fpgm"] = fpgm
    if "cvt " in font:
        del font["cvt "]
    font.save(OUT_DIR / "render-fpgm-no-cvt.ttf", reorderTables=True)


def main() -> None:
    build_render_coverage()
    build_render_prep_only()
    build_render_fpgm_no_cvt()


if __name__ == "__main__":
    main()
