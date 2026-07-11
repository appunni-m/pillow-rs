#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise PostScript `post` name formats."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "metadata"


def save_font(path: Path, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=True)


def write_format_1() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["post"].formatType = 1.0
    save_font(OUT_DIR / "post-format-1.ttf", font)


def write_format_25() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["post"].formatType = 3.0
    num_glyphs = font["maxp"].numGlyphs
    path = OUT_DIR / "post-format-25.ttf"
    save_font(path, font)

    data = bytearray(path.read_bytes())
    records = table_records(data)
    post = records[b"post"]
    header = bytearray(data[post["offset"] : post["offset"] + 32])
    # Pinned FreeType's `ttpost.c` recognizes format 2.5 by the historical
    # fixed value 0x00025000, not the mathematically exact 16.16 encoding.
    header[0:4] = (0x0002_5000).to_bytes(4, "big")

    deltas = [0] * num_glyphs
    if num_glyphs > 1:
        deltas[1] = -5
    payload = bytes(header) + num_glyphs.to_bytes(2, "big") + bytes(delta & 0xFF for delta in deltas)
    patch_table_bytes(path, b"post", payload)


def patch_table_bytes(path: Path, tag: bytes, payload: bytes) -> None:
    data = bytearray(path.read_bytes())
    records = table_records(data)
    record = records[tag]
    data[record["offset"] : record["offset"] + len(payload)] = payload
    data[record["record_offset"] + 4 : record["record_offset"] + 8] = checksum(payload).to_bytes(
        4, "big"
    )
    data[record["record_offset"] + 12 : record["record_offset"] + 16] = len(payload).to_bytes(
        4, "big"
    )

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
    write_format_1()
    write_format_25()


if __name__ == "__main__":
    main()
