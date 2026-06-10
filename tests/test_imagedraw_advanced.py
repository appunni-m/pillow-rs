"""Tests for remaining ImageDraw methods."""
import pytest
from pillow_rs import Image, ImageDraw


def test_draw_arc_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).arc([10, 10, 70, 70], start=0, end=180, fill=(255, 0, 0))
    assert img.size == (80, 80)


def test_draw_chord_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).chord([10, 10, 70, 70], start=0, end=90, fill=(0, 200, 0))
    assert img.size == (80, 80)


def test_draw_pieslice_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).pieslice([10, 10, 70, 70], start=0, end=120, fill=(0, 0, 255))
    assert img.size == (80, 80)


def test_draw_circle_works():
    img = Image.new("RGB", (60, 60), (255, 255, 255))
    ImageDraw.ImageDraw(img).circle((30, 30), 25, fill=(255, 200, 0), outline=(0, 0, 0))
    assert img.size == (60, 60)


def test_draw_rounded_rectangle_works():
    img = Image.new("RGB", (80, 80), (255, 255, 255))
    ImageDraw.ImageDraw(img).rounded_rectangle([10, 10, 70, 70], radius=10, fill=(128, 0, 255))
    assert img.size == (80, 80)
