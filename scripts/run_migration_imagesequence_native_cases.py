#!/usr/bin/env python3
"""Exercise ImageSequence paths that are useful for merged coverage.

The input-only parity corpus covers the declared constructor and iterator
methods. This maintained coverage-only command additionally drives the
protocol through Python's ``for`` machinery and the child-image core path.

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
        except Exception:
            failed += 1
        else:
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
        # Construction through the declared keyword spelling.
        (
            "iterator-keyword-image",
            lambda: _drain(ImageSequence.Iterator(im=Image.new("1", (4, 4)))),
        ),
        # Iterator accepts any seekable image-like object. Exercise the
        # optional private start-frame protocol and preserve its extraction
        # errors without reaching into the Rust handle directly.
        (
            "iterator-custom-min-frame",
            lambda: ImageSequence.Iterator(_SeekableWithMinFrame()),
        ),
        (
            "iterator-custom-invalid-min-frame",
            lambda: _expect_error(
                lambda: ImageSequence.Iterator(_SeekableWithInvalidMinFrame()),
                TypeError,
            ),
        ),
        (
            "iterator-custom-min-frame-error",
            lambda: _expect_error(
                lambda: ImageSequence.Iterator(_SeekableWithMinFrameError()),
                ValueError,
            ),
        ),
        (
            "iterator-custom-seek-error",
            lambda: _expect_error(
                lambda: next(ImageSequence.Iterator(_SeekError())),
                ValueError,
            ),
        ),
        # Single-frame seek succeeds at frame zero and rejects frame one.
        ("seek-frame-zero", lambda: Image.new("L", (4, 4)).seek(0)),
        (
            "seek-frame-one",
            lambda: _expect_error(
                lambda: Image.new("P", (4, 4)).seek(1),
                EOFError,
            ),
        ),
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


class _SeekableWithMinFrame:
    _min_frame = 3

    def seek(self, _frame: int) -> None:
        return None


class _SeekableWithInvalidMinFrame:
    _min_frame = "not-an-integer"

    def seek(self, _frame: int) -> None:
        return None


class _SeekableWithMinFrameError:
    @property
    def _min_frame(self) -> int:
        raise ValueError("min frame unavailable")

    def seek(self, _frame: int) -> None:
        return None


class _SeekError:
    _min_frame = 0

    def seek(self, _frame: int) -> None:
        raise ValueError("seek failed")


def _expect_error(call, error_type: type[Exception]) -> None:
    try:
        call()
    except error_type:
        return
    raise AssertionError(f"expected {error_type.__name__}")


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
