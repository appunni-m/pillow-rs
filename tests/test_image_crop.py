"""PIL parity tests for Image.crop()."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal


@pytest.mark.covers("Image.crop", mode="RGB")
def test_crop_parity(PIL):
    pil = PIL.Image.new("RGB", (100, 100), (255, 128, 64))
    rs = Image.new("RGB", (100, 100), (255, 128, 64))
    pil_c = pil.crop((10, 10, 60, 60))
    rs_c = rs.crop((10, 10, 60, 60))
    assert_images_equal(rs_c, pil_c)


@pytest.mark.covers("Image.crop", mode="RGB", target="cpu", variant="full_image")
def test_crop_full_image_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    pil_c = pil.crop((0, 0, 50, 50))
    rs_c = rs.crop((0, 0, 50, 50))
    assert_images_equal(rs_c, pil_c)


@pytest.mark.covers("Image.crop")


def test_crop_small_region_parity(PIL):
    pil = PIL.Image.new("RGB", (100, 100), (100, 200, 50))
    rs = Image.new("RGB", (100, 100), (100, 200, 50))
    pil_c = pil.crop((50, 50, 60, 60))
    rs_c = rs.crop((50, 50, 60, 60))
    assert_images_equal(rs_c, pil_c)


@pytest.mark.covers("Image.crop", mode="L", target="cpu", variant="default")
def test_crop_grayscale_parity(PIL):
    pil = PIL.Image.new("L", (80, 80), 200)
    rs = Image.new("L", (80, 80), 200)
    pil_c = pil.crop((20, 20, 60, 60))
    rs_c = rs.crop((20, 20, 60, 60))
    assert_images_equal(rs_c, pil_c)


@pytest.mark.covers("Image.crop")


@pytest.mark.covers("Image.crop", mode="RGBA", target="cpu", variant="default")
def test_crop_rgba_parity(PIL):
    pil = PIL.Image.new("RGBA", (80, 80), (255, 0, 0, 128))
    rs = Image.new("RGBA", (80, 80), (255, 0, 0, 128))
    pil_c = pil.crop((10, 10, 70, 70))
    rs_c = rs.crop((10, 10, 70, 70))
    assert_images_equal(rs_c, pil_c)
