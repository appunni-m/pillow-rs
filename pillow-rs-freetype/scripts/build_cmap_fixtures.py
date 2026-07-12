#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise cmap format/language APIs."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "cmap"
CHARMAP_OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "charmap"


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
        0xE0101: [
            (0x0041, None),
            (0x0043, "mark"),
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


def pack_raw_cmap(records: list[tuple[int, int, bytes]]) -> bytes:
    offsets = []
    cursor = 4 + 8 * len(records)
    for _, _, subtable in records:
        offsets.append(cursor)
        cursor += len(subtable)

    header = struct.pack(">HH", 0, len(records))
    directory = b"".join(
        struct.pack(">HHI", platform_id, encoding_id, offset)
        for (platform_id, encoding_id, _), offset in zip(records, offsets)
    )
    return header + directory + b"".join(subtable for _, _, subtable in records)


def raw_cmap_table(data: bytes):
    table = DefaultTable("cmap")
    table.data = data
    return table


def format6_subtable() -> bytes:
    return (
        struct.pack(">HHHHH", 6, 14, 0, 0x0040, 2)
        + struct.pack(">HH", 0, 1)
    )


def format14_length_too_short_subtable() -> bytes:
    return struct.pack(">HI", 14, 9) + struct.pack(">I", 0)


def format14_records_exceed_length_subtable() -> bytes:
    return struct.pack(">HI", 14, 10) + struct.pack(">I", 1)


def format14_physically_short_subtable() -> bytes:
    return struct.pack(">H", 14)


def build_malformed_format14_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["cmap"] = raw_cmap_table(
        pack_raw_cmap(
            [
                (3, 1, format6_subtable()),
                (0, 5, format14_length_too_short_subtable()),
                (0, 6, format14_records_exceed_length_subtable()),
                (0, 7, format14_physically_short_subtable()),
            ]
        )
    )

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-format14-malformed-matrix.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def build_format14_only_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    cmap = newTable("cmap")
    cmap.tableVersion = 0
    cmap.tables = [variation_selector_subtable()]
    font["cmap"] = cmap

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-format14-only.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    build_matrix_font()
    build_malformed_format14_font()
    build_format14_only_font()


if __name__ == "__main__":
    main()
