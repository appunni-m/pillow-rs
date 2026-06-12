"""PIL parity tests for Image.convert()."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal


class TestConvert:
    @pytest.mark.covers("Image.convert", variant="mode_L")
    def test_rgb_to_l_parity(self, PIL):
        pil = PIL.Image.new("RGB", (50, 50), (128, 128, 128))
        rs = Image.new("RGB", (50, 50), (128, 128, 128))
        assert_images_equal(rs.convert("L"), pil.convert("L"))

    @pytest.mark.covers("Image.convert", variant="mode_RGB")
    def test_rgba_to_rgb_parity(self, PIL):
        pil = PIL.Image.new("RGBA", (50, 50), (255, 0, 0, 255))
        rs = Image.new("RGBA", (50, 50), (255, 0, 0, 255))
        assert_images_equal(rs.convert("RGB"), pil.convert("RGB"))

    @pytest.mark.covers("Image.convert", variant="mode_RGBA")
    def test_rgb_to_rgba_parity(self, PIL):
        pil = PIL.Image.new("RGB", (40, 40), (100, 200, 50))
        rs = Image.new("RGB", (40, 40), (100, 200, 50))
        assert_images_equal(rs.convert("RGBA"), pil.convert("RGBA"))

    @pytest.mark.covers("Image.convert", variant="mode_LA")
    def test_rgb_to_la_parity(self, PIL):
        pil = PIL.Image.new("RGB", (30, 30), (200, 100, 50))
        rs = Image.new("RGB", (30, 30), (200, 100, 50))
        assert_images_equal(rs.convert("LA"), pil.convert("LA"))

    @pytest.mark.covers("Image.convert", mode="L", target="cpu", variant="mode_RGB")
    def test_l_to_rgb_parity(self, PIL):
        pil = PIL.Image.new("L", (40, 40), 200)
        rs = Image.new("L", (40, 40), 200)
        assert_images_equal(rs.convert("RGB"), pil.convert("RGB"))

    @pytest.mark.covers("Image.convert", mode="RGB", target="cpu", variant="chain")
    def test_convert_chain_parity(self, PIL):
        """Multiple conversions produce same result."""
        pil = PIL.Image.new("RGB", (30, 30), (255, 128, 64))
        rs = Image.new("RGB", (30, 30), (255, 128, 64))
        pil_c = pil.convert("L").convert("RGB")
        rs_c = rs.convert("L").convert("RGB")
        assert_images_equal(rs_c, pil_c)
