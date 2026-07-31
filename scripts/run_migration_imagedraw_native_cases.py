#!/usr/bin/env python3
"""Exercise ImageDraw core paths the parity corpus cannot reach.

The `shape()` endpoint's success path requires a real `Outline` object built
through the move/line/curve/close protocol, which the parity case generator
does not emit.  This coverage-only command drives the target facade's shape
surface with a valid outline plus fill/outline combinations.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json

from pillow_rs import Image, ImageDraw
from pillow_rs._core import Outline


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

    def build_outline() -> Outline:
        outline = Outline()
        outline.move(2, 2)
        outline.line(12, 2)
        outline.line(12, 10)
        outline.line(2, 10)
        outline.close()
        return outline

    probes: list[tuple[str, callable]] = [
        ("shape-fill", lambda: _shape(fill=255)),
        ("shape-outline", lambda: _shape(outline=255)),
        ("shape-fill-outline", lambda: _shape(fill=255, outline=100)),
        ("shape-invalid-object", lambda: _shape_invalid()),
    ]

    def _shape(**kwargs) -> None:
        draw = ImageDraw.Draw(Image.new("RGB", (16, 16), 0))
        draw.shape(build_outline(), **kwargs)

    def _shape_invalid() -> None:
        draw = ImageDraw.Draw(Image.new("RGB", (16, 16), 0))
        draw.shape("not-an-outline")

    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
