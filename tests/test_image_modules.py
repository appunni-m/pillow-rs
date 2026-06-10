"""Tests for ImageFilter, ImageFont, ImagePalette, ImageStat, ImageSequence."""
import pytest
from pillow_rs import (
    Image, ImageFilter, ImageFont, ImagePalette, ImageStat, ImageSequence, ImageOps
)


class TestImageFilter:
    @pytest.mark.covers("ImageFilter.BLUR")
    def test_blur_constant(self):
        assert ImageFilter.BLUR == "BLUR"

    def test_gaussian_blur_class(self):
        f = ImageFilter.GaussianBlur(radius=2)
        img = Image.new("RGB", (20, 20), (255, 0, 0))
        result = ImageFilter.apply_filter(img, f)
        assert result.size == (20, 20)

    @pytest.mark.covers("ImageFilter.MaxFilter")

    def test_max_filter_class(self):
        f = ImageFilter.MaxFilter(size=3)
        img = Image.new("RGB", (20, 20), (128, 128, 128))
        result = ImageFilter.apply_filter(img, f)
        assert result.size == (20, 20)


class TestImageFont:
    @pytest.mark.covers("ImageFont.load_default")
    def test_load_default_returns_font(self):
        font = ImageFont.load_default(10)
        assert font is not None

    @pytest.mark.covers("ImageFont.FreeTypeFont")

    def test_freetype_stub(self):
        with pytest.raises((NotImplementedError, OSError)):
            ImageFont.FreeTypeFont("nonexistent.ttf")

    @pytest.mark.covers("ImageFont.truetype")

    def test_truetype_stub(self):
        with pytest.raises((NotImplementedError, OSError)):
            ImageFont.truetype("nonexistent.ttf", 12)


class TestImagePalette:
    @pytest.mark.covers("ImagePalette")
    def test_create_palette(self):
        p = ImagePalette.ImagePalette()
        assert p.mode == "RGB"
        assert p.tobytes() == b""

    def test_copy_palette(self):
        p = ImagePalette.ImagePalette()
        p.palette = [255, 0, 0]
        p2 = p.copy()
        assert p2.palette == [255, 0, 0]


class TestImageStat:
    @pytest.mark.covers("ImageStat.Stat")
    def test_stat_basic(self):
        s = ImageStat.Stat([1, 2, 3, 4, 5])
        assert s.count == 5
        assert s.sum == 15
        assert s.mean == 3.0


class TestImageSequence:
    def test_iterator_exists(self):
        img = Image.new("RGB", (10, 10))
        it = ImageSequence.Iterator(img)
        assert it is not None
        assert it._frame == 0


class TestImageFontTruetype:
    def test_truetype_loads_real_font(self):
        import os
        for path in ['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
                     '/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf']:
            if os.path.exists(path):
                font = ImageFont.truetype(path, 14)
                bbox = font.getbbox("Test")
                assert bbox[0] > 0
                return
        pytest.skip("No DejaVuSans.ttf found")


class TestImageOpsExpand:
    def test_expand_border(self):
        img = Image.new("RGB", (50, 50), (255, 0, 0))
        expanded = ImageOps.expand(img, border=5, fill=(0, 255, 0))
        assert expanded.size == (60, 60)
