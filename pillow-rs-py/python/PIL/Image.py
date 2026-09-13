"""Pillow-compatible image module backed by the Rust implementation."""

from __future__ import annotations

from pillow_rs import Dither, ImageFormat, ImageMode, Palette, Resampling, Transpose
from pillow_rs import __version__
from pillow_rs import operations as _operations
from pillow_rs.image import (
    Image as Image,
    ImagingCore,
    PixelAccess,
    UnidentifiedImageError,
)

# Pillow exposes the implementation class as ``PIL.Image.Image``.  The
# internal class is reused directly so instances created by ``PIL.Image`` and
# by the compatibility ``pillow_rs`` namespace remain interchangeable.
Image.Image = Image

NEAREST = Resampling.NEAREST
BILINEAR = Resampling.BILINEAR
BICUBIC = Resampling.BICUBIC
LANCZOS = Resampling.LANCZOS

open = _operations.open
new = _operations.new
save = _operations.save
resize = _operations.resize
crop = _operations.crop
rotate = _operations.rotate
convert = _operations.convert
merge = _operations.merge
blend = _operations.blend
composite = _operations.composite
alpha_composite = _operations.alpha_composite
fromarray = _operations.fromarray
frombuffer = _operations.frombuffer
frombytes = _operations.frombytes
linear_gradient = _operations.linear_gradient
radial_gradient = _operations.radial_gradient
effect_mandelbrot = _operations.effect_mandelbrot
effect_noise = _operations.effect_noise
eval = _operations.eval
thumbnail = _operations.thumbnail

__all__ = [
    "Image",
    "ImagingCore",
    "PixelAccess",
    "UnidentifiedImageError",
    "ImageMode",
    "ImageFormat",
    "Resampling",
    "Transpose",
    "Dither",
    "Palette",
    "NEAREST",
    "BILINEAR",
    "BICUBIC",
    "LANCZOS",
    "open",
    "new",
    "save",
    "resize",
    "crop",
    "rotate",
    "convert",
    "merge",
    "blend",
    "composite",
    "alpha_composite",
    "fromarray",
    "frombuffer",
    "frombytes",
    "linear_gradient",
    "radial_gradient",
    "effect_mandelbrot",
    "effect_noise",
    "eval",
    "thumbnail",
]
