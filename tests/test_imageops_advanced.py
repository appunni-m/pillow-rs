"""PIL parity tests for remaining ImageOps functions."""
import pytest
from pillow_rs import Image, ImageOps
from conftest import assert_images_equal


@pytest.mark.covers("ImageOps.autocontrast")


def test_autocontrast_works():
    img = Image.new("RGB", (50, 50), (128, 128, 128))
    result = ImageOps.autocontrast(img, cutoff=2)
    assert result.size == (50, 50)


@pytest.mark.covers("ImageOps.contain")


def test_contain_parity(PIL):
    pil = PIL.Image.new("RGB", (200, 100), (255, 0, 0))
    rs = Image.new("RGB", (200, 100), (255, 0, 0))
    assert_images_equal(ImageOps.contain(rs, (50, 50)), PIL.ImageOps.contain(pil, (50, 50)))


@pytest.mark.covers("ImageOps.cover", mode="RGB", target="cpu", variant="default")
def test_cover_parity(PIL):
    import PIL.ImageOps
    pil = PIL.Image.new("RGB", (200, 100), (128, 0, 0))
    rs = Image.new("RGB", (200, 100), (128, 0, 0))
    assert_images_equal(ImageOps.cover(rs, (80, 80)), PIL.ImageOps.cover(pil, (80, 80)))


@pytest.mark.covers("ImageOps.expand")


def test_expand_parity(PIL):
    pil = PIL.Image.new("RGB", (30, 30), (255, 0, 0))
    rs = Image.new("RGB", (30, 30), (255, 0, 0))
    assert_images_equal(ImageOps.expand(rs, 5, (0, 255, 0)), PIL.ImageOps.expand(pil, 5, (0, 255, 0)))


@pytest.mark.covers("ImageOps.scale", mode="RGB", target="cpu", variant="default")
def test_scale_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 128, 128))
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    assert_images_equal(ImageOps.scale(rs, 0.5), PIL.ImageOps.scale(pil, 0.5))
