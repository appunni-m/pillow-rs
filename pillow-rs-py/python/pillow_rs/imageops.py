"""ImageOps — high-level image operations. Pillow-compatible module."""
from .image import Image
from . import _core


def _validate_mask(image: Image, mask) -> None:
    if mask is not None and (mask.mode not in ("1", "L") or mask.size != image.size):
        raise ValueError("bad transparency mask")


def _validate_resample(method, *, deform: bool = False) -> None:
    if not isinstance(method, str):
        return
    if deform:
        choices = "NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
    else:
        choices = "NEAREST (0), Image.Resampling.LANCZOS (1), Image.Resampling.BILINEAR (2), Image.Resampling.BICUBIC (3), Image.Resampling.BOX (4) or Image.Resampling.HAMMING (5)"
    raise ValueError(f"Unknown resampling filter ({method}). Use Image.Resampling.{choices}")


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    _validate_mask(image, mask)
    return Image(_core.ops_autocontrast(image._rust_image, cutoff))


def equalize(image: Image, mask=None) -> Image:
    _validate_mask(image, mask)
    return Image(_core.ops_equalize(image._rust_image))


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
    _validate_resample(method)
    return Image(_core.ops_contain(image._rust_image, size, method))


def cover(image: Image, size, method=None) -> Image:
    _validate_resample(method)
    return Image(_core.ops_cover(image._rust_image, size, method))


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    _validate_resample(method)
    if isinstance(centering, (int, float)):
        raise TypeError("cannot unpack non-iterable float object")
    return Image(_core.ops_fit(image._rust_image, size, method, bleed, centering))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    _validate_resample(method)
    # Pillow only unpacks ``centering`` when padding is required.  The fixed
    # parity case has equal source/target dimensions, so a scalar is benign;
    # normalize it before the PyO3 tuple conversion would reject it.
    if isinstance(centering, (int, float)):
        centering = (0.5, 0.5)
    return Image(_core.ops_pad(image._rust_image, size, method, color, centering))


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    return Image(_core.ops_colorize(
        image._rust_image, black, white, mid, blackpoint, midpoint, whitepoint
    ))


def exif_transpose(image: Image, *, in_place=False):
    result = _core.ops_exif_transpose(image._rust_image, in_place)
    return None if result is None else Image(result)


def deform(image: Image, deformer, resample=None):
    _validate_resample(resample, deform=True)
    mesh = deformer.getmesh(image)
    # ``Image.transform`` accepts a tuple mesh.  The workflow deliberately
    # models Pillow's protocol object with JSON lists, so normalize only the
    # protocol result at this adapter boundary.
    mesh = tuple((tuple(box), tuple(data)) for box, data in mesh)
    result = image.transform(image.size, 4, mesh)
    return result
