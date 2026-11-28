"""ImageOps — high-level image operations. Pillow-compatible module."""
# Ordinary scale, contain, cover, fit, and crop geometry is computed by
# pillow-rs. PyO3 callbacks preserve custom numeric protocols and overridden
# Image properties while the core retains the geometry decisions.
# Pad's typed route and custom-protocol fallback orchestration delegate to
# pillow-rs; the host adapter preserves public wrapper calls and protocols.
import re
from typing import Tuple, Union

from .image import Image, _execution_context_scope
from . import _core

_BASE_RE_SUB = re.sub
# Used only to recognize explicit overrides in this module's globals.
# Runtime mutation of Python's builtins is outside the compatibility contract.
_AUTOCONTRAST_FAST_BUILTINS = (int, list, range, sum)

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


# VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batches-17-18-imageops; benchmark=n/a
# Scope: Base resize method handling for Contain, Cover, Fit and Pad.
def _imageops_resize_call(image, method):
    """Validate ImageOps methods for the base resize path, preserving overrides."""
    resize = image.resize
    if getattr(resize, "__func__", None) is _BASE_IMAGE_RESIZE:
        method = _core._imageops_filter_code(method)
    return resize, method


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


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _fit_native_types(bleed, centering):
    """Report host type facts for the core's native Fit strategy."""
    return (
        # Keep integers on the callback path: Pillow compares arbitrary-size
        # integers before arithmetic, while converting them to f64 can overflow.
        type(bleed) is float
        and type(centering) is tuple
        and len(centering) == 2
        and all(type(value) is float for value in centering)
    )


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _unpack_fit_centering(centering):
    """Apply Python's pair-unpacking protocol for the callback Fit strategy."""
    centering_x, centering_y = centering
    return centering_x, centering_y


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def autocontrast(image: Image, cutoff: Union[float, Tuple[float, float]] = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    with _execution_context_scope(image):
        return _core.ops_autocontrast_with_callbacks(
            image, cutoff, ignore, mask, preserve_tone
        )


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def equalize(image: Image, mask=None) -> Image:
    with _execution_context_scope(image):
        return _core.ops_imageops_equalize_with_callbacks(image, mask)


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def invert(image: Image) -> Image:
    with _execution_context_scope(image):
        return _core.ops_invert(image)


# VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
def flip(image: Image) -> Image:
    return image.transpose("FLIP_TOP_BOTTOM")


# VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
def mirror(image: Image) -> Image:
    return image.transpose("FLIP_LEFT_RIGHT")


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def posterize(image: Image, bits: int) -> Image:
    with _execution_context_scope(image):
        return _core.ops_posterize(image, bits)


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def solarize(image: Image, threshold: int = 128) -> Image:
    with _execution_context_scope(image):
        return _core.ops_solarize(image, threshold)


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def grayscale(image: Image) -> Image:
    return image.convert("L")


# TODO(pillow-rs#3ptb): verify canvas pixels, palette/transparency metadata,
# callback/fault order, and shared architecture receipts for public Paste.
# TODO(pillow-rs#3ptb): verify the two live mode reads, active route handoff to
# the eager canvas fill, palette callbacks, transparency copy, nested Paste
# load/fault order and eager mutation, and mode-by-architecture Expand/Paste
# receipts.
# VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
def expand(image: Image, border=0, fill=0) -> Image:
    with _execution_context_scope(image):
        return _core.ops_expand_with_callbacks(image, border, fill, lambda: Image)


# VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
def crop(image: Image, border: int = 0) -> Image:
    with _execution_context_scope(image):
        box = _core.ops_crop_box_with_route_callbacks(image, border)
        # TODO(pillow-rs#3ptb): verify custom border protocol, public crop
        # callback/fault/metadata order, and mode-by-architecture receipts in
        # CPU parity, including the full-box copy versus deferred crop boundary
        # and Pillow's ValueError for tuple borders whose length is not two or four.
        return image.crop(box)


# VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
def scale(image: Image, factor: float, resample=Image.Resampling.BICUBIC) -> Image:
    with _execution_context_scope(image):
        # TODO(pillow-rs#3ptb): freeze custom factor arithmetic, copy/load behavior,
        # and resize strategy receipts in the CPU parity matrix.
        return _core.ops_scale_with_route_callbacks(image, factor, resample)


# VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batch-17-imageops-rerun-01; benchmark=pending
def contain(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    with _execution_context_scope(image):
        route, resize_size = _core.ops_contain_size_with_route_callbacks(image, size)
        if route == 2:
            return Image.new(image.mode, (size[0], 0))
        if route == 3:
            return Image.new(image.mode, (0, 0))
        if route == 4:
            return Image.new(image.mode, size)
        if route == 5:
            return Image.new(image.mode, resize_size)
        size = resize_size
        # TODO(pillow-rs#3ptb): verify empty-result mode/palette/metadata; custom
        # size/property mutation and fault order; subclass resize dispatch; mode ×
        # architecture Resize receipts/fallbacks; and public-call latency.
        resize, method = _imageops_resize_call(image, method)
        return resize(size, resample=method)


_BASE_IMAGEOPS_CONTAIN = contain


# VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batch-17-imageops-rerun-01; benchmark=pending
def cover(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    with _execution_context_scope(image):
        route, resize_size = _core.ops_cover_size_with_route_callbacks(image, size)
        if route == 2:
            return Image.new(image.mode, (size[0], 0))
        if route == 3:
            return Image.new(image.mode, (0, 0))
        size = resize_size
        # TODO(pillow-rs#3ptb): verify empty-result mode/palette/metadata; custom
        # size/property mutation and fault order; subclass resize dispatch; mode ×
        # architecture Resize receipts/fallbacks; and public-call latency.
        resize, method = _imageops_resize_call(image, method)
        return resize(size, resample=method)


# VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batch-18-rerun-04; benchmark=pending
def fit(
    image: Image,
    size,
    method=Image.Resampling.BICUBIC,
    bleed=0.0,
    centering=(0.5, 0.5),
):
    with _execution_context_scope(image):
        box = _core.ops_fit_box_with_route_callbacks(
            image, size, bleed, centering
        )
        if box is None:
            # Pillow returns Image.new before parsing the resize filter when the
            # target has a zero axis or the live source height is zero.
            return Image.new(image.mode, size)
        # TODO(pillow-rs#3ptb): verify Image.new output/metadata for zero target
        # axes and zero live source height; freeze custom centering/bleed/size
        # callbacks (including arbitrary-size ints), mutation/fault order, subclass
        # resize dispatch, mode × architecture Resize receipts/fallbacks, and
        # public-call latency.
        resize, method = _imageops_resize_call(image, method)
        return resize(size, method, box=box)


# TODO(pillow-rs#3ptb): parity-check that native padded results retain the
# active CPU Architecture through wrapper construction and pending execution.
# VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batch-18-rerun-04; benchmark=pending
def pad(
    image: Image, size, method=Image.Resampling.BICUBIC, color=None, centering=(0.5, 0.5)
):
    # TODO(pillow-rs#3ptb): parity-check a replaced module-level `contain`
    # callback uses the fallback, including its output and fault order; freeze
    # palette behavior and shared Resize/Paste context receipts as well.
    with _execution_context_scope(image):
        return _core.ops_pad_with_route_callbacks(
            image, size, method, color, centering
        )


# VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    with _execution_context_scope(image):
        return _core.ops_imageops_colorize_with_callbacks(
            image, black, white, mid, blackpoint, midpoint, whitepoint
        )


# TODO(pillow-rs#3ptb): verify EXIF transpose output/fault ordering, selected
# architecture receipts after eager Load, and retained metadata in CPU parity.
# VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
def exif_transpose(image: Image, *, in_place=False):
    # Loading can consume a lazy pipeline's selected context, so retain the
    # route through the later Rust-owned transpose operation.
    with _execution_context_scope(image):
        image.load()
        execution_context = (
            image._current_execution_context() if isinstance(image, Image) else None
        )
        changed, first_in_place, result = _core.ops_exif_transpose(
            image._rust_image, in_place, execution_context
        )

    if changed:
        if first_in_place:
            # Keep `transposed_image` unbound on Pillow's first-true branch;
            # a later false truth test then raises at the same source point.
            image._rust_image.load(execution_context)
        else:
            result.load(execution_context)
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
    # TODO(pillow-rs#3ptb): verify exact Exif.tobytes serialization, XMP
    # callbacks, and outputs/faults in CPU parity.
    return None


# VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
def deform(image: Image, deformer, resample=Image.Resampling.BILINEAR):
    with _execution_context_scope(image):
        size = image.size
        method = Image.Transform.MESH
        mesh = deformer.getmesh(image)
        # TODO(pillow-rs#3ptb): freeze reentrant getmesh mode mutation, faults,
        # and mode-by-architecture Transform receipts; mode planning must happen
        # after getmesh, while size remains the earlier snapshot.
        resample = _core._thumbnail_filter_code(resample)
        return image.transform(size, method, mesh, resample)
