"""Pillow-compatible public namespace backed by :mod:`pillow_rs`.

Applications can use the normal Pillow import unchanged::

    from PIL import Image

The parity harness imports this package only in the target process.  The
oracle process imports the separately installed Pillow distribution, so both
implementations may expose the same ``PIL`` module name without sharing an
object graph.
"""

from __future__ import annotations

import sys

import pillow_rs as _pillow_rs
from . import Image
from pillow_rs import (
    Dither,
    ImageFormat,
    ImageMode,
    Palette,
    Resampling,
    Transpose,
    active_backends,
    available_backends,
    backend_enabled,
    disable_backend,
    enable_backend,
)
from pillow_rs import imagechops as ImageChops
from pillow_rs import imagecolor as ImageColor
from pillow_rs import imagedraw as ImageDraw
from pillow_rs import imageenhance as ImageEnhance
from pillow_rs import imagefilter as ImageFilter
from pillow_rs import imagefont as ImageFont
from pillow_rs import imageops as ImageOps
from pillow_rs import imagepalette as ImagePalette
from pillow_rs import imagesequence as ImageSequence
from pillow_rs import imagestat as ImageStat

__version__ = _pillow_rs.__version__

# These modules are implemented in the internal binding package, but register
# under their public Pillow names so ``import PIL.ImageOps`` and
# ``from PIL import ImageOps`` behave identically.
for _name, _module in {
    "ImageChops": ImageChops,
    "ImageColor": ImageColor,
    "ImageDraw": ImageDraw,
    "ImageEnhance": ImageEnhance,
    "ImageFilter": ImageFilter,
    "ImageFont": ImageFont,
    "ImageOps": ImageOps,
    "ImagePalette": ImagePalette,
    "ImageSequence": ImageSequence,
    "ImageStat": ImageStat,
}.items():
    sys.modules[f"{__name__}.{_name}"] = _module

__all__ = [
    "Image",
    "ImageMode",
    "ImageFormat",
    "ImageOps",
    "ImageChops",
    "ImageColor",
    "ImageDraw",
    "ImageEnhance",
    "ImageFilter",
    "ImageFont",
    "ImagePalette",
    "ImageStat",
    "ImageSequence",
    "Resampling",
    "Transpose",
    "Dither",
    "Palette",
    "active_backends",
    "available_backends",
    "backend_enabled",
    "disable_backend",
    "enable_backend",
]
