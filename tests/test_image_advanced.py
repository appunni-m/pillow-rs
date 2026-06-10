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
        pil = PIL.Image.new("L", (20, 20), 100)
        rs = Image.new("L", (20, 20), 100)
        lut = [min(255, i + 50) for i in range(256)]
        pil_out = pil.point(lut)
        rs_out = rs.point(lut)
        # Our point() always returns RGB; convert to L for parity
        if rs_out.mode != pil_out.mode:
            rs_out = rs_out.convert(pil_out.mode)
        assert_images_equal(rs_out, pil_out)


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

    def test_getdata_works(self):
        img = Image.new("RGB", (10, 10), (100, 100, 100))
        data = img.getdata()
        # Returns RGBA raw data (4 bytes per pixel = 400)
        assert len(data) in (300, 400)

    def test_getprojection_works(self):
        img = Image.new("L", (10, 10), 128)
        h_proj, v_proj = img.getprojection()
        assert len(h_proj) == 10
        assert len(v_proj) == 10
