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
    """Add a border around the image. Delegates to Rust pipeline."""
    if isinstance(border, int):
        border = (border, border, border, border)
    if isinstance(fill, int):
        fill = (fill, fill, fill, 255)
    elif isinstance(fill, tuple) and len(fill) == 3:
        fill = (fill[0], fill[1], fill[2], 255)
    return Image(_core.ops_expand(image._rust_image, max(border), fill))


def crop(image: Image, border: int = 0) -> Image:
    """Crop border off image edges. Delegates to Rust pipeline."""
    return Image(_core.ops_crop_border(image._rust_image, border))


def scale(image: Image, factor: float, resample=None) -> Image:
    """Scale image by factor. Delegates to Rust pipeline."""
    return Image(_core.ops_scale(image._rust_image, factor, None))


def contain(image: Image, size, method=None) -> Image:
    """Resize to fit within size. Delegates to Rust pipeline."""
    return Image(_core.ops_contain(image._rust_image, (size[0], size[1]), None))


def cover(image: Image, size, method=None) -> Image:
    """Resize to cover size. Delegates to Rust pipeline."""
    return Image(_core.ops_cover(image._rust_image, (size[0], size[1]), None))


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    """Resize and crop to fit exact dimensions. Delegates to Rust pipeline."""
    return Image(_core.ops_fit(image._rust_image, (size[0], size[1]), None, bleed, centering))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    """Pad image to given size. Delegates to Rust pipeline."""
    c = (0, 0, 0, 255) if color is None else (
        (color, color, color, 255) if isinstance(color, int) else
        (color[0], color[1], color[2], 255) if len(color) == 3 else color
    )
    return Image(_core.ops_pad(image._rust_image, (size[0], size[1]), c, centering))


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    """Colorize grayscale image — delegates to Rust."""
    if image.mode != "L":
        image = image.convert("L")
    if isinstance(black, str):
        from PIL.ImageColor import getrgb
        black = getrgb(black)
    if isinstance(white, str):
        from PIL.ImageColor import getrgb
        white = getrgb(white)
    return Image(_core.ops_colorize(image._rust_image, black[:3], white[:3]))


def exif_transpose(image: Image, *, in_place=False):
    """Transpose based on EXIF orientation. Returns unchanged (no EXIF parsing yet)."""
    if in_place:
        return None
    return image.copy()


def deform(image: Image, deformer, resample=None):
    """Deform image using a mesh deformer. Matches PIL error behavior."""
    mesh = deformer.getmesh(image)
    result = image.transform(image.size, "MESH", mesh[0] if mesh else [])
    return result
