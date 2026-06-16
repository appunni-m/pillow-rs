"""ImageFilter — convolution kernels and filter classes. Pillow-compatible module."""
from ._core import Image as RustImage
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
        size_x, size_y = self.size
        if size_x != size_y or size_x not in (3, 5):
            raise NotImplementedError(f"Kernel size {self.size} not supported, only 3x3 and 5x5")
        k = [float(v) for v in (self.kernel or [1] * (size_x * size_y))]
        scale = float(self.scale) if self.scale is not None else sum(k)
        from .image import Image
        from ._core import Image as RustImage
        return Image(rust_image.kernel_filter(k, scale, int(self.offset), size_x))


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

    def __init__(self, size, table=None, channels=3, target_mode=None, **kwargs):
        if channels not in (3, 4):
            raise ValueError("Only 3 or 4 output channels are supported")
        self.size = size = self._check_size(size)
        self.channels = channels
        self.mode = target_mode

        # Hidden flag ``_copy_table=False`` could be used to avoid extra copying
        # of the table if the table is specially made for the constructor.
        copy_table = kwargs.get("_copy_table", True)
        items = size[0] * size[1] * size[2]
        wrong_size = False

        if copy_table:
            table = list(table)

        # Convert a list of tuples into a flat list
        if table and isinstance(table[0], (list, tuple)):
            flat_table = []
            for pixel in table:
                if len(pixel) != channels:
                    raise ValueError(
                        "The elements of the table should "
                        "have a length of %d." % channels
                    )
                flat_table.extend(pixel)
            table = flat_table

        if wrong_size or len(table) != items * channels:
            raise ValueError(
                "The table should have either channels * size**3 float items "
                "or size**3 items of channels-sized tuples with floats. "
                "Table should be: %dx%dx%dx%d. "
                "Actual length: %d" % (channels, size[0], size[1], size[2], len(table))
            )
        self.table = table

    @staticmethod
    def _check_size(size):
        """Validate and normalize LUT size. Converts int to 3-tuple."""
        try:
            _, _, _ = size
        except (TypeError, ValueError):
            size = (size, size, size)
        size = tuple(int(x) for x in size)
        for size_1d in size:
            if not 2 <= size_1d <= 65:
                raise ValueError("Size should be in [2, 65] range.")
        return size

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
        size_1d, size_2d, size_3d = cls._check_size(size)
        if channels not in (3, 4):
            raise ValueError("Only 3 or 4 output channels are supported")

        table = [0.0] * (size_1d * size_2d * size_3d * channels)
        idx_out = 0
        for b in range(size_3d):
            for g in range(size_2d):
                for r in range(size_1d):
                    table[idx_out:idx_out + channels] = callback(
                        r / (size_1d - 1), g / (size_2d - 1), b / (size_3d - 1)
                    )
                    idx_out += channels

        return cls(
            (size_1d, size_2d, size_3d),
            table,
            channels=channels,
            target_mode=target_mode,
            _copy_table=False,
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
        ch_in = self.channels
        ch_out = channels or ch_in
        size_1d, size_2d, size_3d = self.size

        table = [0.0] * (size_1d * size_2d * size_3d * ch_out)
        idx_in = 0
        idx_out = 0
        for b in range(size_3d):
            for g in range(size_2d):
                for r in range(size_1d):
                    values = self.table[idx_in:idx_in + ch_in]
                    if with_normals:
                        values = callback(
                            r / (size_1d - 1),
                            g / (size_2d - 1),
                            b / (size_3d - 1),
                            *values,
                        )
                    else:
                        values = callback(*values)
                    table[idx_out:idx_out + ch_out] = values
                    idx_out += ch_out
                    idx_in += ch_in

        return type(self)(
            self.size,
            table,
            channels=ch_out,
            target_mode=target_mode or self.mode,
            _copy_table=False,
        )

    def _apply(self, rust_image):
        """Apply 3D LUT to image using Rust trilinear interpolation."""
        from .image import Image

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
