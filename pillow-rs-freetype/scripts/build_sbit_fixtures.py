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


def compound_missing_subglyph_tables(index_format: int, image_format: int) -> tuple[bytes, bytes]:
    index_array_offset = 8 + 48
    index_subtable_offset = 8
    if image_format == 8:
        metrics = bytes([1, 1, 0, 1, 1]) + b"\0"
    elif image_format == 9:
        metrics = bytes([1, 1, 0, 1, 1, 0, 0, 1])
    else:
        raise ValueError(f"unsupported compound image format {image_format}")
    image = metrics + struct.pack(">H", 1) + struct.pack(">Hbb", 1, 0, 0)
    if index_format == 1:
        offsets = struct.pack(">III", 0, 0, len(image))
    elif index_format == 3:
        offsets = struct.pack(">HHH", 0, 0, len(image))
    else:
        raise ValueError(f"unsupported compound index format {index_format}")
    index_subtable = struct.pack(">HHI", index_format, image_format, 4) + offsets
    index_array = struct.pack(">HHI", 1, 2, index_subtable_offset)
    index_tables_size = len(index_array) + len(index_subtable)
    horizontal = eblc_line_metrics()
    vertical = bytes(12)
    strike = (
        struct.pack(">IIII", index_array_offset, index_tables_size, 1, 0)
        + horizontal
        + vertical
        + struct.pack(">HHBBBB", 1, 2, 20, 20, 1, 1)
    )
    assert len(strike) == 48
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_array + index_subtable
    ebdt = struct.pack(">I", 0x00020000) + image
    return eblc, ebdt


def save_sbit_font(name: str, eblc: bytes, ebdt: bytes) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["EBLC"] = raw_table("EBLC", eblc)
    font["EBDT"] = raw_table("EBDT", ebdt)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def build_missing_bitmap() -> None:
    save_sbit_font(
        "sbit_missing_bitmap.ttf",
        empty_image_eblc(),
        struct.pack(">I", 0x00020000),
    )


def build_composite_missing_subglyph(name: str, index_format: int, image_format: int) -> None:
    eblc, ebdt = compound_missing_subglyph_tables(index_format, image_format)
    save_sbit_font(name, eblc, ebdt)


def build_composite_missing_subglyphs() -> None:
    build_composite_missing_subglyph("sbit_composite_missing_subglyph.ttf", 1, 8)
    build_composite_missing_subglyph("sbit_composite_missing_subglyph_format3.ttf", 3, 9)


def main() -> None:
    build_missing_bitmap()
    build_composite_missing_subglyphs()


if __name__ == "__main__":
    main()
