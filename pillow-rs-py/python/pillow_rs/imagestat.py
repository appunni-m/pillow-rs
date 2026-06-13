"""ImageStat — statistical analysis of images. Pillow-compatible module."""
from .image import Image


class Stat:
    """Calculate image statistics. Thin wrapper over Rust core stat()."""

    def __init__(self, image_or_list, mask=None):
        if isinstance(image_or_list, Image):
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
        else:
            # List-based stat (fallback)
            data = list(image_or_list)
            self.count = len(data)
            self.sum = sum(data) if data else 0
            n = self.count or 1
            self.mean = self.sum / n
            self.extrema = (min(data), max(data)) if data else (0, 0)
