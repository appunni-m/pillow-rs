#!/usr/bin/env python3
"""Generate deterministic CPAL/COLR fixtures for public color API parity."""

from __future__ import annotations

from pathlib import Path

from fontTools.colorLib.builder import buildCOLR
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.C_O_L_R_ import LayerRecord
from fontTools.ttLib.tables.C_P_A_L_ import Color
from fontTools.ttLib.tables._f_v_a_r import Axis
from fontTools.ttLib.tables import otTables as ot
from fontTools.varLib import builder as var_builder


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
SOURCE_FONT = FIXTURE_ROOT / "input" / "fonts" / "DejaVuSans.ttf"
OUTPUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "color"
COLOR_OUTPUT_DIR = FIXTURE_ROOT / "fonts" / "color"


PALETTES = [
    [
        Color(0x10, 0x20, 0x30, 0x40),
        Color(0x50, 0x60, 0x70, 0x80),
        Color(0x90, 0xA0, 0xB0, 0xC0),
    ],
    [
        Color(0x01, 0x02, 0x03, 0x04),
        Color(0x11, 0x12, 0x13, 0x14),
        Color(0x21, 0x22, 0x23, 0x24),
    ],
]

CPAL_FIXTURE_HEAD_MODIFIED = 3867487964


def build_cpal_font(path: Path) -> None:
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    # Preserve the already-tracked deterministic timestamp for these existing
    # CPAL fixtures.  Their CPAL data is stable; changing only `head.modified`
    # and checksum adjustment creates noisy binary fixture churn.
    font["head"].modified = CPAL_FIXTURE_HEAD_MODIFIED
    cpal = newTable("CPAL")
    cpal.version = 1
    cpal.numPaletteEntries = len(PALETTES[0])
    cpal.palettes = PALETTES
    # FreeType exposes these through FT_Palette_Data as FT_UShort arrays.
    cpal.paletteTypes = [0x0001, 0x0002]
    cpal.paletteLabels = [256, cpal.NO_NAME_ID]
    cpal.paletteEntryLabels = [257, 258, cpal.NO_NAME_ID]
    font["CPAL"] = cpal
    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v0_layers_font(path: Path) -> None:
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)

    colr = newTable("COLR")
    colr.version = 0
    layers = []
    for glyph_name, color_id in (("B", 0), ("C", 1), ("D", 2)):
        layer = LayerRecord()
        layer.name = glyph_name
        layer.colorID = color_id
        layers.append(layer)
    colr.ColorLayers = {"A": layers}
    font["COLR"] = colr

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 3
    cpal.palettes = [
        [
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0xFF),
            Color(0x70, 0x80, 0x90, 0xFF),
        ]
    ]
    font["CPAL"] = cpal

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def solid_paint(palette_index: int, alpha: float = 1.0) -> dict[str, object]:
    return {
        "Format": int(ot.PaintFormat.PaintSolid),
        "PaletteIndex": palette_index,
        "Alpha": alpha,
    }


