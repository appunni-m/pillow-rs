"""Exact shared Pillow TransposedFont checks through the installed Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import Image, ImageFont


ORACLE_PATH = (
    Path(__file__).parent.parent
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "imagefont"
    / "transposed_font.json"
)
MANIFEST = json.loads(ORACLE_PATH.read_text())


def _font(case):
    orientation = (
        getattr(Image.Transpose, case["orientation"])
        if case["orientation"] is not None
        else None
    )
    return ImageFont.TransposedFont(ImageFont.load_default(), orientation)


@pytest.mark.covers("ImageFont.TransposedFont.getbbox")
@pytest.mark.parametrize("case", MANIFEST["cases"], ids=lambda case: case["id"])
def test_transposed_bbox_matches_shared_pillow_oracle(case):
    assert list(_font(case).getbbox(case["text"])) == case["expected"]["bbox"]


@pytest.mark.covers("ImageFont.TransposedFont.getmask")
@pytest.mark.parametrize("case", MANIFEST["cases"], ids=lambda case: case["id"])
def test_transposed_mask_matches_shared_pillow_oracle(case):
    mask = _font(case).getmask(case["text"])
    assert mask.mode == case["expected"]["mask"]["mode"]
    assert list(mask.size) == case["expected"]["mask"]["size"]
    assert bytes(mask).hex() == case["expected"]["mask"]["pixels_hex"]


@pytest.mark.covers("ImageFont.TransposedFont.getlength")
@pytest.mark.parametrize("case", MANIFEST["cases"], ids=lambda case: case["id"])
def test_transposed_length_matches_shared_pillow_oracle(case):
    expected = case["expected"]
    if "length_error" in expected:
        with pytest.raises(ValueError) as error:
            _font(case).getlength(case["text"])
        assert type(error.value).__name__ == expected["length_error"]["type"]
        assert str(error.value) == expected["length_error"]["message"]
    else:
        assert _font(case).getlength(case["text"]) == expected["length"]
