"""Benchmark fixtures — large test images at various sizes."""
import pytest
from pillow_rs import Image


@pytest.fixture(scope="module")
def rgb_small_image():
    return Image.new("RGB", (100, 100), (128, 64, 32))


@pytest.fixture(scope="module")
def rgb_medium_image():
    return Image.new("RGB", (800, 600), (255, 128, 0))


@pytest.fixture(scope="module")
def rgb_large_image():
    return Image.new("RGB", (4000, 3000), (64, 128, 255))


@pytest.fixture(scope="module")
def rgba_image():
    return Image.new("RGBA", (500, 500), (255, 0, 0, 128))


@pytest.fixture(scope="module")
def grayscale_image():
    return Image.new("L", (500, 500), 128)
