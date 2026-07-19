#!/usr/bin/env python3
"""Generate deterministic Windows FNT fixtures for WinFNT public API parity."""

from __future__ import annotations

from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[1]
FONT_ROOT = ROOT / "tests" / "fixtures" / "fonts" / "winfnt"

CHARSETS = {
    "cp1250": 238,
    "cp1251": 204,
    "cp1252": 0,
    "cp1253": 161,
    "cp1254": 162,
    "cp1255": 177,
    "cp1256": 178,
    "cp1257": 186,
    "cp1258": 163,
    "cp1361": 130,
    "cp874": 222,
    "cp932": 128,
    "cp936": 134,
    "cp949": 129,
    "cp950": 136,
    "mac": 77,
    "oem": 255,
    "symbol": 2,
}


def put_u16(buf: bytearray, offset: int, value: int) -> None:
    struct.pack_into("<H", buf, offset, value)


def put_u32(buf: bytearray, offset: int, value: int) -> None:
    struct.pack_into("<I", buf, offset, value)


def build_fnt(path: Path, charset: int, family: str, exercise_header: bool) -> None:
    header_size = 148
    face_name = family.encode("ascii") + b"\0"
    bits_offset = header_size + len(face_name)
    payload = b"\x80" * 8
    file_size = bits_offset + len(payload)
    buf = bytearray(file_size)

    put_u16(buf, 0, 0x0300)
    put_u32(buf, 2, file_size)
    copyright_text = b"pillow-rs deterministic WinFNT fixture"
    buf[6 : 6 + len(copyright_text)] = copyright_text
    put_u16(buf, 66, 0)  # bitmap FNT, not vector
    put_u16(buf, 68, 8 if not exercise_header else 9)
    put_u16(buf, 70, 72)
    put_u16(buf, 72, 96 if exercise_header else 72)
    put_u16(buf, 74, 7)
    put_u16(buf, 76, 1 if exercise_header else 0)
    put_u16(buf, 78, 2 if exercise_header else 0)
    buf[80] = 1 if exercise_header else 0
    buf[81] = 1 if exercise_header else 0
    buf[82] = 0
    put_u16(buf, 83, 700 if exercise_header else 400)
    buf[85] = charset
    put_u16(buf, 86, 0)
    put_u16(buf, 88, 8)
    buf[90] = 0x31 if exercise_header else 0
    put_u16(buf, 91, 5 if exercise_header else 1)
    put_u16(buf, 93, 7 if exercise_header else 1)
    buf[95] = 32
    buf[96] = 33 if exercise_header else 32
    buf[97] = 0
    buf[98] = 0
    put_u16(buf, 99, 1)
    put_u32(buf, 101, 0)
    put_u32(buf, 105, header_size)
    put_u32(buf, 109, 0)
    put_u32(buf, 113, bits_offset)
    buf[117] = 0x5A if exercise_header else 0
    put_u32(buf, 118, 0x01020304 if exercise_header else 0)
    put_u16(buf, 122, 1 if exercise_header else 0)
    put_u16(buf, 124, 5 if exercise_header else 0)
    put_u16(buf, 126, 1 if exercise_header else 0)
    put_u32(buf, 128, 0x22 if exercise_header else 0)
    if exercise_header:
        # FreeType copies these 16 bytes directly into FT_ULong reserved1[4].
        # On the LP64 oracle ABI this yields [0x11121314, 0x21222324, 0, 0].
        put_u32(buf, 132, 0x11121314)
        put_u32(buf, 140, 0x21222324)
    buf[header_size:bits_offset] = face_name
    buf[bits_offset:] = payload

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(buf)


def main() -> None:
    build_fnt(FONT_ROOT / "bitmap-header.fnt", 0, "PillowRsWinFNT", True)
    charset_root = FONT_ROOT / "charset"
    for name, value in sorted(CHARSETS.items()):
        build_fnt(
            charset_root / f"{name}-{value}.fnt",
            value,
            f"WinFNT{name.upper()}",
            False,
        )


if __name__ == "__main__":
    main()
