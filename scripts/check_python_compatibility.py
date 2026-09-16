#!/usr/bin/env python3
"""Check the installed public ``PIL`` replacement namespace.

This runs with the checkout's ``python/`` directory on ``PYTHONPATH``.  It
therefore verifies the import users receive from ``pillow-rs`` without
requiring Pillow to be installed in the same process.
"""

from __future__ import annotations

import importlib
import sys


MODULES = (
    "PIL",
    "PIL.Image",
    "PIL.ImageChops",
    "PIL.ImageColor",
    "PIL.ImageDraw",
    "PIL.ImageEnhance",
    "PIL.ImageFilter",
    "PIL.ImageFont",
    "PIL.ImageOps",
    "PIL.ImagePalette",
    "PIL.ImageSequence",
    "PIL.ImageStat",
    "pillow_rs",
    "pillow_rs.imagedraw",
    "pillow_rs.operations",
)


def main() -> int:
    for module in MODULES:
        importlib.import_module(module)
    from PIL import Image
    import PIL

    if Image.__name__ != "PIL.Image":
        raise AssertionError(f"from PIL import Image returned {Image!r}")
    if not isinstance(Image.new("L", (2, 2)), Image.Image):
        raise AssertionError("PIL.Image.new did not return PIL.Image.Image")
    if PIL.__version__ != importlib.import_module("pillow_rs").__version__:
        raise AssertionError("PIL and pillow_rs versions differ")
    print(
        f"Python PIL replacement imports passed on {sys.version.split()[0]} "
        f"(version {PIL.__version__})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