def build_colr_v1_composite_font(path: Path) -> None:
    """Build a compact COLRv1 paint graph fixture.

    The fixture intentionally starts with the first batchable COLRv1 public
    paint surfaces: root PaintSolid, nested PaintGlyph, and every real
    PaintComposite mode.  Gradients, color lines, transforms, and ClipList rows
    remain separate batches so their pending route counts stay visible until
    same-input C/Rust/C-ABI/WASM comparisons exist.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: solid_paint(1),
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(2, 0.5),
            "Glyph": base_names[2],
        },
    }
    for offset, mode in enumerate(ot.CompositeMode):
        color_glyphs[base_names[3 + offset]] = {
            "Format": int(ot.PaintFormat.PaintComposite),
            "SourcePaint": solid_paint(1),
            "CompositeMode": int(mode),
            "BackdropPaint": solid_paint(2, 0.5),
        }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_layers_font(path: Path) -> None:
    """Build a compact COLRv1 PaintColrLayers fixture.

    FreeType 2.14.3 exposes PaintColrLayers through `FT_Get_Paint` as an
    initialized `FT_LayerIterator`, then consumes that iterator through
    `FT_Get_Paint_Layers`.  Keep this fixture focused on two- and three-layer
    records; FontTools canonicalizes a one-layer PaintColrLayers node to its
    child paint, so single-layer and malformed layer-list behavior should stay
    in separate future fixtures.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
            ],
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
                {
                    "Format": int(ot.PaintFormat.PaintGlyph),
                    "Paint": solid_paint(3, 0.25),
                    "Glyph": base_names[3],
                },
            ],
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_colr_glyph_font(path: Path) -> None:
    """Build a compact COLRv1 PaintColrGlyph recursive fixture."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrGlyph),
            "Glyph": base_names[1],
        },
        base_names[1]: solid_paint(2, 0.5),
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_transform_paints_font(path: Path) -> None:
    """Build compact COLRv1 transform-paint fixture variants.

    FreeType 2.14.3 normalizes several internal COLRv1 table formats to the
    public FT_PaintScale, FT_PaintRotate, and FT_PaintSkew records.  Keep root
    transform synthesis out of this fixture; that depends on active size and
    FT_Set_Transform state and remains a separate route.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:48]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    transform = {
        "xx": 1.5,
        "xy": -0.125,
        "dx": 5.0,
        "yx": 0.25,
        "yy": 0.75,
        "dy": -3.0,
    }

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintTransform),
            "Paint": solid_paint(1),
            "Transform": transform,
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintTranslate),
            "Paint": solid_paint(2, 0.5),
            "dx": 17,
            "dy": -9,
        },
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintScale),
            "Paint": solid_paint(1),
            "scaleX": 0.75,
            "scaleY": -0.5,
        },
        base_names[3]: {
            "Format": int(ot.PaintFormat.PaintScaleAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "scaleX": 1.25,
            "scaleY": 0.625,
            "centerX": 11,
            "centerY": -7,
        },
        base_names[4]: {
            "Format": int(ot.PaintFormat.PaintScaleUniform),
            "Paint": solid_paint(3, 0.25),
            "scale": 1.5,
        },
        base_names[5]: {
            "Format": int(ot.PaintFormat.PaintScaleUniformAroundCenter),
            "Paint": solid_paint(1),
            "scale": 0.5,
            "centerX": -13,
            "centerY": 19,
        },
        base_names[6]: {
            "Format": int(ot.PaintFormat.PaintRotate),
            "Paint": solid_paint(2, 0.5),
            "angle": 0.25,
        },
        base_names[7]: {
            "Format": int(ot.PaintFormat.PaintRotateAroundCenter),
            "Paint": solid_paint(3, 0.25),
            "angle": -0.125,
            "centerX": 23,
            "centerY": -29,
        },
        base_names[8]: {
            "Format": int(ot.PaintFormat.PaintSkew),
            "Paint": solid_paint(1),
            "xSkewAngle": 0.0625,
            "ySkewAngle": -0.1875,
        },
        base_names[9]: {
            "Format": int(ot.PaintFormat.PaintSkewAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "xSkewAngle": -0.25,
            "ySkewAngle": 0.125,
            "centerX": -31,
            "centerY": 37,
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_root_transform_font(path: Path) -> None:
    """Build a compact COLRv1 root-transform fixture.

    The font intentionally keeps the actual root paint simple.  The parity
    surface under test is FreeType's synthetic top-level PaintTransform that
    `FT_Get_Paint` inserts from active size and `FT_Set_Transform` state when
    `FT_Get_Color_Glyph_Paint` is called with `FT_COLOR_INCLUDE_ROOT_TRANSFORM`.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:38]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 3
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(1),
            "Glyph": base_names[1],
        },
        base_names[1]: solid_paint(2, 0.5),
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_all_paints_font(path: Path) -> None:
    """Build one maintained COLRv1 fixture with every supported paint family."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:60]

    cpal = newTable("CPAL")
    cpal.version = 1
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ],
        [
            Color(0x01, 0x02, 0x03, 0xFF),
            Color(0x11, 0x12, 0x13, 0xE0),
            Color(0x21, 0x22, 0x23, 0xC0),
            Color(0x31, 0x32, 0x33, 0xA0),
        ],
    ]
    cpal.paletteTypes = [0x0001, 0x0002]
    cpal.paletteLabels = [256, cpal.NO_NAME_ID]
    cpal.paletteEntryLabels = [257, 258, cpal.NO_NAME_ID, cpal.NO_NAME_ID]
    font["CPAL"] = cpal

    transform = {
        "xx": 1.5,
        "xy": -0.125,
        "dx": 5.0,
        "yx": 0.25,
        "yy": 0.75,
        "dy": -3.0,
    }

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
                {
                    "Format": int(ot.PaintFormat.PaintGlyph),
                    "Paint": solid_paint(3, 0.25),
                    "Glyph": base_names[15],
                },
            ],
        },
        base_names[1]: solid_paint(1),
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(2, 0.5),
            "Glyph": base_names[15],
        },
        base_names[3]: {
            "Format": int(ot.PaintFormat.PaintColrGlyph),
            "Glyph": base_names[1],
        },
        base_names[4]: {
            "Format": int(ot.PaintFormat.PaintLinearGradient),
            "ColorLine": color_line(
                ot.ExtendMode.PAD,
                [(0.0, 1, 1.0), (0.5, 2, 0.5), (1.0, 3, 0.25)],
            ),
            "x0": -10,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
        },
        base_names[5]: {
            "Format": int(ot.PaintFormat.PaintRadialGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REPEAT,
                [(0.25, 2, 0.75), (0.875, 3, 0.125)],
            ),
            "x0": 5,
            "y0": -7,
            "r0": 3,
            "x1": 33,
            "y1": 29,
            "r1": 41,
        },
        base_names[6]: {
            "Format": int(ot.PaintFormat.PaintSweepGradient),
            "ColorLine": color_line(ot.ExtendMode.REFLECT, [(0.75, 1, 0.625)]),
            "centerX": -13,
            "centerY": 17,
            "startAngle": -0.25,
            "endAngle": 0.5,
        },
        base_names[7]: {
            "Format": int(ot.PaintFormat.PaintTransform),
            "Paint": solid_paint(1),
            "Transform": transform,
        },
        base_names[8]: {
            "Format": int(ot.PaintFormat.PaintTranslate),
            "Paint": solid_paint(2, 0.5),
            "dx": 17,
            "dy": -9,
        },
        base_names[9]: {
            "Format": int(ot.PaintFormat.PaintScale),
            "Paint": solid_paint(1),
            "scaleX": 0.75,
            "scaleY": -0.5,
        },
        base_names[10]: {
            "Format": int(ot.PaintFormat.PaintRotateAroundCenter),
            "Paint": solid_paint(3, 0.25),
            "angle": -0.125,
            "centerX": 23,
            "centerY": -29,
        },
        base_names[11]: {
            "Format": int(ot.PaintFormat.PaintSkewAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "xSkewAngle": -0.25,
            "ySkewAngle": 0.125,
            "centerX": -31,
            "centerY": 37,
        },
        base_names[12]: {
            "Format": int(ot.PaintFormat.PaintComposite),
            "SourcePaint": solid_paint(1),
            "CompositeMode": int(ot.CompositeMode.SRC_OVER),
            "BackdropPaint": solid_paint(2, 0.5),
        },
        base_names[13]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(1),
            "Glyph": base_names[15],
        },
        base_names[14]: solid_paint(0xFFFF),
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def color_line(extend: ot.ExtendMode, stops: list[tuple[float, int, float]]) -> dict[str, object]:
    return {
        "Extend": int(extend),
        "ColorStop": [
            {
                "StopOffset": stop_offset,
                "PaletteIndex": palette_index,
                "Alpha": alpha,
            }
            for stop_offset, palette_index, alpha in stops
        ],
    }


