"""ImageChops — channel operations. Pillow-compatible module."""
from .image import Image
from . import _core


def add(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image:
    """Add two images."""
    return Image(_core.chops_add(image1._rust_image, image2._rust_image, scale, offset))


def subtract(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image:
    """Subtract image2 from image1."""
    return Image(_core.chops_subtract(image1._rust_image, image2._rust_image, scale, offset))


def multiply(image1: Image, image2: Image) -> Image:
    """Multiply two images."""
    return Image(_core.chops_multiply(image1._rust_image, image2._rust_image))


def screen(image1: Image, image2: Image) -> Image:
    """Screen blend mode."""
    return Image(_core.chops_screen(image1._rust_image, image2._rust_image))


def darker(image1: Image, image2: Image) -> Image:
    """Return darker pixel at each position."""
    return Image(_core.chops_darker(image1._rust_image, image2._rust_image))


def lighter(image1: Image, image2: Image) -> Image:
    """Return lighter pixel at each position."""
    return Image(_core.chops_lighter(image1._rust_image, image2._rust_image))


def difference(image1: Image, image2: Image) -> Image:
    """Absolute difference between two images."""
    return Image(_core.chops_difference(image1._rust_image, image2._rust_image))


def invert(image: Image) -> Image:
    """Invert image."""
    return Image(_core.chops_invert(image._rust_image))


def duplicate(image: Image) -> Image:
    """Duplicate an image."""
    return image.copy()


def offset(image: Image, xoffset: int, yoffset: int = None) -> Image:
    """Offset image contents."""
    raise NotImplementedError("ImageChops.offset")


def logical_and(image1: Image, image2: Image) -> Image:
    """Bitwise AND."""
    raise NotImplementedError("ImageChops.logical_and")


def logical_or(image1: Image, image2: Image) -> Image:
    """Bitwise OR."""
    raise NotImplementedError("ImageChops.logical_or")
