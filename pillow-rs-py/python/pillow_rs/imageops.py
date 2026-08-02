"""ImageOps — high-level image operations. Pillow-compatible module."""
from .image import Image
from . import _core


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    return Image(_core.ops_autocontrast(image._rust_image, cutoff, mask))


def equalize(image: Image, mask=None) -> Image:
    return Image(_core.ops_equalize(image._rust_image, mask))


def invert(image: Image) -> Image:
    return Image(_core.ops_invert(image._rust_image))


def flip(image: Image) -> Image:
    return Image(_core.ops_flip(image._rust_image))


def mirror(image: Image) -> Image:
    return Image(_core.ops_mirror(image._rust_image))


def posterize(image: Image, bits: int) -> Image:
    return Image(_core.ops_posterize(image._rust_image, bits))


def solarize(image: Image, threshold: int = 128) -> Image:
    return Image(_core.ops_solarize(image._rust_image, threshold))


def grayscale(image: Image) -> Image:
    return Image(_core.ops_grayscale(image._rust_image))


def expand(image: Image, border=0, fill=0) -> Image:
    return Image(_core.ops_expand(image._rust_image, border, fill))


def crop(image: Image, border: int = 0) -> Image:
    return Image(_core.ops_crop_border(image._rust_image, border))


def scale(image: Image, factor: float, resample=None) -> Image:
    return Image(_core.ops_scale(image._rust_image, factor, resample))


def contain(image: Image, size, method=None) -> Image:
    return Image(_core.ops_contain(image._rust_image, size, method))


def cover(image: Image, size, method=None) -> Image:
    return Image(_core.ops_cover(image._rust_image, size, method))


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    return Image(_core.ops_fit(image._rust_image, size, method, bleed, centering))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    return Image(_core.ops_pad(image._rust_image, size, method, color, centering))


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    return Image(_core.ops_colorize(
        image._rust_image, black, white, mid, blackpoint, midpoint, whitepoint
    ))


def exif_transpose(image: Image, *, in_place=False):
    result = _core.ops_exif_transpose(image._rust_image, in_place)
    return None if result is None else Image(result)


def deform(image: Image, deformer, resample=None):
    _core.ops_validate_deform_resample(resample)
    mesh = deformer.getmesh(image)
    return image.transform(image.size, 4, mesh)
