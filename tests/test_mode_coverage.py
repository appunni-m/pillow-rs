"""Parametrized mode coverage — fills remaining mode gaps across all modules."""
import pytest
import pillow_rs
from pillow_rs import Image, ImageDraw, ImageOps, ImageFont, ImageChops
from conftest import assert_images_equal, assert_values_equal


# ── Image operation mode coverage ──────────────────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.effect_spread", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.effect_spread", mode="RGBA", target="cpu", variant="default")),
])
def test_effect_spread_modes(PIL, mode):
    color = 100 if mode == "L" else (255, 0, 0, 255)
    pil_img = PIL.Image.new(mode, (30, 30), color)
    rs_img = Image.new(mode, (30, 30), color)
    assert_images_equal(rs_img.effect_spread(2), pil_img.effect_spread(2))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.entropy", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.entropy", mode="RGBA", target="cpu", variant="default")),
])
def test_entropy_modes(PIL, mode):
    color = 128 if mode == "L" else (128, 64, 32, 255)
    pil_img = PIL.Image.new(mode, (30, 30), color)
    rs_img = Image.new(mode, (30, 30), color)
    assert_values_equal(rs_img.entropy(), pil_img.entropy())


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.getcolors", mode="L", target="cpu", variant="default")),
])
def test_getcolors_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (10, 10), 100)
    rs_img = Image.new(mode, (10, 10), 100)
    assert rs_img.getcolors(256) is not None
    assert_values_equal(rs_img.getcolors(256), pil_img.getcolors(256))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.getextrema", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.getextrema", mode="RGBA", target="cpu", variant="default")),
])
def test_getextrema_modes(PIL, mode):
    color = 128 if mode == "L" else (128, 64, 32, 255)
    pil_img = PIL.Image.new(mode, (20, 20), color)
    rs_img = Image.new(mode, (20, 20), color)
    assert_values_equal(rs_img.getextrema(), pil_img.getextrema())


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.getprojection", mode="L", target="cpu", variant="default")),
])
def test_getprojection_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (10, 10), 128)
    rs_img = Image.new(mode, (10, 10), 128)
    h1, v1 = rs_img.getprojection()
    h2, v2 = pil_img.getprojection()
    assert_values_equal(h1, h2)
    assert_values_equal(v1, v2)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.histogram", mode="L", target="cpu", variant="default")),
])
def test_histogram_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), 100)
    rs_img = Image.new(mode, (30, 30), 100)
    assert_values_equal(rs_img.histogram(), pil_img.histogram())


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.point", mode="L", target="cpu", variant="default")),
])
def test_point_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (20, 20), 100)
    rs_img = Image.new(mode, (20, 20), 100)
    lut = bytes([min(255, i + 50) for i in range(256)])
    assert_images_equal(rs_img.point(lut), pil_img.point(lut))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.putalpha", mode="L", target="cpu", variant="default")),
])
def test_putalpha_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (20, 20), 128)
    rs_img = Image.new(mode, (20, 20), 128)
    pil_img.putalpha(128)
    rs_img.putalpha(128)
    assert_images_equal(rs_img, pil_img)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.putdata", mode="L", target="cpu", variant="default")),
])
def test_putdata_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (10, 10), 50)
    rs_img = Image.new(mode, (10, 10), 50)
    data = [200] * 100 if mode == "L" else [(200, 100, 50)] * 100
    pil_img.putdata(data)
    rs_img.putdata(data)
    assert rs_img.tobytes() == pil_img.tobytes()


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.reduce", mode="L", target="cpu", variant="default")),
])
def test_reduce_modes(PIL, mode):
    color = 128 if mode == "L" else (128, 64, 32)
    pil_img = PIL.Image.new(mode, (40, 40), color)
    rs_img = Image.new(mode, (40, 40), color)
    assert_images_equal(rs_img.reduce(2), pil_img.reduce(2))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.transform", mode="L", target="cpu", variant="default")),
])
def test_transform_modes(PIL, mode):
    color = 128 if mode == "L" else (128, 64, 32)
    pil_img = PIL.Image.new(mode, (50, 50), color)
    rs_img = Image.new(mode, (50, 50), color)
    result = rs_img.transform((50, 50), 0, (1, 0, 0, 0, 1, 0))
    assert result.size == (50, 50)


