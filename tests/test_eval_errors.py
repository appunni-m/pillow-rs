"""Exact fixture-backed Pillow Image.eval errors through the Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import Image


ORACLE = json.loads(
    (
        Path(__file__).parent.parent
        / "pillow-rs"
        / "tests"
        / "fixtures"
        / "image_eval"
        / "python_abi_errors.json"
    ).read_text()
)


@pytest.mark.covers("ImageModule.eval")
@pytest.mark.parametrize(
    "case",
    ORACLE["cases"],
    ids=lambda case: case["id"],
)
def test_eval_argument_errors_match_pillow(case):
    image = Image.new("L", (1, 1))
    args = () if case["argument"] is None else (case["argument"],)
    with pytest.raises(Exception) as error:
        Image.eval(image, *args)
    assert type(error.value).__name__ == case["expected"]["type"]
    assert str(error.value) == case["expected"]["message"]
