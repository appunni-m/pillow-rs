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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.add", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.subtract", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.multiply", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.screen", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.darker", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.lighter", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.difference", mode="L", target="cpu", variant="default")),
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
    pytest.param("L", marks=pytest.mark.covers("ImageChops.invert", mode="L", target="cpu", variant="default")),
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


# ── From test_image_chops_advanced.py ──

"""PIL parity tests for remaining ImageChops functions."""


@pytest.mark.covers("ImageChops.add_modulo")


def test_add_modulo_works():
    im1 = Image.new("RGB", (10, 10), (250, 200, 100))
    im2 = Image.new("RGB", (10, 10), (20, 10, 5))
    result = ImageChops.add_modulo(im1, im2)
    assert result.size == (10, 10)


@pytest.mark.covers("ImageChops.subtract_modulo", mode="RGB", target="cpu", variant="default")
def test_subtract_modulo_works():
    im1 = Image.new("RGB", (10, 10), (10, 20, 30))
    im2 = Image.new("RGB", (10, 10), (5, 10, 15))
    result = ImageChops.subtract_modulo(im1, im2)
    assert result.size == (10, 10)


@pytest.mark.covers("ImageChops.constant")


def test_constant_works():
    img = Image.new("RGB", (10, 10), (255, 0, 0))
    result = ImageChops.constant(img, 128)
    assert result.size == (10, 10)


@pytest.mark.covers("ImageChops.blend", mode="RGB", target="cpu", variant="default")
def test_blend_parity(PIL):
    pil1 = PIL.Image.new("RGB", (10, 10), (255, 0, 0))
    pil2 = PIL.Image.new("RGB", (10, 10), (0, 0, 255))
    rs1 = Image.new("RGB", (10, 10), (255, 0, 0))
    rs2 = Image.new("RGB", (10, 10), (0, 0, 255))
    assert_images_equal(ImageChops.blend(rs1, rs2, 0.5), PIL.ImageChops.blend(pil1, pil2, 0.5))


@pytest.mark.covers("ImageChops.composite", mode="RGB", target="cpu", variant="default")
def test_composite_works():
    im1 = Image.new("RGB", (20, 20), (255, 0, 0))
    im2 = Image.new("RGB", (20, 20), (0, 255, 0))
    mask = Image.new("L", (20, 20), 128)
    result = ImageChops.composite(im1, im2, mask)
    assert result.size == (20, 20)


@pytest.mark.covers("ImageChops.duplicate")


def test_duplicate_parity(PIL):
    pil = PIL.Image.new("RGB", (15, 15), (100, 150, 200))
    rs = Image.new("RGB", (15, 15), (100, 150, 200))
    assert_images_equal(ImageChops.duplicate(rs), PIL.ImageChops.duplicate(pil))