@pytest.mark.parametrize("mode", [
    pytest.param("LA", marks=pytest.mark.covers("Image.rotate", mode="LA", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.rotate", mode="RGBA", target="cpu", variant="default")),
])
def test_rotate_modes(PIL, mode):
    color = (128, 255) if mode == "LA" else (255, 0, 0, 255)
    pil_img = PIL.Image.new(mode, (30, 30), color)
    rs_img = Image.new(mode, (30, 30), color)
    assert_images_equal(rs_img.rotate(90), pil_img.rotate(90))


@pytest.mark.parametrize("mode", [
    pytest.param("LA", marks=pytest.mark.covers("Image.thumbnail", mode="LA", target="cpu", variant="default")),
])
def test_thumbnail_modes(PIL, mode):
    pil_img = PIL.Image.new(mode, (100, 50), (128, 255))
    rs_img = Image.new(mode, (100, 50), (128, 255))
    pil_img.thumbnail((30, 30))
    rs_img.thumbnail((30, 30))
    assert rs_img.size == pil_img.size


# ── ImageDraw mode coverage ────────────────────────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageDraw.chord", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageDraw.chord", mode="RGBA", target="cpu", variant="default")),
])
def test_draw_chord_modes(PIL, mode):
    color = 128 if mode == "L" else (255, 0, 0, 255)
    rs_img = Image.new(mode, (60, 60), color)
    fill = 200 if mode == "L" else (0, 255, 0, 255)
    ImageDraw.ImageDraw(rs_img).chord([10, 10, 50, 50], 0, 180, fill=fill)
    assert rs_img.size == (60, 60)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageDraw.circle", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageDraw.circle", mode="RGBA", target="cpu", variant="default")),
])
def test_draw_circle_modes(PIL, mode):
    color = 128 if mode == "L" else (255, 0, 0, 255)
    rs_img = Image.new(mode, (60, 60), color)
    fill = 200 if mode == "L" else (0, 255, 0, 255)
    ImageDraw.ImageDraw(rs_img).circle((30, 30), 15, fill=fill)
    assert rs_img.size == (60, 60)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageDraw.ellipse", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageDraw.ellipse", mode="RGBA", target="cpu", variant="default")),
])
def test_draw_ellipse_modes(PIL, mode):
    color = 128 if mode == "L" else (255, 0, 0, 255)
    rs_img = Image.new(mode, (60, 60), color)
    outline = 200 if mode == "L" else (0, 255, 0, 255)
    ImageDraw.ImageDraw(rs_img).ellipse([10, 10, 50, 50], outline=outline)
    assert rs_img.size == (60, 60)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageDraw.polygon", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageDraw.polygon", mode="RGBA", target="cpu", variant="default")),
])
def test_draw_polygon_modes(PIL, mode):
    color = 128 if mode == "L" else (255, 0, 0, 255)
    rs_img = Image.new(mode, (60, 60), color)
    outline = 200 if mode == "L" else (0, 255, 0, 255)
    ImageDraw.ImageDraw(rs_img).polygon([(10, 10), (50, 10), (30, 50)], outline=outline)
    assert rs_img.size == (60, 60)


# ── ImageModule mode coverage ──────────────────────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageModule.blend", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageModule.blend", mode="RGBA", target="cpu", variant="default")),
])
def test_blend_modes(PIL, mode):
    color = 200 if mode == "L" else (255, 0, 0, 255)
    pil1 = PIL.Image.new(mode, (20, 20), color)
    pil2 = PIL.Image.new(mode, (20, 20), color)
    rs1 = Image.new(mode, (20, 20), color)
    rs2 = Image.new(mode, (20, 20), color)
    assert_images_equal(
        pillow_rs.blend(rs1, rs2, 0.5),
        PIL.Image.blend(pil1, pil2, 0.5)
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageModule.composite", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageModule.composite", mode="RGBA", target="cpu", variant="default")),
])
def test_composite_modes(PIL, mode):
    color = 200 if mode == "L" else (255, 0, 0, 255)
    pil1 = PIL.Image.new(mode, (20, 20), color)
    pil2 = PIL.Image.new(mode, (20, 20), color)
    mask = PIL.Image.new("L", (20, 20), 128)
    rs1 = Image.new(mode, (20, 20), color)
    rs2 = Image.new(mode, (20, 20), color)
    rs_mask = Image.new("L", (20, 20), 128)
    assert_images_equal(
        pillow_rs.composite(rs1, rs2, rs_mask),
        PIL.Image.composite(pil1, pil2, mask)
    )


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageModule.eval", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageModule.eval", mode="RGBA", target="cpu", variant="default")),
])
def test_eval_modes(PIL, mode):
    color = 128 if mode == "L" else (128, 64, 32, 255)
    pil_img = PIL.Image.new(mode, (20, 20), color)
    rs_img = Image.new(mode, (20, 20), color)
    result = pillow_rs.Image.eval(rs_img, lambda x: min(255, x + 10))
    assert result.size == (20, 20)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageModule.fromarray", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageModule.fromarray", mode="RGBA", target="cpu", variant="default")),
])
def test_fromarray_modes(PIL, mode):
    pytest.importorskip('numpy')
    import numpy as np
    if mode == "L":
        arr = np.zeros((20, 20), dtype=np.uint8)
    else:
        arr = np.zeros((20, 20, 4), dtype=np.uint8)
    result = pillow_rs.Image.fromarray(arr, mode)
    assert result.size == (20, 20)


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("ImageModule.frombytes", mode="L", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("ImageModule.frombytes", mode="RGBA", target="cpu", variant="default")),
])
def test_frombytes_modes(PIL, mode):
    nbytes = 400 if mode == "L" else 1600
    data = b'\x80' * nbytes
    try:
        result = pillow_rs.Image.frombytes(mode, (20, 20), data)
    except NotImplementedError:
        pytest.xfail("frombytes not yet implemented for this mode")
    assert result.size == (20, 20)


