#!/usr/bin/env python3
"""Exercise residual ImageDraw core paths the parity corpus cannot reach.

Valid ``Draw.shape`` inputs are represented by the input-only parity protocol
and are therefore intentionally absent here.  This coverage-only command
keeps only bitmap mode/validation and polygon/line edge paths that are not
selected by the public parity cases.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json

from pillow_rs import Image, ImageDraw


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


def run_native_cases() -> tuple[int, int, int]:
    """Run every image-draw native probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            passed += 1

    probes: list[tuple[str, callable]] = [
        ("bitmap-i-canvas", lambda: _bitmap("I")),
        ("bitmap-f-canvas", lambda: _bitmap("F")),
        ("bitmap-i-canvas-mask255", lambda: _bitmap("I", mask_value=255)),
        ("bitmap-f-canvas-mask255", lambda: _bitmap("F", mask_value=255)),
        ("bitmap-rgb-mask255", lambda: _bitmap("RGB", mask_value=255)),
        ("bitmap-p-mask255", lambda: _bitmap("P", mask_value=255)),
        ("polygon-horizontal-runs", lambda: _polygon_horizontal()),
        ("polygon-out-of-bounds", lambda: _polygon_oob()),
        ("line-out-of-bounds", lambda: _line_oob()),
    ]

    def _bitmap(mode: str, mask_value: int = 128) -> None:
        mask = Image.new("L", (8, 8), mask_value)
        draw = ImageDraw.Draw(Image.new(mode, (16, 16), 0))
        draw.bitmap((2, 2), mask, fill=255)

    def _polygon_horizontal() -> None:
        draw = ImageDraw.Draw(Image.new("RGB", (16, 16), 0))
        draw.polygon([(1, 4), (12, 4), (12, 8), (4, 8), (4, 5), (10, 5), (10, 7), (2, 7)], fill=255)

    def _polygon_oob() -> None:
        draw = ImageDraw.Draw(Image.new("RGB", (16, 16), 0))
        draw.polygon([(-5, 3), (20, 3), (20, 13), (-5, 13)], fill=255)

    def _line_oob() -> None:
        draw = ImageDraw.Draw(Image.new("RGB", (16, 16), 0))
        draw.line([(-4, -4), (20, 20)], fill=255, width=2)

    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
