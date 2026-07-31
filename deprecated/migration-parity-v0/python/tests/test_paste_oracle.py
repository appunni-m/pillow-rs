"""Exact Pillow-oracle execution of Image.paste through the Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import Image


FIXTURE = (
    Path(__file__).parent.parent
    / "pillow-rs"
    / "tests"
    / "fixtures"
    / "image_backend"
    / "backend_parity.json"
)
MANIFEST = json.loads(FIXTURE.read_text())


def _image(spec):
    image = Image.frombytes(
        "LA" if spec["mode"] == "PA" else spec["mode"],
        tuple(spec["size"]),
        bytes.fromhex(spec["pixels_hex"]),
    )
    if spec["palette_hex"] is not None:
        image.putpalette(bytes.fromhex(spec["palette_hex"]), "RGB")
    return image


def _source(spec):
    if spec["kind"] == "image":
        return _image(spec["image"])
    if spec["kind"] == "scalar":
        return spec["value"]
    if spec["kind"] == "tuple":
        return tuple(spec["value"])
    raise AssertionError(f"unsupported paste source {spec['kind']}")


def _assert_image(case_id, actual, expected):
    assert actual.mode == expected["mode"], f"{case_id}: mode"
    assert list(actual.size) == expected["size"], f"{case_id}: size"
    assert actual.tobytes().hex() == expected["pixels_hex"], f"{case_id}: pixels"
    if expected["palette_hex"] is not None:
        assert (
            bytes(actual.getpalette()).hex() == expected["palette_hex"]
        ), f"{case_id}: palette"


@pytest.mark.parametrize("case", MANIFEST["paste_cases"], ids=lambda case: case["id"])
@pytest.mark.covers("Image.paste")
def test_paste_matches_exact_pillow_fixture(case):
    assert MANIFEST["oracle"]["implementation"] == "Pillow"
    assert MANIFEST["oracle"]["version"] == "12.2.0"
    destination = _image(case["destination"])
    mask = _image(case["mask"]) if case["mask"] is not None else None
    destination.paste(_source(case["source"]), tuple(case["box"]), mask)
    _assert_image(case["id"], destination, case["expected"])


@pytest.mark.parametrize(
    "case", MANIFEST["paste_error_cases"], ids=lambda case: case["id"]
)
@pytest.mark.covers("Image.paste")
def test_paste_matches_exact_pillow_error(case):
    destination = _image(case["destination"])
    mask = _image(case["mask"]) if case["mask"] is not None else None
    error_type = {"TypeError": TypeError, "ValueError": ValueError}[
        case["expected_error"]["type"]
    ]
    with pytest.raises(error_type) as caught:
        destination.paste(
            _source(case["source"]),
            None if case["box"] is None else tuple(case["box"]),
            mask,
        )
    assert str(caught.value) == case["expected_error"]["message"]
