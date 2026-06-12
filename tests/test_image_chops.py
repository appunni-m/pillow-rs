"""PIL parity tests for ImageChops."""
import pytest
from pillow_rs import Image, ImageChops
from conftest import assert_images_equal


def _make_color(mode, base=128):
    """Create a mode-appropriate color from a base value."""
    if mode == "L":
        return base
    elif mode == "RGB":
        return (base, base, base)
    elif mode == "RGBA":
        return (base, base, base, 255)
    return base


def _dual_color(mode, base1=128, base2=64):
    """Create two mode-appropriate colors."""
    return _make_color(mode, base1), _make_color(mode, base2)


# ── Parametrized mode coverage for dual-image ops ──────────────────

CHOps_MODES = [
    pytest.param("L", marks=pytest.mark.covers("ImageChops.add", mode="L", target="cpu", variant="default")),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.add", mode="RGB", target="cpu", variant="default")),
]


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.add", mode="L", target="cpu", variant="default"),
                              pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.add", mode="RGB", target="cpu", variant="default")),
])
def test_chops_add_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 100, 50)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.add(im1_rs, im2_rs),
        PIL.ImageChops.add(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.subtract", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.subtract", mode="RGB", target="cpu", variant="default")),
])
def test_chops_subtract_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 200, 50)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.subtract(im1_rs, im2_rs),
        PIL.ImageChops.subtract(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.multiply", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.multiply", mode="RGB", target="cpu", variant="default")),
])
def test_chops_multiply_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 255, 200)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.multiply(im1_rs, im2_rs),
        PIL.ImageChops.multiply(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.screen", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.screen", mode="RGB", target="cpu", variant="default")),
])
def test_chops_screen_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 100, 50)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.screen(im1_rs, im2_rs),
        PIL.ImageChops.screen(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.darker", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.darker", mode="RGB", target="cpu", variant="default")),
])
def test_chops_darker_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 200, 100)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.darker(im1_rs, im2_rs),
        PIL.ImageChops.darker(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.lighter", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.lighter", mode="RGB", target="cpu", variant="default")),
])
def test_chops_lighter_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 200, 100)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.lighter(im1_rs, im2_rs),
        PIL.ImageChops.lighter(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.difference", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.difference", mode="RGB", target="cpu", variant="default")),
])
def test_chops_difference_parity(PIL, mode):
    c1, c2 = _dual_color(mode, 255, 200)
    im1_pil = PIL.Image.new(mode, (20, 20), c1)
    im2_pil = PIL.Image.new(mode, (20, 20), c2)
    im1_rs = Image.new(mode, (20, 20), c1)
    im2_rs = Image.new(mode, (20, 20), c2)
    assert_images_equal(
        ImageChops.difference(im1_rs, im2_rs),
        PIL.ImageChops.difference(im1_pil, im2_pil),
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=[pytest.mark.covers("ImageChops.invert", mode="L", target="cpu", variant="default"), pytest.mark.xfail(reason="L-mode not preserved in ImageChops implementation")]),
    pytest.param("RGB", marks=pytest.mark.covers("ImageChops.invert", mode="RGB", target="cpu", variant="default")),
])
def test_chops_invert_parity(PIL, mode):
    color = _make_color(mode, 100)
    pil = PIL.Image.new(mode, (20, 20), color)
    rs = Image.new(mode, (20, 20), color)
    assert_images_equal(
        ImageChops.invert(rs),
        PIL.ImageChops.invert(pil),
    )
