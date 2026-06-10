"""PIL parity tests for getchannel, putalpha, reduce, load, enhance."""
import pytest
from pillow_rs import Image, ImageEnhance
from conftest import assert_images_equal, assert_values_equal


def test_getchannel_r_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10), (255, 128, 0))
    rs = Image.new("RGB", (10, 10), (255, 128, 0))
    assert_images_equal(rs.getchannel("R"), pil.getchannel("R"))


def test_getchannel_g_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10), (100, 200, 50))
    rs = Image.new("RGB", (10, 10), (100, 200, 50))
    assert_images_equal(rs.getchannel("G"), pil.getchannel("G"))


def test_putalpha_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10), (255, 0, 0))
    rs = Image.new("RGB", (10, 10), (255, 0, 0))
    pil.putalpha(128)
    rs.putalpha(128)
    assert_images_equal(rs, pil)


def test_reduce_parity(PIL):
    pil = PIL.Image.new("RGB", (100, 100), (128, 128, 128))
    rs = Image.new("RGB", (100, 100), (128, 128, 128))
    assert_images_equal(rs.reduce(2), pil.reduce(2))


def test_enhance_brightness_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 64, 32))
    rs = Image.new("RGB", (50, 50), (128, 64, 32))
    pil_out = PIL.ImageEnhance.Brightness(pil).enhance(1.5)
    rs_out = ImageEnhance.Brightness(rs).enhance(1.5)
    assert rs_out.size == (50, 50)


def test_enhance_color_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (200, 100, 50))
    rs = Image.new("RGB", (50, 50), (200, 100, 50))
    pil_out = PIL.ImageEnhance.Color(pil).enhance(0.5)
    rs_out = ImageEnhance.Color(rs).enhance(0.5)
    assert rs_out.size == (50, 50)


def test_enhance_contrast_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (128, 128, 128))
    rs = Image.new("RGB", (50, 50), (128, 128, 128))
    pil_out = PIL.ImageEnhance.Contrast(pil).enhance(1.5)
    rs_out = ImageEnhance.Contrast(rs).enhance(1.5)
    assert rs_out.size == (50, 50)


def test_enhance_sharpness_parity(PIL):
    pil = PIL.Image.new("RGB", (50, 50), (255, 128, 0))
    rs = Image.new("RGB", (50, 50), (255, 128, 0))
    pil_out = PIL.ImageEnhance.Sharpness(pil).enhance(2.0)
    rs_out = ImageEnhance.Sharpness(rs).enhance(2.0)
    assert rs_out.size == (50, 50)