def add_color_variation_axes(font: TTFont) -> None:
    """Add the compact `wght` and `GRAD` axes used by COLR VarStore fixtures."""
    fvar = newTable("fvar")
    fvar.axes = []
    fvar.instances = []
    for tag, minimum, default, maximum, name_id, label in (
        ("wght", 100.0, 400.0, 900.0, 300, "Weight"),
        ("GRAD", 0.0, 0.0, 1.0, 301, "Gradient"),
    ):
        axis = Axis()
        axis.axisTag = tag
        axis.minValue = minimum
        axis.defaultValue = default
        axis.maxValue = maximum
        axis.flags = 0
        axis.axisNameID = name_id
        fvar.axes.append(axis)
        font["name"].setName(label, name_id, 3, 1, 0x0409)
        font["name"].setName(label, name_id, 1, 0, 0)
    font["fvar"] = fvar


def colr_v1_color_var_store(font: TTFont) -> ot.VarStore:
    """Build a deterministic COLR VarStore for VarColorStop/gradient deltas.

    The single region peaks at `wght=max, GRAD=max`.  FreeType applies COLR
    variation deltas through the COLR VarStore using the public VarIndexBase
    fields documented for VarColorStop and PaintVarLinearGradient in the
    OpenType COLR v1 format.
    """
    axis_tags = [axis.axisTag for axis in font["fvar"].axes]
    region_list = var_builder.buildVarRegionList(
        [{"wght": (0.0, 1.0, 1.0), "GRAD": (0.0, 1.0, 1.0)}],
        axis_tags,
    )
    deltas = [
        [4096],  # stop 0 offset: +0.25 in F2Dot14 units.
        [1024],  # stop 0 alpha: +0.0625 in F2Dot14 units.
        [-2048],  # stop 1 offset: -0.125 in F2Dot14 units.
        [-2048],  # stop 1 alpha: -0.125 in F2Dot14 units.
        [5],  # PaintVarLinearGradient x0.
        [0],  # PaintVarLinearGradient y0.
        [10],  # PaintVarLinearGradient x1.
        [0],  # PaintVarLinearGradient y1.
        [10],  # PaintVarLinearGradient x2.
        [5],  # PaintVarLinearGradient y2.
    ]
    var_data = var_builder.buildVarData([0], deltas, optimize=False)
    return var_builder.buildVarStore(region_list, [var_data])


