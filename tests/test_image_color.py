"""PIL parity tests for ImageColor."""
import pytest
from pillow_rs import ImageColor
from conftest import assert_values_equal


@pytest.mark.covers("ImageColor.getrgb")


def test_getrgb_hex_parity(PIL):
    assert_values_equal(ImageColor.getrgb("#FF0000"), PIL.ImageColor.getrgb("#FF0000"))
    assert_values_equal(ImageColor.getrgb("#00FF00"), PIL.ImageColor.getrgb("#00FF00"))
    assert_values_equal(ImageColor.getrgb("#0000FF"), PIL.ImageColor.getrgb("#0000FF"))


@pytest.mark.covers("ImageColor.getrgb", target="cpu", variant="named")
def test_getrgb_named_parity(PIL):
    assert_values_equal(ImageColor.getrgb("red"), PIL.ImageColor.getrgb("red"))
    assert_values_equal(ImageColor.getrgb("green"), PIL.ImageColor.getrgb("green"))
    assert_values_equal(ImageColor.getrgb("blue"), PIL.ImageColor.getrgb("blue"))
    assert_values_equal(ImageColor.getrgb("white"), PIL.ImageColor.getrgb("white"))
    assert_values_equal(ImageColor.getrgb("black"), PIL.ImageColor.getrgb("black"))


@pytest.mark.covers("ImageColor.getcolor")


def test_getcolor_rgb_parity(PIL):
    assert_values_equal(
        ImageColor.getcolor("red", "RGB"),
        PIL.ImageColor.getcolor("red", "RGB"),
    )


@pytest.mark.covers("ImageColor.getcolor", mode="L", target="cpu", variant="default")
def test_getcolor_l_parity(PIL):
    assert_values_equal(
        ImageColor.getcolor("red", "L"),
        PIL.ImageColor.getcolor("red", "L"),
    )
