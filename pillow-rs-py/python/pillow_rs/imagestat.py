"""ImageStat — statistical analysis of images. Pillow-compatible module."""
from .image import Image


class Stat:
    """Calculate image statistics. Thin wrapper over Rust core stat()."""

    def __init__(self, image_or_list, mask=None):
        if isinstance(image_or_list, Image):
            bands = image_or_list._rust_image.stat()
            n = len(bands)
            self.count = [int(b[0]) for b in bands]
            self.sum = [b[1] for b in bands]
            self.sum2 = [b[2] for b in bands]
            self.mean = [b[3] for b in bands]
            self.median = [b[4] for b in bands]
            self.rms = [b[5] for b in bands]
            self.var = [b[6] for b in bands]
            self.stddev = [b[7] for b in bands]
            self.extrema = [(b[8], b[9]) for b in bands]
            if n == 1:
                self.count = self.count[0]
                self.sum = self.sum[0]
                self.mean = self.mean[0]
                self.median = self.median[0]
                self.rms = self.rms[0]
                self.var = self.var[0]
                self.stddev = self.stddev[0]
                self.extrema = self.extrema[0]
        else:
            # List-based stat (fallback)
            data = list(image_or_list)
            self.count = len(data)
            self.sum = sum(data) if data else 0
            n = self.count or 1
            self.mean = self.sum / n
            self.extrema = (min(data), max(data)) if data else (0, 0)
