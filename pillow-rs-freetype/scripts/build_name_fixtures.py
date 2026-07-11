#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise `name` table selection paths."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASE_STATIC = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
BASE_VARIABLE = ROOT / "tests" / "fixtures" / "fonts" / "variable" / "compact-variable.ttf"
NAME_OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "names"
VARIABLE_OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "variable"


@dataclass(frozen=True)
class NameRecordSpec:
    platform_id: int
    encoding_id: int
    language_id: int
    name_id: int
    data: bytes
    offset_override: int | None = None


def utf16be(text: str) -> bytes:
    return text.encode("utf-16-be")


def mac_roman(text: str) -> bytes:
    return text.encode("ascii")


def write_static_selection_fallbacks() -> None:
    records = [
        NameRecordSpec(5, 0, 0, 16, b"Ignored"),
        NameRecordSpec(1, 0, 0, 16, b"Broken", offset_override=0xFF00),
        NameRecordSpec(0, 3, 0, 16, utf16be("UniFamily")),
        NameRecordSpec(1, 0, 1, 17, mac_roman("RomanStyle")),
        NameRecordSpec(3, 1, 0x0409, 6, utf16be("StaticPS")),
    ]
    write_name_payload(
        BASE_STATIC,
        NAME_OUT_DIR / "name-selection-fallbacks.ttf",
        build_name_table(records),
    )


def write_apple_postscript() -> None:
    records = [
        NameRecordSpec(3, 1, 0x0409, 1, utf16be("AppleOnly")),
        NameRecordSpec(3, 1, 0x0409, 2, utf16be("Regular")),
        NameRecordSpec(1, 0, 0, 6, mac_roman("ApplePS")),
    ]
    write_name_payload(
        BASE_STATIC,
        NAME_OUT_DIR / "name-apple-postscript.ttf",
        build_name_table(records),
    )


def write_odd_windows_postscript_with_apple_fallback() -> None:
    records = [
        NameRecordSpec(3, 1, 0x0409, 1, utf16be("OddWin")),
        NameRecordSpec(3, 1, 0x0409, 2, utf16be("Regular")),
        NameRecordSpec(3, 1, 0x0409, 6, b"X"),
        NameRecordSpec(1, 0, 0, 6, mac_roman("ApplePS")),
    ]
    write_name_payload(
        BASE_STATIC,
        NAME_OUT_DIR / "name-win-postscript-odd-apple.ttf",
        build_name_table(records),
    )


def write_postscript_branch_matrix() -> None:
    records = [
        NameRecordSpec(3, 1, 0x0409, 16, b""),
        NameRecordSpec(1, 1, 2, 16, mac_roman("AppleEncodingIgnored")),
        NameRecordSpec(3, 1, 0x0411, 16, utf16be("WinJPFamily")),
        NameRecordSpec(3, 1, 0x0412, 16, utf16be("WinKRFamily")),
        NameRecordSpec(1, 0, 1, 16, mac_roman("AppleFamily")),
        NameRecordSpec(3, 1, 0x0409, 17, b""),
        NameRecordSpec(1, 0, 0, 17, mac_roman("AppleEnglishStyle")),
        NameRecordSpec(3, 1, 0x0409, 6, b""),
        NameRecordSpec(3, 1, 0x0411, 6, utf16be("WinJPPS")),
        NameRecordSpec(3, 1, 0x0412, 6, utf16be("WinKRPS")),
        NameRecordSpec(3, 1, 0x0409, 6, b"\0 "),
        NameRecordSpec(1, 0, 1, 6, mac_roman("Apple PS")),
        NameRecordSpec(1, 0, 2, 6, mac_roman("IgnoredApple")),
    ]
    write_name_payload(
        BASE_STATIC,
        NAME_OUT_DIR / "name-postscript-branch-matrix.ttf",
        build_name_table(records),
    )


def variable_base_records() -> list[NameRecordSpec]:
    return [
        NameRecordSpec(3, 1, 0x0409, 1, utf16be("Ubuntu")),
        NameRecordSpec(3, 1, 0x0409, 2, utf16be("Regular")),
        NameRecordSpec(3, 1, 0x0409, 6, utf16be("Ubuntu-Regular")),
        NameRecordSpec(3, 1, 0x0409, 257, utf16be("Width")),
        NameRecordSpec(3, 1, 0x0409, 258, utf16be("Weight")),
    ]


def variable_base_without_instance_names() -> list[NameRecordSpec]:
    return [
        NameRecordSpec(3, 1, 0x0409, 1, utf16be("Ubuntu")),
        NameRecordSpec(3, 1, 0x0409, 2, utf16be("Regular")),
        NameRecordSpec(3, 1, 0x0409, 6, utf16be("Ubuntu-Regular")),
        NameRecordSpec(3, 1, 0x0409, 257, utf16be("Width")),
        NameRecordSpec(3, 1, 0x0409, 258, utf16be("Weight")),
    ]


def write_variable_apple_prefix() -> None:
    records = variable_base_records()
    records.extend(
        [
            NameRecordSpec(5, 0, 0, 25, b"Ignored"),
            NameRecordSpec(1, 0, 0, 25, mac_roman("AppleVar")),
            NameRecordSpec(1, 0, 1, 259, mac_roman("Thin")),
            NameRecordSpec(1, 0, 1, 260, mac_roman("Light")),
        ]
    )
    write_name_payload(
        BASE_VARIABLE,
        VARIABLE_OUT_DIR / "variable-name-apple-prefix.ttf",
        build_name_table(records),
    )


