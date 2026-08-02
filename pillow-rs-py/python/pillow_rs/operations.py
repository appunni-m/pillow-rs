"""Functional API for image operations — Pillow-compatible module-level functions."""
from pathlib import Path
from typing import Optional, Tuple, Union

from .enums import Resampling
from .image import Image
from . import _core


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
    return Image(_core.image_merge(mode, bands))


def blend(im1: Image, im2: Image, alpha: float) -> Image:
    """Linear interpolation between two images."""
    from . import _core
    return Image(_core.image_blend(im1._rust_image, im2._rust_image, alpha))


def composite(image1: Image, image2: Image, mask: Image) -> Image:
    """Composite image1 over image2 using mask."""
    from . import _core
    return Image(_core.image_composite(image1._rust_image, image2._rust_image, mask._rust_image))


def alpha_composite(im1: Image, im2: Image) -> Image:
    """Alpha composite im2 over im1, returning a new image.

    PIL: ``Image.alpha_composite(im1, im2)`` composites im2 over im1
    and returns a new RGBA image.
    """
    from . import _core
    return Image(_core.image_alpha_composite(im1._rust_image, im2._rust_image))


def fromarray(obj, mode=None):
    """Create an image from an array-like object through the Rust core."""
    from . import _core

    return Image(_core.fromarray(obj, mode))


def frombytes(mode, size, data, decoder_name="raw", *args):
    """Create an image from raw bytes using Pillow's decoder contract."""
    return Image.frombytes(mode, size, data, decoder_name, *args)


def linear_gradient(mode: str) -> Image:
    """Generate 256x256 linear gradient from black to white, top to bottom."""
    from . import _core
    return Image(_core.image_linear_gradient(mode))


def radial_gradient(mode: str) -> Image:
    """Generate 256x256 radial gradient from white (center) to black (edges)."""
    from . import _core
    return Image(_core.image_radial_gradient(mode))


def effect_mandelbrot(
    size: tuple[int, int],
    extent: tuple[float, float, float, float],
    quality: int,
) -> Image:
    """Generate a Mandelbrot set covering the given extent."""
    from . import _core
    return Image(_core.image_effect_mandelbrot(size, extent, quality))


def effect_noise(size: tuple[int, int], sigma: float) -> Image:
    """Generate a deterministic Gaussian-noise image."""
    return Image.effect_noise(size, sigma)


def frombuffer(mode: str, size: tuple[int, int], data, decoder_name: str = "raw", *args):
    """Create an image from pixel data in a byte buffer. Delegates to frombytes."""
    return Image.frombytes(mode, size, data, decoder_name, *args)


def eval(image: Image, *args):
    """Apply a function to each pixel. The first arg is a callable."""
    _core.eval_validate_input(args[0])
    return image.point(args[0])


def thumbnail(
    image: Image,
    size: Tuple[int, int],
    resample: Union[int, str] = Resampling.BICUBIC,
) -> None:
    image.thumbnail(size, resample)
