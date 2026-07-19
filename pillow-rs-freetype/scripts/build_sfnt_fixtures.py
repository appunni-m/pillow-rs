#!/usr/bin/env python3
"""Build compact SFNT table fixtures for public table APIs."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "sfnt"
GENERATED_OUT_DIR = ROOT / "tests" / "fixtures" / "generated" / "sfnt"
MALFORMED_TTC_OUT_DIR = ROOT / "tests" / "fixtures" / "malformed" / "ttc"


def save_font(name: str, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def save_generated_font(name: str, font: TTFont) -> None:
    GENERATED_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = GENERATED_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def save_malformed_ttc(name: str, data: bytes) -> None:
    MALFORMED_TTC_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = MALFORMED_TTC_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    out.write_bytes(data)


def base_font() -> TTFont:
    return TTFont(BASE_FONT, recalcTimestamp=False)


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def pclt_table(version: int) -> DefaultTable:
    typeface = b"Compact SFNT".ljust(16, b"\0")
    complement = b"COVRAGE1"
    filename = b"CSFNT1"
    data = struct.pack(
        ">LLHHHHHH16s8s6sbbBB",
        version,
        42,
        640,
        450,
        1,
        2,
        700,
        0x04E4,
        typeface,
        complement,
        filename,
        -3,
        5,
        2,
        0,
    )
    return raw_table("PCLT", data)


def add_vertical_metrics(font: TTFont) -> None:
    glyph_order = font.getGlyphOrder()
    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (1000, 120) for name in glyph_order}
    font["vmtx"] = vmtx

    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 880
    vhea.descent = -120
    vhea.lineGap = 20
    vhea.advanceHeightMax = 1000
    vhea.minTopSideBearing = 120
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 1000
    vhea.caretSlopeRise = 1
    vhea.caretSlopeRun = 0
    vhea.caretOffset = 0
    vhea.reserved1 = 0
    vhea.reserved2 = 0
    vhea.reserved3 = 0
    vhea.reserved4 = 0
    vhea.metricDataFormat = 0
    vhea.numberOfVMetrics = len(glyph_order)
    font["vhea"] = vhea


def write_basic() -> None:
    font = base_font()
    save_font("basic-ttf.ttf", font)


def write_basic_alias(name: str) -> None:
    font = base_font()
    save_font(name, font)


def write_pclt_present() -> None:
    font = base_font()
    font["PCLT"] = pclt_table(0x00010000)
    save_font("pclt-present.ttf", font)


def write_pclt_short() -> None:
    font = base_font()
    font["PCLT"] = raw_table("PCLT", b"\0" * 12)
    save_font("pclt-short.ttf", font)


def write_pclt_version_zero() -> None:
    font = base_font()
    font["PCLT"] = pclt_table(0)
    save_font("pclt-version-zero.ttf", font)


def write_vertical_present() -> None:
    font = base_font()
    add_vertical_metrics(font)
    save_font("vhea-vmtx-present.ttf", font)


def write_no_os2() -> None:
    font = base_font()
    del font["OS/2"]
    save_font("no-os2.ttf", font)


def write_missing_hmtx() -> None:
    font = base_font()
    # FreeType 2.14.3 sfnt/sfobjs.c reports FT_Err_Hmtx_Table_Missing when
    # opening a TrueType SFNT with `hhea` present but no `hmtx` metrics table.
    del font["hmtx"]
    save_generated_font("missing-hmtx.ttf", font)


def write_ttc_count_overflow() -> None:
    # FreeType 2.14.3 sfnt/sfobjs.c rejects this TTC header as
    # FT_Err_Array_Too_Large because the declared face-count makes the offset
    # array larger than the stream before any face directory is read.
    save_malformed_ttc(
        "count-overflows-offset-array.ttc",
        b"ttcf" + (0x0001_0000).to_bytes(4, "big") + (0x4000_0000).to_bytes(4, "big"),
    )


def main() -> None:
    write_basic()
    write_basic_alias("pclt-missing.ttf")
    write_basic_alias("no-vhea.ttf")
    write_pclt_present()
    write_pclt_short()
    write_pclt_version_zero()
    write_vertical_present()
    write_no_os2()
    write_missing_hmtx()
    write_ttc_count_overflow()


if __name__ == "__main__":
    main()
