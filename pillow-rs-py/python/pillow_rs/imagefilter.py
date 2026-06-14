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
    def __init__(self, size, table=None, channels=3, **kwargs):
        self.size = size
        self.table = table
        self.channels = channels
    def _apply(self, rust_image):
        raise NotImplementedError("Color3DLUT")


# PIL-compatible apply filter function
def apply_filter(image: Image, filter_obj) -> Image:
    """Apply a filter to an image."""
    if isinstance(filter_obj, str):
        return image.filter(filter_obj)
    if hasattr(filter_obj, '_apply'):
        return filter_obj._apply(image._rust_image)
    return image.filter(str(filter_obj))
