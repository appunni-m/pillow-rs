"""
pillow-rs — Pillow drop-in replacement powered by Rust.
Import as: from RSPIL import Image
"""

from . import imagechops as ImageChops
from . import imagecolor as ImageColor
from . import imagedraw as ImageDraw
from . import imageenhance as ImageEnhance
from . import imageops as ImageOps
from .enums import Dither, ImageFormat, ImageMode, Palette, Resampling, Transpose
from .image import Image
from .operations import blend, composite, convert, crop, merge, new, open, resize, rotate, save

__version__ = "0.1.0"

__all__ = [
    "Image",
    "ImageMode",
    "ImageFormat",
    "ImageOps",
    "ImageChops",
    "ImageColor",
    "ImageDraw",
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
