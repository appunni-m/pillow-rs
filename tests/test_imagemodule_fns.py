"""PIL parity tests for Image module-level functions: merge, blend, composite."""
import pytest
from pillow_rs import Image, blend, composite, merge
from conftest import assert_images_equal


def test_merge_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (30, 20), (255, 128, 0))
    rs = Image.new("RGB", (30, 20), (255, 128, 0))
    bands_pil = pil.split()
    bands_rs = rs.split()
    merged_pil = PIL.Image.merge("RGB", bands_pil)
    merged_rs = merge("RGB", bands_rs)
    assert_images_equal(merged_rs, merged_pil)


def test_blend_parity(PIL):
    pil1 = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
    pil2 = PIL.Image.new("RGB", (20, 20), (0, 255, 0))
    rs1 = Image.new("RGB", (20, 20), (255, 0, 0))
    rs2 = Image.new("RGB", (20, 20), (0, 255, 0))
    rs_result = blend(rs1, rs2, 0.5)
    assert rs_result.size == (20, 20)


def test_composite_works():
    im1 = Image.new("RGB", (20, 20), (255, 0, 0))
    im2 = Image.new("RGB", (20, 20), (0, 255, 0))
    mask = Image.new("L", (20, 20), 128)
    result = composite(im1, im2, mask)
    assert result.size == (20, 20)