def write_variable_unicode_prefix() -> None:
    records = variable_base_without_instance_names()
    records.extend(
        [
            NameRecordSpec(5, 0, 0, 25, b"Ignored"),
            NameRecordSpec(0, 3, 0, 25, utf16be("UniVar")),
            NameRecordSpec(0, 3, 0, 259, utf16be("Thin")),
            NameRecordSpec(0, 3, 0, 260, utf16be("Light")),
        ]
    )
    write_name_payload(
        BASE_VARIABLE,
        VARIABLE_OUT_DIR / "variable-name-unicode-prefix.ttf",
        build_name_table(records),
    )


def write_variable_odd_windows_prefix() -> None:
    records = variable_base_without_instance_names()
    records.extend(
        [
            NameRecordSpec(3, 1, 0x0409, 25, b"X"),
            NameRecordSpec(1, 0, 0, 25, mac_roman("AppleVar")),
            NameRecordSpec(1, 0, 1, 259, mac_roman("Thin")),
        ]
    )
    write_name_payload(
        BASE_VARIABLE,
        VARIABLE_OUT_DIR / "variable-name-odd-win-prefix.ttf",
        build_name_table(records),
    )


def write_variable_prefix_branch_matrix() -> None:
    records = variable_base_without_instance_names()
    records.extend(
        [
            NameRecordSpec(3, 1, 0x0411, 25, utf16be("WinJPVar")),
            NameRecordSpec(3, 1, 0x0412, 25, utf16be("WinKRVar")),
            NameRecordSpec(3, 1, 0x0409, 25, b"X\0\0 "),
            NameRecordSpec(1, 0, 1, 25, mac_roman("Apple Var")),
            NameRecordSpec(1, 0, 2, 25, mac_roman("IgnoredApple")),
            NameRecordSpec(5, 0, 0, 259, b"Ignored"),
        ]
    )
    write_name_payload(
        BASE_VARIABLE,
        VARIABLE_OUT_DIR / "variable-name-prefix-branch-matrix.ttf",
        build_name_table(records),
    )


def write_variable_missing_subfamily() -> None:
    records = variable_base_without_instance_names()
    records.extend(
        [
            NameRecordSpec(3, 1, 0x0409, 25, utf16be("MissingVar")),
            NameRecordSpec(5, 0, 0, 259, b"Ignored"),
            NameRecordSpec(5, 0, 0, 260, b"Ignored"),
            NameRecordSpec(5, 0, 0, 261, b"Ignored"),
            NameRecordSpec(5, 0, 0, 262, b"Ignored"),
        ]
    )
    path = VARIABLE_OUT_DIR / "variable-name-missing-subfamily.ttf"
    write_name_payload(
        BASE_VARIABLE,
        path,
        build_name_table(records),
    )
    replace_table_bytes(path, b"fvar", build_missing_subfamily_fvar())


def build_missing_subfamily_fvar() -> bytes:
    payload = bytearray(table_payload(BASE_VARIABLE, b"fvar"))
    axes_offset = int.from_bytes(payload[4:6], "big")
    axis_count = int.from_bytes(payload[8:10], "big")
    axis_size = int.from_bytes(payload[10:12], "big")
    instance_size = int.from_bytes(payload[14:16], "big")
    instances_offset = axes_offset + axis_count * axis_size

    axis0 = axes_offset
    payload[axis0 : axis0 + 4] = b"w d!"
    payload[axis0 + 4 : axis0 + 8] = (-1 << 16).to_bytes(4, "big", signed=True)
    payload[axis0 + 8 : axis0 + 12] = (0).to_bytes(4, "big", signed=True)
    payload[axis0 + 12 : axis0 + 16] = (1 << 16).to_bytes(4, "big", signed=True)

    axis1 = axes_offset + axis_size
    payload[axis1 + 4 : axis1 + 8] = (-1 << 16).to_bytes(4, "big", signed=True)

    coordinate_rows = [
        (0, 100 << 16),
        (0, 0),
        (0, -1 << 16),
        (1, 400 << 16),
    ]
    for index, (wdth, wght) in enumerate(coordinate_rows):
        off = instances_offset + index * instance_size
        payload[off + 4 : off + 8] = wdth.to_bytes(4, "big", signed=True)
        payload[off + 8 : off + 12] = wght.to_bytes(4, "big", signed=True)
    return bytes(payload)


def build_name_table(records: list[NameRecordSpec]) -> bytes:
    string_offset = 6 + len(records) * 12
    storage = bytearray()
    rows = bytearray()
    for record in records:
        if record.offset_override is None:
            offset = len(storage)
            storage.extend(record.data)
        else:
            offset = record.offset_override
        rows.extend(record.platform_id.to_bytes(2, "big"))
        rows.extend(record.encoding_id.to_bytes(2, "big"))
        rows.extend(record.language_id.to_bytes(2, "big"))
        rows.extend(record.name_id.to_bytes(2, "big"))
        rows.extend(len(record.data).to_bytes(2, "big"))
        rows.extend(offset.to_bytes(2, "big"))
    return (
        (0).to_bytes(2, "big")
        + len(records).to_bytes(2, "big")
        + string_offset.to_bytes(2, "big")
        + bytes(rows)
        + bytes(storage)
    )


def write_name_payload(base: Path, path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    path.write_bytes(base.read_bytes())
    replace_table_bytes(path, b"name", payload)


def table_payload(path: Path, tag: bytes) -> bytes:
    data = bytearray(path.read_bytes())
    record = table_records(data)[tag]
    start = record["offset"]
    end = start + record["length"]
    return bytes(data[start:end])


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
    write_static_selection_fallbacks()
    write_apple_postscript()
    write_odd_windows_postscript_with_apple_fallback()
    write_postscript_branch_matrix()
    write_variable_apple_prefix()
    write_variable_unicode_prefix()
    write_variable_odd_windows_prefix()
    write_variable_prefix_branch_matrix()
    write_variable_missing_subfamily()


if __name__ == "__main__":
    main()
