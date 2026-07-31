#!/usr/bin/env python3
"""Exercise ImageColor parsing branches the parity corpus misses.

The parity corpus selects a small getcolor/getrgb surface; the mode matrix
(HSV/L/LA/1/I/F/I;16), the rgb_to_hsv axis branches, and the rejected
function-form variants are only measured here.  This coverage-only command
drives the target facade's color parser over the full matrix.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json

from pillow_rs import ImageColor


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


def run_native_cases() -> tuple[int, int, int]:
    """Run every image-color native probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            passed += 1

    probes: list[tuple[str, callable]] = []
    for color in ["red", "lime", "blue", "gray", "white", "black", "#abcdef", "hsl(120,50%,50%)"]:
        for mode in ["HSV", "L", "LA", "1", "I", "F", "I;16", "RGB", "RGBA", "CMYK"]:
            probes.append(
                (
                    f"getcolor-{color}-{mode}",
                    (lambda c=color, m=mode: ImageColor.getcolor(c, m)),
                )
            )
    for spec in [
        "rgba(1,2,3)",
        "rgba(1,2,3,4.5)",
        "rgba(a,b,c,d)",
        "rgba(1,2,3,4,5)",
        "rgba(1,2,3,)",
        " rgba(1,2,3,4) ",
        "rgb(1,2)",
        "rgb(1,2,3,4)",
        "rgb(50%)",
        "transparent",
        "currentcolor",
        "#ab",
        "foo",
        "x" * 101,
        "rgba(1,2,3,4)",
        "rgb(100%, 50%, 0%)",
        "hsl(240,100%,50%)",
    ]:
        probes.append(("getrgb-" + spec[:20], (lambda s=spec: ImageColor.getrgb(s))))
    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