def build_colr_v1_static_gradients_font(path: Path) -> None:
    """Build compact static COLRv1 gradient and ColorLine fixture.

    FreeType 2.14.3 exposes PaintLinearGradient, PaintRadialGradient, and
    PaintSweepGradient coordinates as 16.16 public values and initializes
    ColorLine iterators from static ColorStop records.  This fixture covers
    the static PAD/REPEAT/REFLECT routes only; variable ColorLine rows remain
    pending until VarColorStop deltas are implemented and compared.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintLinearGradient),
            "ColorLine": color_line(
                ot.ExtendMode.PAD,
                [
                    (0.0, 1, 1.0),
                    (0.5, 2, 0.5),
                    (1.0, 3, 0.25),
                ],
            ),
            "x0": -10,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintRadialGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REPEAT,
                [
                    (0.25, 2, 0.75),
                    (0.875, 3, 0.125),
                ],
            ),
            "x0": 5,
            "y0": -7,
            "r0": 3,
            "x1": 33,
            "y1": 29,
            "r1": 41,
        },
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintSweepGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REFLECT,
                [
                    (0.75, 1, 0.625),
                ],
            ),
            "centerX": -13,
            "centerY": 17,
            "startAngle": -0.25,
            "endAngle": 0.5,
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_variable_gradients_font(path: Path) -> None:
    """Build compact variable COLRv1 gradient and VarColorStop fixture."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    add_color_variation_axes(font)
    glyph_order = font.getGlyphOrder()
    base_name = glyph_order[36]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_name: {
            "Format": int(ot.PaintFormat.PaintVarLinearGradient),
            "ColorLine": {
                "Extend": int(ot.ExtendMode.PAD),
                "ColorStop": [
                    {
                        "StopOffset": 0.0,
                        "PaletteIndex": 1,
                        "Alpha": 0.5,
                        "VarIndexBase": 0,
                    },
                    {
                        "StopOffset": 1.0,
                        "PaletteIndex": 2,
                        "Alpha": 1.0,
                        "VarIndexBase": 2,
                    },
                ],
            },
            "x0": 0,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
            "VarIndexBase": 4,
        }
    }
    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        varStore=colr_v1_color_var_store(font),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def clip_box(x_min: int, y_min: int, x_max: int, y_max: int, fmt: int = 1) -> ot.ClipBox:
    box = ot.ClipBox()
    box.Format = fmt
    box.xMin = x_min
    box.yMin = y_min
    box.xMax = x_max
    box.yMax = y_max
    if fmt == 2:
        box.VarIndexBase = 0
    return box


