"""ImageOps — high-level image operations. Pillow-compatible module."""
# Ordinary scale, contain, cover, fit, and crop geometry is computed by
# pillow-rs. PyO3 callbacks preserve custom numeric protocols and overridden
# Image properties while the core retains the geometry decisions.
# Pad's ordinary typed route delegates composition and palette handling to
# pillow-rs; Python composition remains only where custom protocols or wrapper
# overrides make those calls observable.
import re
from typing import Tuple, Union

from .image import Image
from . import _core

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


# TODO(pillow-rs#3ptb): move native-vs-callback strategy selection behind a
# core-owned dispatch contract; keep this adapter limited to host capability facts.
def _native_size_pair(image, size):
    if (
        not isinstance(image, Image)
        or type(image).__getattribute__ is not object.__getattribute__
        or getattr(type(image), "size", None) is not _BASE_IMAGE_SIZE
        or getattr(type(image), "width", None) is not _BASE_IMAGE_WIDTH
        or getattr(type(image), "height", None) is not _BASE_IMAGE_HEIGHT
        or type(size) is not tuple
        or len(size) != 2
    ):
        return None
    width, height = size
    if (
        type(width) is int
        and type(height) is int
        and 0 <= width <= 0xFFFFFFFF
        and 0 <= height <= 0xFFFFFFFF
    ):
        return width, height


def _native_pad_size(image, size, method, color, centering):
    dimensions = _native_size_pair(image, size)
    if (
        dimensions is None
        or type(image) is not Image
        or Image.mode is not _BASE_IMAGE_MODE
        or Image.resize is not _BASE_IMAGE_RESIZE
        or Image.copy is not _BASE_IMAGE_COPY
        or Image.paste is not _BASE_IMAGE_PASTE
        or Image.__init__ is not _BASE_IMAGE_INIT
        or Image._realize_dirty_palette is not _BASE_IMAGE_REALIZE_DIRTY_PALETTE
        or getattr(Image.new, "__func__", Image.new) is not _BASE_IMAGE_NEW
        or Image._new is not _BASE_IMAGE_WRAPPER_NEW
        or any(
            name in image.__dict__
            for name in (
                "mode",
                "resize",
                "copy",
                "_new",
                "_realize_dirty_palette",
            )
        )
        or type(method) not in (int, Image.Resampling)
        or type(centering) is not tuple
        or len(centering) != 2
        or any(type(value) not in (int, float) for value in centering)
    ):
        return None
    if color is not None and type(color) not in (int, float):
        if type(color) is not tuple or any(
            type(value) not in (int, float) for value in color
        ):
            return None
    return dimensions


