"""
pillow-rs — Pillow drop-in replacement powered by Rust.
Import as: from RSPIL import Image
"""

from . import imagechops as ImageChops
from . import imagecolor as ImageColor
from . import imagedraw as ImageDraw
from . import imageenhance as ImageEnhance
from . import imagefilter as ImageFilter
from . import imagefont as ImageFont
from . import imageops as ImageOps
from . import imagepalette as ImagePalette
from . import imagestat as ImageStat
from . import imagesequence as ImageSequence
from .enums import Dither, ImageFormat, ImageMode, Palette, Resampling, Transpose
from .image import Image
from .operations import (
    alpha_composite, blend, composite, convert, crop, effect_mandelbrot, fromarray, frombuffer, frombytes,
    linear_gradient, merge, new, open, radial_gradient, resize, rotate, save,
)

__version__ = "0.1.0"

__all__ = [
    "Image", "ImageMode", "ImageFormat",
    "ImageOps", "ImageChops", "ImageColor", "ImageDraw",
    "ImageEnhance", "ImageFilter", "ImageFont",
    "ImagePalette", "ImageStat", "ImageSequence",
    "Resampling", "Transpose", "Dither", "Palette",
    "open", "new", "save", "resize", "crop", "rotate", "convert", "merge", "blend", "composite",
    "linear_gradient", "radial_gradient", "effect_mandelbrot", "frombuffer", "fromarray", "frombytes",
    "alpha_composite",
]

def enable_backend(name):
    """Activate a compute backend. Returns True if the backend exists.

    Args:
        name: Backend name - ``"cpu"``, ``"gpu"``
    """
    return _core.enable_backend(name)

def disable_backend(name):
    """Deactivate a compute backend. Returns True if it was active.

    Args:
        name: Backend name - ``"cpu"``, ``"gpu"``
    """
    return _core.disable_backend(name)

def available_backends():
    """List backends that exist on this machine.

    Returns:
        List of backend name strings (e.g. ``["gpu", "cpu"]``)
    """
    return _core.available_backends()

def active_backends():
    """List currently active backends in priority order.

    Returns:
        List of backend name strings (e.g. ``["gpu", "cpu"]``)
    """
    return _core.active_backends()

def backend_enabled(name):
    """Check if a specific backend is active.

    Args:
        name: Backend name - ``"cpu"``, ``"gpu"``
    """
    return _core.backend_enabled(name)
