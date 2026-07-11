#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise cmap format/language APIs."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "cmap"


def cmap_subtable(
    format_: int,
    platform_id: int,
    encoding_id: int,
    language: int,
    mapping: dict[int, str],
):
    table = CmapSubtable.newSubtable(format_)
    table.platformID = platform_id
    table.platEncID = encoding_id
    table.language = language
    if format_ == 12:
        table.reserved = 0
    table.cmap = mapping
    return table


def variation_selector_subtable():
    table = CmapSubtable.newSubtable(14)
    table.platformID = 0
    table.platEncID = 5
    table.cmap = {}
    table.uvsDict = {
        0xFE0F: [
            (0x0041, None),
            (0x0042, "base"),
        ],
    }
    return table


def build_matrix_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    cmap = newTable("cmap")
    cmap.tableVersion = 0
    cmap.tables = [
        cmap_subtable(
            4,
            3,
            1,
            0x0409,
            {
                0x0041: "base",
                0x0042: "mark",
            },
        ),
        cmap_subtable(
            6,
            1,
            0,
            17,
            {
                0x0020: "base",
                0x0021: "mark",
            },
        ),
        cmap_subtable(
            12,
            3,
            10,
            0x1234_5678,
            {
                0x1F600: "base",
                0x20000: "mark",
            },
        ),
        variation_selector_subtable(),
    ]
    font["cmap"] = cmap

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "cmap-format-language-matrix.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    build_matrix_font()


if __name__ == "__main__":
    main()
