"""Tests for remaining Image methods and ImageDraw additions."""
import pytest
from pillow_rs import Image, ImageDraw, ImageOps, ImageChops


class TestPoint:
    @pytest.mark.covers("Image.point", mode="RGB", target="cpu", variant="lut")
    def test_point_lut(self):
        img = Image.new("RGB", (10, 10), (100, 100, 100))
        lut = bytes([min(255, i + 50) for i in range(256)])
        result = img.point(lut)
        assert result.size == (10, 10)

    @pytest.mark.covers("Image.point", mode="RGB", target="cpu", variant="callable")
    def test_point_callable(self, PIL=None):
        img = Image.new("RGB", (10, 10), (100, 100, 100))
        result = img.point(lambda x: x * 2)
        assert result.size == (10, 10)


class TestEffectSpread:
    @pytest.mark.covers("Image.effect_spread", mode="RGB", target="cpu", variant="default")
    def test_effect_spread_works(self):
        img = Image.new("RGB", (50, 50), (255, 0, 0))
        result = img.effect_spread(3)
        assert result.size == (50, 50)


class TestDrawArcPieslice:
    @pytest.mark.covers("ImageDraw.arc", mode="RGB", target="cpu", variant="default")
    def test_draw_arc_works(self):
        img = Image.new("RGB", (80, 80), (255, 255, 255))
        ImageDraw.ImageDraw(img).arc([10, 10, 70, 70], start=0, end=180, fill=(255, 0, 0))
        assert img.size == (80, 80)

    @pytest.mark.covers("ImageDraw.pieslice", mode="RGB", target="cpu", variant="default")
    def test_draw_pieslice_works(self):
        img = Image.new("RGB", (80, 80), (255, 255, 255))
        ImageDraw.ImageDraw(img).pieslice([10, 10, 70, 70], start=0, end=90, fill=(0, 255, 0))
        assert img.size == (80, 80)

    @pytest.mark.covers("ImageDraw.circle", mode="RGB", target="cpu", variant="default")
    def test_draw_circle_works(self):
        img = Image.new("RGB", (50, 50), (255, 255, 255))
        ImageDraw.ImageDraw(img).circle((25, 25), 20, fill=(0, 0, 255))
        assert img.size == (50, 50)


class TestImageOpsContain:
    @pytest.mark.covers("ImageOps.contain", mode="RGB", target="cpu", variant="default")
    def test_contain_works(self):
        img = Image.new("RGB", (200, 100), (255, 0, 0))
        result = ImageOps.contain(img, (50, 50))
        assert result.size == (50, 25)


class TestImageChopsNew:
    @pytest.mark.covers("ImageChops.duplicate", mode="RGB", target="cpu", variant="default")
    def test_duplicate_works(self):
        img = Image.new("RGB", (20, 20), (128, 0, 0))
        dup = ImageChops.duplicate(img)
        assert dup.size == img.size