def autocontrast(image: Image, cutoff: Union[float, Tuple[float, float]] = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    if preserve_tone:
        histogram = image.convert("L").histogram(mask)
    else:
        histogram = image.histogram(mask)
    lut = []
    for layer in range(0, len(histogram), 256):
        channel = histogram[layer : layer + 256]
        if ignore is not None:
            if isinstance(ignore, int):
                channel[ignore] = 0
            else:
                for index in ignore:
                    channel[index] = 0
        if cutoff:
            if not isinstance(cutoff, tuple):
                cutoff = (cutoff, cutoff)
            channel_cutoff = cutoff
        else:
            channel_cutoff = None
        lut.extend(_core.ops_autocontrast_lut(channel, channel_cutoff))
    if image.mode == "P":
        raise NotImplementedError("mode P support coming soon")
    if image.mode not in ("L", "RGB"):
        raise OSError(f"not supported for mode {image.mode}")
    if image.mode == "RGB" and len(lut) == 256:
        lut = lut * 3
    return image.point(lut)


def equalize(image: Image, mask=None) -> Image:
    # Pillow converts P to RGB before histogramming, preserving palette edits
    # and conversion-time transparency metadata through Image.convert.
    if image.mode == "P":
        image = image.convert("RGB")
    histogram = image.histogram(mask)
    lut = _core.ops_equalize_lut(image._rust_image, histogram)
    return image.point(lut)


def invert(image: Image) -> Image:
    return image._new(_core.ops_invert(image._rust_image))


def flip(image: Image) -> Image:
    return image.transpose("FLIP_TOP_BOTTOM")


def mirror(image: Image) -> Image:
    return image.transpose("FLIP_LEFT_RIGHT")


def posterize(image: Image, bits: int) -> Image:
    # NO_PILLOW_SUPPORT: The Rust P strategy applies the LUT to native indices.
    return image._new(_core.ops_posterize(image._rust_image, bits))


def solarize(image: Image, threshold: int = 128) -> Image:
    # NO_PILLOW_SUPPORT: The Rust P strategy applies the LUT to native indices.
    return image._new(_core.ops_solarize(image._rust_image, threshold))


def grayscale(image: Image) -> Image:
    return image.convert("L")


def expand(image: Image, border=0, fill=0) -> Image:
    # TODO(pillow-rs#3ptb): move remaining fill, palette, canvas, and paste
    # orchestration into core callbacks while retaining observable host methods.
    width, height, left, top = _core.ops_expand_geometry_with_callbacks(
        image, border
    )
    fill_mode = image.mode
    if isinstance(fill, str):
        from .imagecolor import getcolor

        fill = getcolor(fill, fill_mode)

    # Pillow snapshots an existing palette only after parsing the fill. Read
    # its mode before getpalette loads the source and realizes dirty state.
    source_palette = image.palette
    palette = None
    palette_mode = None
    if source_palette:
        palette_mode = source_palette.mode
        from .imagepalette import ImagePalette

        palette = ImagePalette(palette_mode, image.getpalette(palette_mode))
        if isinstance(fill, tuple) and len(fill) in (3, 4):
            fill = palette.getcolor(fill)

    # Image.new evaluates image.mode before size validation. Capture the
    # returned Pillow mode before effectful size comparisons; the binding
    # converts it to the flat Rust Mode enum at the typed operation boundary.
    target_mode = image.mode
    size = (width, height)
    if size[0] < 0 or size[1] < 0:
        raise ValueError("Width and height must be >= 0")
    target, size = _core.ops_expand_capture_target(size, target_mode)
    prepared_fill = _core.ops_expand_prepare_fill(target, fill)
    prepared_fill = _core.ops_expand_resolve_palette_fill(target, prepared_fill)

    # Pillow allocates the target-mode Image.new canvas before Image.paste
    # loads or converts the current source wrapper.
    output = Image(_core.ops_expand_canvas(size, prepared_fill))
    if palette:
        output.putpalette(palette.palette, palette_mode)
    output.paste(image, (left, top))
    output.load()
    return output


def crop(image: Image, border: int = 0) -> Image:
    if (
        type(image) is Image
        and Image.size is _BASE_IMAGE_SIZE
        and Image.crop is _BASE_IMAGE_CROP
        and "crop" not in image.__dict__
        and type(border) is int
        and 0 <= border <= 0xFFFFFFFF
    ):
        box = _core.ops_crop_box_geometry_preflight(image._rust_image, border)
    else:
        box = _core.ops_crop_box_with_callbacks(image, border)
    # TODO(pillow-rs#3ptb): verify custom border protocol and public crop
    # callback/fault/metadata behavior in CPU parity.
    return image.crop(box)


def scale(image: Image, factor: float, resample=Image.Resampling.BICUBIC) -> Image:
    if factor == 1:
        return image.copy()
    native_geometry = (
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
    if native_geometry:
        size = _core.ops_scale_geometry_preflight(image._rust_image, float(factor))
    else:
        # The core owns validation and dimension selection. The binding calls
        # Python's operators only when a custom numeric or image protocol runs.
        size = _core.ops_scale_size_with_callbacks(image, factor)
    # TODO(pillow-rs#3ptb): freeze custom factor arithmetic, copy/load behavior,
    # and resize strategy receipts in the CPU parity matrix.
    return image.resize(size, resample)


def contain(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    native_size = _native_size_pair(image, size)
    if native_size is not None:
        size = _core.ops_contain_geometry_preflight(
            image._rust_image, *native_size
        )
    else:
        size = _core.ops_contain_size_with_callbacks(image, size)
    # TODO(pillow-rs#3ptb): freeze custom size arithmetic, mutation/fault order,
    # and Resize strategy receipts in the CPU parity matrix.
    return image.resize(size, resample=method)


def cover(image: Image, size, method=Image.Resampling.BICUBIC) -> Image:
    native_size = _native_size_pair(image, size)
    if native_size is not None:
        size = _core.ops_cover_geometry_preflight(
            image._rust_image, *native_size
        )
    else:
        size = _core.ops_cover_size_with_callbacks(image, size)
    # TODO(pillow-rs#3ptb): freeze custom size arithmetic, mutation/fault order,
    # and Resize strategy receipts in the CPU parity matrix.
    return image.resize(size, resample=method)


def fit(
    image: Image,
    size,
    method=Image.Resampling.BICUBIC,
    bleed=0.0,
    centering=(0.5, 0.5),
):
    native_size = _native_size_pair(image, size)
    if (
        native_size is not None
        and type(bleed) in (int, float)
        and type(centering) is tuple
        and len(centering) == 2
        and all(type(value) in (int, float) for value in centering)
    ):
        box = _core.ops_fit_geometry_preflight(
            image._rust_image,
            *native_size,
            float(bleed),
            centering,
        )
        return image.resize(size, method, box=box)

    centering_x, centering_y = centering
    box = _core.ops_fit_box_with_callbacks(
        image, size, bleed, centering_x, centering_y
    )
    # TODO(pillow-rs#3ptb): freeze custom centering/bleed/size callbacks,
    # mutation/fault order, and Resize strategy receipts in CPU parity.
    return image.resize(size, method, box=box)


def pad(
    image: Image, size, method=Image.Resampling.BICUBIC, color=None, centering=(0.5, 0.5)
):
    # TODO(pillow-rs#3ptb): freeze Pad output, callback-path fault order, and
    # mode/architecture receipts in CPU parity.
    native_size = _native_pad_size(image, size, method, color, centering)
    if native_size is not None:
        result, is_contained = _core.ops_pad_with_input_and_containment(
            image, *native_size, method, color, centering
        )
        if is_contained:
            return image._new(result)
        return Image(result)

    # TODO(pillow-rs#3ptb): move custom fallback composition and centering
    # decisions to core callbacks while keeping public Image methods observable.
    resized = contain(image, size, method)
    if resized.size == size:
        return resized

    # Keep Pillow's wrapper order for custom protocols and overridden methods:
    # create the filled canvas before indexing centering, then paste through
    # the public mode-aware Image operation.
    output = Image.new(image.mode, size, color)
    if resized.palette:
        palette = resized.getpalette()
        if palette is not None:
            output.putpalette(palette)
    if resized.width != size[0]:
        x = round((size[0] - resized.width) * max(0, min(centering[0], 1)))
        output.paste(resized, (x, 0))
    else:
        y = round((size[1] - resized.height) * max(0, min(centering[1], 1)))
        output.paste(resized, (0, y))
    return output


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    # Pillow checks the source mode and point ordering before parsing colors.
    assert image.mode == "L"
    if mid is None:
        assert 0 <= blackpoint <= whitepoint <= 255
    else:
        assert 0 <= blackpoint <= midpoint <= whitepoint <= 255
    lut = _core.ops_colorize_lut(
        black, white, mid, blackpoint, midpoint, whitepoint
    )
    image = image.convert("RGB")
    # TODO(pillow-rs#3ptb): freeze callback-driven mutation, output/fault order,
    # and Convert/Point selection receipts in the CPU parity matrix.
    return image.point(lut)


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
            for pattern in (
                r'tiff:Orientation="([0-9])"',
                r"<tiff:Orientation>([0-9])</tiff:Orientation>",
            ):
                if isinstance(value, str):
                    value = re.sub(pattern, "", value)
                elif isinstance(value, tuple):
                    value = tuple(
                        re.sub(pattern.encode(), b"", item) for item in value
                    )
                else:
                    value = re.sub(pattern.encode(), b"", value)
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
    return image.transform(size, method, mesh, resample)
