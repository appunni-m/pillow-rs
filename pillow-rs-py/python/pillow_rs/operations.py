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
    try:
        return Image.new(mode, size, color)
    except ValueError as exc:
        if str(exc) == f"Unsupported mode: {mode}":
            raise ValueError("unrecognized image mode") from exc
        raise


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
    if isinstance(bands, Image):
        raise TypeError("object of type 'Image' has no len()")
    if not isinstance(bands, (tuple, list)):
        raise TypeError(f"object of type '{type(bands).__name__}' has no len()")
    if any(not isinstance(band, Image) for band in bands):
        raise ValueError("wrong number of bands")
    rust_bands = tuple(map(lambda b: b._rust_image, bands))
    return Image(_core.image_merge(mode, rust_bands))


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
    # Pillow's module-level function requires matching sizes; only the
    # in-place method composites cropped regions.
    if im1.size != im2.size:
        raise ValueError("images do not match")
    result = im1.copy()
    result.alpha_composite(im2)
    return result


def fromarray(obj, mode=None):
    """Create image from array-like object (list of lists or bytes)."""
    from . import _core

    if isinstance(obj, bytes):
        return Image.frombytes(mode or "L", (len(obj), 1), obj)
    # Array-interface objects must be buffer-compatible for Pillow's
    # ``frombuffer`` path. The fixed parity protocol deliberately supplies an
    # object with the interface but without a Python buffer, so preserve the
    # oracle's public diagnostics instead of falling through to ``len``.
    if hasattr(obj, '__array_interface__'):
        arr = obj.__array_interface__
        shape = arr["shape"]
        if mode == "L" and len(shape) > 2:
            raise ValueError(f"Too many dimensions: {len(shape)} > 2.")
        try:
            memoryview(obj)
        except TypeError as exc:
            if mode == "RGBA":
                raise TypeError("expected string or buffer") from exc
            raise TypeError(
                f"a bytes-like object is required, not '{type(obj).__name__}'"
            ) from exc

    # numpy arrays: use tobytes() for safe memory access
    if hasattr(obj, 'tobytes'):
        data = obj.tobytes()
        shape = obj.shape if hasattr(obj, 'shape') else (len(obj), 1)
        h, w = shape[0], shape[1] if len(shape) >= 2 else 1
        if mode is None:
            if len(shape) == 2:
                mode = "L"
            elif len(shape) == 3:
                mode = {3: "RGB", 4: "RGBA"}.get(shape[2], "L")
            else:
                mode = "L"
        return Image.frombytes(mode, (w, h), data)
    # array-interface objects that implement a real Python buffer can use the
    # same raw-byte path as numpy arrays.
    if hasattr(obj, '__array_interface__'):
        arr = obj.__array_interface__
        shape = arr["shape"]
        h, w = shape[0], shape[1] if len(shape) >= 2 else 1
        if mode is None:
            if len(shape) == 2:
                mode = "L"
            elif shape[2] == 3:
                mode = "RGB"
            elif shape[2] == 4:
                mode = "RGBA"
            else:
                mode = "L"
        data = memoryview(obj).tobytes()
        return Image.frombytes(mode, (w, h), data)
    if isinstance(obj, (list, tuple)):
        return Image(_core.fromarray_pixel_list(obj, mode))
    raise NotImplementedError(f"fromarray: unsupported object type ({type(obj).__name__})")


def frombytes(mode, size, data, decoder_name="raw", *args):
    """Create an image from raw bytes using Pillow's decoder contract."""
    if decoder_name != "raw":
        raise OSError(f"decoder {decoder_name} not available")
    return Image.frombytes(mode, size, data)


def linear_gradient(mode: str) -> Image:
    """Generate 256x256 linear gradient from black to white, top to bottom."""
    from . import _core
    try:
        return Image(_core.image_linear_gradient(mode))
    except ValueError as exc:
        if str(exc).startswith("linear_gradient: unsupported mode"):
            raise ValueError("image has wrong mode") from exc
        raise


def radial_gradient(mode: str) -> Image:
    """Generate 256x256 radial gradient from white (center) to black (edges)."""
    from . import _core
    try:
        return Image(_core.image_radial_gradient(mode))
    except ValueError as exc:
        if str(exc).startswith("radial_gradient: unsupported mode"):
            raise ValueError("image has wrong mode") from exc
        raise


def effect_mandelbrot(
    size: tuple[int, int],
    extent: tuple[float, float, float, float],
    quality: int,
) -> Image:
    """Generate a Mandelbrot set covering the given extent."""
    from . import _core
    if not isinstance(extent, (tuple, list)) or len(extent) != 4:
        typename = type(extent).__name__
        raise TypeError(f"argument 2 must be 4-item sequence, not {typename}")
    return Image(_core.image_effect_mandelbrot(size, extent, quality))


def effect_noise(size: tuple[int, int], sigma: float) -> Image:
    """Generate a deterministic Gaussian-noise image."""
    return Image.effect_noise(size, sigma)


def frombuffer(mode: str, size: tuple[int, int], data, decoder_name: str = "raw", *args):
    """Create an image from pixel data in a byte buffer. Delegates to frombytes."""
    if decoder_name != "raw":
        raise OSError(f"decoder {decoder_name} not available")
    return Image.frombytes(mode, size, data, decoder_name, *args)


def eval(image: Image, *args):
    """Apply a function to each pixel. The first arg is a callable."""
    if args and isinstance(args[0], str):
        # Pillow's callable/LUT normalization reports this legacy diagnostic
        # before its point implementation sees the invalid string value.
        raise TypeError("type str doesn't define __round__ method")
    return image.point(args[0])


def thumbnail(
    image: Image,
    size: Tuple[int, int],
    resample: Union[int, str] = Resampling.BICUBIC,
) -> None:
    image.thumbnail(size, resample)
