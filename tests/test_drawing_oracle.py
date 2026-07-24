"""Exact Pillow-oracle execution of ImageDraw through the installed Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import Image, ImageDraw


FIXTURE = (
    Path(__file__).parent.parent
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "image_backend"
    / "backend_parity.json"
)
MANIFEST = json.loads(FIXTURE.read_text())
CASES = [
    pytest.param(
        case,
        id=case["id"],
        marks=pytest.mark.covers(f"ImageDraw.{case['operation']}"),
    )
    for case in MANIFEST["draw_cases"]
]


def _image(spec):
    image = Image.frombytes(
        "LA" if spec["mode"] == "PA" else spec["mode"],
        tuple(spec["size"]),
        bytes.fromhex(spec["pixels_hex"]),
    )
    if spec["palette_hex"] is not None:
        image.putpalette(bytes.fromhex(spec["palette_hex"]), "RGB")
    return image


def _parameters(case):
    parameters = dict(case["parameters"])
    xy = parameters["xy"]
    parameters["xy"] = (
        [tuple(point) for point in xy]
        if xy and isinstance(xy[0], list)
        else tuple(xy)
    )
    for name in ("fill", "outline"):
        if isinstance(parameters.get(name), list):
            parameters[name] = tuple(parameters[name])
    return parameters


@pytest.mark.parametrize("case", CASES)
def test_drawing_matches_exact_pillow_fixture(case):
    assert MANIFEST["oracle"]["implementation"] == "Pillow"
    assert MANIFEST["oracle"]["version"] == "12.2.0"
    assert case["backends"] == ["cpu"]
    assert case["unsupported_backends"] == ["simd", "gpu"]

    image = _image(case["source"])
    draw = ImageDraw.Draw(image)
    getattr(draw, case["operation"])(**_parameters(case))

    expected = case["expected"]
    assert image.mode == expected["mode"], f"{case['id']}: mode"
    assert list(image.size) == expected["size"], f"{case['id']}: size"
    assert image.tobytes().hex() == expected["pixels_hex"], f"{case['id']}: pixels"
    if expected["palette_hex"] is not None:
        assert (
            bytes(image.getpalette()).hex() == expected["palette_hex"]
        ), f"{case['id']}: palette"
