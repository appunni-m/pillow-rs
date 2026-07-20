#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise structural `fvar` parsing paths."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._f_v_a_r import Axis


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "variable" / "compact-variable.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "variable"


def write_short_header() -> None:
    write_fvar_payload("fvar-short.ttf", base_fvar_payload()[:8])


def write_unsupported_version() -> None:
    payload = bytearray(base_fvar_payload()[:20])
    payload[0:2] = (2).to_bytes(2, "big")
    write_fvar_payload("fvar-version-2.ttf", bytes(payload))


def write_unsupported_minor_version() -> None:
    payload = bytearray(base_fvar_payload())
    payload[2:4] = (1).to_bytes(2, "big")
    write_fvar_payload("fvar-minor-version-1.ttf", bytes(payload))


def write_axis_size_short() -> None:
    payload = bytearray(base_fvar_payload())
    axes_offset = int.from_bytes(payload[4:6], "big")
    axis_count = int.from_bytes(payload[8:10], "big")
    instance_count = int.from_bytes(payload[12:14], "big")
    instance_size = int.from_bytes(payload[14:16], "big")
    short_axis_size = 19
    payload[10:12] = short_axis_size.to_bytes(2, "big")
    instances_offset = axes_offset + axis_count * short_axis_size
    table_len = instances_offset + instance_count * instance_size
    write_fvar_payload("fvar-axis-size-short.ttf", bytes(payload[:table_len]))


def write_axis_size_long() -> None:
    payload = bytearray(base_fvar_payload())
    long_axis_size = 21
    payload[10:12] = long_axis_size.to_bytes(2, "big")
    resize_for_declared_arrays(payload)
    write_fvar_payload("fvar-axis-size-long.ttf", bytes(payload))


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


def write_instance_size_long() -> None:
    payload = bytearray(base_fvar_payload())
    axis_count = int.from_bytes(payload[8:10], "big")
    long_instance_size = 4 + axis_count * 4 + 4
    payload[14:16] = long_instance_size.to_bytes(2, "big")
    resize_for_declared_arrays(payload)
    write_fvar_payload("fvar-instance-size-long.ttf", bytes(payload))


def write_axis_count_limit() -> None:
    payload = bytearray(base_fvar_payload()[:20])
    payload[8:10] = (0x3FFF).to_bytes(2, "big")
    payload[10:12] = (20).to_bytes(2, "big")
    payload[12:14] = (0).to_bytes(2, "big")
    payload[14:16] = (4).to_bytes(2, "big")
    write_fvar_payload("fvar-axis-count-too-large.ttf", bytes(payload))


def write_instance_count_limit() -> None:
    payload = bytearray(base_fvar_payload()[:20])
    payload[12:14] = (0x7F00).to_bytes(2, "big")
    write_fvar_payload("fvar-instance-count-too-large.ttf", bytes(payload))


def write_instance_postscript_name() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    for instance in font["fvar"].instances:
        instance.postscriptNameID = 6
    save_font(OUT_DIR / "fvar-instance-postscript-name.ttf", font)


def write_three_axis_opsz_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    fvar = font["fvar"]

    axis = Axis()
    axis.axisTag = "opsz"
    axis.minValue = 8.0
    axis.defaultValue = 14.0
    axis.maxValue = 72.0
    axis.flags = 0
    axis.axisNameID = 271
    fvar.axes.append(axis)

    name = font["name"]
    name.setName("Optical Size", axis.axisNameID, 3, 1, 0x0409)
    name.setName("Optical Size", axis.axisNameID, 1, 0, 0)
    for instance in fvar.instances:
        instance.coordinates[axis.axisTag] = axis.defaultValue

    save_font(OUT_DIR / "wght-wdth-opsz.ttf", font)


def write_multi_axis_visible_font() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    save_font(OUT_DIR / "multi-axis-visible.ttf", font)


def write_hidden_axis_fonts() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # FreeType stores OpenType fvar axis flags in the FT_MM_Var-adjacent
    # FT_UShort array and exposes them through FT_Get_Var_Axis_Flags
    # (src/base/ftmm.c:604-613).  Keep one visible axis and mark one axis
    # hidden so public rows can compare both classes against the C oracle.
    font["fvar"].axes[1].flags = 1
    save_font(OUT_DIR / "hidden-axis.ttf", font)
    save_font(OUT_DIR / "named-instances-hidden-axis.ttf", font)


def write_zero_axis() -> None:
    payload = bytearray(base_fvar_payload()[:20])
    payload[8:10] = (0).to_bytes(2, "big")
    payload[12:14] = (0).to_bytes(2, "big")
    payload[14:16] = (4).to_bytes(2, "big")
    write_fvar_payload("fvar-zero-axis.ttf", bytes(payload))


def resize_for_declared_arrays(payload: bytearray) -> None:
    axes_offset = int.from_bytes(payload[4:6], "big")
    axis_count = int.from_bytes(payload[8:10], "big")
    axis_size = int.from_bytes(payload[10:12], "big")
    instance_count = int.from_bytes(payload[12:14], "big")
    instance_size = int.from_bytes(payload[14:16], "big")
    table_len = (
        axes_offset + axis_count * axis_size + instance_count * instance_size
    )
    if len(payload) < table_len:
        payload.extend(b"\0" * (table_len - len(payload)))
    else:
        del payload[table_len:]


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
    write_unsupported_minor_version()
    write_axis_size_short()
    write_axis_size_long()
    write_instance_array_short()
    write_instance_size_short()
    write_instance_size_long()
    write_axis_count_limit()
    write_instance_count_limit()
    write_instance_postscript_name()
    write_three_axis_opsz_font()
    write_multi_axis_visible_font()
    write_hidden_axis_fonts()
    write_zero_axis()


if __name__ == "__main__":
    main()
