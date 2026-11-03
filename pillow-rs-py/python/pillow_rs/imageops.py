"""ImageOps — high-level image operations. Pillow-compatible module."""
from typing import Tuple, Union

from .image import Image
from . import _core


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
    else:
        left = top = right = bottom = border
    return left, top, right, bottom


def expand(image: Image, border=0, fill=0) -> Image:
    left, top, right, bottom = _expand_border(border)

    # Match ImageOps._border and its Python arithmetic before Pillow's fill
    # conversion. Retaining these values also preserves custom __add__ results.
    width = left + image.size[0] + right
    height = top + image.size[1] + bottom
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

    # Pillow evaluates image.mode before size validation. Keep that property
    # access in sequence; the captured Rust raster currently supplies the
    # prepared target mode for ordinary Image wrappers.
    # TODO(pillow-rs#3ptb): translate this captured spelling to Mode and carry
    # it into the prepared target. A subclass mode override that returns a
    # different valid mode currently affects access order but not output mode.
    target_mode = image.mode
    target = _core.ops_expand_capture_target(image._rust_image)
    size = (width, height)
    if size[0] < 0 or size[1] < 0:
        raise ValueError("Width and height must be >= 0")
    size = _core.ops_expand_validate_size(size, target)
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
