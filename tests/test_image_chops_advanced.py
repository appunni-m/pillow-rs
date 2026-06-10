"""PIL parity tests for remaining ImageChops functions."""
import pytest
from pillow_rs import Image, ImageChops
from conftest import assert_images_equal


def test_add_modulo_works():
    im1 = Image.new("RGB", (10, 10), (250, 200, 100))
    im2 = Image.new("RGB", (10, 10), (20, 10, 5))
    result = ImageChops.add_modulo(im1, im2)
    assert result.size == (10, 10)


def test_subtract_modulo_works():
    im1 = Image.new("RGB", (10, 10), (10, 20, 30))
    im2 = Image.new("RGB", (10, 10), (5, 10, 15))
    result = ImageChops.subtract_modulo(im1, im2)
    assert result.size == (10, 10)


def test_constant_works():
    img = Image.new("RGB", (10, 10), (255, 0, 0))
    result = ImageChops.constant(img, 128)
    assert result.size == (10, 10)


def test_blend_parity(PIL):
    pil1 = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
    pil2 = PIL.Image.new("RGB", (20, 20), (0, 0, 255))
    rs1 = Image.new("RGB", (20, 20), (255, 0, 0))
    rs2 = Image.new("RGB", (20, 20), (0, 0, 255))
    rs_result = ImageChops.blend(rs1, rs2, 0.5)
    assert rs_result.size == (20, 20)


def test_composite_works():
    im1 = Image.new("RGB", (20, 20), (255, 0, 0))
    im2 = Image.new("RGB", (20, 20), (0, 255, 0))
    mask = Image.new("L", (20, 20), 128)
    result = ImageChops.composite(im1, im2, mask)
    assert result.size == (20, 20)


def test_duplicate_parity(PIL):
    pil = PIL.Image.new("RGB", (15, 15), (100, 150, 200))
    rs = Image.new("RGB", (15, 15), (100, 150, 200))
    assert_images_equal(ImageChops.duplicate(rs), PIL.ImageChops.duplicate(pil))
