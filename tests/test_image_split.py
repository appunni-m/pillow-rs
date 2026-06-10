"""PIL parity tests for Image.split() and getbands()."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal, assert_values_equal


@pytest.mark.covers("Image.split")


def test_split_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (30, 20), (255, 128, 64))
    rs = Image.new("RGB", (30, 20), (255, 128, 64))
    for band_rs, band_pil in zip(rs.split(), pil.split()):
        assert_images_equal(band_rs, band_pil)


def test_split_rgba_parity(PIL):
    pil = PIL.Image.new("RGBA", (25, 15), (255, 0, 0, 128))
    rs = Image.new("RGBA", (25, 15), (255, 0, 0, 128))
    for band_rs, band_pil in zip(rs.split(), pil.split()):
        assert_images_equal(band_rs, band_pil)


@pytest.mark.covers("Image.split")


def test_split_grayscale_parity(PIL):
    pil = PIL.Image.new("L", (30, 20), 128)
    rs = Image.new("L", (30, 20), 128)
    for band_rs, band_pil in zip(rs.split(), pil.split()):
        assert_images_equal(band_rs, band_pil)


@pytest.mark.covers("Image.getbands")


def test_getbands_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10))
    rs = Image.new("RGB", (10, 10))
    assert_values_equal(rs.getbands(), pil.getbands())


def test_getbands_rgba_parity(PIL):
    pil = PIL.Image.new("RGBA", (10, 10))
    rs = Image.new("RGBA", (10, 10))
    assert_values_equal(rs.getbands(), pil.getbands())


@pytest.mark.covers("Image.getbands")


def test_getbands_l_parity(PIL):
    pil = PIL.Image.new("L", (10, 10))
    rs = Image.new("L", (10, 10))
    assert_values_equal(rs.getbands(), pil.getbands())
