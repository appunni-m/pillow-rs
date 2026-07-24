#!/usr/bin/env python3
"""Generate independent Pillow ImageFont.getmask2 fixture inputs."""

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def write_suite(relative_dir, suite, cases):
    path = ROOT / relative_dir / "input/jsons/ImageFont.getmask2.json"
    document = {
        "format_version": 2,
        **({"suite": suite} if suite else {}),
        "operation": {"module": "ImageFont", "target": "getmask2"},
        "cases": cases,
    }
    path.write_text(json.dumps(document, indent=2) + "\n")


write_suite(
    "tests/fixtures",
    0,
    [
        {"id": "baseline", "params": {"text": "Hello"}},
        {
            "id": "integer_start",
            "params": {"text": "Hello", "start": [3, 4]},
        },
        {
            "id": "fractional_start",
            "params": {"text": "Hello", "start": [0.5, 0.75]},
        },
    ],
)
write_suite(
    "tests/fixtures_2",
    1,
    [
        {"id": "empty_suite1", "params": {"text": ""}},
        {"id": "unicode_suite1", "params": {"text": "é"}},
    ],
)
