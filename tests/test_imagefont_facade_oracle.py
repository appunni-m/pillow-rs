from io import BytesIO
from pathlib import Path
import sys

import pytest
from PIL import ImageFont as PILImageFont


FONT_ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/font/input/fonts"
PILFONT_ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/font/input/pilfont"
DEJAVU = FONT_ROOT / "DejaVuSans.ttf"
VARIABLE = FONT_ROOT / "variable-named-instances.ttf"
COURB08 = PILFONT_ROOT / "courb08.pil"


def _font_observation(font):
    return {
        "name": font.getname(),
        "metrics": font.getmetrics(),
        "length": font.getlength("AV"),
        "bbox": font.getbbox("AV"),
    }


def _bitmap_font_observation(font):
    mask = font.getmask("ABC")
    return {
        "info": list(font.info),
        "bbox": font.getbbox("ABC"),
        "length": font.getlength("ABC"),
        "mask_mode": mask.mode,
        "mask_size": mask.size,
        "mask_bytes": bytes(mask),
    }


@pytest.mark.covers("ImageFont.truetype")
def test_imagefont_layout_enum_and_no_raqm_fallback_match_pillow(RSPIL):
    assert int(RSPIL.ImageFont.Layout.BASIC) == int(PILImageFont.Layout.BASIC)
    assert int(RSPIL.ImageFont.Layout.RAQM) == int(PILImageFont.Layout.RAQM)

    with pytest.warns(UserWarning, match="Raqm layout was requested"):
        rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20, layout_engine=RSPIL.ImageFont.Layout.RAQM)
    with pytest.warns(UserWarning, match="Raqm layout was requested"):
        pil_font = PILImageFont.truetype(DEJAVU, 20, layout_engine=PILImageFont.Layout.RAQM)

    assert int(rs_font.layout_engine) == int(pil_font.layout_engine) == int(PILImageFont.Layout.BASIC)
    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.load")
def test_imagefont_load_bitmap_font_matches_pillow(RSPIL):
    for source in (str(COURB08), COURB08):
        assert _bitmap_font_observation(RSPIL.ImageFont.load(source)) == (
            _bitmap_font_observation(PILImageFont.load(source))
        )


@pytest.mark.covers("ImageFont.load_path")
def test_imagefont_load_path_bitmap_font_matches_pillow(RSPIL):
    sys.path.insert(0, str(PILFONT_ROOT))
    try:
        for source in ("courb08.pil", b"courb08.pil"):
            assert _bitmap_font_observation(RSPIL.ImageFont.load_path(source)) == (
                _bitmap_font_observation(PILImageFont.load_path(source))
            )
    finally:
        sys.path.remove(str(PILFONT_ROOT))


@pytest.mark.covers("ImageFont.load_default_imagefont")
def test_imagefont_load_default_imagefont_matches_pillow(RSPIL):
    assert _bitmap_font_observation(RSPIL.ImageFont.load_default_imagefont()) == (
        _bitmap_font_observation(PILImageFont.load_default_imagefont())
    )


@pytest.mark.covers("ImageFont.load_default")
def test_imagefont_load_default_freetype_matches_pillow(RSPIL):
    for size in (None, 12):
        assert _font_observation(RSPIL.ImageFont.load_default(size)) == (
            _font_observation(PILImageFont.load_default(size))
        )


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