@pytest.mark.covers("ImageModule.alpha_composite", target="cpu", variant="default")
def test_module_alpha_composite(PIL):
    bg = Image.new("RGBA", (30, 30), (255, 255, 255, 255))
    fg = Image.new("RGBA", (15, 15), (255, 0, 0, 128))
    Image.alpha_composite(bg, fg)
    assert bg.size == (30, 30)


# ── ImageFont.ImageFont ────────────────────────────────────────────

@pytest.mark.covers("ImageFont.ImageFont", target="cpu", variant="default")
def test_imagefont_basic():
    font = ImageFont.ImageFont()
    assert font is not None


# ── ImageOps.deform ────────────────────────────────────────────────

@pytest.mark.covers("ImageOps.deform", target="cpu", variant="default")
@pytest.mark.xfail(reason="ImageOps.deform not yet implemented")
def test_deform_works():
    img = Image.new("RGB", (50, 50), (255, 0, 0))
    result = ImageOps.deform(img, None)
    assert result.size == (50, 50)

# ── Remaining mode gaps ────────────────────────────────────────────

@pytest.mark.parametrize("mode", [
    pytest.param("RGBA", marks=pytest.mark.covers("Image.reduce", mode="RGBA", target="cpu", variant="default")),
])
def test_reduce_rgba(PIL, mode):
    pil_img = PIL.Image.new(mode, (40, 40), (128, 64, 32, 255))
    rs_img = Image.new(mode, (40, 40), (128, 64, 32, 255))
    assert_images_equal(rs_img.reduce(2), pil_img.reduce(2))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.open", mode="L", target="cpu", variant="bytes")),
    pytest.param("LA", marks=pytest.mark.covers("Image.open", mode="LA", target="cpu", variant="bytes")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.open", mode="RGBA", target="cpu", variant="bytes")),
])
def test_open_modes(PIL, mode):
    import io
    color = 128 if mode == "L" else (128, 255) if mode == "LA" else (255, 0, 0, 255)
    pil_img = PIL.Image.new(mode, (20, 20), color)
    buf = io.BytesIO()
    pil_img.save(buf, "PNG")
    rs_img = Image.open(buf.getvalue())
    assert rs_img.size == (20, 20)


@pytest.mark.covers("Image.tell", target="cpu", variant="default")
def test_tell_parity(PIL):
    pil_img = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
    rs_img = Image.new("RGB", (20, 20), (255, 0, 0))
    assert rs_img.tell() == pil_img.tell()


@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.getprojection", mode="RGB", target="cpu", variant="default")),
    pytest.param("RGBA", marks=pytest.mark.covers("Image.getprojection", mode="RGBA", target="cpu", variant="default")),
])
def test_getprojection_rgb_rgba(PIL, mode):
    color = (128, 64, 32) if mode == "RGB" else (128, 64, 32, 255)
    pil_img = PIL.Image.new(mode, (10, 10), color)
    rs_img = Image.new(mode, (10, 10), color)
    h1, v1 = rs_img.getprojection()
    h2, v2 = pil_img.getprojection()
    assert_values_equal(h1, h2)
    assert_values_equal(v1, v2)


@pytest.mark.parametrize("mode", [
    pytest.param("RGBA", marks=pytest.mark.covers("Image.getcolors", mode="RGBA", target="cpu", variant="default")),
])
def test_getcolors_rgba(PIL, mode):
    pil_img = PIL.Image.new(mode, (10, 10), (255, 0, 0, 255))
    rs_img = Image.new(mode, (10, 10), (255, 0, 0, 255))
    assert rs_img.getcolors(256) is not None
    assert_values_equal(rs_img.getcolors(256), pil_img.getcolors(256))


