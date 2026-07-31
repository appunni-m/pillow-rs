"""Shared Pillow array-descriptor oracle through the installed Python ABI."""

import json
from pathlib import Path

import pytest

from pillow_rs import _core


ORACLE = json.loads(
    (
        Path(__file__).parent.parent
        / "pillow-rs"
        / "tests"
        / "fixtures"
        / "fromarray"
        / "descriptor.json"
    ).read_text()
)


@pytest.mark.covers("ImageModule.fromarray")
@pytest.mark.parametrize("case", ORACLE["cases"], ids=lambda case: case["id"])
def test_array_descriptor_resolution_matches_pillow(case):
    expected_error = case["expected"].get("error")
    if expected_error:
        with pytest.raises(Exception) as error:
            _core.resolve_array_layout(case["shape"], case["typestr"], case["mode"])
        assert type(error.value).__name__ == expected_error["type"]
        assert str(error.value) == expected_error["message"]
        return

    layout = _core.resolve_array_layout(case["shape"], case["typestr"], case["mode"])
    assert layout[0] == case["expected"]["mode"]
    assert list(layout[2:4]) == case["expected"]["size"]
