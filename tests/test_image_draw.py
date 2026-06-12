"""Tests for ImageDraw — validates functionality, not pixel-exact parity (drawing is algorithm-specific)."""
import pytest
from pillow_rs import Image, ImageDraw


@pytest.mark.covers("ImageDraw.line")


def test_draw_line_works():
    img = Image.new("RGB", (100, 100), (255, 255, 255))
    draw = ImageDraw.ImageDraw(img)
    draw.line([(10, 10), (90, 90)], fill=(255, 0, 0))
    assert img.size == (100, 100)
    assert img.mode == "RGB"


@pytest.mark.covers("ImageDraw.rectangle", mode="RGB", target="cpu", variant="outline")
def test_draw_rectangle_outline():
    img = Image.new("RGB", (100, 100), (255, 255, 255))
    ImageDraw.ImageDraw(img).rectangle([20, 20, 80, 80], outline=(255, 0, 0))
    assert img.size == (100, 100)


@pytest.mark.covers("ImageDraw.rectangle")


def test_draw_rectangle_filled():
    img = Image.new("RGB", (50, 50), (255, 255, 255))
    ImageDraw.ImageDraw(img).rectangle([10, 10, 40, 40], fill=(0, 255, 0))
    assert img.size == (50, 50)


@pytest.mark.covers("ImageDraw.ellipse", mode="RGB", target="cpu", variant="default")
def test_draw_ellipse():
    img = Image.new("RGB", (50, 50), (255, 255, 255))
    ImageDraw.ImageDraw(img).ellipse([5, 5, 45, 45], outline=(255, 0, 0))
    assert img.size == (50, 50)


@pytest.mark.covers("ImageDraw.point")


def test_draw_point():
    img = Image.new("RGB", (30, 30), (255, 255, 255))
    ImageDraw.ImageDraw(img).point([(15, 15)], fill=(255, 0, 0))
    assert img.size == (30, 30)


@pytest.mark.covers("ImageDraw.polygon", mode="RGB", target="cpu", variant="default")
def test_draw_polygon():
    img = Image.new("RGB", (50, 50), (255, 255, 255))
    ImageDraw.ImageDraw(img).polygon([(10, 10), (40, 10), (25, 40)], fill=(0, 0, 255))
    assert img.size == (50, 50)


# ── From test_imagedraw_advanced.py ──

"""Tests for remaining ImageDraw methods."""


@pytest.mark.covers("ImageDraw.arc")


def test_draw_arc_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).arc([10, 10, 70, 70], start=0, end=180, fill=(255, 0, 0))
    assert img.size == (80, 80)


@pytest.mark.covers("ImageDraw.chord", mode="RGB", target="cpu", variant="default")
def test_draw_chord_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).chord([10, 10, 70, 70], start=0, end=90, fill=(0, 200, 0))
    assert img.size == (80, 80)


@pytest.mark.covers("ImageDraw.pieslice")


def test_draw_pieslice_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).pieslice([10, 10, 70, 70], start=0, end=120, fill=(0, 0, 255))
    assert img.size == (80, 80)


@pytest.mark.covers("ImageDraw.circle", mode="RGB", target="cpu", variant="default")
def test_draw_circle_works():
    img = Image.new("RGB", (60, 60), (255, 255, 255))
    ImageDraw.ImageDraw(img).circle((30, 30), 25, fill=(255, 200, 0), outline=(0, 0, 0))
    assert img.size == (60, 60)


@pytest.mark.covers("ImageDraw.rounded_rectangle")


def test_draw_rounded_rectangle_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).rounded_rectangle([10, 10, 70, 70], radius=10, fill=(128, 0, 255))
    assert img.size == (80, 80)



