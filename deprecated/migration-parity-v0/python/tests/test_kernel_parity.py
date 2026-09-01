"""Exact Pillow 12.2 ``ImageFilter.Kernel`` parameter parity."""

import math

import pytest
from PIL import Image as PILImage
from PIL import ImageFilter as PILImageFilter

from pillow_rs import Image, ImageFilter


def _source(mode, size=(7, 7)):
    """Build a deterministic non-uniform byte image for every public mode."""
    width, height = size
    channels = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4}[mode]
    raw = bytes(
        (pixel * 37 + channel * 53 + 11) % 256
        for pixel in range(width * height)
        for channel in range(channels)
    )
    return PILImage.frombytes(mode, size, raw), Image.frombytes(mode, size, raw)


@pytest.mark.covers("ImageFilter.Kernel")
@pytest.mark.parametrize("mode", ["L", "LA", "RGB", "RGBA"])
@pytest.mark.parametrize(
    ("size", "kernel", "scale", "offset"),
    [
        ((3, 3), [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], 1.0, 1.9),
        (
            (5, 5),
            [((-1) ** index) * (index + 1) / 7 for index in range(25)],
            2.5,
            -1.9,
        ),
    ],
)
def test_kernel_fractional_parameters_match_pillow(mode, size, kernel, scale, offset):
    pil_image, rspil_image = _source(mode)
    pil_result = pil_image.filter(PILImageFilter.Kernel(size, kernel, scale, offset))
    rspil_result = rspil_image.filter(ImageFilter.Kernel(size, kernel, scale, offset))

    assert rspil_result.mode == pil_result.mode
    assert rspil_result.size == pil_result.size
    assert rspil_result.tobytes() == pil_result.tobytes()


@pytest.mark.covers("ImageFilter.Kernel")
@pytest.mark.parametrize("mode", ["L", "LA", "RGB", "RGBA"])
@pytest.mark.parametrize("scale", [0.0, -1.0, math.nan, math.inf, -math.inf])
def test_kernel_raw_scale_matches_pillow(mode, scale):
    size = (3, 3)
    kernel = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]
    pil_image, rspil_image = _source(mode)
    pil_result = pil_image.filter(PILImageFilter.Kernel(size, kernel, scale, 0.0))
    rspil_result = rspil_image.filter(ImageFilter.Kernel(size, kernel, scale, 0.0))

    assert rspil_result.mode == pil_result.mode
    assert rspil_result.size == pil_result.size
    assert rspil_result.tobytes() == pil_result.tobytes()
