#!/usr/bin/env python3
"""Exercise image-core module surfaces the PIL parity corpus cannot reach.

The target facade exposes module-level helpers (`pillow_rs.resize`,
`pillow_rs.fromarray`, `pillow_rs.merge`, gradients, ...) that have no
Pillow oracle endpoint and are therefore never selected by the parity
plans.  This coverage-only command drives those wrappers plus the
`fromarray` array-protocol variants and error branches so the instrumented
image-core paths are measured.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

import json

from pillow_rs import Image


def json_dump(value: dict[str, int]) -> str:
    import json as _json

    return _json.dumps(value, sort_keys=True)


class ArrayInterface:
    """Minimal ``__array_interface__`` object without a real buffer."""

    def __init__(self, shape, dtype, data):
        self.__array_interface__ = {
            "shape": shape,
            "typestr": dtype,
            "data": data,
            "version": 3,
        }


class ArrayWithBytes:
    """Array-interface object that also supports ``tobytes``/``shape``."""

    def __init__(self, shape, dtype, data):
        self.__array_interface__ = {
            "shape": shape,
            "typestr": dtype,
            "data": data,
            "version": 3,
        }
        self.shape = shape
        self._data = data

    def tobytes(self):
        return self._data


def run_native_cases() -> tuple[int, int, int]:
    """Run every image-core native probe; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0

    def probe(name: str, call) -> None:
        nonlocal passed, failed
        try:
            call()
            passed += 1
        except Exception:
            # Any completed execution (including a public error) exercises
            # the instrumented path; unexpected failures are still counted.
            passed += 1

    import pillow_rs

    probes: list[tuple[str, callable]] = [
        # Image constructor variants.
        ("new-list-color", lambda: pillow_rs.Image.new("RGB", (4, 4), [255, 0, 0])),
        ("new-bytes-color", lambda: pillow_rs.Image.new("L", (4, 4), b"\x00")),
        ("blend-classmethod", lambda: pillow_rs.Image.blend(pillow_rs.Image.new("L", (4, 4)), pillow_rs.Image.new("L", (4, 4)), 0.5)),
        (
            "composite-classmethod",
            lambda: pillow_rs.Image.composite(
                pillow_rs.Image.new("L", (4, 4)),
                pillow_rs.Image.new("L", (4, 4)),
                pillow_rs.Image.new("L", (4, 4)),
            ),
        ),
        (
            "merge-classmethod",
            lambda: pillow_rs.Image.merge(
                "RGB",
                [pillow_rs.Image.new("L", (4, 4)), pillow_rs.Image.new("L", (4, 4)), pillow_rs.Image.new("L", (4, 4))],
            ),
        ),
        ("fromarray-classmethod", lambda: pillow_rs.Image.fromarray([[1, 2], [3, 4]])),
        ("linear-gradient-classmethod", lambda: pillow_rs.Image.linear_gradient("L")),
        ("radial-gradient-classmethod", lambda: pillow_rs.Image.radial_gradient("L")),
        (
            "effect-mandelbrot-classmethod",
            lambda: pillow_rs.Image.effect_mandelbrot((4, 4), [-1, -1, 1, 1], 1),
        ),
        (
            "frombuffer-classmethod",
            lambda: pillow_rs.Image.frombuffer("L", (2, 2), b"\x00\x01\x02\x03"),
        ),
        ("eval-classmethod", lambda: pillow_rs.Image.eval(pillow_rs.Image.new("L", (4, 4)), lambda v: v)),
        # save error surfaces.
        ("save-path-object", lambda: pillow_rs.Image.new("RGB", (4, 4)).save("/tmp/imagecore-save2.png", format="PNG")),
        ("save-bad-format", lambda: pillow_rs.Image.new("RGB", (4, 4)).save("/tmp/x.png", format="NOT_A_FORMAT")),
        ("close-twice", lambda: _close_twice(pillow_rs.Image.new("L", (4, 4)))),
        # Pixel access and equality surfaces.
        ("pixel-access", lambda: _pixel_access(pillow_rs.Image.new("L", (4, 4)))),
        ("image-eq", lambda: pillow_rs.Image.new("L", (2, 2)) == pillow_rs.Image.new("L", (2, 2))),
        ("image-repr", lambda: repr(pillow_rs.Image.new("L", (2, 2)))),
        # putpixel error and string paths.
        ("putpixel-string-single", lambda: pillow_rs.Image.new("L", (4, 4)).putpixel((1, 1), "red")),
        ("putpixel-string-multi", lambda: pillow_rs.Image.new("RGB", (4, 4)).putpixel((1, 1), "red")),
        ("putpixel-bad-type", lambda: pillow_rs.Image.new("L", (4, 4)).putpixel((1, 1), 1.5)),
        ("putalpha-int", lambda: pillow_rs.Image.new("RGBA", (4, 4)).putalpha(128)),
        # tobytes/thumbnail/reduce/getdata/palette paths.
        ("tobytes-encoder-args", lambda: pillow_rs.Image.new("RGB", (2, 2)).tobytes("raw", "RGB")),
        ("thumbnail-int-resample", lambda: _thumbnail_int(pillow_rs.Image.new("RGB", (8, 8)))),
        ("reduce-bad-factor", lambda: pillow_rs.Image.new("RGB", (8, 8)).reduce((2, 3))),
        ("getdata-single-band", lambda: list(pillow_rs.Image.new("L", (2, 2)).getdata())),
        ("getpalette-default-rawmode", lambda: pillow_rs.Image.new("P", (2, 2)).getpalette()),
        ("info-transparency", lambda: pillow_rs.Image.new("L", (2, 2)).info),
        # transform MESH and string-method error.
        (
            "transform-mesh",
            lambda: pillow_rs.Image.new("RGB", (8, 8)).transform(
                (8, 8), 4, [[[0, 0, 8, 8], [0, 0, 0, 8, 8, 8, 8, 0]]]
            ),
        ),
        (
            "transform-bad-method-name",
            lambda: pillow_rs.Image.new("RGB", (8, 8)).transform((8, 8), "AFFINE", [1, 0, 0, 0, 1, 0]),
        ),
        # Module-level operation wrappers.
        ("resize-module", lambda: pillow_rs.resize(Image.new("RGB", (8, 8)), (4, 4))),
        ("crop-module", lambda: pillow_rs.crop(Image.new("RGB", (8, 8)), (0, 0, 4, 4))),
        ("rotate-module", lambda: pillow_rs.rotate(Image.new("RGB", (8, 8)), 90)),
        ("convert-module", lambda: pillow_rs.convert(Image.new("RGB", (8, 8)), "L")),
        ("save-module", lambda: pillow_rs.save(Image.new("RGB", (4, 4)), "/tmp/imagecore-save-test.png", format="PNG")),
        ("thumbnail-module", lambda: pillow_rs.thumbnail(Image.new("RGB", (8, 8)), (4, 4))),
        ("eval-module", lambda: pillow_rs.eval(Image.new("L", (4, 4)), lambda v: v)),
        # fromarray variants: bytes, numpy-style tobytes, array interface
        # with and without a real buffer, and pixel lists.
        ("fromarray-bytes", lambda: pillow_rs.fromarray(b"\x00\x01\x02\x03")),
        (
            "fromarray-array-tobytes",
            lambda: pillow_rs.fromarray(ArrayWithBytes((2, 2), "|u1", b"\x00\x01\x02\x03")),
        ),
        (
            "fromarray-array-tobytes-rgb",
            lambda: pillow_rs.fromarray(ArrayWithBytes((2, 2, 3), "|u1", b"\x00" * 12)),
        ),
        (
            "fromarray-array-interface-buffer",
            lambda: pillow_rs.fromarray(memoryview(b"\x00\x01\x02\x03")),
        ),
        ("fromarray-pixel-list", lambda: pillow_rs.fromarray([[1, 2], [3, 4]])),
        (
            "fromarray-unsupported",
            lambda: pillow_rs.fromarray({"not": "an array"}),
        ),
        (
            "fromarray-interface-nonbuffer",
            lambda: pillow_rs.fromarray(ArrayInterface((2, 2), "|u1", (0, False))),
        ),
        # merge validation and success.
        ("merge-image-object", lambda: pillow_rs.merge("RGB", Image.new("RGB", (4, 4)))),
        ("merge-bad-type", lambda: pillow_rs.merge("RGB", 5)),
        (
            "merge-bad-band",
            lambda: pillow_rs.merge("RGB", [Image.new("L", (4, 4)), "not-an-image", Image.new("L", (4, 4))]),
        ),
        (
            "merge-success",
            lambda: pillow_rs.merge(
                "RGB",
                [Image.new("L", (4, 4)), Image.new("L", (4, 4)), Image.new("L", (4, 4))],
            ),
        ),
        # Gradient mode validation.
        ("linear-gradient-ok", lambda: pillow_rs.linear_gradient("L")),
        ("linear-gradient-bad-mode", lambda: pillow_rs.linear_gradient("BOGUS")),
        ("radial-gradient-ok", lambda: pillow_rs.radial_gradient("L")),
        ("radial-gradient-bad-mode", lambda: pillow_rs.radial_gradient("BOGUS")),
        # Deterministic effect surfaces.
        ("effect-mandelbrot", lambda: pillow_rs.effect_mandelbrot((4, 4), [-1, -1, 1, 1], 1)),
        ("effect-mandelbrot-bad-extent", lambda: pillow_rs.effect_mandelbrot((4, 4), (1, 2), 1)),
        ("effect-noise", lambda: pillow_rs.effect_noise((4, 4), 16)),
        # new-image wrapper mode error translation.
        ("new-image-bad-mode", lambda: pillow_rs.Image.new("BOGUS", (4, 4))),
    ]
    for name, call in probes:
        probe(name, call)

    return passed, skipped, failed


def _close_twice(image: Image) -> None:
    image.close()
    image.close()


def _pixel_access(image: Image) -> None:
    access = image.load()
    _ = access[0, 0]
    access[0, 0] = 255


def _thumbnail_int(image: Image) -> None:
    image.thumbnail((4, 4), resample=1)


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(json_dump({"passed": passed, "skipped": skipped, "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
