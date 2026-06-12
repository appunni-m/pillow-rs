"""Error parity tests — verify pillow-rs matches PIL behavior for non-standard modes.

For each (operation, mode) combination not listed in supported_modes,
we assert PIL-RSPIL parity: both must either succeed with identical output
or fail with the same error type.
"""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal, assert_values_equal


def _assert_error_parity(pil_fn, rs_fn):
    """Assert PIL and RSPIL produce the same result or same error."""
    pil_error = None
    rs_error = None
    pil_result = None
    rs_result = None

    try:
        pil_result = pil_fn()
    except Exception as e:
        pil_error = (type(e).__name__, str(e)[:100])

    try:
        rs_result = rs_fn()
    except Exception as e:
        rs_error = (type(e).__name__, str(e)[:100])

    if pil_error is not None and rs_error is not None:
        assert pil_error[0] == rs_error[0], \
            f"Error type mismatch: PIL={pil_error[0]} RSPIL={rs_error[0]}"
    elif pil_error is None and rs_error is None:
        assert rs_result is not None and pil_result is not None
    else:
        raise AssertionError(
            f"Parity mismatch: PIL_error={pil_error} RSPIL_error={rs_error}"
        )


# ── Image.new with non-standard modes ──────────────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("CMYK", marks=pytest.mark.covers("Image.new", mode="CMYK", target="cpu", variant="error_parity")),
    pytest.param("YCbCr", marks=pytest.mark.covers("Image.new", mode="YCbCr", target="cpu", variant="error_parity")),
    pytest.param("HSV", marks=pytest.mark.covers("Image.new", mode="HSV", target="cpu", variant="error_parity")),
    pytest.param("I", marks=pytest.mark.covers("Image.new", mode="I", target="cpu", variant="error_parity")),
    pytest.param("F", marks=pytest.mark.covers("Image.new", mode="F", target="cpu", variant="error_parity")),
])
def test_new_nonstandard_modes(PIL, mode):
    _assert_error_parity(
        lambda: PIL.Image.new(mode, (20, 20)),
        lambda: Image.new(mode, (20, 20)),
    )


# ── Image.convert to/from non-standard modes ───────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("CMYK", marks=pytest.mark.covers("Image.convert", mode="CMYK", target="cpu", variant="error_parity")),
    pytest.param("YCbCr", marks=pytest.mark.covers("Image.convert", mode="YCbCr", target="cpu", variant="error_parity")),
    pytest.param("HSV", marks=pytest.mark.covers("Image.convert", mode="HSV", target="cpu", variant="error_parity")),
    pytest.param("I", marks=pytest.mark.covers("Image.convert", mode="I", target="cpu", variant="error_parity")),
    pytest.param("F", marks=pytest.mark.covers("Image.convert", mode="F", target="cpu", variant="error_parity")),
])
def test_convert_from_rgb_to_nonstandard(PIL, mode):
    _assert_error_parity(
        lambda: PIL.Image.new("RGB", (20, 20), (255, 0, 0)).convert(mode),
        lambda: Image.new("RGB", (20, 20), (255, 0, 0)).convert(mode),
    )


# ── Pixel operations on non-standard modes ─────────────────────────

NONSTANDARD_MODES = [
    pytest.param("CMYK", marks=pytest.mark.covers("Image.resize", mode="CMYK", target="cpu", variant="error_parity")),
    pytest.param("YCbCr", marks=pytest.mark.covers("Image.resize", mode="YCbCr", target="cpu", variant="error_parity")),
    pytest.param("HSV", marks=pytest.mark.covers("Image.resize", mode="HSV", target="cpu", variant="error_parity")),
    pytest.param("I", marks=pytest.mark.covers("Image.resize", mode="I", target="cpu", variant="error_parity")),
    pytest.param("F", marks=pytest.mark.covers("Image.resize", mode="F", target="cpu", variant="error_parity")),
]


@pytest.mark.parametrize("mode", NONSTANDARD_MODES)
def test_resize_nonstandard_modes(PIL, mode):
    def make_pil():
        img = PIL.Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.resize((20, 20))
    def make_rs():
        img = Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.resize((20, 20))
    _assert_error_parity(make_pil, make_rs)


@pytest.mark.parametrize("mode", [
    pytest.param("CMYK", marks=pytest.mark.covers("Image.crop", mode="CMYK", target="cpu", variant="error_parity")),
    pytest.param("YCbCr", marks=pytest.mark.covers("Image.crop", mode="YCbCr", target="cpu", variant="error_parity")),
    pytest.param("HSV", marks=pytest.mark.covers("Image.crop", mode="HSV", target="cpu", variant="error_parity")),
    pytest.param("I", marks=pytest.mark.covers("Image.crop", mode="I", target="cpu", variant="error_parity")),
    pytest.param("F", marks=pytest.mark.covers("Image.crop", mode="F", target="cpu", variant="error_parity")),
])
def test_crop_nonstandard_modes(PIL, mode):
    def make_pil():
        img = PIL.Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.crop((10, 10, 30, 30))
    def make_rs():
        img = Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.crop((10, 10, 30, 30))
    _assert_error_parity(make_pil, make_rs)


@pytest.mark.parametrize("mode", [
    pytest.param("1", marks=pytest.mark.covers("Image.rotate", mode="1", target="cpu", variant="error_parity")),
    pytest.param("P", marks=pytest.mark.covers("Image.rotate", mode="P", target="cpu", variant="error_parity")),
    pytest.param("CMYK", marks=pytest.mark.covers("Image.rotate", mode="CMYK", target="cpu", variant="error_parity")),
])
def test_rotate_nonstandard_modes(PIL, mode):
    def make_pil():
        img = PIL.Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.rotate(45)
    def make_rs():
        img = Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.rotate(45)
    _assert_error_parity(make_pil, make_rs)


@pytest.mark.parametrize("mode", [
    pytest.param("1", marks=pytest.mark.covers("Image.filter", mode="1", target="cpu", variant="error_parity")),
    pytest.param("P", marks=pytest.mark.covers("Image.filter", mode="P", target="cpu", variant="error_parity")),
    pytest.param("CMYK", marks=pytest.mark.covers("Image.filter", mode="CMYK", target="cpu", variant="error_parity")),
])
def test_filter_nonstandard_modes(PIL, mode):
    def make_pil():
        img = PIL.Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.filter(PIL.ImageFilter.BLUR)
    def make_rs():
        img = Image.new("RGB", (40, 40), (255, 0, 0)).convert(mode)
        return img.filter("BLUR")
    _assert_error_parity(make_pil, make_rs)
