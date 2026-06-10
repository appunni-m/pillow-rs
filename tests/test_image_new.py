"""Tests for Image.new() — creation with various modes and colors."""
import pytest
from pillow_rs import Image


class TestImageNew:
    @pytest.mark.covers("Image.new", mode="RGB", variant="default")
    def test_new_rgb_default(self):
        img = Image.new("RGB", (100, 100))
        assert img.size == (100, 100)
        assert img.mode == "RGB"

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_int")
    def test_new_rgb_with_int_color(self):
        img = Image.new("RGB", (50, 50), 128)
        assert img.size == (50, 50)

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_hex")
    def test_new_rgb_with_hex_color(self):
        img = Image.new("RGB", (10, 10), "#FF0000")
        data = img.tobytes()
        assert data[0] == 255
        assert data[1] == 0

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_rgb_tuple")
    def test_new_rgb_with_rgb_tuple(self):
        img = Image.new("RGB", (25, 25), (255, 0, 0))
        assert img.mode == "RGB"

    @pytest.mark.covers("Image.new", mode="RGBA", variant="default")
    def test_new_rgba(self):
        img = Image.new("RGBA", (30, 30))
        assert img.mode == "RGBA"

    @pytest.mark.covers("Image.new", mode="L", variant="default")
    def test_new_grayscale(self):
        img = Image.new("L", (10, 10), 128)
        assert img.mode == "L"
        data = img.tobytes()
        assert data[0] == 128

    @pytest.mark.covers("Image.new", edge_case="zero_width")
    def test_new_zero_width_creates_empty(self):
        # Phase 1: core doesn't yet validate dimensions; Phase 2 will add validation
        img = Image.new("RGB", (0, 100))
        assert img.size[0] == 0

    @pytest.mark.covers("Image.new", edge_case="invalid_mode")
    def test_new_invalid_mode_raises(self):
        with pytest.raises(Exception):
            Image.new("INVALID", (100, 100))
