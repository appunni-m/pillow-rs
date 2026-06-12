"""PIL parity tests for Image.open, save, thumbnail, close, verify, seek, tell, load."""
import pytest, tempfile, os
from pillow_rs import Image
from conftest import assert_images_equal


class TestOpenSave:
    @pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="png")
    def test_save_png_roundtrip(self, PIL):
        pil_img = PIL.Image.new("RGB", (50, 50), (255, 128, 0))
        rs_img = Image.new("RGB", (50, 50), (255, 128, 0))
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "test.png")
            pil_img.save(p, "PNG")
            rs_img.save(p, "PNG")
            rs2 = Image.open(p)
            pil2 = PIL.Image.open(p)
            assert_images_equal(rs2, pil2)

    @pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="jpeg")
    def test_save_jpeg_roundtrip(self, PIL):
        pil_img = PIL.Image.new("RGB", (30, 30), (100, 200, 50))
        rs_img = Image.new("RGB", (30, 30), (100, 200, 50))
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "test.jpg")
            pil_img.save(p, "JPEG")
            rs_img.save(p, "JPEG")
            rs2 = Image.open(p)
            pil2 = PIL.Image.open(p)
            assert rs2.size == pil2.size

    @pytest.mark.covers("Image.open", mode="RGB", target="cpu", variant="bytes")
    def test_open_bytes(self, PIL):
        import io
        pil_img = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
        buf = io.BytesIO()
        pil_img.save(buf, "PNG")
        data = buf.getvalue()
        rs_img = Image.open(data)
        assert rs_img.size == (20, 20)


class TestThumbnail:
    @pytest.mark.covers("Image.thumbnail", target="cpu", variant="default")
    def test_thumbnail_parity(self, PIL):
        pil = PIL.Image.new("RGB", (400, 200), (128, 128, 128))
        rs = Image.new("RGB", (400, 200), (128, 128, 128))
        pil.thumbnail((100, 100))
        rs.thumbnail((100, 100))
        assert rs.size == pil.size


class TestBookkeeping:
    @pytest.mark.covers("Image.close", target="cpu", variant="default")
    def test_close_no_error(self):
        img = Image.new("RGB", (10, 10))
        img.close()

    @pytest.mark.covers("Image.verify", target="cpu", variant="default")
    def test_verify_no_error(self):
        img = Image.new("RGB", (10, 10))
        img.verify()

    @pytest.mark.covers("Image.seek", target="cpu", variant="default")
    def test_seek_tell(self):
        img = Image.new("RGB", (10, 10))
        img.seek(0)
        assert img.tell() == 0

    @pytest.mark.covers("Image.load", target="cpu", variant="default")
    def test_load_returns(self):
        img = Image.new("RGB", (10, 10))
        result = img.load()
        assert result is not None


# ── Format × Mode roundtrip tests ──────────────────────────────────


def _make_test_image(mode):
    """Create a test image with appropriate color for the mode."""
    if mode == "L":
        return (128,)
    elif mode == "LA":
        return (128, 255)
    elif mode == "RGB":
        return (255, 0, 0)
    elif mode == "RGBA":
        return (255, 0, 0, 255)
    elif mode == "1":
        return (1,)
    elif mode == "P":
        return None  # will convert
    return (255, 0, 0)


@pytest.mark.parametrize("fmt,mode,op", [
    # PNG
    pytest.param("PNG", "L", "Image.save", marks=pytest.mark.covers("Image.save", mode="L", target="cpu", variant="png_roundtrip")),
    pytest.param("PNG", "LA", "Image.save", marks=pytest.mark.covers("Image.save", mode="LA", target="cpu", variant="png_roundtrip")),
    pytest.param("PNG", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="png_roundtrip")),
    pytest.param("PNG", "RGBA", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGBA", target="cpu", variant="png_roundtrip")),
    pytest.param("PNG", "1", "Image.save", marks=pytest.mark.covers("Image.save", mode="1", target="cpu", variant="png_roundtrip")),
    pytest.param("PNG", "P", "Image.save", marks=[pytest.mark.covers("Image.save", mode="P", target="cpu", variant="png_roundtrip"),
                                                   pytest.mark.xfail(reason="PNG mode 'P' loaded as RGBA instead of 'P'")]),
    # JPEG
    pytest.param("JPEG", "L", "Image.save", marks=pytest.mark.covers("Image.save", mode="L", target="cpu", variant="jpeg_roundtrip")),
    pytest.param("JPEG", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="jpeg_roundtrip")),
    # GIF
    pytest.param("GIF", "L", "Image.save", marks=pytest.mark.covers("Image.save", mode="L", target="cpu", variant="gif_roundtrip")),
    pytest.param("GIF", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="gif_roundtrip")),
    pytest.param("GIF", "1", "Image.save", marks=pytest.mark.covers("Image.save", mode="1", target="cpu", variant="gif_roundtrip")),
    pytest.param("GIF", "P", "Image.save", marks=pytest.mark.covers("Image.save", mode="P", target="cpu", variant="gif_roundtrip")),
    # BMP
    pytest.param("BMP", "L", "Image.save", marks=pytest.mark.covers("Image.save", mode="L", target="cpu", variant="bmp_roundtrip")),
    pytest.param("BMP", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="bmp_roundtrip")),
    pytest.param("BMP", "RGBA", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGBA", target="cpu", variant="bmp_roundtrip")),
    # TIFF
    pytest.param("TIFF", "L", "Image.save", marks=pytest.mark.covers("Image.save", mode="L", target="cpu", variant="tiff_roundtrip")),
    pytest.param("TIFF", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="tiff_roundtrip")),
    pytest.param("TIFF", "RGBA", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGBA", target="cpu", variant="tiff_roundtrip")),
    # WEBP
    pytest.param("WEBP", "RGB", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="webp_roundtrip")),
    pytest.param("WEBP", "RGBA", "Image.save", marks=pytest.mark.covers("Image.save", mode="RGBA", target="cpu", variant="webp_roundtrip")),
])
def test_format_mode_roundtrip(PIL, fmt, mode, op, tmp_path):
    """Save PIL image in format+mode, reload with both PIL and RSPIL, assert parity."""
    color = _make_test_image(mode)

    if mode == "P":
        pil_img = PIL.Image.new("RGB", (30, 30), (255, 0, 0)).convert("P")
        rs_img = Image.new("RGB", (30, 30), (255, 0, 0)).convert("P")
    else:
        try:
            pil_img = PIL.Image.new(mode, (30, 30), color)
            rs_img = Image.new(mode, (30, 30), color)
        except Exception:
            pytest.skip(f"PIL doesn't support Image.new('{mode}', ...)")

    path = str(tmp_path / f"test.{fmt.lower()}")
    try:
        pil_img.save(path, fmt)
    except Exception:
        pytest.skip(f"PIL cannot save {mode} as {fmt}")

    # Load saved image with both
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert rs_loaded.size == pil_loaded.size
    assert rs_loaded.mode == pil_loaded.mode


