#!/usr/bin/env python3
"""Build compact embedded bitmap/SBIT fixtures."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fixtures" / "assets" / "fonts"


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def signed_byte(value: int) -> int:
    return value & 0xFF


def eblc_line_metrics() -> bytes:
    # SBitLineMetrics: ascender, descender, widthMax, caretSlopeNumerator,
    # caretSlopeDenominator, caretOffset, minOriginSB, minAdvanceSB,
    # maxBeforeBL, minAfterBL, pad1, pad2.
    return bytes(
        [
            signed_byte(16),
            signed_byte(-4),
            8,
            1,
            0,
            0,
            0,
            0,
            16,
            signed_byte(-4),
            0,
            0,
        ]
    )


def empty_image_eblc() -> bytes:
    index_array_offset = 8 + 48
    index_subtable_offset = 8
    index_subtable = (
        struct.pack(">HHI", 1, 1, 0)
        # Index format 1 stores one more offset than covered glyphs.  Equal
        # start/end offsets are FreeType's Missing_Bitmap branch.
        + struct.pack(">II", 0, 0)
    )
    index_array = struct.pack(">HHI", 1, 1, index_subtable_offset)
    index_tables_size = len(index_array) + len(index_subtable)
    horizontal = eblc_line_metrics()
    vertical = bytes(12)
    strike = (
        struct.pack(">IIII", index_array_offset, index_tables_size, 1, 0)
        + horizontal
        + vertical
        + struct.pack(">HHBBBB", 1, 1, 20, 20, 1, 1)
    )
    assert len(strike) == 48
    return struct.pack(">II", 0x00020000, 1) + strike + index_array + index_subtable


def build_missing_bitmap() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["EBLC"] = raw_table("EBLC", empty_image_eblc())
    font["EBDT"] = raw_table("EBDT", struct.pack(">I", 0x00020000))
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "sbit_missing_bitmap.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    build_missing_bitmap()


if __name__ == "__main__":
    main()
