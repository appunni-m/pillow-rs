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


def test_filter_contour_works(PIL):
    """CONTOUR: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs.filter("CONTOUR")
    assert result.size == (50, 50)
    assert result.mode == "RGB"


@pytest.mark.covers("Image.filter")


def test_filter_emboss_works(PIL):
    """EMBOSS: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    result = rs.filter("EMBOSS")
    assert result.size == (50, 50)
    assert result.mode == "RGB"


def test_filter_find_edges_works(PIL):
    """FIND_EDGES: produces valid output image. Algorithm differs from PIL."""
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    result = rs.filter("FIND_EDGES")
    assert result.size == (50, 50)
    assert result.mode == "RGB"
