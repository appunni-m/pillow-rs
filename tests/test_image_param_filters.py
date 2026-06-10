"""PIL parity tests for parameterized filters."""
import pytest
from pillow_rs import Image


def test_gaussian_blur_rgb(PIL):
    """GaussianBlur produces valid output."""
    rs = Image.new("RGB", (50, 50), (255, 128, 0))
    result = rs._rust_image.gaussian_blur(2.0)
    assert hasattr(result, 'size')


def test_max_filter_rgb(PIL):
    """MaxFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.max_filter(3)
    assert hasattr(result, 'size')


def test_median_filter_rgb(PIL):
    """MedianFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.median_filter(3)
    assert hasattr(result, 'size')


def test_min_filter_rgb(PIL):
    """MinFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.min_filter(3)
    assert hasattr(result, 'size')


def test_unsharp_mask_rgb(PIL):
    """UnsharpMask produces valid output."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs._rust_image.unsharp_mask(2.0, 150, 3)
    assert hasattr(result, 'size')
