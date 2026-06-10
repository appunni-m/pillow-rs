"""Tests for Image.convert(), split(), getbands(), filter(), thumbnail()."""
import pytest
from pillow_rs import Image


class TestImageConvert:
    @pytest.mark.covers("Image.convert", variant="mode_L")
    def test_convert_rgb_to_l(self):
        img = Image.new("RGB", (50, 50), (128, 128, 128))
        gray = img.convert("L")
        assert gray.mode == "L"
        assert gray.size == (50, 50)

    @pytest.mark.covers("Image.convert", variant="mode_RGBA")
    def test_convert_rgb_to_rgba(self):
        img = Image.new("RGB", (50, 50))
        rgba = img.convert("RGBA")
        assert rgba.mode == "RGBA"

    @pytest.mark.covers("Image.convert", variant="mode_RGB")
    def test_convert_rgba_to_rgb(self):
        img = Image.new("RGBA", (50, 50), (128, 0, 0, 255))
        rgb = img.convert("RGB")
        assert rgb.mode == "RGB"

    @pytest.mark.covers("Image.convert", variant="mode_1")
    def test_convert_rgb_to_bilevel(self):
        img = Image.new("RGB", (20, 20), (200, 200, 200))
        bw = img.convert("1")
        assert bw.mode == "L"

    @pytest.mark.covers("Image.convert", variant="mode_1_dither_none")
    def test_convert_to_bilevel_no_dither(self):
        img = Image.new("RGB", (20, 20), (200, 200, 200))
        bw = img.convert("1", dither="NONE")
        assert bw.mode == "L"

    @pytest.mark.covers("Image.convert", variant="mode_LA")
    def test_convert_rgb_to_la(self):
        img = Image.new("RGB", (50, 50))
        la = img.convert("LA")
        assert la.mode == "LA"


class TestImageSplit:
    def test_split_rgb(self):
        img = Image.new("RGB", (30, 20), (255, 0, 0))
        bands = img.split()
        assert len(bands) == 3
        for band in bands:
            assert band.mode == "L"
            assert band.size == (30, 20)

    def test_split_rgba(self):
        img = Image.new("RGBA", (30, 20), (255, 0, 0, 128))
        bands = img.split()
        assert len(bands) == 4

    def test_split_grayscale(self):
        img = Image.new("L", (30, 20), 128)
        bands = img.split()
        assert len(bands) == 1

    def test_getbands_rgb(self):
        img = Image.new("RGB", (10, 10))
        bands = img.getbands()
        assert bands == ("R", "G", "B")


class TestImageFilter:
    def test_filter_blur(self):
        img = Image.new("RGB", (50, 50), (128, 0, 0))
        blurred = img.filter("BLUR")
        assert blurred.size == (50, 50)

    def test_filter_sharpen(self):
        img = Image.new("RGB", (50, 50))
        sharp = img.filter("SHARPEN")
        assert sharp.size == (50, 50)

    def test_filter_unknown_raises(self):
        img = Image.new("RGB", (10, 10))
        with pytest.raises(Exception):
            img.filter("UNKNOWN_FILTER")


class TestImageThumbnail:
    def test_thumbnail_preserves_aspect(self):
        img = Image.new("RGB", (400, 200))
        img.thumbnail((100, 100))
        assert img.size[0] == 100
        assert img.size[1] == 50

    def test_thumbnail_zero_raises(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(Exception):
            img.thumbnail((0, 0))
