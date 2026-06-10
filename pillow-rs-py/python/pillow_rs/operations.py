"""Functional API for image operations — Pillow-compatible module-level functions."""
from pathlib import Path
from typing import Optional, Tuple, Union

from .enums import Resampling
from .image import Image


def open(
    fp: Union[str, Path, bytes],
    mode: Optional[str] = None,
    formats: Optional[list] = None,
) -> Image:
    return Image.open(fp, mode, formats)


def new(
    mode: str,
    size: Tuple[int, int],
    color: Union[int, Tuple[int, ...], str] = 0,
) -> Image:
    return Image.new(mode, size, color)


def save(
    image: Image, fp: Union[str, Path], format: Optional[str] = None, **options
) -> None:
    image.save(fp, format, **options)


def resize(
    image: Image,
    size: Tuple[int, int],
    resample: Union[int, str] = Resampling.BILINEAR,
) -> Image:
    return image.resize(size, resample)


def crop(image: Image, box: Tuple[int, int, int, int]) -> Image:
    return image.crop(box)


def rotate(image: Image, angle: float, expand: bool = False) -> Image:
    return image.rotate(angle, expand)


def convert(image: Image, mode: str) -> Image:
    return image.convert(mode)


def merge(mode: str, bands):
    """Merge single-band images into a multi-band image."""
    from . import _core
    return Image(_core.image_merge(mode, [b._rust_image for b in bands]))


def blend(im1: Image, im2: Image, alpha: float) -> Image:
    """Linear interpolation between two images."""
    from . import _core
    return Image(_core.image_blend(im1._rust_image, im2._rust_image, alpha))


def composite(image1: Image, image2: Image, mask: Image) -> Image:
    """Composite image1 over image2 using mask."""
    from . import _core
    return Image(_core.image_composite(image1._rust_image, image2._rust_image, mask._rust_image))


def thumbnail(
    image: Image,
    size: Tuple[int, int],
    resample: Union[int, str] = Resampling.BICUBIC,
) -> None:
    image.thumbnail(size, resample)
