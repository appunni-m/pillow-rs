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
        0xFE00: [],
        0xFE0E: [
            (0x0045, ".notdef"),
        ],
        0xFE0F: [
            (0x0041, None),
            (0x0042, "base"),
        ],
        0xE0101: [
            (0x0041, None),
            (0x0043, "mark"),
        ],
        0xE0102: [
            (0x0044, "mark"),
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


def compile_cmap_subtable(
    font: TTFont,
    format_: int,
    platform_id: int,
    encoding_id: int,
    mapping: dict[int, str],
) -> bytes:
    table = cmap_subtable(format_, platform_id, encoding_id, 0, mapping)
    return table.compile(font)


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


def pack_u24(value: int) -> bytes:
    return bytes([(value >> 16) & 0xFF, (value >> 8) & 0xFF, value & 0xFF])


def raw_format14_subtable(
    records: list[tuple[int, int, int]],
    payload: bytes = b"",
    *,
    length: int | None = None,
) -> bytes:
    if length is None:
        length = 10 + len(records) * 11 + len(payload)
    record_bytes = b"".join(
        pack_u24(selector) + struct.pack(">II", default_offset, non_default_offset)
        for selector, default_offset, non_default_offset in records
    )
    return (
        struct.pack(">HI", 14, length)
        + struct.pack(">I", len(records))
        + record_bytes
        + payload
    )


def format14_offset_out_of_range_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 21, 0)])


def format14_non_default_offset_out_of_range_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 0, 21)])


def format14_empty_selector_subtable(selector: int) -> bytes:
    return raw_format14_subtable([(selector, 0, 0)])


def format14_selectors_out_of_order_subtable() -> bytes:
    return raw_format14_subtable([(0xFE0F, 0, 0), (0xFE00, 0, 0)])


def format14_default_count_missing_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 21, 0)], b"\0")


def format14_default_records_exceed_length_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 21, 0)], struct.pack(">I", 1))


def format14_default_range_exceeds_unicode_subtable() -> bytes:
    return raw_format14_subtable(
        [(0xFE00, 21, 0)],
        struct.pack(">I", 1) + pack_u24(0x10FFFF) + b"\x01",
    )


def format14_default_ranges_out_of_order_subtable() -> bytes:
    return raw_format14_subtable(
        [(0xFE00, 21, 0)],
        struct.pack(">I", 2)
        + pack_u24(0x0041)
        + b"\x02"
        + pack_u24(0x0042)
        + b"\x00",
    )


def format14_non_default_count_missing_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 0, 21)], b"\0")


def format14_non_default_records_exceed_length_subtable() -> bytes:
    return raw_format14_subtable([(0xFE00, 0, 21)], struct.pack(">I", 1))


def format14_non_default_codepoint_exceeds_unicode_subtable() -> bytes:
    return raw_format14_subtable(
        [(0xFE00, 0, 21)],
        struct.pack(">I", 1) + pack_u24(0x110000) + struct.pack(">H", 1),
    )


def format14_non_default_mappings_out_of_order_subtable() -> bytes:
    return raw_format14_subtable(
        [(0xFE00, 0, 21)],
        struct.pack(">I", 2)
        + pack_u24(0x0043)
        + struct.pack(">H", 1)
        + pack_u24(0x0042)
        + struct.pack(">H", 2),
    )


def build_malformed_format14_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["cmap"] = raw_cmap_table(
        pack_raw_cmap(
            [
                (3, 1, format6_subtable()),
                (0, 5, format14_length_too_short_subtable()),
                (0, 6, format14_records_exceed_length_subtable()),
                (0, 7, format14_physically_short_subtable()),
                (1, 8, format14_offset_out_of_range_subtable()),
                (1, 19, format14_non_default_offset_out_of_range_subtable()),
                (1, 9, format14_selectors_out_of_order_subtable()),
                (1, 10, format14_default_count_missing_subtable()),
                (1, 11, format14_default_records_exceed_length_subtable()),
                (1, 12, format14_default_range_exceeds_unicode_subtable()),
                (1, 13, format14_default_ranges_out_of_order_subtable()),
                (1, 14, format14_non_default_count_missing_subtable()),
                (1, 15, format14_non_default_records_exceed_length_subtable()),
                (1, 16, format14_non_default_codepoint_exceeds_unicode_subtable()),
                (1, 17, format14_non_default_mappings_out_of_order_subtable()),
                (1, 18, format14_physically_short_subtable()),
            ]
        )
    )

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-format14-malformed-matrix.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def build_non_unicode_format6_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["cmap"] = raw_cmap_table(
        pack_raw_cmap(
            [
                (1, 0, format6_subtable()),
            ]
        )
    )

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-nonunicode-format6.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def build_default_charmap_order_fonts() -> None:
    cases = [
        (
            "cmap-default-ucs4-order.ttf",
            [
                (3, 10, 12, {0x0041: "base"}),
                (0, 4, 12, {0x0041: "mark"}),
            ],
        ),
        (
            "cmap-default-unicode-fallback-order.ttf",
            [
                (3, 1, 4, {0x0041: "base"}),
                (0, 3, 4, {0x0041: "mark"}),
            ],
        ),
        (
            "cmap-default-iso-fallback-order.ttf",
            [
                (3, 1, 4, {0x0041: "base"}),
                (2, 0, 4, {0x0041: "mark"}),
            ],
        ),
    ]
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, records in cases:
        font = TTFont(BASE_FONT, recalcTimestamp=False)
        font["cmap"] = raw_cmap_table(
            pack_raw_cmap(
                [
                    (
                        platform_id,
                        encoding_id,
                        compile_cmap_subtable(
                            font,
                            format_,
                            platform_id,
                            encoding_id,
                            mapping,
                        ),
                    )
                    for platform_id, encoding_id, format_, mapping in records
                ]
            )
        )
        out = OUT_DIR / name
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


def build_platform0_variation_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    cmap = newTable("cmap")
    cmap.tableVersion = 0
    cmap.tables = [
        cmap_subtable(
            4,
            0,
            3,
            0,
            {
                0x0041: "base",
                0x0042: "mark",
            },
        ),
        variation_selector_subtable(),
    ]
    font["cmap"] = cmap

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-platform0-variation.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def build_non_uvs_format14_platforms_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["cmap"] = raw_cmap_table(
        pack_raw_cmap(
            [
                (3, 1, format6_subtable()),
                (1, 8, format14_empty_selector_subtable(0xFE00)),
                (0, 6, format14_empty_selector_subtable(0xFE0F)),
            ]
        )
    )

    CHARMAP_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = CHARMAP_OUT_DIR / "cmap-format14-non-uvs-platforms.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    build_matrix_font()
    build_malformed_format14_font()
    build_non_unicode_format6_font()
    build_default_charmap_order_fonts()
    build_format14_only_font()
    build_platform0_variation_font()
    build_non_uvs_format14_platforms_font()


if __name__ == "__main__":
    main()
