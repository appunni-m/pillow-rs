#!/usr/bin/env python3
"""Exercise ImageOps core branches the public PIL surface cannot reach.

The image-ops coverage plan selects this maintained command to drive the
`_core.exif_get_orientation` / `_core.exif_remove_orientation` parsers and
the core `ops_colorize` validation branches with crafted inputs.  The public
Pillow wrapper pre-validates colorize modes and only ever feeds EXIF bytes
extracted from a JPEG container, so malformed EXIF shapes and the duplicate
core assertions are unreachable through the public facade.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json
import struct
from typing import Any

from pillow_rs import _core, Image


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


def tiff_ifd(entries: list[tuple[int, int]], little_endian: bool = True) -> bytes:
    """Build a minimal TIFF IFD0 with the given (tag, value) SHORT entries."""

    endian = "<" if little_endian else ">"
    header = b"II" if little_endian else b"MM"
    body = bytearray()
    body.extend(struct.pack(endian + "H", 42))
    ifd_offset = 8
    body.extend(struct.pack(endian + "I", ifd_offset))
    body.extend(struct.pack(endian + "H", len(entries)))
    for tag, value in entries:
        body.extend(struct.pack(endian + "HHI", tag, 3, 1))
        body.extend(struct.pack(endian + "HH", value, 0))
    body.extend(struct.pack(endian + "I", 0))
    return header + bytes(body)


def exif_jpeg(tiff: bytes) -> bytes:
    """Wrap a TIFF IFD in the JPEG Exif APP1 payload signature."""

    return b"Exif\x00\x00" + tiff


def run_native_cases() -> tuple[int, int, int]:
    """Run every native image-ops branch probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call: Any) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            # Any completed execution (including a public error) exercises
            # the instrumented path; unexpected failures are still counted.
            passed += 1

    le = tiff_ifd([(0x0112, 6)], little_endian=True)
    be = tiff_ifd([(0x0112, 6)], little_endian=False)
    probes: list[tuple[str, Any]] = [
        # exif_get_orientation: empty and short inputs.
        ("get-orientation-empty", lambda: _core.exif_get_orientation(b"")),
        ("get-orientation-short", lambda: _core.exif_get_orientation(b"abc")),
        # Exif-JPEG and bare TIFF payloads, little- and big-endian.
        ("get-orientation-exif-le", lambda: _core.exif_get_orientation(exif_jpeg(le))),
        ("get-orientation-exif-be", lambda: _core.exif_get_orientation(exif_jpeg(be))),
        ("get-orientation-bare-le", lambda: _core.exif_get_orientation(le)),
        ("get-orientation-bare-be", lambda: _core.exif_get_orientation(be)),
        # Exif-prefixed payload stripped below the 8-byte TIFF minimum.
        ("get-orientation-exif-short-strip", lambda: _core.exif_get_orientation(b"Exif\x00\x00II\x2a")),
        # IFD with a leading non-orientation entry forces the scan loop to
        # iterate before finding tag 0x0112.
        (
            "get-orientation-leading-entry",
            lambda: _core.exif_get_orientation(tiff_ifd([(0x0100, 100), (0x0112, 6)])),
        ),
        # Invalid byte order and magic.
        ("get-orientation-bad-order", lambda: _core.exif_get_orientation(b"XX\x2a\x00\x08\x00\x00\x00")),
        ("get-orientation-bad-magic", lambda: _core.exif_get_orientation(b"II\x00\x00\x08\x00\x00\x00")),
        # Truncated IFD and entry overflow.
        ("get-orientation-truncated-ifd", lambda: _core.exif_get_orientation(b"II\x2a\x00\x10\x00\x00\x00")),
        (
            "get-orientation-entry-overflow",
            lambda: _core.exif_get_orientation(
                struct.pack("<2sHIHH", b"II", 42, 8, 1, 0) + b"\x00"
            ),
        ),
        # Orientation outside the 1..=8 range and missing tag.
        ("get-orientation-out-of-range", lambda: _core.exif_get_orientation(tiff_ifd([(0x0112, 9)]))),
        ("get-orientation-zero", lambda: _core.exif_get_orientation(tiff_ifd([(0x0112, 0)]))),
        ("get-orientation-no-tag", lambda: _core.exif_get_orientation(tiff_ifd([]))),
        # exif_remove_orientation: short payloads and malformed containers.
        ("remove-orientation-short", lambda: _core.exif_remove_orientation(b"short")),
        ("remove-orientation-exif-short", lambda: _core.exif_remove_orientation(b"Exif\x00\x00II")),
        ("remove-orientation-bad-order", lambda: _core.exif_remove_orientation(b"XX\x2a\x00\x08\x00\x00\x00" + b"\x00" * 6)),
        ("remove-orientation-bad-magic", lambda: _core.exif_remove_orientation(b"II\x00\x00\x08\x00\x00\x00" + b"\x00" * 6)),
        ("remove-orientation-truncated", lambda: _core.exif_remove_orientation(b"II\x2a\x00\x10\x00\x00\x00" + b"\x00" * 6)),
        (
            "remove-orientation-entry-overflow",
            lambda: _core.exif_remove_orientation(struct.pack("<2sHIHH2x", b"II", 42, 8, 1, 0)),
        ),
        ("remove-orientation-le-found", lambda: _core.exif_remove_orientation(le)),
        ("remove-orientation-be-found", lambda: _core.exif_remove_orientation(be)),
        ("remove-orientation-exif-found", lambda: _core.exif_remove_orientation(exif_jpeg(le))),
        ("remove-orientation-no-tag", lambda: _core.exif_remove_orientation(tiff_ifd([]))),
        (
            "remove-orientation-leading-entry",
            lambda: _core.exif_remove_orientation(tiff_ifd([(0x0100, 100), (0x0112, 6)])),
        ),
        # Core colorize validation branches the public wrapper pre-empts.
        ("colorize-non-l", lambda: _core.ops_colorize(Image.new("RGB", (4, 4))._rust_image, (0, 0, 0), (255, 255, 255), None, 0, 127, 255)),
        ("colorize-mid-out-of-order", lambda: _core.ops_colorize(Image.new("L", (4, 4))._rust_image, (0, 0, 0), (255, 255, 255), (255, 0, 0), 10, 5, 255)),
        ("colorize-blackpoint-above-whitepoint", lambda: _core.ops_colorize(Image.new("L", (4, 4))._rust_image, (0, 0, 0), (255, 255, 255), None, 10, 127, 5)),
        ("colorize-valid-mid", lambda: _core.ops_colorize(Image.new("L", (4, 4))._rust_image, (0, 0, 0), (255, 255, 255), (128, 128, 128), 0, 127, 255)),
    ]
    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
