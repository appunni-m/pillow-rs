"""PIL parity tests for Image.paste()."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal


@pytest.mark.covers("Image.paste", variant="image")
def test_paste_image_parity(PIL):
    pil_bg = PIL.Image.new("RGB", (200, 200), (255, 255, 255))
    pil_fg = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    pil_bg.paste(pil_fg, (10, 10))

    rs_bg = Image.new("RGB", (200, 200), (255, 255, 255))
    rs_fg = Image.new("RGB", (50, 50), (255, 0, 0))
    rs_bg.paste(rs_fg, (10, 10))

    assert_images_equal(rs_bg, pil_bg)


@pytest.mark.covers("Image.paste", variant="color")
def test_paste_color_fill_parity(PIL):
    pil_bg = PIL.Image.new("RGB", (100, 100), (255, 255, 255))
    pil_bg.paste((0, 255, 0), (20, 20, 70, 70))

    rs_bg = Image.new("RGB", (100, 100), (255, 255, 255))
    rs_bg.paste((0, 255, 0), (20, 20, 70, 70))

    assert_images_equal(rs_bg, pil_bg)


@pytest.mark.covers("Image.paste", variant="mask")
def test_paste_with_mask_parity(PIL):
    pil_bg = PIL.Image.new("RGB", (100, 100), (255, 255, 255))
    pil_fg = PIL.Image.new("RGB", (30, 30), (255, 0, 0))
    pil_mask = PIL.Image.new("L", (30, 30), 128)  # 50% transparency
    pil_bg.paste(pil_fg, (10, 10), pil_mask)

    rs_bg = Image.new("RGB", (100, 100), (255, 255, 255))
    rs_fg = Image.new("RGB", (30, 30), (255, 0, 0))
    rs_mask = Image.new("L", (30, 30), 128)
    rs_bg.paste(rs_fg, (10, 10), rs_mask)

    assert_images_equal(rs_bg, pil_bg)


@pytest.mark.covers("Image.paste", mode="RGB", target="cpu", variant="origin")
def test_paste_at_origin_parity(PIL):
    pil_bg = PIL.Image.new("RGB", (50, 50), (255, 255, 255))
    pil_fg = PIL.Image.new("RGB", (20, 20), (0, 0, 255))
    pil_bg.paste(pil_fg)

    rs_bg = Image.new("RGB", (50, 50), (255, 255, 255))
    rs_fg = Image.new("RGB", (20, 20), (0, 0, 255))
    rs_bg.paste(rs_fg)

    assert_images_equal(rs_bg, pil_bg)
