"""ImageFilter — convolution kernels and filter classes. Pillow-compatible module."""
from abc import ABC, abstractmethod
from functools import reduce

from ._core import Image as RustImage
from . import _core
from .image import Image


class Filter(ABC):
    @abstractmethod
    def filter(self, rust_image: RustImage) -> RustImage:
        """Transform a Rust-owned image handle and return the filtered handle."""


class MultibandFilter(Filter):
    """Marker for filters that process all image bands in one call."""


# Built-ins use Rust's named kernels. Custom subclasses can supply Pillow's
# ``filterargs`` tuple and use the native generic convolution entry point.
class BuiltinFilter(MultibandFilter):
    kernel_name = ""
    _kernel_name = ""
    _kernel_filterargs = None

    def filter(self, rust_image):
        if rust_image.mode == "P":
            raise ValueError("cannot filter palette images")
        # Preserve the mode-specific Rust fast path unless callers override
        # Pillow's public filterargs or kernel name on this filter class.
        if (
            self.kernel_name == self._kernel_name
            and self.filterargs is self._kernel_filterargs
            and self.kernel_name
        ):
            return rust_image.filter_name(self.kernel_name)
        return rust_image.filter(*self.filterargs)


class BLUR(BuiltinFilter):
    name = "Blur"
    kernel_name = "BLUR"
    _kernel_name = kernel_name
    filterargs = ((5, 5), 16, 0, (
        1, 1, 1, 1, 1,
        1, 0, 0, 0, 1,
        1, 0, 0, 0, 1,
        1, 0, 0, 0, 1,
        1, 1, 1, 1, 1,
    ))
    _kernel_filterargs = filterargs


class CONTOUR(BuiltinFilter):
    name = "Contour"
    kernel_name = "CONTOUR"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 1, 255, (-1, -1, -1, -1, 8, -1, -1, -1, -1))
    _kernel_filterargs = filterargs


class DETAIL(BuiltinFilter):
    name = "Detail"
    kernel_name = "DETAIL"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 6, 0, (0, -1, 0, -1, 10, -1, 0, -1, 0))
    _kernel_filterargs = filterargs


class EDGE_ENHANCE(BuiltinFilter):
    name = "Edge-enhance"
    kernel_name = "EDGE_ENHANCE"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 2, 0, (-1, -1, -1, -1, 10, -1, -1, -1, -1))
    _kernel_filterargs = filterargs


class EDGE_ENHANCE_MORE(BuiltinFilter):
    name = "Edge-enhance More"
    kernel_name = "EDGE_ENHANCE_MORE"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 1, 0, (-1, -1, -1, -1, 9, -1, -1, -1, -1))
    _kernel_filterargs = filterargs


class EMBOSS(BuiltinFilter):
    name = "Emboss"
    kernel_name = "EMBOSS"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 1, 128, (-1, 0, 0, 0, 1, 0, 0, 0, 0))
    _kernel_filterargs = filterargs


class FIND_EDGES(BuiltinFilter):
    name = "Find Edges"
    kernel_name = "FIND_EDGES"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 1, 0, (-1, -1, -1, -1, 8, -1, -1, -1, -1))
    _kernel_filterargs = filterargs


class SHARPEN(BuiltinFilter):
    name = "Sharpen"
    kernel_name = "SHARPEN"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 16, 0, (-2, -2, -2, -2, 32, -2, -2, -2, -2))
    _kernel_filterargs = filterargs


class SMOOTH(BuiltinFilter):
    name = "Smooth"
    kernel_name = "SMOOTH"
    _kernel_name = kernel_name
    filterargs = ((3, 3), 13, 0, (1, 1, 1, 1, 5, 1, 1, 1, 1))
    _kernel_filterargs = filterargs


class SMOOTH_MORE(BuiltinFilter):
    name = "Smooth More"
    kernel_name = "SMOOTH_MORE"
    _kernel_name = kernel_name
    filterargs = ((5, 5), 100, 0, (
        1, 1, 1, 1, 1,
        1, 5, 5, 5, 1,
        1, 5, 44, 5, 1,
        1, 5, 5, 5, 1,
        1, 1, 1, 1, 1,
    ))
    _kernel_filterargs = filterargs


