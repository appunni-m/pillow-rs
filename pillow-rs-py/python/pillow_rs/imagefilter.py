"""ImageFilter — convolution kernels and filter classes. Pillow-compatible module."""
from ._core import Image as RustImage
from . import _core
from .image import Image


# Built-in kernel filters. Pillow exposes each one as a callable public class,
# not as a string selector. The target keeps the same class shape while the
# Rust core receives the stable kernel name at application time.
class _BuiltinFilter:
    kernel_name = ""

    def _apply(self, rust_image):
        return rust_image.filter_name(self.kernel_name)


class BLUR(_BuiltinFilter):
    kernel_name = "BLUR"


class CONTOUR(_BuiltinFilter):
    kernel_name = "CONTOUR"


class DETAIL(_BuiltinFilter):
    kernel_name = "DETAIL"


class EDGE_ENHANCE(_BuiltinFilter):
    kernel_name = "EDGE_ENHANCE"


class EDGE_ENHANCE_MORE(_BuiltinFilter):
    kernel_name = "EDGE_ENHANCE_MORE"


class EMBOSS(_BuiltinFilter):
    kernel_name = "EMBOSS"


class FIND_EDGES(_BuiltinFilter):
    kernel_name = "FIND_EDGES"


class SHARPEN(_BuiltinFilter):
    kernel_name = "SHARPEN"


class SMOOTH(_BuiltinFilter):
    kernel_name = "SMOOTH"


class SMOOTH_MORE(_BuiltinFilter):
    kernel_name = "SMOOTH_MORE"


class GaussianBlur:
    def __init__(self, radius=2):
        self.radius = radius
    def _apply(self, rust_image):
        return rust_image.gaussian_blur(self.radius)


class BoxBlur:
    def __init__(self, radius=2):
        xy = radius if isinstance(radius, (tuple, list)) else (radius, radius)
        if xy[0] < 0 or xy[1] < 0:
            raise ValueError("radius must be >= 0")
        self.radius = radius
    def _apply(self, rust_image):
        return rust_image.box_blur(self.radius)


class UnsharpMask:
    def __init__(self, radius=2, percent=150, threshold=3):
        self.radius = radius
        self.percent = percent
        self.threshold = threshold
    def _apply(self, rust_image):
        return rust_image.unsharp_mask(self.radius, self.percent, self.threshold)


class MaxFilter:
    def __init__(self, size=3):
        self.size = size
    def _apply(self, rust_image):
        return rust_image.max_filter(self.size)


class MinFilter:
    def __init__(self, size=3):
        self.size = size
    def _apply(self, rust_image):
        return rust_image.min_filter(self.size)


class MedianFilter:
    def __init__(self, size=3):
        self.size = size
    def _apply(self, rust_image):
        return rust_image.median_filter(self.size)


class ModeFilter:
    name = "Mode"

    def __init__(self, size=3):
        self.size = size
    def _apply(self, rust_image):
        return rust_image.mode_filter(self.size)


class RankFilter:
    def __init__(self, size=3, rank=0):
        self.size = size
        self.rank = rank
    def _apply(self, rust_image):
        return rust_image.rank_filter(self.size, self.rank)


class Kernel:
    def __init__(self, size=(3, 3), kernel=None, scale=None, offset=0):
        self.size = size
        self.kernel = kernel
        _core.kernel_validate_coefficients(self.kernel, self.size)
        self.scale = scale
        self.offset = offset

    def _apply(self, rust_image):
        return rust_image.kernel_filter(self.kernel, self.scale, self.offset, self.size)


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
        return type(self)(
            self.size,
            table,
            channels=ch_out,
            target_mode=self.mode if target_mode is None else target_mode,
        )

    def _apply(self, rust_image):
        """Apply 3D LUT to image using Rust trilinear interpolation."""
        return rust_image.color3dlut(self.size, self.table, self.channels, self.mode)

    def __repr__(self):
        return _core.color3dlut_repr(
            self.table.__class__.__name__, self.size, self.channels, self.mode
        )


# PIL-compatible apply filter function
def apply_filter(image: Image, filter_obj) -> Image:
    """Apply a filter to an image."""
    return image.filter(filter_obj)
