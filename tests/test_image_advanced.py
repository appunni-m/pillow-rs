"""PIL parity tests for alpha_composite, point, effect_spread, quantize, entropy, getcolors, getdata, getprojection."""
import pytest
from pillow_rs import Image
from conftest import assert_images_equal, assert_values_equal


class TestAlphaComposite:
    def test_alpha_composite_works(self):
        bg = Image.new("RGBA", (50, 50), (255, 255, 255, 255))
        fg = Image.new("RGBA", (20, 20), (255, 0, 0, 128))
        bg.alpha_composite(fg)
        assert bg.size == (50, 50)


class TestPoint:
    def test_point_lut_parity(self, PIL):
        pil = PIL.Image.new("L", (5, 5), 100)
        rs = Image.new("L", (5, 5), 100)
        lut = bytes([min(255, i + 50) for i in range(256)])
        assert_images_equal(rs.point(lut), pil.point(lut))


class TestEffectSpread:
    def test_effect_spread_works(self):
        img = Image.new("RGB", (50, 50), (255, 0, 0))
        result = img.effect_spread(3)
        assert result.size == (50, 50)


class TestQuantize:
    def test_quantize_parity(self, PIL):
        pil = PIL.Image.new("RGB", (50, 50), (128, 64, 32))
        rs = Image.new("RGB", (50, 50), (128, 64, 32))
        pil_q = pil.quantize(16)
        rs_q = rs.quantize(16)
        assert rs_q.size == pil_q.size


class TestAnalysis:
    def test_entropy_works(self):
        img = Image.new("RGB", (50, 50), (128, 128, 128))
        e = img.entropy()
        assert 0 <= e <= 10

    def test_getcolors_works(self):
        img = Image.new("RGB", (10, 10), (255, 0, 0))
        colors = img.getcolors(256)
        assert colors is not None

    def test_getdata_rgb_parity(self, PIL):
        pil = PIL.Image.new("RGB", (5, 5), (100, 150, 200))
        rs = Image.new("RGB", (5, 5), (100, 150, 200))
        assert rs.tobytes() == pil.tobytes()

    def test_getprojection_works(self):
        img = Image.new("L", (10, 10), 128)
        h_proj, v_proj = img.getprojection()
        assert len(h_proj) == 10
        assert len(v_proj) == 10
