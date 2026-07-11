#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise structural `fvar` parsing paths."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "variable" / "compact-variable.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "variable"


def write_short_header() -> None:
    write_fvar_payload("fvar-short.ttf", base_fvar_payload()[:8])


def write_unsupported_version() -> None:
    payload = bytearray(base_fvar_payload()[:16])
    payload[0:2] = (2).to_bytes(2, "big")
    write_fvar_payload("fvar-version-2.ttf", bytes(payload))


def write_instance_array_short() -> None:
    payload = base_fvar_payload()
    axes_offset = int.from_bytes(payload[4:6], "big")
    axis_count = int.from_bytes(payload[8:10], "big")
    axis_size = int.from_bytes(payload[10:12], "big")
    instances_offset = axes_offset + axis_count * axis_size
    write_fvar_payload("fvar-instance-array-short.ttf", payload[: instances_offset + 1])


def write_instance_size_short() -> None:
    payload = bytearray(base_fvar_payload())
    axes_offset = int.from_bytes(payload[4:6], "big")
    axis_count = int.from_bytes(payload[8:10], "big")
    axis_size = int.from_bytes(payload[10:12], "big")
    instance_count = int.from_bytes(payload[12:14], "big")
    min_instance_size = 4 + axis_count * 4
    short_instance_size = min_instance_size - 1
    payload[14:16] = short_instance_size.to_bytes(2, "big")
    instances_offset = axes_offset + axis_count * axis_size
    table_len = instances_offset + instance_count * short_instance_size
    write_fvar_payload("fvar-instance-size-short.ttf", bytes(payload[:table_len]))


def write_instance_postscript_name() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    for instance in font["fvar"].instances:
        instance.postscriptNameID = 6
    save_font(OUT_DIR / "fvar-instance-postscript-name.ttf", font)


def save_font(path: Path, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=False)


def base_fvar_payload() -> bytes:
    font = TTFont(BASE_FONT, lazy=True)
    record = font.reader.tables["fvar"]
    font.reader.file.seek(record.offset)
    return font.reader.file.read(record.length)


def write_fvar_payload(name: str, payload: bytes) -> None:
    path = OUT_DIR / name
    if path.exists() or path.is_symlink():
        path.unlink()
    path.write_bytes(BASE_FONT.read_bytes())
    replace_table_bytes(path, b"fvar", payload)


def replace_table_bytes(path: Path, tag: bytes, payload: bytes) -> None:
    data = bytearray(path.read_bytes())
    records = table_records(data)
    record = records[tag]
    old_offset = record["offset"]
    old_length = record["length"]
    old_storage_len = padded_length(old_length)
    new_storage = bytes(payload) + (b"\0" * ((4 - len(payload) % 4) % 4))
    delta = len(new_storage) - old_storage_len

    data = (
        data[:old_offset]
        + bytearray(new_storage)
        + data[old_offset + old_storage_len :]
    )
    for other in records.values():
        if other["offset"] > old_offset:
            other["offset"] += delta
            data[other["record_offset"] + 8 : other["record_offset"] + 12] = other[
                "offset"
            ].to_bytes(4, "big")

    data[record["record_offset"] + 4 : record["record_offset"] + 8] = checksum(
        payload
    ).to_bytes(4, "big")
    data[record["record_offset"] + 12 : record["record_offset"] + 16] = len(
        payload
    ).to_bytes(4, "big")
    update_head_checksum_adjustment(data, records)
    path.write_bytes(data)


def update_head_checksum_adjustment(
    data: bytearray, records: dict[bytes, dict[str, int]]
) -> None:
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


def table_records(data: bytearray) -> dict[bytes, dict[str, int]]:
    count = int.from_bytes(data[4:6], "big")
    records = {}
    for index in range(count):
        record_offset = 12 + index * 16
        tag = bytes(data[record_offset : record_offset + 4])
        records[tag] = {
            "record_offset": record_offset,
            "offset": int.from_bytes(
                data[record_offset + 8 : record_offset + 12], "big"
            ),
            "length": int.from_bytes(
                data[record_offset + 12 : record_offset + 16], "big"
            ),
        }
    return records


def padded_length(length: int) -> int:
    return length + ((4 - length % 4) % 4)


def checksum(table_data: bytes | bytearray) -> int:
    padded = bytes(table_data) + (b"\0" * ((4 - len(table_data) % 4) % 4))
    total = 0
    for offset in range(0, len(padded), 4):
        total = (
            total + int.from_bytes(padded[offset : offset + 4], "big")
        ) & 0xFFFF_FFFF
    return total


def main() -> None:
    write_short_header()
    write_unsupported_version()
    write_instance_array_short()
    write_instance_size_short()
    write_instance_postscript_name()


if __name__ == "__main__":
    main()
