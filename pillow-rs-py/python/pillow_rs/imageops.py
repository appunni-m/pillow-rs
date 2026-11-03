"""ImageOps — high-level image operations. Pillow-compatible module."""
from .image import Image
from . import _core


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    return image._new(_core.ops_autocontrast(image._rust_image, cutoff, mask))


def equalize(image: Image, mask=None) -> Image:
    return image._new(_core.ops_equalize(image._rust_image, mask))


def invert(image: Image) -> Image:
    return image._new(_core.ops_invert(image._rust_image))


def flip(image: Image) -> Image:
    return image._new(_core.ops_flip(image._rust_image))


def mirror(image: Image) -> Image:
    return image._new(_core.ops_mirror(image._rust_image))


def posterize(image: Image, bits: int) -> Image:
    return image._new(_core.ops_posterize(image._rust_image, bits))


def solarize(image: Image, threshold: int = 128) -> Image:
    return image._new(_core.ops_solarize(image._rust_image, threshold))


def grayscale(image: Image) -> Image:
    return image._new(_core.ops_grayscale(image._rust_image))


def _expand_border(border):
    if isinstance(border, tuple):
        if len(border) == 2:
            left, top = right, bottom = border
        elif len(border) == 4:
            left, top, right, bottom = border
    else:
        left = top = right = bottom = border
    return left, top, right, bottom


def expand(image: Image, border=0, fill=0) -> Image:
    left, top, right, bottom = _expand_border(border)

    # Match ImageOps._border and its Python arithmetic before Pillow's fill
    # conversion. Retaining these values also preserves custom __add__ results.
    width = left + image.size[0] + right
    height = top + image.size[1] + bottom
    prepared_fill = _core.ops_expand_prepare_fill(image._rust_image, fill)

    # Pillow reads image.palette after _color. For indexed modes this property
    # can load the source and realize dirty palette state before Image.new.
    palette = image.palette
    if palette and palette.dirty:
        image._realize_dirty_palette()

    prepared_fill = _core.ops_expand_resolve_palette_fill(
        image._rust_image, prepared_fill
    )
    size = (width, height)
    # Image.new evaluates image.mode before its Python size comparisons.
    size_image = image._rust_image
    if size[0] < 0 or size[1] < 0:
        raise ValueError("Width and height must be >= 0")
    size = _core.ops_expand_validate_size(size_image, size)

    # Image.paste forms the full box from the source size after Image.new has
    # accepted its dimensions.
    source_size = image.size
    paste_box = (
        left,
        top,
        left + source_size[0],
        top + source_size[1],
    )

    # TODO(pillow-rs#3ptb): preserve mode/source mutations caused by effectful
    # size comparisons, then audit eager allocation failure timing and Paste's
    # source-load-before-coordinate-parser order while retaining lazy Expand.
    return Image(
        _core.ops_expand(
            image._rust_image,
            size,
            paste_box,
            prepared_fill,
        )
    )


def crop(image: Image, border: int = 0) -> Image:
    return image._new(_core.ops_crop_border(image._rust_image, border))


def scale(image: Image, factor: float, resample=None) -> Image:
    return image._new(_core.ops_scale(image._rust_image, factor, resample))


def _resize_after_geometry(image, size, filter_code, box=None):
    # TODO(pillow-rs#3ptb): add frozen CPU parity for reentrant filter callbacks
    # that replace/mutate the wrapper, including mode and getdata identity.
    full_box = (0, 0) + image.size
    resize_box = full_box if box is None else box
    if image.size == size and resize_box == full_box:
        return image.copy()
    if box is None:
        return image.resize(size, filter_code)
    return image.resize(size, filter_code, box)


def contain(image: Image, size, method=None) -> Image:
    output_size = _core.ops_contain_geometry(image._rust_image, size)
    filter_code = _core.ops_imageops_filter_code(method)
    return _resize_after_geometry(image, output_size, filter_code)


def cover(image: Image, size, method=None) -> Image:
    output_size = _core.ops_cover_geometry(image._rust_image, size)
    filter_code = _core.ops_imageops_filter_code(method)
    return _resize_after_geometry(image, output_size, filter_code)


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    size, box = _core.ops_fit_geometry(image._rust_image, size, bleed, centering)
    filter_code = _core.ops_imageops_filter_code(method)
    return _resize_after_geometry(image, size, filter_code, box)


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    palette = image.__dict__.get("_palette_object")
    if palette is not None and palette.dirty and image.mode in ("P", "PA"):
        # Contain geometry and resize filter/dimensions precede palette
        # realization. Keep that order before Pad resolves fill or centering.
        method = _core.ops_pad_preflight(image._rust_image, size, method)
        image._realize_dirty_palette()
    result, uses_contain_result = _core.ops_pad(
        image._rust_image, size, method, color, centering
    )
    if uses_contain_result:
        return image._new(result)
    return Image(result)


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    return image._new(_core.ops_colorize(
        image._rust_image, black, white, mid, blackpoint, midpoint, whitepoint
    ))


def exif_transpose(image: Image, *, in_place=False):
    result = _core.ops_exif_transpose(image._rust_image, in_place)
    return None if result is None else image._new(result)


def deform(image: Image, deformer, resample=None):
    _core.ops_validate_deform_resample(resample)
    mesh = deformer.getmesh(image)
    return image.transform(image.size, 4, mesh)
