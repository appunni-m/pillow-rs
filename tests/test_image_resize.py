"""PIL parity tests for Image.resize()."""
import pytest
from pillow_rs import Image, Resampling
from conftest import assert_images_equal


@pytest.mark.covers("Image.resize", mode="RGB", variant="default")
def test_resize_bilinear_parity(PIL):
    pil = PIL.Image.new("RGB", (100, 100), (255, 128, 64))
    rs = Image.new("RGB", (100, 100), (255, 128, 64))
    pil_r = pil.resize((50, 50), PIL.Image.BILINEAR)
    rs_r = rs.resize((50, 50), Resampling.BILINEAR)
    assert_images_equal(rs_r, pil_r)


@pytest.mark.covers("Image.resize", mode="RGB", variant="nearest")
def test_resize_nearest_parity(PIL):
    pil = PIL.Image.new("RGB", (80, 60), (255, 0, 0))
    rs = Image.new("RGB", (80, 60), (255, 0, 0))
    pil_r = pil.resize((40, 30), PIL.Image.NEAREST)
    rs_r = rs.resize((40, 30), Resampling.NEAREST)
    assert_images_equal(rs_r, pil_r)


@pytest.mark.covers("Image.resize", mode="L", variant="default")
def test_resize_grayscale_parity(PIL):
    pil = PIL.Image.new("L", (100, 100), 200)
    rs = Image.new("L", (100, 100), 200)
    pil_r = pil.resize((50, 50), PIL.Image.BILINEAR)
    rs_r = rs.resize((50, 50), Resampling.BILINEAR)
    assert_images_equal(rs_r, pil_r)


@pytest.mark.covers("Image.resize", mode="RGBA", variant="default")
def test_resize_rgba_parity(PIL):
    pil = PIL.Image.new("RGBA", (100, 100), (255, 0, 0, 128))
    rs = Image.new("RGBA", (100, 100), (255, 0, 0, 128))
    pil_r = pil.resize((50, 50), PIL.Image.BILINEAR)
    rs_r = rs.resize((50, 50), Resampling.BILINEAR)
    assert_images_equal(rs_r, pil_r)


@pytest.mark.covers("Image.resize")


def test_resize_same_size_parity(PIL):
    """Resizing to same dimensions returns identical image."""
    pil = PIL.Image.new("RGB", (50, 50), (100, 200, 50))
    rs = Image.new("RGB", (50, 50), (100, 200, 50))
    pil_r = pil.resize((50, 50))
    rs_r = rs.resize((50, 50))
    assert_images_equal(rs_r, pil_r)


def test_resize_upscale_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 128, 128))
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    pil_r = pil.resize((100, 100), PIL.Image.BILINEAR)
    rs_r = rs.resize((100, 100), Resampling.BILINEAR)
    assert_images_equal(rs_r, pil_r)
