"""Tests for Image properties — size, width, height, mode, format."""
import pytest
from pillow_rs import Image


class TestImageProperties:
    def test_size(self):
        img = Image.new("RGB", (150, 75))
        assert img.size == (150, 75)

    def test_width_height(self):
        img = Image.new("RGB", (200, 100))
        assert img.width == 200
        assert img.height == 100

    def test_mode(self):
        img = Image.new("RGBA", (10, 10))
        assert img.mode == "RGBA"

    def test_format_none_for_new(self):
        img = Image.new("RGB", (10, 10))
        assert img.format is None

    def test_repr(self):
        img = Image.new("RGB", (20, 30))
        r = repr(img)
        assert "Image" in r
        assert "20" in r
        assert "30" in r
