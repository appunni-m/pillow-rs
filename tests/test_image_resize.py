"""Tests for Image.resize() — verifies signature and error on stubs."""
import pytest
from pillow_rs import Image, Resampling


class TestImageResize:
    @pytest.mark.covers("Image.resize", mode="RGB", variant="default")
    def test_resize_signature_exists(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError, match="Image.resize"):
            img.resize((50, 50))

    @pytest.mark.covers("Image.resize", mode="RGB", variant="nearest")
    def test_resize_with_nearest(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError):
            img.resize((50, 50), Resampling.NEAREST)

    @pytest.mark.covers("Image.resize", mode="RGB", variant="lanczos")
    def test_resize_with_lanczos(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError):
            img.resize((200, 200), Resampling.LANCZOS)
