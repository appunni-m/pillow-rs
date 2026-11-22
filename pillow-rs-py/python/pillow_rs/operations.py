"""Functional API for image operations — Pillow-compatible module-level functions."""

from __future__ import annotations

from pathlib import Path
from typing import Optional, Tuple, Union

from .enums import Resampling
from .image import Image, _execution_context_scope
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
    color: Union[int, Tuple[int, ...], str, None] = None,
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
    (
        expected_band_count,
        subsequent_band_mode,
        count_rejection,
        mode_rejection,
        size_rejection,
    ) = _core.image_merge_callback_contract(mode)
    if len(bands) != expected_band_count:
        raise ValueError(count_rejection)
    # Pillow observes these public callbacks before loading every input and
    # passing the materialized rasters to the core merger.
    if subsequent_band_mode is not None:
        for band in bands[1:]:
            if band.mode != subsequent_band_mode:
                raise ValueError(mode_rejection)
            if band.size != bands[0].size:
                raise ValueError(size_rejection)
    with _execution_context_scope(bands[0]):
        for band in bands:
            band.load()
        merged = _core.image_merge(mode, bands)
        return bands[0]._new(merged)


# VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
def _image_blend_with_context(im1, im2, alpha, context_owner, *, load_inputs):
    """Bind a blend graph to its source route, even after eager input loads."""
    with _execution_context_scope(context_owner):
        if load_inputs:
            im1.load()
            im2.load()
        result = _core.image_blend(im1._rust_image, im2._rust_image, alpha)
        return context_owner._bind_current_execution_context(result)


# VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
def blend(im1: Image, im2: Image, alpha: float) -> Image:
    """Linear interpolation between two images."""
    rust_result = _image_blend_with_context(
        im1, im2, alpha, im1, load_inputs=True
    )
    return im1._new(rust_result)


# TODO(pillow-rs#3ptb): verify composite output, source/paste callback order,
# metadata, and route receipts across destination/source/mask mode combinations.
# VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
def composite(image1: Image, image2: Image, mask: Image) -> Image:
    """Composite image1 over image2 using mask."""
    # Pillow composes this from destination copy plus public Paste callbacks.
    # Keep those callbacks on the destination's route after wrapper validation.
    with _execution_context_scope(image2):
        result = image2.copy()
        result.paste(image1, None, mask)
        return result


# VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
def alpha_composite(im1: Image, im2: Image) -> Image:
    """Alpha composite im2 over im1, returning a new image.

    PIL: ``Image.alpha_composite(im1, im2)`` composites im2 over im1
    and returns a new RGBA image.
    """
    with _execution_context_scope(im1):
        execution_context = im1._current_execution_context()
        if execution_context is None:
            result = _core.image_alpha_composite(im1._rust_image, im2._rust_image)
        else:
            result = _core.image_alpha_composite(
                im1._rust_image, im2._rust_image, execution_context
            )
        return im1._new(result)


def fromarray(obj, mode=None):
    """Create an image from an array-interface object through the Rust core."""
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
    return image.point(args[0])


def thumbnail(
    image: Image,
    size: Tuple[int, int],
    resample: Union[int, str] = Resampling.BICUBIC,
    reducing_gap=2.0,
) -> None:
    image.thumbnail(size, resample, reducing_gap)
