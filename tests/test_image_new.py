"""PIL parity tests for Image.new() — creation with all modes and color types."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal, assert_values_equal


@pytest.mark.covers("Image.new", mode="RGB", variant="default")
def test_new_rgb_default(PIL):
    pil_img = PIL.Image.new("RGB", (100, 100))
    rs_img = Image.new("RGB", (100, 100))
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new", mode="RGB", variant="color_int")
def test_new_rgb_with_int(PIL):
    pil_img = PIL.Image.new("RGB", (50, 50), 128)
    rs_img = Image.new("RGB", (50, 50), 128)
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new", mode="RGB", variant="color_hex")
def test_new_rgb_hex(PIL):
    pil_img = PIL.Image.new("RGB", (20, 20), "#FF8040")
    rs_img = Image.new("RGB", (20, 20), "#FF8040")
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new", mode="RGB", variant="color_rgb_tuple")
def test_new_rgb_tuple(PIL):
    pil_img = PIL.Image.new("RGB", (30, 30), (100, 200, 50))
    rs_img = Image.new("RGB", (30, 30), (100, 200, 50))
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new", mode="RGBA", variant="default")
def test_new_rgba(PIL):
    pil_img = PIL.Image.new("RGBA", (40, 40), (255, 0, 0, 128))
    rs_img = Image.new("RGBA", (40, 40), (255, 0, 0, 128))
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new", mode="L", variant="default")
def test_new_grayscale(PIL):
    pil_img = PIL.Image.new("L", (25, 25), 200)
    rs_img = Image.new("L", (25, 25), 200)
    assert_images_equal(rs_img, pil_img)


@pytest.mark.covers("Image.new")


def test_new_properties_match(PIL):
    """Basic properties match PIL for new images."""
    pil_img = PIL.Image.new("RGB", (150, 75), (10, 20, 30))
    rs_img = Image.new("RGB", (150, 75), (10, 20, 30))
    assert_values_equal(rs_img.size, pil_img.size)
    assert_values_equal(rs_img.width, pil_img.width)
    assert_values_equal(rs_img.height, pil_img.height)
    assert_values_equal(rs_img.mode, pil_img.mode)


def test_new_copy_parity(PIL):
    """Image.copy() produces identical images."""
    pil_img = PIL.Image.new("RGB", (50, 50), (255, 128, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 128, 0))
    pil_copy = pil_img.copy()
    rs_copy = rs_img.copy()
    assert_images_equal(rs_copy, pil_copy)


@pytest.mark.covers("Image.tobytes")


def test_new_tobytes_parity(PIL):
    """tobytes() matches PIL."""
    pil_img = PIL.Image.new("RGB", (20, 20), (100, 150, 200))
    rs_img = Image.new("RGB", (20, 20), (100, 150, 200))
    assert rs_img.tobytes() == pil_img.tobytes()
