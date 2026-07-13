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


def bitmap_size_table(
    index_array_offset: int,
    index_tables_size: int,
    start_glyph: int,
    end_glyph: int,
    x_ppem: int = 20,
    y_ppem: int = 20,
) -> bytes:
    horizontal = eblc_line_metrics()
    vertical = bytes(12)
    strike = (
        struct.pack(">IIII", index_array_offset, index_tables_size, 1, 0)
        + horizontal
        + vertical
        + struct.pack(">HHBBBB", start_glyph, end_glyph, x_ppem, y_ppem, 1, 1)
    )
    assert len(strike) == 48
    return strike


def eblc_with_index_tables(
    index_array: bytes,
    index_subtable: bytes,
    *,
    start_glyph: int = 1,
    end_glyph: int = 1,
    x_ppem: int = 20,
    y_ppem: int = 20,
    index_array_offset: int = 8 + 48,
) -> bytes:
    index_tables = index_array + index_subtable
    strike = bitmap_size_table(
        index_array_offset,
        len(index_tables),
        start_glyph,
        end_glyph,
        x_ppem,
        y_ppem,
    )
    return struct.pack(">II", 0x00020000, 1) + strike + index_tables


def empty_image_eblc() -> bytes:
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = (
        struct.pack(">HHI", 1, 1, 0)
        # Index format 1 stores one more offset than covered glyphs.  Equal
        # start/end offsets are FreeType's Missing_Bitmap branch internally;
        # scalable TT_Load_Glyph with FT_LOAD_SBITS_ONLY reports Invalid_Argument.
        + struct.pack(">II", 0, 0)
    )
    return eblc_with_index_tables(index_array, index_subtable)


def no_matching_strike_eblc() -> bytes:
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = struct.pack(">HHI", 1, 1, 0) + struct.pack(">II", 0, 0)
    return eblc_with_index_tables(index_array, index_subtable, x_ppem=19, y_ppem=19)


def range_miss_eblc() -> bytes:
    index_array = struct.pack(">HHI", 2, 2, 8)
    index_subtable = struct.pack(">HHI", 1, 1, 0) + struct.pack(">II", 0, 0)
    return eblc_with_index_tables(index_array, index_subtable, start_glyph=2, end_glyph=2)


def missing_range_array_eblc() -> bytes:
    strike = bitmap_size_table(4096, 0, 1, 1)
    return struct.pack(">II", 0x00020000, 1) + strike


def missing_subtable_header_eblc() -> bytes:
    index_array = struct.pack(">HHI", 1, 1, 8)
    return eblc_with_index_tables(index_array, b"")


def unsupported_index_format_eblc() -> bytes:
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = struct.pack(">HHI", 6, 1, 0)
    return eblc_with_index_tables(index_array, index_subtable)


def invalid_version_eblc() -> bytes:
    return struct.pack(">II", 0x00030000, 0)


def strike_count_overflow_eblc() -> bytes:
    return struct.pack(">II", 0x00020000, 0x00010000)


def valid_empty_eblc() -> bytes:
    return struct.pack(">II", 0x00020000, 0)


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


def compound_malformed_tables(
    index_format: int,
    image_format: int,
    malformed: str,
) -> tuple[bytes, bytes]:
    index_array_offset = 8 + 48
    index_subtable_offset = 8
    if image_format == 8:
        metrics = bytes([1, 1, 0, 1, 1]) + b"\0"
    elif image_format == 9:
        metrics = bytes([1, 1, 0, 1, 1, 0, 0, 1])
    else:
        raise ValueError(f"unsupported compound image format {image_format}")
    if malformed == "missing_count":
        image = metrics
    elif malformed == "truncated_records":
        image = metrics + struct.pack(">H", 2) + struct.pack(">Hbb", 1, 0, 0)
    else:
        raise ValueError(f"unsupported malformed compound mode {malformed}")
    if index_format == 1:
        offsets = struct.pack(">III", 0, 0, len(image))
    elif index_format == 3:
        offsets = struct.pack(">HHH", 0, 0, len(image))
    else:
        raise ValueError(f"unsupported compound index format {index_format}")
    index_subtable = struct.pack(">HHI", index_format, image_format, 4) + offsets
    index_array = struct.pack(">HHI", 1, 2, index_subtable_offset)
    index_tables_size = len(index_array) + len(index_subtable)
    strike = bitmap_size_table(index_array_offset, index_tables_size, 1, 2)
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


def build_sbit_error_branch_fixtures() -> None:
    ebdt = struct.pack(">I", 0x00020000)
    save_sbit_font("sbit_empty_ebdt.ttf", valid_empty_eblc(), b"")
    save_sbit_font("sbit_invalid_eblc_version.ttf", invalid_version_eblc(), ebdt)
    save_sbit_font("sbit_strike_count_overflow.ttf", strike_count_overflow_eblc(), ebdt)
    save_sbit_font("sbit_no_matching_strike.ttf", no_matching_strike_eblc(), ebdt)
    save_sbit_font("sbit_range_miss.ttf", range_miss_eblc(), ebdt)
    save_sbit_font("sbit_missing_range_array.ttf", missing_range_array_eblc(), ebdt)
    save_sbit_font("sbit_missing_subtable_header.ttf", missing_subtable_header_eblc(), ebdt)
    save_sbit_font("sbit_unsupported_index_format.ttf", unsupported_index_format_eblc(), ebdt)


def build_composite_missing_subglyph(name: str, index_format: int, image_format: int) -> None:
    eblc, ebdt = compound_missing_subglyph_tables(index_format, image_format)
    save_sbit_font(name, eblc, ebdt)


def build_composite_malformed(
    name: str,
    index_format: int,
    image_format: int,
    malformed: str,
) -> None:
    eblc, ebdt = compound_malformed_tables(index_format, image_format, malformed)
    save_sbit_font(name, eblc, ebdt)


def build_composite_missing_subglyphs() -> None:
    build_composite_missing_subglyph("sbit_composite_missing_subglyph.ttf", 1, 8)
    build_composite_missing_subglyph("sbit_composite_missing_subglyph_format3.ttf", 3, 9)
    build_composite_malformed("sbit_composite_missing_count.ttf", 1, 8, "missing_count")
    build_composite_malformed(
        "sbit_composite_truncated_records.ttf",
        3,
        9,
        "truncated_records",
    )


def main() -> None:
    build_missing_bitmap()
    build_sbit_error_branch_fixtures()
    build_composite_missing_subglyphs()


if __name__ == "__main__":
    main()
