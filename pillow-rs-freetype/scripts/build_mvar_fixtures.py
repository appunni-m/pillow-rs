#!/usr/bin/env python3
"""Build compact MVAR fixtures for variation metric table parity."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables import otTables as ot
from fontTools.varLib import builder


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "variable" / "compact-variable.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "variation"


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


def mvar_table(font: TTFont):
    axis_tags = [axis.axisTag for axis in font["fvar"].axes]
    # Activate the deltas at the maximum `wght` design coordinate while keeping
    # the `wdth` axis neutral.  FreeType maps these MVAR tags to TT_VertHeader
    # fields in `truetype/ttgxvar.c:1406-1472`.
    region_list = builder.buildVarRegionList(
        [{"wght": (0.0, 1.0, 1.0)}],
        axis_tags,
    )
    deltas = [32, -24, 12, 2, 3, 4]
    var_data = builder.buildVarData([0], [[delta] for delta in deltas], optimize=False)
    var_store = builder.buildVarStore(region_list, [var_data])

    mvar = ot.MVAR()
    mvar.Version = 0x00010000
    mvar.Reserved = 0
    mvar.ValueRecordSize = 8
    mvar.VarStore = var_store
    mvar.ValueRecord = []
    for inner_index, tag in enumerate(["vasc", "vdsc", "vlgp", "vcrs", "vcrn", "vcof"]):
        record = ot.MetricsValueRecord()
        record.ValueTag = tag
        record.VarIdx = inner_index
        mvar.ValueRecord.append(record)
    mvar.ValueRecordCount = len(mvar.ValueRecord)

    table = newTable("MVAR")
    table.table = mvar
    return table


def write_mvar_vertical_metrics() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    add_vertical_metrics(font)
    font["MVAR"] = mvar_table(font)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "mvar-vertical-metrics.ttf"
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    write_mvar_vertical_metrics()


if __name__ == "__main__":
    main()
