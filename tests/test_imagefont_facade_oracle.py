from io import BytesIO
from pathlib import Path

import pytest
from PIL import ImageFont as PILImageFont


FONT_ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/font/input/fonts"
DEJAVU = FONT_ROOT / "DejaVuSans.ttf"
VARIABLE = FONT_ROOT / "variable-named-instances.ttf"


def _font_observation(font):
    return {
        "name": font.getname(),
        "metrics": font.getmetrics(),
        "length": font.getlength("AV"),
        "bbox": font.getbbox("AV"),
    }


@pytest.mark.covers("ImageFont.truetype")
def test_imagefont_truetype_pathlike_and_bytes_path_match_pillow(RSPIL):
    for source in (DEJAVU, bytes(str(DEJAVU), "utf-8")):
        assert _font_observation(RSPIL.ImageFont.truetype(source, 20)) == _font_observation(
            PILImageFont.truetype(source, 20)
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.get_variation_names")
def test_imagefont_variation_names_match_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    assert RSPIL.ImageFont.truetype(BytesIO(data), 20).get_variation_names() == (
        PILImageFont.truetype(BytesIO(data), 20).get_variation_names()
    )


@pytest.mark.covers("ImageFont.FreeTypeFont.get_variation_axes")
def test_imagefont_variation_axes_match_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    assert RSPIL.ImageFont.truetype(BytesIO(data), 20).get_variation_axes() == (
        PILImageFont.truetype(BytesIO(data), 20).get_variation_axes()
    )


@pytest.mark.covers("ImageFont.FreeTypeFont.set_variation_by_name")
def test_imagefont_set_variation_by_name_matches_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20)
    pil_font = PILImageFont.truetype(BytesIO(data), 20)
    name = pil_font.get_variation_names()[0]

    rs_font.set_variation_by_name(name)
    pil_font.set_variation_by_name(name)

    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.FreeTypeFont.set_variation_by_axes")
def test_imagefont_set_variation_by_axes_matches_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20)
    pil_font = PILImageFont.truetype(BytesIO(data), 20)
    axes = [axis["default"] for axis in pil_font.get_variation_axes()]

    rs_font.set_variation_by_axes(axes)
    pil_font.set_variation_by_axes(axes)

    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.FreeTypeFont.font_variant")
def test_imagefont_file_like_font_variant_matches_pillow(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20).font_variant(size=21)
    pil_font = PILImageFont.truetype(BytesIO(data), 20).font_variant(size=21)

    assert _font_observation(rs_font) == _font_observation(pil_font)
