"""PIL parity tests for Image.rotate() and Image.transpose()."""
import pytest
from pillow_rs import Image, Transpose
from conftest import assert_images_equal


class TestRotate:
    @pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_90")
    def test_rotate_90_parity(self, PIL):
        pil = PIL.Image.new("RGB", (200, 100), (255, 128, 0))
        rs = Image.new("RGB", (200, 100), (255, 128, 0))
        assert_images_equal(rs.rotate(90), pil.rotate(90, expand=True))

    @pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_180")
    def test_rotate_180_parity(self, PIL):
        pil = PIL.Image.new("RGB", (200, 100), (255, 0, 0))
        rs = Image.new("RGB", (200, 100), (255, 0, 0))
        assert_images_equal(rs.rotate(180), pil.rotate(180))

    @pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_270")
    def test_rotate_270_parity(self, PIL):
        pil = PIL.Image.new("RGB", (200, 100), (0, 255, 0))
        rs = Image.new("RGB", (200, 100), (0, 255, 0))
        assert_images_equal(rs.rotate(270), pil.rotate(270, expand=True))


class TestTranspose:
    @pytest.mark.covers("Image.transpose", variant="FLIP_LEFT_RIGHT")
    def test_flip_left_right_parity(self, PIL):
        pil = PIL.Image.new("RGB", (100, 50), (255, 0, 0))
        rs = Image.new("RGB", (100, 50), (255, 0, 0))
        assert_images_equal(
            rs.transpose(Transpose.FLIP_LEFT_RIGHT),
            pil.transpose(PIL.Image.FLIP_LEFT_RIGHT),
        )

    @pytest.mark.covers("Image.transpose", variant="FLIP_TOP_BOTTOM")
    def test_flip_top_bottom_parity(self, PIL):
        pil = PIL.Image.new("RGB", (100, 50), (0, 255, 0))
        rs = Image.new("RGB", (100, 50), (0, 255, 0))
        assert_images_equal(
            rs.transpose(Transpose.FLIP_TOP_BOTTOM),
            pil.transpose(PIL.Image.FLIP_TOP_BOTTOM),
        )

    @pytest.mark.covers("Image.transpose", variant="ROTATE_90")
    def test_rotate_90_parity(self, PIL):
        pil = PIL.Image.new("RGB", (200, 100), (0, 0, 255))
        rs = Image.new("RGB", (200, 100), (0, 0, 255))
        assert_images_equal(
            rs.transpose(Transpose.ROTATE_90),
            pil.transpose(PIL.Image.ROTATE_90),
        )

    @pytest.mark.covers("Image.transpose", variant="ROTATE_180")
    def test_rotate_180_parity(self, PIL):
        pil = PIL.Image.new("RGB", (100, 100), (255, 255, 0))
        rs = Image.new("RGB", (100, 100), (255, 255, 0))
        assert_images_equal(
            rs.transpose(Transpose.ROTATE_180),
            pil.transpose(PIL.Image.ROTATE_180),
        )

    @pytest.mark.covers("Image.transpose", variant="ROTATE_270")
    def test_rotate_270_parity(self, PIL):
        pil = PIL.Image.new("RGB", (200, 100), (128, 0, 255))
        rs = Image.new("RGB", (200, 100), (128, 0, 255))
        assert_images_equal(
            rs.transpose(Transpose.ROTATE_270),
            pil.transpose(PIL.Image.ROTATE_270),
        )

    @pytest.mark.covers("Image.transpose", mode="RGB", target="cpu", variant="TRANSPOSE")
    def test_transpose_parity(self, PIL):
        pil = PIL.Image.new("RGB", (100, 80), (100, 200, 50))
        rs = Image.new("RGB", (100, 80), (100, 200, 50))
        assert_images_equal(
            rs.transpose(Transpose.TRANSPOSE),
            pil.transpose(PIL.Image.TRANSPOSE),
        )

    @pytest.mark.covers("Image.transpose", mode="RGB", target="cpu", variant="TRANSVERSE")
    def test_transverse_parity(self, PIL):
        pil = PIL.Image.new("RGB", (100, 80), (200, 100, 50))
        rs = Image.new("RGB", (100, 80), (200, 100, 50))
        assert_images_equal(
            rs.transpose(Transpose.TRANSVERSE),
            pil.transpose(PIL.Image.TRANSVERSE),
        )
