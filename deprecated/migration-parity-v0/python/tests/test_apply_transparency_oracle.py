"""Exact Pillow-oracle execution of Image.apply_transparency via Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import Image


FIXTURE_ROOT = (
    Path(__file__).parent.parent
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "image_backend"
)
MANIFEST = json.loads((FIXTURE_ROOT / "backend_parity.json").read_text())
CASES = MANIFEST["apply_transparency_cases"]


def _transparency_info(image):
    value = image.info.get("transparency")
    if value is None:
        return {}
    if isinstance(value, int):
        return {"transparency": {"kind": "index", "value": value}}
    return {"transparency": {"kind": "table", "value_hex": bytes(value).hex()}}


@pytest.mark.parametrize("case", CASES, ids=lambda case: case["id"])
@pytest.mark.covers("Image.apply_transparency")
def test_apply_transparency_matches_exact_pillow_fixture(case):
    assert MANIFEST["oracle"]["implementation"] == "Pillow"
    assert MANIFEST["oracle"]["version"] == "12.2.0"

    image = Image.open(FIXTURE_ROOT / case["input"])
    if case["prepare_alpha"] is not None:
        image.putalpha(case["prepare_alpha"])

    expected = case["expected"]
    assert _transparency_info(image) == expected["before_info"]
    assert image.palette.mode == expected["before_palette_mode"]
    assert image.has_transparency_data is expected["before_has_transparency_data"]

    assert image.apply_transparency() is None

    assert _transparency_info(image) == expected["info"]
    assert image.palette.mode == expected["palette_mode"]
    assert image.has_transparency_data is expected["has_transparency_data"]
    assert image.mode == expected["mode"]
    assert list(image.size) == expected["size"]
    assert image.tobytes().hex() == expected["pixels_hex"]
    assert bytes(image.getpalette("RGBA")).hex() == expected["palette_rgba_hex"]
