"""PIL parity tests for Image.open, save, thumbnail, close, verify, seek, tell, load."""
import pytest, tempfile, os
from pillow_rs import Image
from conftest import assert_images_equal


class TestOpenSave:
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

    def test_open_bytes(self, PIL):
        import io
        pil_img = PIL.Image.new("RGB", (20, 20), (255, 0, 0))
        buf = io.BytesIO()
        pil_img.save(buf, "PNG")
        data = buf.getvalue()
        rs_img = Image.open(data)
        assert rs_img.size == (20, 20)


class TestThumbnail:
    def test_thumbnail_parity(self, PIL):
        pil = PIL.Image.new("RGB", (400, 200), (128, 128, 128))
        rs = Image.new("RGB", (400, 200), (128, 128, 128))
        pil.thumbnail((100, 100))
        rs.thumbnail((100, 100))
        assert rs.size == pil.size


class TestBookkeeping:
    def test_close_no_error(self):
        img = Image.new("RGB", (10, 10))
        img.close()

    def test_verify_no_error(self):
        img = Image.new("RGB", (10, 10))
        img.verify()

    def test_seek_tell(self):
        img = Image.new("RGB", (10, 10))
        img.seek(0)
        assert img.tell() == 0

    def test_load_returns(self):
        img = Image.new("RGB", (10, 10))
        result = img.load()
        assert result is not None
