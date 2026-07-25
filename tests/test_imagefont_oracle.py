"""Exact shared Pillow ImageFont oracle checks for the installed Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import ImageFont


ORACLE_PATH = (
    Path(__file__).parent.parent
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "imagefont"
    / "getmask2.json"
)


@pytest.mark.covers("ImageFont.FreeTypeFont.getname")
def test_default_font_name_matches_shared_pillow_oracle():
    """Family and style must come directly from Rust without Python repair."""
    manifest = json.loads(ORACLE_PATH.read_text())
    assert manifest["oracle"] == {
        "implementation": "Pillow",
        "version": "12.2.0",
        "freetype_version": "2.14.3",
    }

    font = ImageFont.load_default()
    assert font.getname() == tuple(manifest["font"]["expected_name"])
