"""ImageStat — statistical analysis of images. Pillow-compatible stub."""
import math
from .image import Image


class Stat:
    """Calculate image statistics."""

    def __init__(self, image_or_list, mask=None):
        if isinstance(image_or_list, Image):
            data = list(image_or_list.getdata())
        else:
            data = list(image_or_list)
        self.extrema = (min(data), max(data)) if data else (0, 0)
        self.count = len(data)
        self.sum = sum(data) if data else 0
        self.sum2 = sum(x * x for x in data) if data else 0
        n = self.count or 1
        self.mean = self.sum / n
        self.median = sorted(data)[n // 2] if data else 0
        variance = (self.sum2 / n) - (self.mean ** 2)
        self.rms = math.sqrt(self.sum2 / n)
        self.var = variance
        self.stddev = math.sqrt(max(variance, 0))
