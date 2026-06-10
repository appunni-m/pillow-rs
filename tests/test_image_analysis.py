"""PIL parity tests for getpixel, putpixel, getbbox, getextrema, histogram."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal, assert_values_equal


@pytest.mark.covers("Image.getpixel")


def test_getpixel_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (3, 3), (255, 128, 0))
    rs = Image.new("RGB", (3, 3), (255, 128, 0))
    assert_values_equal(rs.getpixel((1, 1)), pil.getpixel((1, 1)))


def test_getpixel_rgba_parity(PIL):
    pil = PIL.Image.new("RGBA", (3, 3), (100, 200, 50, 128))
    rs = Image.new("RGBA", (3, 3), (100, 200, 50, 128))
    assert_values_equal(rs.getpixel((0, 0)), pil.getpixel((0, 0)))


@pytest.mark.covers("Image.getpixel")


def test_getpixel_grayscale_parity(PIL):
    pil = PIL.Image.new("L", (5, 5), 128)
    rs = Image.new("L", (5, 5), 128)
    assert_values_equal(rs.getpixel((2, 2)), pil.getpixel((2, 2)))


@pytest.mark.covers("Image.putpixel")


def test_putpixel_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (3, 3), (0, 0, 0))
    rs = Image.new("RGB", (3, 3), (0, 0, 0))
    pil.putpixel((1, 1), (255, 0, 0))
    rs.putpixel((1, 1), (255, 0, 0))
    assert_images_equal(rs, pil)


@pytest.mark.covers("Image.getbbox")


def test_getbbox_parity(PIL):
    # Create image with non-zero region
    pil = PIL.Image.new("RGB", (100, 100), (0, 0, 0))
    rs = Image.new("RGB", (100, 100), (0, 0, 0))
    # Draw a red square
    for y in range(20, 60):
        for x in range(30, 70):
            pil.putpixel((x, y), (255, 0, 0))
            rs.putpixel((x, y), (255, 0, 0))
    assert_values_equal(rs.getbbox(), pil.getbbox())


def test_getextrema_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10), (128, 64, 32))
    rs = Image.new("RGB", (10, 10), (128, 64, 32))
    assert_values_equal(rs.getextrema(), pil.getextrema())


@pytest.mark.covers("Image.histogram")


def test_histogram_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (10, 10), (100, 150, 200))
    rs = Image.new("RGB", (10, 10), (100, 150, 200))
    assert_values_equal(rs.histogram(), pil.histogram())
