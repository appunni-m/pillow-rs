"""PIL parity tests for ImageChops."""
import pytest
from pillow_rs import Image, ImageChops
from conftest import assert_images_equal


def test_chops_add_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (100, 50, 0))
    im2_pil = PIL.Image.new("RGB", (20, 20), (50, 50, 50))
    im1_rs = Image.new("RGB", (20, 20), (100, 50, 0))
    im2_rs = Image.new("RGB", (20, 20), (50, 50, 50))
    assert_images_equal(
        ImageChops.add(im1_rs, im2_rs),
        PIL.ImageChops.add(im1_pil, im2_pil),
    )


def test_chops_subtract_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (200, 150, 100))
    im2_pil = PIL.Image.new("RGB", (20, 20), (50, 50, 50))
    im1_rs = Image.new("RGB", (20, 20), (200, 150, 100))
    im2_rs = Image.new("RGB", (20, 20), (50, 50, 50))
    assert_images_equal(
        ImageChops.subtract(im1_rs, im2_rs),
        PIL.ImageChops.subtract(im1_pil, im2_pil),
    )


def test_chops_multiply_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (255, 128, 64))
    im2_pil = PIL.Image.new("RGB", (20, 20), (200, 200, 200))
    im1_rs = Image.new("RGB", (20, 20), (255, 128, 64))
    im2_rs = Image.new("RGB", (20, 20), (200, 200, 200))
    assert_images_equal(
        ImageChops.multiply(im1_rs, im2_rs),
        PIL.ImageChops.multiply(im1_pil, im2_pil),
    )


def test_chops_screen_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (100, 150, 200))
    im2_pil = PIL.Image.new("RGB", (20, 20), (50, 50, 50))
    im1_rs = Image.new("RGB", (20, 20), (100, 150, 200))
    im2_rs = Image.new("RGB", (20, 20), (50, 50, 50))
    assert_images_equal(
        ImageChops.screen(im1_rs, im2_rs),
        PIL.ImageChops.screen(im1_pil, im2_pil),
    )


def test_chops_darker_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (200, 100, 50))
    im2_pil = PIL.Image.new("RGB", (20, 20), (100, 150, 100))
    im1_rs = Image.new("RGB", (20, 20), (200, 100, 50))
    im2_rs = Image.new("RGB", (20, 20), (100, 150, 100))
    assert_images_equal(
        ImageChops.darker(im1_rs, im2_rs),
        PIL.ImageChops.darker(im1_pil, im2_pil),
    )


def test_chops_lighter_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (200, 100, 50))
    im2_pil = PIL.Image.new("RGB", (20, 20), (100, 150, 100))
    im1_rs = Image.new("RGB", (20, 20), (200, 100, 50))
    im2_rs = Image.new("RGB", (20, 20), (100, 150, 100))
    assert_images_equal(
        ImageChops.lighter(im1_rs, im2_rs),
        PIL.ImageChops.lighter(im1_pil, im2_pil),
    )


def test_chops_difference_parity(PIL):
    im1_pil = PIL.Image.new("RGB", (20, 20), (255, 128, 0))
    im2_pil = PIL.Image.new("RGB", (20, 20), (200, 100, 50))
    im1_rs = Image.new("RGB", (20, 20), (255, 128, 0))
    im2_rs = Image.new("RGB", (20, 20), (200, 100, 50))
    assert_images_equal(
        ImageChops.difference(im1_rs, im2_rs),
        PIL.ImageChops.difference(im1_pil, im2_pil),
    )


def test_chops_invert_parity(PIL):
    pil = PIL.Image.new("RGB", (20, 20), (100, 150, 200))
    rs = Image.new("RGB", (20, 20), (100, 150, 200))
    assert_images_equal(
        ImageChops.invert(rs),
        PIL.ImageChops.invert(pil),
    )
