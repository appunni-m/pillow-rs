#!/usr/bin/env python3
"""Generate deterministic CPAL/COLR fixtures for public color API parity."""

from __future__ import annotations

from pathlib import Path

from fontTools.colorLib.builder import buildCOLR
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.C_O_L_R_ import LayerRecord
from fontTools.ttLib.tables.C_P_A_L_ import Color
from fontTools.ttLib.tables import otTables as ot


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


if __name__ == "__main__":
    main()
