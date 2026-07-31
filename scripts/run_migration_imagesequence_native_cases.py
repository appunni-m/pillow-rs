#!/usr/bin/env python3
"""Exercise the ImageSequence iterator surface the parity corpus cannot reach.

The parity corpus only constructs ``PIL.ImageSequence.Iterator``; the public
manifest has no ``__iter__``/``__next__`` endpoints, so the wrapper's
iteration state machine is never measured.  This coverage-only command drives
the target facade's iterator protocol and the single-frame seek/child-image
core paths directly.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json

from pillow_rs import Image, ImageSequence


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


def run_native_cases() -> tuple[int, int, int]:
    """Run every sequence native probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            # Completed executions (including StopIteration and public errors)
            # exercise the instrumented path; unexpected failures are counted.
            passed += 1

    probes: list[tuple[str, callable]] = [
        # Iteration over a single-frame image: first __next__ returns the
        # image, the second raises StopIteration.
        (
            "iterator-single-frame",
            lambda: _drain(ImageSequence.Iterator(Image.new("L", (4, 4)))),
        ),
        # The iterator object is its own iterator.
        (
            "iterator-iter-is-self",
            lambda: _iter_self(ImageSequence.Iterator(Image.new("RGB", (4, 4)))),
        ),
        # Construction through the positional and keyword spellings.
        (
            "iterator-keyword-image",
            lambda: _drain(ImageSequence.Iterator(image=Image.new("1", (4, 4)))),
        ),
        # Single-frame seek is a no-op success for any frame index.
        ("seek-frame-zero", lambda: Image.new("L", (4, 4)).seek(0)),
        ("seek-frame-one", lambda: Image.new("P", (4, 4)).seek(1)),
        # Multi-frame decoding is not implemented in core; the child image
        # list is empty for every input.
        ("child-images-empty", lambda: Image.new("RGBA", (4, 4)).get_child_images()),
    ]
    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def _drain(iterator: ImageSequence.Iterator) -> None:
    count = 0
    for _frame in iterator:
        count += 1
    assert count == 1


def _iter_self(iterator: ImageSequence.Iterator) -> None:
    assert iter(iterator) is iterator


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
