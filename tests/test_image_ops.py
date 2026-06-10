"""PIL parity tests for ImageOps."""
import pytest
from pillow_rs import Image, ImageOps
from conftest import assert_images_equal


@pytest.mark.covers("ImageOps.invert")


def test_ops_invert_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 64, 32))
    rs = Image.new("RGB", (50, 50), (128, 64, 32))
    assert_images_equal(ImageOps.invert(rs), PIL.ImageOps.invert(pil))


def test_ops_flip_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    rs = Image.new("RGB", (50, 50), (255, 0, 0))
    assert_images_equal(ImageOps.flip(rs), PIL.ImageOps.flip(pil))


@pytest.mark.covers("ImageOps.mirror")


def test_ops_mirror_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (0, 255, 0))
    rs = Image.new("RGB", (50, 50), (0, 255, 0))
    assert_images_equal(ImageOps.mirror(rs), PIL.ImageOps.mirror(pil))


def test_ops_grayscale_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (255, 128, 0))
    rs = Image.new("RGB", (50, 50), (255, 128, 0))
    assert_images_equal(ImageOps.grayscale(rs), PIL.ImageOps.grayscale(pil))


@pytest.mark.covers("ImageOps.posterize")


def test_ops_posterize_parity(PIL):
    pil = PIL.Image.new("RGB", (30, 30), (200, 150, 100))
    rs = Image.new("RGB", (30, 30), (200, 150, 100))
    assert_images_equal(ImageOps.posterize(rs, 4), PIL.ImageOps.posterize(pil, 4))


def test_ops_solarize_parity(PIL):
    pil = PIL.Image.new("RGB", (30, 30), (200, 200, 200))
    rs = Image.new("RGB", (30, 30), (200, 200, 200))
    assert_images_equal(ImageOps.solarize(rs, 128), PIL.ImageOps.solarize(pil, 128))


@pytest.mark.covers("ImageOps.equalize")


def test_ops_equalize_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 128, 128))
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    assert_images_equal(ImageOps.equalize(rs), PIL.ImageOps.equalize(pil))
