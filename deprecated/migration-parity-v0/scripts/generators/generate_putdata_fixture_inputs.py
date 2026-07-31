#!/usr/bin/env python3
"""Generate the semantic ``Image.putdata`` fixture inputs.

The companion Pillow oracles are generated with::

    make putdata-fixtures

Use ``--check`` to verify that the committed input specs are reproducible
without changing them.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).parent.parent
MODE_ORDER = ("1", "CMYK", "F", "I", "L", "LA", "P", "PA", "RGB", "RGBA")

GENERATED_CASES = {
    0: [
        {
            "id": "Image_putdata_F",
            "mode": "F",
            "input": {
                "source": "constant",
                "size": [4, 1],
                "color": 0.0,
            },
            "params": {
                "data": [-1.25, 0.5, 128.75, 300.0],
                "scale": 1.5,
                "offset": -2.0,
            },
        },
        {
            "id": "Image_putdata_I",
            "mode": "I",
            "input": {
                "source": "constant",
                "size": [4, 1],
                "color": 0,
            },
            "params": {
                "data": [-2, 0, 127, 65536],
                "scale": 2.0,
                "offset": 3.0,
            },
        },
        {
            "id": "Image_putdata_PA",
            "mode": "PA",
            "input": {
                "source": "constant",
                "size": [4, 1],
                "color": [9, 240],
            },
            "params": {
                "data": [[1, 2], [255, 128], [17, 64], [200, 0]],
            },
        },
    ],
    1: [
        {
            "id": "Image_putdata_F_suite1",
            "mode": "F",
            "input": {
                "source": "constant",
                "size": [3, 2],
                "color": 2.5,
            },
            "params": {
                "data": [0.25, -4.0, 64.5],
                "scale": 0.5,
                "offset": 1.0,
            },
        },
        {
            "id": "Image_putdata_I_suite1",
            "mode": "I",
            "input": {
                "source": "constant",
                "size": [3, 2],
                "color": -7,
            },
            "params": {
                "data": [-2147483648, 0, 2147483647],
            },
        },
        {
            "id": "Image_putdata_PA_suite1",
            "mode": "PA",
            "input": {
                "source": "constant",
                "size": [3, 2],
                "color": [7, 192],
            },
            "params": {
                "data": [[0, 255], [127, 64], [255, 1]],
            },
        },
    ],
}

FIXTURE_PATHS = {
    0: ROOT / "tests" / "fixtures" / "input" / "jsons" / "Image.putdata.json",
    1: ROOT / "tests" / "fixtures_2" / "input" / "jsons" / "Image.putdata.json",
}


def render_fixture(path: Path, suite: int) -> str:
    """Return one fixture with its generated mode cases synchronized."""
    fixture = json.loads(path.read_text())
    expected_operation = {"module": "Image", "target": "putdata"}
    if fixture.get("operation") != expected_operation:
        raise ValueError(f"{path}: expected operation {expected_operation}")
    if fixture.get("suite", 0) != suite:
        raise ValueError(f"{path}: expected suite {suite}")

    generated_ids = {case["id"] for case in GENERATED_CASES[suite]}
    cases = [
        case for case in fixture.get("cases", [])
        if case.get("id") not in generated_ids
    ]
    cases.extend(GENERATED_CASES[suite])
    cases.sort(key=lambda case: MODE_ORDER.index(case["mode"]))
    fixture["cases"] = cases
    return json.dumps(fixture, indent=2) + "\n"


def main() -> None:
    """Write or verify the generated semantic input cases."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail when a committed input fixture is not reproducible.",
    )
    args = parser.parse_args()

    stale = []
    for suite, path in FIXTURE_PATHS.items():
        rendered = render_fixture(path, suite)
        if path.read_text() == rendered:
            continue
        if args.check:
            stale.append(path.relative_to(ROOT))
        else:
            path.write_text(rendered)
            print(f"updated {path.relative_to(ROOT)}")

    if stale:
        names = "\n".join(f"  {path}" for path in stale)
        raise SystemExit(f"putdata fixture inputs are stale:\n{names}")
    if args.check:
        print("putdata fixture inputs are reproducible")


if __name__ == "__main__":
    main()
