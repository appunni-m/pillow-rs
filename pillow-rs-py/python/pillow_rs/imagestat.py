"""ImageStat — statistical analysis of images. Pillow-compatible module."""
from . import _core
from .image import Image, _execution_context_scope


class Stat:
    """Calculate image statistics. Thin wrapper over Rust core stat()."""

    # TODO(pillow-rs#3ptb): verify custom histogram callback results, faults,
    # and ImageEnhance.Contrast construction order in the frozen CPU matrix.
    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def __init__(self, image_or_list, mask=None):
        if isinstance(image_or_list, Image):
            with _execution_context_scope(image_or_list):
                histogram = image_or_list.histogram(mask)
            result = _core.stat_from_histogram(histogram)
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
            result = _core.stat_from_histogram(list(image_or_list))
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
            # PIL accepts only an Image or a list (not a tuple).
            raise TypeError("first argument must be image or list")
