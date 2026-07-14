#!/usr/bin/env python3
"""Build compact embedded bitmap/SBIT fixtures."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont, newTable
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
    bit_depth: int = 1,
    index_subtable_count: int = 1,
) -> bytes:
    horizontal = eblc_line_metrics()
    vertical = bytes(12)
    strike = (
        struct.pack(">IIII", index_array_offset, index_tables_size, index_subtable_count, 0)
        + horizontal
        + vertical
        + struct.pack(
            ">HHBBBB",
            start_glyph,
            end_glyph,
            x_ppem,
            y_ppem,
            bit_depth,
            1,
        )
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


def gray_format1_tables(hori_advance: int = 3) -> tuple[bytes, bytes]:
    # EBDT image format 1 stores small metrics followed by byte-aligned bitmap
    # bytes. With an 8-bit strike, FreeType exposes FT_PIXEL_MODE_GRAY and a
    # pitch equal to the bitmap width.
    image = bytes([2, 2, 1, 2, hori_advance]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = (
        struct.pack(">HHI", 1, 1, 4)
        + struct.pack(">II", 0, len(image))
    )
    index_tables = index_array + index_subtable
    strike = bitmap_size_table(
        8 + 48,
        len(index_tables),
        1,
        1,
        bit_depth=8,
    )
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_tables
    ebdt = struct.pack(">I", 0x00020000) + image
    return eblc, ebdt


def packed_format1_tables(bit_depth: int, image: bytes) -> tuple[bytes, bytes]:
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = (
        struct.pack(">HHI", 1, 1, 4)
        + struct.pack(">II", 0, len(image))
    )
    index_tables = index_array + index_subtable
    strike = bitmap_size_table(
        8 + 48,
        len(index_tables),
        1,
        1,
        bit_depth=bit_depth,
    )
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_tables
    ebdt = struct.pack(">I", 0x00020000) + image
    return eblc, ebdt


def mono_format1_tables() -> tuple[bytes, bytes]:
    # The same image format with a 1-bit strike exposes FT_PIXEL_MODE_MONO and
    # a byte pitch of ceil(width / 8).  A 9-pixel width keeps the final-byte mask
    # observable without needing another glyph.
    image = bytes([2, 9, 1, 2, 10]) + bytes([0xA5, 0x80, 0x5A, 0x00])
    return packed_format1_tables(1, image)


def gray2_format1_tables() -> tuple[bytes, bytes]:
    # FreeType's byte-aligned SBIT decoder exposes bit depth 2 as
    # FT_PIXEL_MODE_GRAY2 with a pitch of ceil(width / 4).
    image = bytes([2, 5, 1, 2, 6]) + bytes([0x1B, 0x80, 0xE4, 0x40])
    return packed_format1_tables(2, image)


def gray4_format1_tables() -> tuple[bytes, bytes]:
    # Bit depth 4 maps to FT_PIXEL_MODE_GRAY4 with a pitch of ceil(width / 2).
    image = bytes([2, 3, 1, 2, 4]) + bytes([0x12, 0x30, 0xAB, 0xC0])
    return packed_format1_tables(4, image)


def bgra_format1_tables() -> tuple[bytes, bytes]:
    # Bit depth 32 uses packed BGRA bytes and FreeType reports
    # FT_PIXEL_MODE_BGRA with pitch width * 4.
    image = bytes([1, 2, 1, 1, 3]) + bytes(
        [0x10, 0x20, 0x30, 0xFF, 0x40, 0x50, 0x60, 0x80]
    )
    return packed_format1_tables(32, image)


def unsupported_bit_depth_format1_tables() -> tuple[bytes, bytes]:
    image = bytes([1, 1, 1, 1, 1]) + bytes([0x80])
    return packed_format1_tables(7, image)


def unsupported_image_format_tables() -> tuple[bytes, bytes]:
    image = bytes([1, 1, 1, 1, 1]) + bytes([0x80])
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = (
        struct.pack(">HHI", 1, 10, 4)
        + struct.pack(">II", 0, len(image))
    )
    index_tables = index_array + index_subtable
    strike = bitmap_size_table(
        8 + 48,
        len(index_tables),
        1,
        1,
        bit_depth=8,
    )
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_tables
    ebdt = struct.pack(">I", 0x00020000) + image
    return eblc, ebdt


def missing_small_metrics_width_tables() -> tuple[bytes, bytes]:
    # Offsets select a non-empty image, but the image stops after the height
    # byte.  That reaches FreeType's malformed small-metrics branch through the
    # normal public FT_LOAD_SBITS_ONLY path.
    return packed_format1_tables(8, bytes([2]))


def gray_format3_tables() -> tuple[bytes, bytes]:
    image = bytes([2, 2, 1, 2, 3]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    index_array = struct.pack(">HHI", 1, 1, 8)
    index_subtable = (
        struct.pack(">HHI", 3, 1, 4)
        + struct.pack(">HH", 0, len(image))
    )
    index_tables = index_array + index_subtable
    strike = bitmap_size_table(
        8 + 48,
        len(index_tables),
        1,
        1,
        bit_depth=8,
    )
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_tables
    ebdt = struct.pack(">I", 0x00020000) + image
    return eblc, ebdt


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


def compound_image_format8(metrics: bytes, components: list[tuple[int, int, int]]) -> bytes:
    records = b"".join(struct.pack(">Hbb", glyph, dx, dy) for glyph, dx, dy in components)
    return metrics + b"\0" + struct.pack(">H", len(components)) + records


def compound_image_format9(metrics: bytes, components: list[tuple[int, int, int]]) -> bytes:
    records = b"".join(struct.pack(">Hbb", glyph, dx, dy) for glyph, dx, dy in components)
    return metrics + struct.pack(">H", len(components)) + records


def compound_pair_tables(
    bit_depth: int,
    simple_image: bytes,
    image_format: int,
    compound_image: bytes,
) -> tuple[bytes, bytes]:
    index_array = struct.pack(">HHI", 1, 1, 16) + struct.pack(">HHI", 2, 2, 32)
    simple_subtable = (
        struct.pack(">HHI", 1, 1, 4)
        + struct.pack(">II", 0, len(simple_image))
    )
    compound_subtable = (
        struct.pack(">HHI", 1, image_format, 4)
        + struct.pack(">II", len(simple_image), len(simple_image) + len(compound_image))
    )
    index_tables = index_array + simple_subtable + compound_subtable
    strike = bitmap_size_table(
        8 + 48,
        len(index_tables),
        1,
        2,
        bit_depth=bit_depth,
        index_subtable_count=2,
    )
    eblc = struct.pack(">II", 0x00020000, 1) + strike + index_tables
    ebdt = struct.pack(">I", 0x00020000) + simple_image + compound_image
    return eblc, ebdt


def compound_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 2, 1, 2, 3]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    compound_image = compound_image_format8(
        bytes([2, 2, 1, 2, 3]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(8, simple_image, 8, compound_image)


def compound_success_format9_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 2, 1, 2, 3]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    compound_image = compound_image_format9(
        bytes([2, 2, 1, 2, 3, 0, 0, 3]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(8, simple_image, 9, compound_image)


def compound_mono_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 9, 1, 2, 10]) + bytes([0xA5, 0x80, 0x5A, 0x00])
    compound_image = compound_image_format8(
        bytes([2, 9, 1, 2, 10]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(1, simple_image, 8, compound_image)


def compound_mono_zero_width_component_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 0, 1, 2, 1])
    compound_image = compound_image_format8(
        bytes([2, 1, 1, 2, 2]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(1, simple_image, 8, compound_image)


def compound_mono_shifted_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 9, 1, 2, 10]) + bytes([0xA5, 0x80, 0x5A, 0x00])
    compound_image = compound_image_format8(
        bytes([2, 10, 1, 2, 11]),
        [(1, 1, 0)],
    )
    return compound_pair_tables(1, simple_image, 8, compound_image)


def compound_mono_carry_success_format8_tables() -> tuple[bytes, bytes]:
    # A 10-bit component shifted by 7 pixels leaves a two-bit tail; FreeType's
    # packed compound blitter carries that tail into a second target byte.
    simple_image = bytes([2, 10, 1, 2, 11]) + bytes([0xA5, 0xC0, 0x5A, 0x80])
    compound_image = compound_image_format8(
        bytes([2, 17, 1, 2, 18]),
        [(1, 7, 0)],
    )
    return compound_pair_tables(1, simple_image, 8, compound_image)


def compound_gray2_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 5, 1, 2, 6]) + bytes([0x1B, 0x80, 0xE4, 0x40])
    compound_image = compound_image_format8(
        bytes([2, 5, 1, 2, 6]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(2, simple_image, 8, compound_image)


def compound_gray4_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 3, 1, 2, 4]) + bytes([0x12, 0x30, 0xAB, 0xC0])
    compound_image = compound_image_format8(
        bytes([2, 3, 1, 2, 4]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(4, simple_image, 8, compound_image)


def compound_bgra_success_format8_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([1, 2, 1, 1, 3]) + bytes(
        [0x10, 0x20, 0x30, 0xFF, 0x40, 0x50, 0x60, 0x80]
    )
    compound_image = compound_image_format8(
        bytes([1, 2, 1, 1, 3]),
        [(1, 0, 0)],
    )
    return compound_pair_tables(32, simple_image, 8, compound_image)


def compound_negative_offset_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 2, 1, 2, 3]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    compound_image = compound_image_format8(
        bytes([2, 2, 1, 2, 3]),
        [(1, -1, 0)],
    )
    return compound_pair_tables(8, simple_image, 8, compound_image)


def compound_out_of_bounds_tables() -> tuple[bytes, bytes]:
    simple_image = bytes([2, 2, 1, 2, 3]) + bytes([0x11, 0x80, 0xC0, 0xFF])
    compound_image = compound_image_format8(
        bytes([2, 2, 1, 2, 3]),
        [(1, 1, 0)],
    )
    return compound_pair_tables(8, simple_image, 8, compound_image)


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


def add_vertical_metrics(font: TTFont, glyph_index: int, advance_height: int) -> None:
    glyph_order = font.getGlyphOrder()
    if glyph_index >= len(glyph_order):
        raise ValueError(f"glyph index {glyph_index} not present in base font")
    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 800
    vhea.descent = -200
    vhea.lineGap = 0
    vhea.advanceHeightMax = advance_height
    vhea.minTopSideBearing = 0
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 800
    vhea.caretSlopeRise = 1
    vhea.caretSlopeRun = 0
    vhea.caretOffset = 0
    vhea.reserved0 = 0
    vhea.reserved1 = 0
    vhea.reserved2 = 0
    vhea.reserved3 = 0
    vhea.reserved4 = 0
    vhea.metricDataFormat = 0
    vhea.numberOfVMetrics = len(glyph_order)
    font["vhea"] = vhea

    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (advance_height, 0) for name in glyph_order}
    vmtx.metrics[glyph_order[glyph_index]] = (advance_height, 0)
    font["vmtx"] = vmtx


def save_sbit_font(
    name: str,
    eblc: bytes,
    ebdt: bytes,
    *,
    vertical_metrics: tuple[int, int] | None = None,
) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["EBLC"] = raw_table("EBLC", eblc)
    font["EBDT"] = raw_table("EBDT", ebdt)
    if vertical_metrics is not None:
        add_vertical_metrics(font, *vertical_metrics)
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


def build_gray_format1_bitmap() -> None:
    eblc, ebdt = gray_format1_tables()
    save_sbit_font("sbit_gray_format1.ttf", eblc, ebdt)

    eblc, ebdt = gray_format1_tables(hori_advance=0)
    save_sbit_font("sbit_gray_format1_vmtx.ttf", eblc, ebdt, vertical_metrics=(1, 880))


def build_mono_format1_bitmap() -> None:
    eblc, ebdt = mono_format1_tables()
    save_sbit_font("sbit_mono_format1.ttf", eblc, ebdt)


def build_gray2_format1_bitmap() -> None:
    eblc, ebdt = gray2_format1_tables()
    save_sbit_font("sbit_gray2_format1.ttf", eblc, ebdt)


def build_gray4_format1_bitmap() -> None:
    eblc, ebdt = gray4_format1_tables()
    save_sbit_font("sbit_gray4_format1.ttf", eblc, ebdt)


def build_bgra_format1_bitmap() -> None:
    eblc, ebdt = bgra_format1_tables()
    save_sbit_font("sbit_bgra_format1.ttf", eblc, ebdt)


def build_unsupported_bit_depth_bitmap() -> None:
    eblc, ebdt = unsupported_bit_depth_format1_tables()
    save_sbit_font("sbit_unsupported_bit_depth_format1.ttf", eblc, ebdt)


def build_unsupported_image_format_bitmap() -> None:
    eblc, ebdt = unsupported_image_format_tables()
    save_sbit_font("sbit_unsupported_image_format.ttf", eblc, ebdt)


def build_missing_small_metrics_width_bitmap() -> None:
    eblc, ebdt = missing_small_metrics_width_tables()
    save_sbit_font("sbit_missing_small_metrics_width.ttf", eblc, ebdt)


def build_gray_format3_bitmap() -> None:
    eblc, ebdt = gray_format3_tables()
    save_sbit_font("sbit_gray_format3.ttf", eblc, ebdt)


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


def build_composite_success() -> None:
    eblc, ebdt = compound_success_format8_tables()
    save_sbit_font("sbit_composite_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_success_format9_tables()
    save_sbit_font("sbit_composite_success_format9.ttf", eblc, ebdt)
    eblc, ebdt = compound_mono_success_format8_tables()
    save_sbit_font("sbit_composite_mono_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_mono_zero_width_component_format8_tables()
    save_sbit_font("sbit_composite_mono_zero_width_component_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_mono_shifted_success_format8_tables()
    save_sbit_font("sbit_composite_mono_shifted_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_mono_carry_success_format8_tables()
    save_sbit_font("sbit_composite_mono_carry_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_gray2_success_format8_tables()
    save_sbit_font("sbit_composite_gray2_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_gray4_success_format8_tables()
    save_sbit_font("sbit_composite_gray4_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_bgra_success_format8_tables()
    save_sbit_font("sbit_composite_bgra_success_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_negative_offset_tables()
    save_sbit_font("sbit_composite_negative_offset_format8.ttf", eblc, ebdt)
    eblc, ebdt = compound_out_of_bounds_tables()
    save_sbit_font("sbit_composite_out_of_bounds_format8.ttf", eblc, ebdt)


def build_composite_malformed(
    name: str,
    index_format: int,
    image_format: int,
    malformed: str,
) -> None:
    eblc, ebdt = compound_malformed_tables(index_format, image_format, malformed)
    save_sbit_font(name, eblc, ebdt)


def build_composite_missing_subglyphs() -> None:
    build_composite_success()
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
    build_gray_format1_bitmap()
    build_mono_format1_bitmap()
    build_gray2_format1_bitmap()
    build_gray4_format1_bitmap()
    build_bgra_format1_bitmap()
    build_unsupported_bit_depth_bitmap()
    build_unsupported_image_format_bitmap()
    build_missing_small_metrics_width_bitmap()
    build_gray_format3_bitmap()
    build_sbit_error_branch_fixtures()
    build_composite_missing_subglyphs()


if __name__ == "__main__":
    main()
