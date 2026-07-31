#!/usr/bin/env python3
"""Generate Font fixtures with variation-name fallback records.

The source fixture has variation axis and instance names only as Windows
Unicode/en-US records.  The generated fixtures change only those `name` table
records so Font parity tests exercise fallback paths without embedding expected
oracle output.
"""

from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[1]
DEJAVU_SOURCE = ROOT / "tests/fixtures/assets/font/fonts/DejaVuSans.ttf"
SOURCE = ROOT / "tests/fixtures/assets/font/fonts/variable-named-instances.ttf"
WINDOWS_TARGET = ROOT / "tests/fixtures/assets/font/fonts/variable-name-windows-fallback.ttf"
PLATFORM1_TARGET = ROOT / "tests/fixtures/assets/font/fonts/variable-name-platform1-fallback.ttf"
MISSING_FAMILY_STYLE_TARGET = (
    ROOT / "tests/fixtures/assets/font/fonts/dejavu-missing-family-style.ttf"
)


def table(data: bytes, tag: bytes) -> tuple[int, int]:
    table_count = struct.unpack_from(">H", data, 4)[0]
    for index in range(table_count):
        offset = 12 + index * 16
        record_tag, _, table_offset, table_length = struct.unpack_from(">4sIII", data, offset)
        if record_tag == tag:
            return table_offset, table_length
    raise ValueError(f"missing table {tag!r}")


def variation_name_ids(data: bytes) -> set[int]:
    offset, _ = table(data, b"fvar")
    _, _, axes_offset, _, axis_count, axis_size, instance_count, instance_size = (
        struct.unpack_from(">HHHHHHHH", data, offset)
    )
    ids = set()
    axes_base = offset + axes_offset
    for index in range(axis_count):
        axis_offset = axes_base + index * axis_size
        ids.add(struct.unpack_from(">H", data, axis_offset + 18)[0])
    instance_base = axes_base + axis_count * axis_size
    for index in range(instance_count):
        instance_offset = instance_base + index * instance_size
        ids.add(struct.unpack_from(">H", data, instance_offset)[0])
    return ids


def write_missing_family_style_fixture() -> None:
    """Retag family/style name records so FreeType reports no family/style."""
    data = bytearray(DEJAVU_SOURCE.read_bytes())
    name_offset, _ = table(data, b"name")
    _, record_count, _ = struct.unpack_from(">HHH", data, name_offset)

    changed = 0
    for index in range(record_count):
        record_offset = name_offset + 6 + index * 12
        name_id = struct.unpack_from(">H", data, record_offset + 6)[0]
        if name_id in {1, 2, 16, 17}:
            struct.pack_into(">H", data, record_offset + 6, 65000 + name_id)
            changed += 1

    if changed != 4:
        raise SystemExit(f"expected to retag 4 family/style records, patched {changed}")
    MISSING_FAMILY_STYLE_TARGET.write_bytes(data)


def main() -> None:
    write_missing_family_style_fixture()

    source = SOURCE.read_bytes()
    ids = variation_name_ids(source)

    windows = bytearray(source)
    name_offset, _ = table(windows, b"name")
    _, record_count, _ = struct.unpack_from(">HHH", windows, name_offset)

    changed = 0
    for index in range(record_count):
        record_offset = name_offset + 6 + index * 12
        platform_id, encoding_id, language_id, name_id = struct.unpack_from(
            ">HHHH", windows, record_offset
        )
        if (
            name_id in ids
            and platform_id == 3
            and encoding_id in {1, 10}
            and language_id == 0x0409
        ):
            struct.pack_into(">H", windows, record_offset + 4, 0x040C)
            changed += 1

    if changed != len(ids):
        raise SystemExit(
            f"expected to patch {len(ids)} Windows fallback records, patched {changed}"
        )
    WINDOWS_TARGET.write_bytes(windows)

    platform1 = bytearray(source)
    name_offset, _ = table(platform1, b"name")
    _, record_count, _ = struct.unpack_from(">HHH", platform1, name_offset)

    changed = 0
    for index in range(record_count):
        record_offset = name_offset + 6 + index * 12
        platform_id, encoding_id, language_id, name_id = struct.unpack_from(
            ">HHHH", platform1, record_offset
        )
        if (
            name_id in ids
            and platform_id == 3
            and encoding_id in {1, 10}
            and language_id == 0x0409
        ):
            struct.pack_into(">HHH", platform1, record_offset, 1, 0, 0)
            changed += 1

    if changed != len(ids):
        raise SystemExit(
            f"expected to patch {len(ids)} Platform 1 fallback records, patched {changed}"
        )
    PLATFORM1_TARGET.write_bytes(platform1)


if __name__ == "__main__":
    main()
