"""ImageStat — statistical analysis of images. Pillow-compatible module."""
from . import _core
from .image import Image


class Stat:
    """Calculate image statistics. Thin wrapper over Rust core stat()."""

    def __init__(self, image_or_list, mask=None):
        if isinstance(image_or_list, Image):
            if mask is not None and (mask.mode not in ("1", "L") or mask.size != image_or_list.size):
                raise ValueError("bad transparency mask")
            result = image_or_list._rust_image.stat_formatted()
            self.count = result['count']
            self.sum = result['sum']
            self.sum2 = result['sum2']
            self.mean = result['mean']
            self.median = result['median']
            self.rms = result['rms']
            self.var = result['var']
            self.stddev = result['stddev']
            self.extrema = result['extrema']
        elif isinstance(image_or_list, list):
            result = _core.stat_from_list(list(image_or_list))
            self.count = result['count']
            self.sum = result['sum']
            self.mean = result['mean']
            self.extrema = (result['min'], result['max'])
        else:
            # PIL accepts only an Image or an exact list (not a tuple).
            raise TypeError("first argument must be image or list")
