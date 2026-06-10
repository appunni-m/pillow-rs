"""
pillow-rs — Pillow drop-in replacement powered by Rust.
Import as: from RSPIL import Image
"""

from .enums import Dither, ImageFormat, ImageMode, Palette, Resampling, Transpose
from .image import Image
from .operations import convert, crop, new, open, resize, rotate, save

__version__ = "0.1.0"

__all__ = [
    "Image",
    "ImageMode",
    "ImageFormat",
    "Resampling",
    "Transpose",
    "Dither",
    "Palette",
    "open",
    "new",
    "save",
    "resize",
    "crop",
    "rotate",
    "convert",
]
