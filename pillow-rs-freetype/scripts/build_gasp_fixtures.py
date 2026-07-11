#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise FT_Get_Gasp behavior."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont, newTable


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "gasp"


def make_gasp_table(version: int, ranges: dict[int, int]):
    table = newTable("gasp")
    table.version = version
    table.gaspRange = ranges
    return table


def write_font(name: str, gasp_table) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    if gasp_table is None:
        if "gasp" in font:
            del font["gasp"]
    else:
        font["gasp"] = gasp_table

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)
    if gasp_table is not None:
        patch_gasp_version(out, gasp_table.version)


def patch_gasp_version(path: Path, version: int) -> None:
    data = bytearray(path.read_bytes())
    records = table_records(data)
    gasp = records[b"gasp"]
    data[gasp["offset"] : gasp["offset"] + 2] = version.to_bytes(2, "big")
    data[gasp["record_offset"] + 4 : gasp["record_offset"] + 8] = checksum(
        data[gasp["offset"] : gasp["offset"] + gasp["length"]]
    ).to_bytes(4, "big")

    head = records[b"head"]
    data[head["offset"] + 8 : head["offset"] + 12] = b"\0\0\0\0"
    data[head["record_offset"] + 4 : head["record_offset"] + 8] = checksum(
        data[head["offset"] : head["offset"] + head["length"]]
    ).to_bytes(4, "big")
    adjustment = (0xB1B0AFBA - checksum(data)) & 0xFFFF_FFFF
    data[head["offset"] + 8 : head["offset"] + 12] = adjustment.to_bytes(4, "big")
    data[head["record_offset"] + 4 : head["record_offset"] + 8] = checksum(
        data[head["offset"] : head["offset"] + head["length"]]
    ).to_bytes(4, "big")
    path.write_bytes(data)


def table_records(data: bytearray) -> dict[bytes, dict[str, int]]:
    count = int.from_bytes(data[4:6], "big")
    records = {}
    for index in range(count):
        record_offset = 12 + index * 16
        tag = bytes(data[record_offset : record_offset + 4])
        records[tag] = {
            "record_offset": record_offset,
            "offset": int.from_bytes(data[record_offset + 8 : record_offset + 12], "big"),
            "length": int.from_bytes(data[record_offset + 12 : record_offset + 16], "big"),
        }
    return records


def checksum(table_data: bytes | bytearray) -> int:
    padded = bytes(table_data) + (b"\0" * ((4 - len(table_data) % 4) % 4))
    total = 0
    for offset in range(0, len(padded), 4):
        total = (total + int.from_bytes(padded[offset : offset + 4], "big")) & 0xFFFF_FFFF
    return total


def main() -> None:
    write_font(
        "gasp-ranges-v1.ttf",
        make_gasp_table(
            1,
            {
                5: 1,
                8: 2,
                16: 4,
                65535: 15,
            },
        ),
    )
    write_font("no-gasp-table.ttf", None)
    write_font(
        "version0-high-bits.ttf",
        make_gasp_table(
            0,
            {
                8: 15,
                65535: 3,
            },
        ),
    )
    write_font(
        "unsupported-version.ttf",
        make_gasp_table(
            2,
            {
                8: 15,
                65535: 3,
            },
        ),
    )


if __name__ == "__main__":
    main()
