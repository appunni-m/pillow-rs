"""ImageOps — high-level image operations. Pillow-compatible module."""
from .image import Image
from . import _core


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    """Normalize image contrast."""
    return Image(_core.ops_autocontrast(image._rust_image, cutoff))


def equalize(image: Image, mask=None) -> Image:
    """Equalize the image histogram."""
    return Image(_core.ops_equalize(image._rust_image))


def invert(image: Image) -> Image:
    """Invert all pixel values (negative)."""
    return Image(_core.ops_invert(image._rust_image))


def flip(image: Image) -> Image:
    """Flip image vertically (top to bottom)."""
    return Image(_core.ops_flip(image._rust_image))


def mirror(image: Image) -> Image:
    """Mirror image horizontally (left to right)."""
    return Image(_core.ops_mirror(image._rust_image))


def posterize(image: Image, bits: int) -> Image:
    """Reduce number of bits per color channel."""
    return Image(_core.ops_posterize(image._rust_image, bits))


def solarize(image: Image, threshold: int = 128) -> Image:
    """Invert all pixel values above threshold."""
    return Image(_core.ops_solarize(image._rust_image, threshold))


def grayscale(image: Image) -> Image:
    """Convert image to grayscale."""
    return Image(_core.ops_grayscale(image._rust_image))


def expand(image: Image, border=0, fill=0) -> Image:
    """Add a border around the image. Not yet implemented."""
    raise NotImplementedError("ImageOps.expand")


def crop(image: Image, border: int = 0) -> Image:
    """Crop border off image edges."""
    w, h = image.size
    return image.crop((border, border, w - border, h - border))


def scale(image: Image, factor: float, resample=None) -> Image:
    """Scale image by factor."""
    w, h = image.size
    return image.resize((int(w * factor), int(h * factor)), resample)


def contain(image: Image, size, method=None) -> Image:
    """Resize to fit within size, preserving aspect ratio."""
    from .enums import Resampling
    if method is None:
        method = Resampling.BICUBIC
    w, h = image.size
    tw, th = size
    scale = min(tw / w, th / h)
    return image.resize((int(w * scale), int(h * scale)), method)


def cover(image: Image, size, method=None) -> Image:
    """Resize to cover size, preserving aspect ratio, then crop."""
    from .enums import Resampling
    if method is None:
        method = Resampling.BICUBIC
    w, h = image.size
    tw, th = size
    scale = max(tw / w, th / h)
    resized = image.resize((int(w * scale), int(h * scale)), method)
    rw, rh = resized.size
    left = (rw - tw) // 2
    top = (rh - th) // 2
    return resized.crop((left, top, left + tw, top + th))