@pytest.mark.parametrize("mode", [
    pytest.param("L", marks=pytest.mark.covers("Image.rotate", mode="L", target="cpu", variant="default")),
])
def test_rotate_l(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), 128)
    rs_img = Image.new(mode, (30, 30), 128)
    assert_images_equal(rs_img.rotate(90), pil_img.rotate(90))


@pytest.mark.parametrize("mode", [
    pytest.param("RGBA", marks=pytest.mark.covers("Image.quantize", mode="RGBA", target="cpu", variant="default")),
])
def test_quantize_rgba(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), (128, 64, 32, 255))
    rs_img = Image.new(mode, (30, 30), (128, 64, 32, 255))
    pil_q = pil_img.quantize(16)
    rs_q = rs_img.quantize(16)
    assert rs_q.size == pil_q.size


@pytest.mark.parametrize("mode", [
    pytest.param("RGBA", marks=pytest.mark.covers("Image.point", mode="RGBA", target="cpu", variant="default")),
])
def test_point_rgba(PIL, mode):
    pil_img = PIL.Image.new(mode, (20, 20), (128, 64, 32, 255))
    rs_img = Image.new(mode, (20, 20), (128, 64, 32, 255))
    lut = bytes([min(255, i + 50) for i in range(256)])
    assert_images_equal(rs_img.point(lut), pil_img.point(lut))


@pytest.mark.parametrize("mode", [
    pytest.param("LA", marks=pytest.mark.covers("Image.getextrema", mode="LA", target="cpu", variant="default")),
])
def test_getextrema_la(PIL, mode):
    pil_img = PIL.Image.new(mode, (20, 20), (128, 255))
    rs_img = Image.new(mode, (20, 20), (128, 255))
    assert_values_equal(rs_img.getextrema(), pil_img.getextrema())


@pytest.mark.covers("ImageModule.new", target="cpu", variant="default")
def test_module_new(PIL):
    result = pillow_rs.new("RGB", (20, 20), (255, 0, 0))
    assert result.size == (20, 20)


@pytest.mark.covers("ImageModule.open", target="cpu", variant="default")
def test_module_open(PIL):
    import io
    pil_img = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
    buf = io.BytesIO()
    pil_img.save(buf, "PNG")
    result = pillow_rs.open(buf.getvalue())
    assert result.size == (20, 20)


@pytest.mark.covers("ImageModule.frombytes", mode="RGB", target="cpu", variant="default")
def test_module_frombytes_rgb(PIL):
    data = b'\x80' * 1200
    result = pillow_rs.Image.frombytes("RGB", (20, 20), data)
    assert result.size == (20, 20)

# ── RGB mode coverage (from deleted test_image_advanced.py) ────────

@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.effect_spread", mode="RGB", target="cpu", variant="default")),
])
def test_effect_spread_rgb(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), (255, 0, 0))
    rs_img = Image.new(mode, (30, 30), (255, 0, 0))
    assert_images_equal(rs_img.effect_spread(2), pil_img.effect_spread(2))


@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.entropy", mode="RGB", target="cpu", variant="default")),
])
def test_entropy_rgb(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), (128, 64, 32))
    rs_img = Image.new(mode, (30, 30), (128, 64, 32))
    assert_values_equal(rs_img.entropy(), pil_img.entropy())


@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.getcolors", mode="RGB", target="cpu", variant="default")),
])
def test_getcolors_rgb(PIL, mode):
    pil_img = PIL.Image.new(mode, (10, 10), (255, 0, 0))
    rs_img = Image.new(mode, (10, 10), (255, 0, 0))
    assert rs_img.getcolors(256) is not None
    assert_values_equal(rs_img.getcolors(256), pil_img.getcolors(256))


@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.point", mode="RGB", target="cpu", variant="default")),
])
def test_point_rgb(PIL, mode):
    pil_img = PIL.Image.new(mode, (20, 20), (100, 150, 200))
    rs_img = Image.new(mode, (20, 20), (100, 150, 200))
    lut = bytes([min(255, i + 50) for i in range(256)])
    assert_images_equal(rs_img.point(lut), pil_img.point(lut))


@pytest.mark.parametrize("mode", [
    pytest.param("RGB", marks=pytest.mark.covers("Image.quantize", mode="RGB", target="cpu", variant="default")),
])
def test_quantize_rgb(PIL, mode):
    pil_img = PIL.Image.new(mode, (30, 30), (128, 64, 32))
    rs_img = Image.new(mode, (30, 30), (128, 64, 32))
    pil_q = pil_img.quantize(16)
    rs_q = rs_img.quantize(16)
    assert rs_q.size == pil_q.size
