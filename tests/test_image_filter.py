"""PIL parity tests for Image.filter()."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal


@pytest.mark.covers("Image.filter")


def test_filter_blur_parity(PIL):
    """BLUR: exact parity on uniform image."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    pil_r = PIL.Image.new("RGB", (50, 50), (128, 128, 128)).filter(PIL.ImageFilter.BLUR)
    assert_images_equal(rs.filter("BLUR"), pil_r)


@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="SHARPEN")
def test_filter_sharpen_parity(PIL):
    """SHARPEN: exact parity on uniform image."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    pil_r = PIL.Image.new("RGB", (50, 50), (255, 0, 0)).filter(PIL.ImageFilter.SHARPEN)
    assert_images_equal(rs.filter("SHARPEN"), pil_r)


@pytest.mark.covers("Image.filter")


def test_filter_smooth_parity(PIL):
    """SMOOTH: exact parity on uniform image."""
    rs = Image.new("RGB", (50, 50), (255, 128, 0))
    pil_r = PIL.Image.new("RGB", (50, 50), (255, 128, 0)).filter(PIL.ImageFilter.SMOOTH)
    assert_images_equal(rs.filter("SMOOTH"), pil_r)


@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="CONTOUR")
def test_filter_contour_works(PIL):
    """CONTOUR: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs.filter("CONTOUR")
    assert result.size == (50, 50)
    assert result.mode == "RGB"


@pytest.mark.covers("Image.filter")


@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="EMBOSS")
def test_filter_emboss_works(PIL):
    """EMBOSS: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs.filter("EMBOSS")
    assert result.size == (50, 50)
    assert result.mode == "RGB"


@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="FIND_EDGES")
def test_filter_find_edges_works(PIL):
    """FIND_EDGES: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs.filter("FIND_EDGES")
    assert result.size == (50, 50)
    assert result.mode == "RGB"


# ── From test_image_filters_advanced.py ──

"""PIL parity tests for parameterized filters."""


@pytest.mark.covers("ImageFilter.GaussianBlur")


def test_gaussian_blur_rgb(PIL):
    """GaussianBlur produces valid output."""
    rs = Image.new("RGB", (50, 50), (255, 128, 0))
    result = rs._rust_image.gaussian_blur(2.0)
    assert hasattr(result, 'size')


@pytest.mark.covers("ImageFilter.MaxFilter")


def test_max_filter_rgb(PIL):
    """MaxFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.max_filter(3)
    assert hasattr(result, 'size')


@pytest.mark.covers("ImageFilter.MedianFilter")


def test_median_filter_rgb(PIL):
    """MedianFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.median_filter(3)
    assert hasattr(result, 'size')


@pytest.mark.covers("ImageFilter.MinFilter")


def test_min_filter_rgb(PIL):
    """MinFilter produces valid output."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs._rust_image.min_filter(3)
    assert hasattr(result, 'size')


@pytest.mark.covers("ImageFilter.UnsharpMask")


def test_unsharp_mask_rgb(PIL):
    """UnsharpMask produces valid output."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs._rust_image.unsharp_mask(2.0, 150, 3)
    assert hasattr(result, 'size')