class GaussianBlur(MultibandFilter):
    name = "GaussianBlur"

    def __init__(self, radius=2):
        self.radius = radius
    def filter(self, rust_image):
        return rust_image.gaussian_blur(self.radius)


class BoxBlur(MultibandFilter):
    name = "BoxBlur"

    def __init__(self, radius):
        xy = radius if isinstance(radius, (tuple, list)) else (radius, radius)
        if xy[0] < 0 or xy[1] < 0:
            raise ValueError("radius must be >= 0")
        self.radius = radius
    def filter(self, rust_image):
        return rust_image.box_blur(self.radius)


class UnsharpMask(MultibandFilter):
    name = "UnsharpMask"

    def __init__(self, radius=2, percent=150, threshold=3):
        self.radius = radius
        self.percent = percent
        self.threshold = threshold
    def filter(self, rust_image):
        return rust_image.unsharp_mask(self.radius, self.percent, self.threshold)


class MaxFilter(Filter):
    name = "Max"

    def __init__(self, size=3):
        self.size = size
    def filter(self, rust_image):
        return rust_image.max_filter(self.size)


class MinFilter(Filter):
    name = "Min"

    def __init__(self, size=3):
        self.size = size
    def filter(self, rust_image):
        return rust_image.min_filter(self.size)


class MedianFilter(Filter):
    name = "Median"

    def __init__(self, size=3):
        self.size = size
    def filter(self, rust_image):
        return rust_image.median_filter(self.size)


class ModeFilter(Filter):
    name = "Mode"

    def __init__(self, size=3):
        self.size = size
    def filter(self, rust_image):
        return rust_image.modefilter(self.size)


class RankFilter(Filter):
    name = "Rank"

    def __init__(self, size, rank):
        self.size = size
        self.rank = rank
    def filter(self, rust_image):
        return rust_image.rank_filter(self.size, self.rank)


class Kernel(BuiltinFilter):
    name = "Kernel"

    def __init__(self, size, kernel, scale=None, offset=0):
        # Pillow computes the default scale before validating coefficient count.
        if scale is None:
            scale = reduce(lambda a, b: a + b, kernel)
        if size[0] * size[1] != len(kernel):
            raise ValueError("not enough coefficients in kernel")
        self.size = size
        self.kernel = kernel
        self.scale = scale
        self.offset = offset
        self.filterargs = size, scale, offset, kernel


class Color3DLUT(MultibandFilter):
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
        table, ch_out = _core.color3dlut_transform(
            self.table, self.size, self.channels, channels, with_normals, callback
        )
        result_mode = self.mode if target_mode is None else target_mode

        # The core has already converted callback results to floats. For the
        # built-in class, a correctly sized result also satisfies the table
        # constructor's remaining checks. Keep that returned list directly
        # instead of copying it through color3dlut_new a second time. Preserve
        # subclass construction and malformed callback-length errors below.
        if type(self) is Color3DLUT and len(table) == (
            self.size[0] * self.size[1] * self.size[2] * ch_out
        ):
            result = object.__new__(Color3DLUT)
            result.size = self.size
            result.channels = ch_out
            result.mode = result_mode
            result.table = table
            return result

        return type(self)(
            self.size,
            table,
            channels=ch_out,
            target_mode=result_mode,
        )

    def filter(self, rust_image):
        """Apply 3D LUT to image using Rust trilinear interpolation."""
        return rust_image.color_lut_3d(
            self.mode or rust_image.mode,
            Image.Resampling.BILINEAR,
            self.channels,
            self.size,
            self.table,
        )

    def __repr__(self):
        return _core.color3dlut_repr(
            self.table.__class__.__name__, self.size, self.channels, self.mode
        )


# PIL-compatible apply filter function
def apply_filter(image: Image, filter_obj) -> Image:
    """Apply a filter to an image."""
    return image.filter(filter_obj)
