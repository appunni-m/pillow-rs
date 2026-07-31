#!/usr/bin/env python3
"""Exercise ImagePalette paths the parity corpus misses.

The corpus only constructs ImagePalette and exercises a few getcolor error
paths.  The success append/lookup shapes, short-tuple handling, copy/getdata/
tobytes/save surfaces, and the HSV<->RGB conversions that share color.rs are
measured here.  This is a coverage-only command: it never compares oracle
values and cannot satisfy parity requirements.
"""

from __future__ import annotations

import io
import json
import os
import tempfile

from pillow_rs import Image
from pillow_rs.imagepalette import ImagePalette


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


def run_native_cases() -> tuple[int, int, int]:
    """Run every image-palette native probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            passed += 1

    probes: list[tuple[str, callable]] = [
        ("getcolor-new-rgb", lambda: ImagePalette("RGB").getcolor((255, 0, 0))),
        ("getcolor-existing", lambda: ImagePalette("RGB", bytearray([1, 2, 3, 4, 5, 6])).getcolor((1, 2, 3))),
        ("getcolor-rgba-three", lambda: ImagePalette("RGBA").getcolor((1, 2, 3))),
        ("getcolor-rgba-four", lambda: ImagePalette("RGBA").getcolor((1, 2, 3, 128))),
        ("getcolor-short-two", lambda: ImagePalette("RGB").getcolor((1, 2))),
        ("getcolor-short-one", lambda: ImagePalette("RGB").getcolor((1,))),
        ("getcolor-string", lambda: ImagePalette("RGB").getcolor("red")),
        ("getcolor-nonopaque", lambda: ImagePalette("RGB").getcolor((1, 2, 3, 128))),
        ("getcolor-empty", lambda: ImagePalette("RGB").getcolor(())),
        ("getcolor-full", lambda: _getcolor_full()),
        ("copy", lambda: ImagePalette("RGB", bytearray([1, 2, 3])).copy().palette),
        ("getdata", lambda: ImagePalette("RGB", bytearray([1, 2, 3])).getdata()),
        ("tobytes", lambda: ImagePalette("RGB", bytearray([1, 2, 3])).tobytes()),
        ("save-text-stream", lambda: _save_text()),
        ("save-path", lambda: _save_path()),
        # HSV<->RGB sector conversions sharing color.rs.
        ("hsv-gray", lambda: Image.new("HSV", (1, 1), (0, 0, 128)).convert("RGB")),
        ("hsv-sector0", lambda: Image.new("HSV", (1, 1), (0, 255, 128)).convert("RGB")),
        ("hsv-sector1", lambda: Image.new("HSV", (1, 1), (60, 255, 128)).convert("RGB")),
        ("hsv-sector2", lambda: Image.new("HSV", (1, 1), (120, 255, 128)).convert("RGB")),
        ("hsv-sector3", lambda: Image.new("HSV", (1, 1), (180, 255, 128)).convert("RGB")),
        ("hsv-sector4", lambda: Image.new("HSV", (1, 1), (240, 255, 128)).convert("RGB")),
        ("hsv-sector5", lambda: Image.new("HSV", (1, 1), (220, 255, 128)).convert("RGB")),
        ("hsv-sector3b", lambda: Image.new("HSV", (1, 1), (140, 255, 128)).convert("RGB")),
        ("rgb-to-hsv-red", lambda: Image.new("RGB", (1, 1), (255, 0, 0)).convert("HSV")),
        ("rgb-to-hsv-green", lambda: Image.new("RGB", (1, 1), (0, 255, 0)).convert("HSV")),
        ("rgb-to-hsv-blue", lambda: Image.new("RGB", (1, 1), (0, 0, 255)).convert("HSV")),
        ("rgb-to-hsv-gray", lambda: Image.new("RGB", (1, 1), (128, 128, 128)).convert("HSV")),
        ("rgb-to-hsv-cyan", lambda: Image.new("RGB", (1, 1), (0, 255, 255)).convert("HSV")),
        ("rgb-to-hsv-magenta", lambda: Image.new("RGB", (1, 1), (255, 0, 255)).convert("HSV")),
        ("rgb-to-hsv-yellow", lambda: Image.new("RGB", (1, 1), (255, 255, 0)).convert("HSV")),
        # Direct _core append/lookup surfaces the wrapper bypasses.
        ("core-append-rgb", lambda: _core_append("RGB")),
        ("core-append-rgba", lambda: _core_append("RGBA")),
        ("core-append-full", lambda: _core_append_full()),
        ("core-lookup-found", lambda: _core_lookup_found()),
        ("core-lookup-missing", lambda: _core_lookup_missing()),
    ]
    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def _getcolor_full() -> None:
    palette = ImagePalette("RGB")
    for i in range(256):
        palette.getcolor((i, 0, 0))
    palette.getcolor((255, 1, 1))


def _save_text() -> None:
    stream = io.StringIO()
    ImagePalette("RGB", bytearray([1, 2, 3])).save(stream)


def _save_path() -> None:
    path = tempfile.mktemp()
    try:
        ImagePalette("RGB", bytearray([1, 2, 3])).save(path)
    finally:
        if os.path.exists(path):
            os.unlink(path)


def _core_append(mode: str) -> None:
    from pillow_rs import _core

    palette = bytearray()
    _core.palette_getcolor_append(palette, 1, 2, 3, 255, mode)


def _core_append_full() -> None:
    from pillow_rs import _core

    palette = bytearray()
    for i in range(256):
        _core.palette_getcolor_append(palette, i % 256, 0, 0, 255, "RGB")
    _core.palette_getcolor_append(palette, 0, 1, 0, 255, "RGB")


def _core_lookup_found() -> None:
    from pillow_rs import _core

    palette = bytearray([1, 2, 3, 4, 5, 6])
    _core.palette_getcolor(palette, 4, 5, 6)


def _core_lookup_missing() -> None:
    from pillow_rs import _core

    palette = bytearray([1, 2, 3])
    _core.palette_getcolor(palette, 9, 9, 9)


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