def build_colr_v1_clipbox_font(path: Path, include_clip_list: bool) -> None:
    """Build deterministic COLRv1 ClipList fixtures for FT_Get_Color_Glyph_ClipBox.

    The success fixture includes a tested format 1 ClipBox plus a format 2
    record kept as an explicit future variation row input.  The current routed
    parity cases call the format 1 glyph because no variable ClipBox row is
    classified as real parity until it has a dedicated expected-output case.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:39]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 2
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x20, 0x40, 0x60, 0xFF),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: solid_paint(1),
        base_names[1]: solid_paint(1),
    }
    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    if include_clip_list:
        glyph_map = font.getReverseGlyphMap()
        clip_list = ot.ClipList()
        clip_list.Format = 1
        clip_list.ClipRecord = []
        for glyph_name, box in (
            (base_names[0], clip_box(-120, -80, 340, 510)),
            (base_names[1], clip_box(-64, -32, 256, 384, fmt=2)),
        ):
            record = ot.ClipRecord()
            record.StartGlyphID = glyph_map[glyph_name]
            record.EndGlyphID = glyph_map[glyph_name]
            record.ClipBox = box
            clip_list.ClipRecord.append(record)
        font["COLR"].table.ClipList = clip_list

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def main() -> None:
    for name in (
        "cpal-palettes-names-flags.ttf",
        "cpal-palettes-light-dark.ttf",
    ):
        build_cpal_font(OUTPUT_DIR / name)
    build_colr_v0_layers_font(COLOR_OUTPUT_DIR / "colr-v0-layers-cpal.ttf")
    build_colr_v0_layers_font(COLOR_OUTPUT_DIR / "colr-v0-layer-control.ttf")
    build_colr_v1_composite_font(COLOR_OUTPUT_DIR / "colr_v1_composite_modes.ttf")
    build_colr_v1_layers_font(COLOR_OUTPUT_DIR / "colr-v1-paint-colr-layers-cpal.ttf")
    build_colr_v1_colr_glyph_font(COLOR_OUTPUT_DIR / "colr-v1-colr-glyph-recursive.ttf")
    build_colr_v1_transform_paints_font(COLOR_OUTPUT_DIR / "colr-v1-transform-paints.ttf")
    build_colr_v1_root_transform_font(COLOR_OUTPUT_DIR / "colr-v1-root-transform.ttf")
    build_colr_v1_all_paints_font(COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf")
    build_colr_v1_static_gradients_font(COLOR_OUTPUT_DIR / "colr-v1-static-gradients.ttf")
    build_colr_v1_variable_gradients_font(COLOR_OUTPUT_DIR / "colr-v1-variable-gradients.ttf")
    build_colr_v1_clipbox_font(COLOR_OUTPUT_DIR / "colr-v1-clipbox-format1-format2.ttf", True)
    build_colr_v1_clipbox_font(COLOR_OUTPUT_DIR / "colr-v1-no-clipbox-control.ttf", False)


if __name__ == "__main__":
    main()
