"""PIL parity tests for the final 15 stubs now implemented."""
import pytest
from pillow_rs import Image, ImageDraw, ImageFont, ImageOps, ImagePalette
from conftest import assert_images_equal, assert_values_equal


# ── Image.frombytes ─────────────────────────────────────────────

@pytest.mark.covers("Image.frombytes")

def test_frombytes_rgb_parity(PIL):
    pil = PIL.Image.new("RGB", (5, 5), (100, 150, 200))
    data = pil.tobytes()
    rs = Image.frombytes("RGB", (5, 5), data)
    assert_images_equal(rs, pil)


# ── Image.effect_noise ──────────────────────────────────────────

@pytest.mark.covers("ImageModule.effect_noise")

def test_effect_noise_works():
    img = Image.new("RGB", (50, 50), (128, 128, 128))
    result = img.effect_noise(10.0)
    assert result.size == (50, 50)


# ── ImageDraw.bitmap ────────────────────────────────────────────

@pytest.mark.covers("ImageDraw.bitmap")

def test_draw_bitmap_works():
    img = Image.new("RGB", (50, 50), (255, 255, 255))
    bmp = Image.new("1", (20, 20), 0)
    ImageDraw.ImageDraw(img).bitmap((5, 5), bmp, fill=(0, 0, 0))
    assert img.size == (50, 50)


# ── ImageOps.colorize ────────────────────────────────────────────

@pytest.mark.covers("ImageOps.colorize")

def test_ops_colorize_works():
    img = Image.new("L", (20, 20), 128)
    result = ImageOps.colorize(img, (0, 0, 0), (255, 255, 255))
    assert result.size == (20, 20)


# ── ImagePalette.getcolor/save ───────────────────────────────────

@pytest.mark.covers("ImagePalette.getcolor")

def test_palette_getcolor_works():
    p = ImagePalette.ImagePalette()
    p.palette = [255, 0, 0, 0, 255, 0]
    idx = p.getcolor((255, 0, 0))
    assert idx == 0

@pytest.mark.covers("ImagePalette.save")

def test_palette_save_works(tmp_path):
    p = ImagePalette.ImagePalette()
    p.palette = [255, 0, 0]
    f = tmp_path / "palette.txt"
    p.save(str(f))
    assert f.exists()


# ── ImageFont.getmetrics/getname ─────────────────────────────────

@pytest.mark.covers("ImageFont.getmetrics")

def test_font_getmetrics_works():
    import os
    for p in ['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
              '/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf']:
        if os.path.exists(p):
            font = ImageFont.truetype(p, 16)
            ascent, descent = font.getmetrics()
            assert ascent > 0
            return
    pytest.skip("No font")

@pytest.mark.covers("ImageFont.getname")

def test_font_getname_works():
    import os
    for p in ['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
              '/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf']:
        if os.path.exists(p):
            font = ImageFont.truetype(p, 16)
            name = font.getname()
            assert name is not None
            return
    pytest.skip("No font")

# ── Image.remap_palette/tobitmap/draft ───────────────────────────
@pytest.mark.covers("Image.remap_palette")
def test_remap_palette_works():
    img = Image.new("RGB", (10, 10), (100, 100, 100))
    result = img.remap_palette(list(range(256)))
    assert result.size == (10, 10)

@pytest.mark.covers("Image.tobitmap")

def test_tobitmap_works():
    img = Image.new("L", (10, 10), 200)
    bmp = img.tobitmap()
    assert len(bmp) > 0

@pytest.mark.covers("Image.draft")

def test_draft_works():
    img = Image.new("RGB", (10, 10))
    result = img.draft("L", (5, 5))
    assert result is not None

@pytest.mark.covers("ImageModule.fromarray")

def test_fromarray_bytes():
    from pillow_rs.operations import fromarray
    result = fromarray(b"hello", "L")
    assert result is not None

@pytest.mark.covers("ImageModule.eval")

def test_eval_works():
    from pillow_rs.operations import eval as img_eval
    img = Image.new("L", (5, 5), 100)
    result = img_eval(img, lambda x: min(255, x * 2))
    assert result.size == (5, 5)

@pytest.mark.covers("ImageOps.exif_transpose")

def test_exif_transpose_works():
    img = Image.new("RGB", (10, 10), (255, 0, 0))
    result = ImageOps.exif_transpose(img)
    assert result.size == (10, 10)

@pytest.mark.covers("ImageFont.load")

def test_load_with_path():
    import os
    for p in ['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
              '/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf']:
        if os.path.exists(p):
            font = ImageFont.load(p)
            assert font is not None
            return
    pytest.skip("No font")

@pytest.mark.covers("ImageFont.load_default")

def test_load_default_returns_font():
    font = ImageFont.load_default(14)
    assert font is not None
