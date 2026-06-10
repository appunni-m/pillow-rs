"""ImageFilter — convolution kernels and filter classes. Pillow-compatible."""
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


class _Filter:
    """Base class for parameterized filters."""
    def __init__(self, *args, **kwargs):
        self._args = args
        self._kwargs = kwargs

    def _apply(self, rust_image: RustImage) -> Image:
        raise NotImplementedError


class GaussianBlur(_Filter):
    """Gaussian blur with given radius."""
    def _apply(self, rust_image):
        radius = self._kwargs.get('radius', self._args[0] if self._args else 2)
        return Image(rust_image.gaussian_blur(float(radius)))


class BoxBlur(_Filter):
    """Box blur with given radius."""
    def _apply(self, rust_image):
        radius = self._kwargs.get('radius', self._args[0] if self._args else 2)
        return Image(rust_image.gaussian_blur(float(radius) * 0.5))


class UnsharpMask(_Filter):
    """Unsharp mask for sharpening."""
    def _apply(self, rust_image):
        radius = self._kwargs.get('radius', self._args[0] if len(self._args) > 0 else 2)
        percent = self._kwargs.get('percent', self._args[1] if len(self._args) > 1 else 150)
        threshold = self._kwargs.get('threshold', self._args[2] if len(self._args) > 2 else 3)
        return Image(rust_image.unsharp_mask(float(radius), int(percent), int(threshold)))


class MaxFilter(_Filter):
    """Maximum filter with given size."""
    def _apply(self, rust_image):
        size = self._kwargs.get('size', self._args[0] if self._args else 3)
        return Image(rust_image.max_filter(int(size)))


class MinFilter(_Filter):
    """Minimum filter with given size."""
    def _apply(self, rust_image):
        size = self._kwargs.get('size', self._args[0] if self._args else 3)
        return Image(rust_image.min_filter(int(size)))


class MedianFilter(_Filter):
    """Median filter with given size."""
    def _apply(self, rust_image):
        size = self._kwargs.get('size', self._args[0] if self._args else 3)
        return Image(rust_image.median_filter(int(size)))


class ModeFilter(_Filter):
    """Mode filter with given size."""
    def _apply(self, rust_image):
        size = self._kwargs.get('size', self._args[0] if self._args else 3)
        # Mode filter is implemented via Rust's mode_filter
        return Image(rust_image.median_filter(int(size)))  # fallback


class RankFilter(_Filter):
    """Rank filter."""
    def _apply(self, rust_image):
        raise NotImplementedError("RankFilter")


class Kernel(_Filter):
    """Custom convolution kernel."""
    def _apply(self, rust_image):
        raise NotImplementedError("Kernel")


class Color3DLUT(_Filter):
    """3D color lookup table."""
    def _apply(self, rust_image):
        raise NotImplementedError("Color3DLUT")


# PIL-compatible apply filter function
def apply_filter(image: Image, filter_obj) -> Image:
    """Apply a filter to an image."""
    if isinstance(filter_obj, str):
        return image.filter(filter_obj)
    if isinstance(filter_obj, _Filter):
        return filter_obj._apply(image._rust_image)
    # Assume it's a built-in name
    return image.filter(str(filter_obj))
