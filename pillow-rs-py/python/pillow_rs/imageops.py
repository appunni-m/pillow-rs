"""ImageOps — high-level image operations. Pillow-compatible module."""
from .image import Image
from . import _core


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    return Image(_core.ops_autocontrast(image._rust_image, cutoff))


def equalize(image: Image, mask=None) -> Image:
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
    return Image(_core.ops_contain(image._rust_image, size, method))


def cover(image: Image, size, method=None) -> Image:
    return Image(_core.ops_cover(image._rust_image, size, method))


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    return Image(_core.ops_fit(image._rust_image, size, method, bleed, centering))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    return Image(_core.ops_pad(image._rust_image, size, method, color, centering))


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    if isinstance(black, str):
        black = _core.getrgb(black)
    if isinstance(white, str):
        white = _core.getrgb(white)
    return Image(_core.ops_colorize(image._rust_image, black[:3], white[:3]))


def exif_transpose(image: Image, *, in_place=False):
    image.load()
    exif_data = image.getexif()
    orientation = _core.exif_get_orientation(exif_data) if isinstance(exif_data, bytes) else None
    orientation = orientation or 1

    method_map = {
        2: "FLIP_LEFT_RIGHT", 3: "ROTATE_180", 4: "FLIP_TOP_BOTTOM",
        5: "TRANSPOSE", 6: "ROTATE_270", 7: "TRANSVERSE", 8: "ROTATE_90",
    }
    method = method_map.get(orientation)

    if method is not None:
        if in_place:
            transposed = image.transpose(method)
            image._rust_image = transposed._rust_image
            image._explicit_mode = transposed._explicit_mode
        else:
            result = image.transpose(method)

        if isinstance(exif_data, bytes) and len(exif_data) >= 14:
            _core.exif_remove_orientation(exif_data)

        if not in_place:
            return result
        return None
    elif not in_place:
        return image.copy()
    return None


def deform(image: Image, deformer, resample=None):
    mesh = deformer.getmesh(image)
    result = image.transform(image.size, "MESH", mesh[0] if mesh else [])
    return result
