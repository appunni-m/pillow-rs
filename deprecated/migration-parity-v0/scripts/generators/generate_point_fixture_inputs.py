#!/usr/bin/env python3
"""Generate independent-path Image.point fixture inputs.

Expected results remain owned by ``scripts/generate_fixtures.py`` running
against the pinned Pillow oracle. This generator owns inputs only.
"""

import argparse
import json
from pathlib import Path


MODES = ("LA", "RGB", "RGBA", "1", "L", "P")
SUITE_ONE_INPUTS = {
    "LA": {"reference": "ref_city", "size": [32, 32]},
    "RGB": {"reference": "ref_people", "size": [80, 80]},
    "RGBA": {"reference": "ref_animal", "size": [64, 64]},
    "1": {"reference": "ref_flower", "size": [128, 128]},
    "L": {"reference": "ref_mountain", "size": [64, 128]},
    "P": {"reference": "ref_nature", "size": [128, 64]},
}


def case_id(mode: str, suite: int) -> str:
    base = f"Image.point_{mode}" if mode in {"LA", "RGB", "RGBA"} else f"Image_point_{mode}"
    return f"{base}_suite{suite}" if suite else base


def build_cases(suite: int) -> list[dict]:
    identity = list(range(256))
    cases = []
    for mode in MODES:
        image_input = {
            "source": "reference_rgb",
            **(SUITE_ONE_INPUTS[mode] if suite else {"size": [256, 256]}),
        }
        if suite:
            case = {
                "id": case_id(mode, suite),
                "params": {"lut": identity},
                "mode": mode,
                "input": image_input,
            }
        else:
            case = {
                "id": case_id(mode, suite),
                "mode": mode,
                "input": image_input,
                "params": {"lut": identity},
            }
        cases.append(case)
    callable_id = "Image_point_callable_RGB"
    if suite:
        callable_id += f"_suite{suite}"
    cases.append(
        {
            "id": callable_id,
            "params": {"function": "invert"},
            "mode": "RGB",
            "input": {
                "source": "reference_rgb",
                **(
                    {"reference": "ref_water", "size": [96, 64]}
                    if suite
                    else {"size": [256, 256]}
                ),
            },
        }
    )
    return cases


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixtures-dir", type=Path, required=True)
    parser.add_argument("--suite", type=int, required=True)
    args = parser.parse_args()

    payload = {"format_version": 2}
    if args.suite:
        payload["suite"] = args.suite
    payload.update(
        {
            "operation": {"module": "Image", "target": "point"},
            "cases": build_cases(args.suite),
        }
    )
    path = args.fixtures_dir / "input" / "jsons" / "Image.point.json"
    path.write_text(json.dumps(payload, indent=2) + "\n")


if __name__ == "__main__":
    main()
