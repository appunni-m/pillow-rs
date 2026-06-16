"""ImageFilter — convolution kernels and filter classes. Pillow-compatible module."""
from ._core import Image as RustImage
from . import _core
from .image import Image


# Built-in kernel filters (applied via Image.filter(name))
BLUR = "BLUR"
CONTOUR = "CONTOUR"
DETAIL = "DETAIL"
EDGE_ENHANCE = "EDGE_ENHANCE"
EDGE_ENHANCE_MORE = "EDGE_ENHANCE_MORE"
EMBOSS = "EMBOSS"
FIND_EDGES = "FIND_EDGES"
SHARPEN = "SHARPEN"
SMOOTH = "SMOOTH"
SMOOTH_MORE = "SMOOTH_MORE"


class GaussianBlur:
    def __init__(self, radius=2):
        self.radius = float(radius)
    def _apply(self, rust_image):
        return Image(rust_image.gaussian_blur(self.radius))


class BoxBlur:
    def __init__(self, radius=2):
        self.radius = float(radius)
    def _apply(self, rust_image):
        return Image(rust_image.box_blur(self.radius))


class UnsharpMask:
    def __init__(self, radius=2, percent=150, threshold=3):
        self.radius = float(radius)
        self.percent = int(percent)
        self.threshold = int(threshold)
    def _apply(self, rust_image):
        return Image(rust_image.unsharp_mask(self.radius, self.percent, self.threshold))


class MaxFilter:
    def __init__(self, size=3):
        self.size = int(size)
    def _apply(self, rust_image):
        return Image(rust_image.max_filter(self.size))


class MinFilter:
    def __init__(self, size=3):
        self.size = int(size)
    def _apply(self, rust_image):
        return Image(rust_image.min_filter(self.size))


class MedianFilter:
    def __init__(self, size=3):
        self.size = int(size)
    def _apply(self, rust_image):
        return Image(rust_image.median_filter(self.size))


class ModeFilter:
    name = "Mode"

    def __init__(self, size=3):
        self.size = int(size)
    def _apply(self, rust_image):
        return Image(rust_image.mode_filter(self.size))


class RankFilter:
    def __init__(self, size=3, rank=0):
        self.size = int(size)
        self.rank = int(rank)
    def _apply(self, rust_image):
        return Image(rust_image.rank_filter(self.size, self.rank))


class Kernel:
    def __init__(self, size=(3, 3), kernel=None, scale=None, offset=0):
        self.size = size
        self.kernel = kernel
        self.scale = scale
        self.offset = offset
    def _apply(self, rust_image):
        k, scale, offset, size_x = _core.kernel_prepare(
            self.kernel, self.scale, self.offset, self.size
        )
        return Image(rust_image.kernel_filter(k, scale, offset, size_x))


class Color3DLUT:
    """Three-dimensional color lookup table.

    Transforms 3-channel pixels using the values of the channels as coordinates
    in the 3D lookup table and interpolating the nearest elements.

    This method allows you to apply almost any color transformation
    in constant time by using pre-calculated decimated tables.

    .. versionadded:: 5.2.0

    :param size: Size of the table. One int or tuple of (int, int, int).
                 Minimal size in any dimension is 2, maximum is 65.
    :param table: Flat lookup table. A list of ``channels * size**3``
                  float elements or a list of ``size**3`` channels-sized
                  tuples with floats. Channels are changed first,
                  then first dimension, then second, then third.
                  Value 0.0 corresponds lowest value of output, 1.0 highest.
    :param channels: Number of channels in the table. Could be 3 or 4.
                     Default is 3.
    :param target_mode: A mode for the result image. Should have not less
                        than ``channels`` channels. Default is ``None``,
                        which means that mode wouldn't be changed.
    """

    name = "Color 3D LUT"

    def __init__(self, size, table=None, channels=3, target_mode=None, **_kwargs):
        if channels not in (3, 4):
            raise ValueError("Only 3 or 4 output channels are supported")
        self.size = _core.color3dlut_check_size(size)
        self.channels = channels
        self.mode = target_mode
        self.table = _core.color3dlut_new(table, self.size, channels)

    @classmethod
    def generate(cls, size, callback, channels=3, target_mode=None):
        """Generates new LUT using provided callback.

        :param size: Size of the table. Passed to the constructor.
        :param callback: Function with three parameters which correspond
                         three color channels. Will be called ``size**3``
                         times with values from 0.0 to 1.0 and should return
                         a tuple with ``channels`` elements.
        :param channels: The number of channels which should return callback.
        :param target_mode: Passed to the constructor of the resulting
                            lookup table.
        """
        validated_size = _core.color3dlut_check_size(size)
        if channels not in (3, 4):
            raise ValueError("Only 3 or 4 output channels are supported")
        table = _core.color3dlut_generate(validated_size, channels, callback)
        return cls(
            validated_size,
            table,
            channels=channels,
            target_mode=target_mode,
        )

    def transform(self, callback, with_normals=False, channels=None, target_mode=None):
        """Transforms the table values using provided callback and returns
        a new LUT with altered values.

        :param callback: A function which takes old lookup table values
                         and returns a new set of values. The number
                         of arguments which function should take is
                         ``self.channels`` or ``3 + self.channels``
                         if ``with_normals`` flag is set.
                         Should return a tuple of ``self.channels`` or
                         ``channels`` elements if it is set.
        :param with_normals: If true, ``callback`` will be called with
                             coordinates in the color cube as the first
                             three arguments. Otherwise, ``callback``
                             will be called only with actual color values.
        :param channels: The number of channels in the resulting lookup table.
        :param target_mode: Passed to the constructor of the resulting
                            lookup table.
        """
        if channels not in (None, 3, 4):
            raise ValueError("Only 3 or 4 output channels are supported")
        ch_out = channels or self.channels
        table = _core.color3dlut_transform(
            self.table, self.size, self.channels, ch_out, with_normals, callback
        )
        return type(self)(
            self.size,
            table,
            channels=ch_out,
            target_mode=target_mode or self.mode,
        )

    def _apply(self, rust_image):
        """Apply 3D LUT to image using Rust trilinear interpolation."""
        img = Image(rust_image)
        src_mode = img.mode

        # Convert to RGB for processing if needed
        if src_mode not in ("RGB", "RGBA"):
            rgb_img = img.convert("RGB")
            has_alpha = False
        else:
            rgb_img = img
            has_alpha = src_mode == "RGBA"

        # Use Rust implementation for trilinear interpolation
        result = rgb_img._rust_image.color3dlut(
            self.size, list(map(float, self.table)), self.channels
        )

        # Determine output mode
        if self.mode:
            out_mode = self.mode
        elif self.channels == 4:
            out_mode = "RGBA"
        elif has_alpha:
            out_mode = "RGBA"
        else:
            out_mode = "RGB"

        return Image(result)

    def __repr__(self):
        r = [
            "Color3DLUT from %s" % self.table.__class__.__name__,
            "size=%dx%dx%d" % self.size,
            "channels=%d" % self.channels,
        ]
        if self.mode:
            r.append("target_mode=%s" % self.mode)
        return "<%s>" % " ".join(r)


# PIL-compatible apply filter function
def apply_filter(image: Image, filter_obj) -> Image:
    """Apply a filter to an image."""
    if isinstance(filter_obj, str):
        return image.filter(filter_obj)
    if hasattr(filter_obj, '_apply'):
        return filter_obj._apply(image._rust_image)
    return image.filter(str(filter_obj))
