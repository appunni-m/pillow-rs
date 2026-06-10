"""Tests for Image.crop(), rotate(), transpose()."""
import pytest
from pillow_rs import Image, Transpose


class TestImageCrop:
    @pytest.mark.covers("Image.crop", mode="RGB")
    def test_crop_basic(self):
        img = Image.new("RGB", (100, 100), (255, 0, 0))
        # Pillow: (left, top, right, bottom) → wrapper converts to (x, y, w, h)
        cropped = img.crop((10, 10, 60, 60))
        assert cropped.size == (50, 50)

    def test_crop_out_of_bounds_raises(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(Exception):
            img.crop((50, 50, 200, 200))

    def test_crop_invalid_box_raises(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(Exception):
            img.crop((50, 50, 50, 50))


class TestImageRotate:
    @pytest.mark.covers("Image.rotate", variant="angle_90")
    def test_rotate_90(self):
        img = Image.new("RGB", (200, 100), (255, 0, 0))
        r90 = img.rotate(90)
        assert r90.size == (100, 200)

    @pytest.mark.covers("Image.rotate", variant="angle_180")
    def test_rotate_180(self):
        img = Image.new("RGB", (200, 100))
        r180 = img.rotate(180)
        assert r180.size == (200, 100)

    @pytest.mark.covers("Image.rotate", variant="angle_270")
    def test_rotate_270(self):
        img = Image.new("RGB", (200, 100))
        r270 = img.rotate(270)
        assert r270.size == (100, 200)

    def test_rotate_arbitrary_raises(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(Exception):
            img.rotate(45)


class TestImageTranspose:
    @pytest.mark.covers("Image.transpose", variant="FLIP_LEFT_RIGHT")
    def test_flip_left_right(self):
        img = Image.new("RGB", (100, 50))
        flipped = img.transpose(Transpose.FLIP_LEFT_RIGHT)
        assert flipped.size == (100, 50)

    @pytest.mark.covers("Image.transpose", variant="FLIP_TOP_BOTTOM")
    def test_flip_top_bottom(self):
        img = Image.new("RGB", (100, 50))
        flipped = img.transpose(Transpose.FLIP_TOP_BOTTOM)
        assert flipped.size == (100, 50)

    @pytest.mark.covers("Image.transpose", variant="ROTATE_90")
    def test_transpose_rotate_90(self):
        img = Image.new("RGB", (200, 100))
        r90 = img.transpose(Transpose.ROTATE_90)
        assert r90.size == (100, 200)

    @pytest.mark.covers("Image.transpose", variant="ROTATE_180")
    def test_transpose_rotate_180(self):
        img = Image.new("RGB", (100, 100))
        r180 = img.transpose(Transpose.ROTATE_180)
        assert r180.size == (100, 100)

    @pytest.mark.covers("Image.transpose", variant="ROTATE_270")
    def test_transpose_rotate_270(self):
        img = Image.new("RGB", (200, 100))
        r270 = img.transpose(Transpose.ROTATE_270)
        assert r270.size == (100, 200)
