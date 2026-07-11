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


def write_malformed_gasp_font(name: str, gasp_payload: bytes, *, truncate_at_payload: bool) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["gasp"] = make_gasp_table(1, {8: 1})

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)
    patch_table_bytes(out, b"gasp", gasp_payload, truncate_at_payload=truncate_at_payload)


def patch_gasp_version(path: Path, version: int) -> None:
    patch_table_bytes(path, b"gasp", version.to_bytes(2, "big"), replace_prefix=True)


def patch_table_bytes(
    path: Path,
    tag: bytes,
    payload: bytes,
    *,
    replace_prefix: bool = False,
    truncate_at_payload: bool = False,
) -> None:
    data = bytearray(path.read_bytes())
    records = table_records(data)
    record = records[tag]
    if replace_prefix:
        payload_len = record["length"]
        table_data = bytearray(data[record["offset"] : record["offset"] + payload_len])
        table_data[: len(payload)] = payload
    else:
        payload_len = len(payload)
        table_data = bytearray(payload)
    data[record["offset"] : record["offset"] + payload_len] = table_data
    data[record["record_offset"] + 4 : record["record_offset"] + 8] = checksum(
        table_data
    ).to_bytes(4, "big")
    data[record["record_offset"] + 12 : record["record_offset"] + 16] = payload_len.to_bytes(
        4, "big"
    )
    if truncate_at_payload:
        del data[record["offset"] + payload_len :]

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
    write_malformed_gasp_font("record-length-short.ttf", b"\0", truncate_at_payload=False)
    write_malformed_gasp_font("short-header.ttf", b"\0", truncate_at_payload=True)
    write_malformed_gasp_font(
        "truncated-ranges.ttf",
        bytes([0, 1, 0, 2, 0, 8, 0, 1]),
        truncate_at_payload=True,
    )


if __name__ == "__main__":
    main()
