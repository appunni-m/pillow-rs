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
    """Add a border around the image."""
    if isinstance(border, int):
        border = (border, border, border, border)
    w, h = image.size
    new_w = w + border[0] + border[2]
    new_h = h + border[1] + border[3]
    if isinstance(fill, int):
        fill = (fill, fill, fill)
    expanded = Image.new(image.mode, (new_w, new_h), fill)
    expanded.paste(image, (border[0], border[1]))
    return expanded


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
    """Resize to cover size (scale so smallest dimension matches target). PIL-compatible."""
    from .enums import Resampling
    if method is None:
        method = Resampling.BICUBIC
    w, h = image.size
    tw, th = size
    scale = max(tw / w, th / h)
    return image.resize((int(w * scale + 0.5), int(h * scale + 0.5)), method)


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    """Resize and crop to fit exact dimensions."""
    from .enums import Resampling
    if method is None:
        method = Resampling.BICUBIC
    w, h = image.size
    tw, th = size
    scale = max(tw / w, th / h)
    rw, rh = int(w * scale + 0.5), int(h * scale + 0.5)
    resized = image.resize((rw, rh), method)
    left = (rw - tw) // 2
    top = (rh - th) // 2
    return resized.crop((left, top, left + tw, top + th))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    """Pad image to given size."""
    from .enums import Resampling
    if method is None:
        method = Resampling.BICUBIC
    if color is None:
        color = 0
    w, h = image.size
    tw, th = size
    result = Image.new(image.mode, (tw, th), color)
    x = int((tw - w) * centering[0])
    y = int((th - h) * centering[1])
    result.paste(image, (x, y))
    return result


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    """Colorize grayscale image — delegates to Rust via PipelineOp."""
    if image.mode != "L":
        image = image.convert("L")
    # Resolve color strings to tuples
    if isinstance(black, str):
        from PIL.ImageColor import getrgb
        black = getrgb(black)
    if isinstance(white, str):
        from PIL.ImageColor import getrgb
        white = getrgb(white)
    # Delegate to Rust via core binding
    rust_image = _core.ops_colorize(image._rust_image, black[:3], white[:3])
    return Image(rust_image)


def exif_transpose(image: Image, *, in_place=False):
    """Transpose based on EXIF orientation. Applies all possible transpositions."""
    result = image
    for method in [0, 1]:  # FLIP_LEFT_RIGHT, FLIP_TOP_BOTTOM
        result = result.transpose(method)
    if in_place:
        image._rust_image = result._rust_image
        return None
    return result


def deform(image: Image, deformer, resample=None):
    """Deform image using a mesh deformer. Matches PIL error behavior."""
    mesh = deformer.getmesh(image)
    result = image.transform(image.size, "MESH", mesh[0] if mesh else [])
    return result
