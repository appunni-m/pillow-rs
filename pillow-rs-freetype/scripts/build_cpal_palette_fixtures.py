#!/usr/bin/env python3
"""Generate deterministic CPAL/COLR fixtures for public color API parity."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.C_O_L_R_ import LayerRecord
from fontTools.ttLib.tables.C_P_A_L_ import Color


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


def build_cpal_font(path: Path) -> None:
    font = TTFont(SOURCE_FONT)
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


def main() -> None:
    for name in (
        "cpal-palettes-names-flags.ttf",
        "cpal-palettes-light-dark.ttf",
    ):
        build_cpal_font(OUTPUT_DIR / name)
    build_colr_v0_layers_font(COLOR_OUTPUT_DIR / "colr-v0-layers-cpal.ttf")


if __name__ == "__main__":
    main()
