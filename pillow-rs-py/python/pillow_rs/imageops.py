"""ImageOps — high-level image operations. Pillow-compatible module."""
# Ordinary scale, contain, cover, fit, and crop geometry is computed by
# pillow-rs. PyO3 callbacks preserve custom numeric protocols and overridden
# Image properties while the core retains the geometry decisions.
# Pad's typed route and custom-protocol fallback orchestration delegate to
# pillow-rs; the host adapter preserves public wrapper calls and protocols.
import re
from typing import Tuple, Union

from .image import Image
from . import _core

_BASE_RE_SUB = re.sub
_EQUALIZE_FAST_BUILTINS = (len, range, sum, list)

_BASE_IMAGE_SIZE = Image.size
_BASE_IMAGE_WIDTH = Image.width
_BASE_IMAGE_HEIGHT = Image.height
_BASE_IMAGE_CROP = Image.crop
_BASE_IMAGE_MODE = Image.mode
_BASE_IMAGE_RESIZE = Image.resize
_BASE_IMAGE_COPY = Image.copy
_BASE_IMAGE_PASTE = Image.paste
_BASE_IMAGE_INIT = Image.__init__
_BASE_IMAGE_NEW = Image.new.__func__
_BASE_IMAGE_WRAPPER_NEW = Image._new
_BASE_IMAGE_REALIZE_DIRTY_PALETTE = Image._realize_dirty_palette


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def _scale_native_types(image, factor):
    """Report host type facts for the core's native Scale strategy."""
    return (
        isinstance(image, Image)
        and type(image).__getattribute__ is object.__getattribute__
        and getattr(type(image), "size", None) is _BASE_IMAGE_SIZE
        and getattr(type(image), "width", None) is _BASE_IMAGE_WIDTH
        and getattr(type(image), "height", None) is _BASE_IMAGE_HEIGHT
        and (
            type(factor) is float
            or (type(factor) is int and factor <= 2**20)
        )
    )


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def _fit_native_types(bleed, centering):
    """Report host type facts for the core's native Fit strategy."""
    return (
        type(bleed) in (int, float)
        and type(centering) is tuple
        and len(centering) == 2
        and all(type(value) in (int, float) for value in centering)
    )


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def _unpack_fit_centering(centering):
    """Apply Python's pair-unpacking protocol for the callback Fit strategy."""
    centering_x, centering_y = centering
    return centering_x, centering_y


def autocontrast(image: Image, cutoff: Union[float, Tuple[float, float]] = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    return _core.ops_autocontrast_with_callbacks(
        image, cutoff, ignore, mask, preserve_tone
    )


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
def equalize(image: Image, mask=None) -> Image:
    return _core.ops_imageops_equalize_with_callbacks(image, mask)


def invert(image: Image) -> Image:
    return _core.ops_invert(image)


def flip(image: Image) -> Image:
    return image.transpose("FLIP_TOP_BOTTOM")


def mirror(image: Image) -> Image:
    return image.transpose("FLIP_LEFT_RIGHT")


def posterize(image: Image, bits: int) -> Image:
    return _core.ops_posterize(image, bits)


def solarize(image: Image, threshold: int = 128) -> Image:
    return _core.ops_solarize(image, threshold)


def grayscale(image: Image) -> Image:
    return image.convert("L")


def expand(image: Image, border=0, fill=0) -> Image:
    return _core.ops_expand_with_callbacks(image, border, fill, lambda: Image)


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def crop(image: Image, border: int = 0) -> Image:
    box = _core.ops_crop_box_with_route_callbacks(image, border)
    # TODO(pillow-rs#3ptb): verify custom border protocol and public crop
    # callback/fault/metadata behavior in CPU parity.
    return image.crop(box)


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def scale(image: Image, factor: float, resample=Image.Resampling.BICUBIC) -> Image:
    # TODO(pillow-rs#3ptb): freeze custom factor arithmetic, copy/load behavior,
    # and resize strategy receipts in the CPU parity matrix.
    return _core.ops_scale_with_route_callbacks(image, factor, resample)


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def contain(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    size = _core.ops_contain_size_with_route_callbacks(image, size)
    # TODO(pillow-rs#3ptb): freeze custom size arithmetic, mutation/fault order,
    # and Resize strategy receipts in the CPU parity matrix.
    method = _core._imageops_filter_code(method)
    return image.resize(size, resample=method)


_BASE_IMAGEOPS_CONTAIN = contain


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def cover(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    size = _core.ops_cover_size_with_route_callbacks(image, size)
    # TODO(pillow-rs#3ptb): freeze custom size arithmetic, mutation/fault order,
    # and Resize strategy receipts in the CPU parity matrix.
    method = _core._imageops_filter_code(method)
    return image.resize(size, resample=method)


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def fit(
    image: Image,
    size,
    method=Image.Resampling.BICUBIC,
    bleed=0.0,
    centering=(0.5, 0.5),
):
    box = _core.ops_fit_box_with_route_callbacks(
        image, size, bleed, centering
    )
    # TODO(pillow-rs#3ptb): freeze custom centering/bleed/size callbacks,
    # mutation/fault order, and Resize strategy receipts in CPU parity.
    method = _core._imageops_filter_code(method)
    return image.resize(size, method, box=box)


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def pad(
    image: Image, size, method=Image.Resampling.BICUBIC, color=None, centering=(0.5, 0.5)
):
    # TODO(pillow-rs#3ptb): parity-check a replaced module-level `contain`
    # callback uses the fallback, including its output and fault order; freeze
    # native/callback output, palette behavior, and route receipts as well.
    return _core.ops_pad_with_route_callbacks(
        image, size, method, color, centering
    )


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    return _core.ops_imageops_colorize_with_callbacks(
        image, black, white, mid, blackpoint, midpoint, whitepoint
    )


def exif_transpose(image: Image, *, in_place=False):
    image.load()
    changed, first_in_place, result = _core.ops_exif_transpose(
        image._rust_image, in_place
    )

    if changed:
        if first_in_place:
            # Keep `transposed_image` unbound on Pillow's first-true branch;
            # a later false truth test then raises at the same source point.
            pass
        else:
            transposed_image = image._new(result)
        exif_image = image if in_place else transposed_image
        # The core owns transposed pixels and retained EXIF bytes. Keep the
        # wrapper-owned info mapping in step with that result, then remove the
        # orientation marker from either supported XMP representation.
        info = exif_image.info
        exif = bytes(exif_image._rust_image.getexif())
        if "exif" in info:
            info["exif"] = exif
        elif "Raw profile type exif" in info:
            info["Raw profile type exif"] = exif.hex()
        for key in ("XML:com.adobe.xmp", "xmp"):
            if key not in info:
                continue
            value = info[key]
            value = _core.ops_exif_transpose_xmp_value(re, _BASE_RE_SUB, value)
            info[key] = value

        if not in_place:
            return transposed_image
    elif not in_place:
        return image.copy()
    # TODO(pillow-rs#3ptb): verify exact Exif.tobytes serialization and Pillow's
    # EXIF serialization, XMP callbacks, and outputs/faults in CPU parity.
    return None


def deform(image: Image, deformer, resample=Image.Resampling.BILINEAR):
    size = image.size
    method = Image.Transform.MESH
    mesh = deformer.getmesh(image)
    # TODO(pillow-rs#3ptb): freeze reentrant getmesh mutation, faults, and
    # Transform strategy receipts in CPU parity.
    resample = _core._thumbnail_filter_code(resample)
    return image.transform(size, method, mesh, resample)
