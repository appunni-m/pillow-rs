"""Tests for Image.resize() — all resample filters and edge cases."""
import pytest
from pillow_rs import Image, Resampling


class TestImageResize:
    @pytest.mark.covers("Image.resize", mode="RGB", variant="default")
    def test_resize_default_bilinear(self):
        img = Image.new("RGB", (100, 100), (255, 0, 0))
        small = img.resize((50, 50))
        assert small.size == (50, 50)
        assert small is not img

    @pytest.mark.covers("Image.resize", mode="RGB", variant="nearest")
    def test_resize_nearest(self):
        img = Image.new("RGB", (100, 100))
        small = img.resize((30, 30), Resampling.NEAREST)
        assert small.size == (30, 30)

    @pytest.mark.covers("Image.resize", mode="RGB", variant="lanczos")
    def test_resize_lanczos(self):
        img = Image.new("RGB", (100, 100))
        big = img.resize((200, 200), Resampling.LANCZOS)
        assert big.size == (200, 200)

    def test_resize_same_size_returns_copy(self):
        img = Image.new("RGB", (50, 50))
        same = img.resize((50, 50))
        assert same.size == (50, 50)
        assert same is not img

    def test_resize_zero_dimension_raises(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(Exception):
            img.resize((0, 100))

    def test_resize_rgba_mode(self):
        img = Image.new("RGBA", (100, 100), (255, 0, 0, 128))
        small = img.resize((50, 50))
        assert small.size == (50, 50)
        assert small.mode == "RGBA"

    def test_resize_grayscale_mode(self):
        img = Image.new("L", (100, 100), 128)
        small = img.resize((50, 50))
        assert small.size == (50, 50)
        assert small.mode == "L"
