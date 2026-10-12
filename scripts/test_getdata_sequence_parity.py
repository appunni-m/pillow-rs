"""Check Image.getdata sequence edge behavior against live Pillow."""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TARGET_PYTHON = ROOT / "pillow-rs-py" / "python"

_PROBE = r'''
import json
import warnings
warnings.filterwarnings("ignore", category=DeprecationWarning)
from PIL import Image

image = Image.new("RGB", (2, 1))
image.putdata([(10, 20, 30), (40, 50, 60)])
data = image.getdata()
banded = image.getdata(0)

def observe_from(sequence, index):
    try:
        return {"value": sequence[index]}
    except Exception as error:
        return {"error": type(error).__name__, "message": str(error)}

def observe(index):
    return observe_from(data, index)

def observe_banded_mode(mode, color, band):
    image = Image.new(mode, (2, 1), color)
    if mode == "P":
        image.putpalette([((index * 17) + 9) % 256 for index in range(768)])
    values = image.getdata(band)
    return {
        "image_mode": image.mode,
        "mode": values.mode,
        "size": values.size,
        "values": list(values),
    }

def observe_paletted_snapshot_after_mutation():
    image = Image.new("P", (2, 1), 23)
    image.putpalette([((index * 17) + 9) % 256 for index in range(768)])
    values = image.getdata(0)
    image.putpixel((0, 0), 91)
    return {"retained": list(values), "current": list(image.getdata(0))}

print(json.dumps({
    "first": observe(0),
    "last": observe(-1),
    "past_end": observe(2),
    "float": observe(1.5),
    "slice": observe(slice(None)),
    "banded_first": observe_from(banded, 0),
    "banded_past_end": observe_from(banded, 2),
    "banded_slice": observe_from(banded, slice(None)),
    "banded_modes": [
        observe_banded_mode("1", 1, 0),
        observe_banded_mode("L", 137, 0),
        observe_banded_mode("P", 23, 0),
        observe_banded_mode("LA", (73, 209), 1),
        observe_banded_mode("RGB", (31, 109, 197), 2),
    ],
    "paletted_snapshot": observe_paletted_snapshot_after_mutation(),
}))
'''


def observe(side):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if side == "target":
        env["PYTHONPATH"] = str(TARGET_PYTHON)
    result = subprocess.run(
        [sys.executable, "-c", _PROBE],
        cwd=ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


class GetdataSequenceParityTests(unittest.TestCase):
    def test_index_and_slice_behavior_matches_live_pillow(self):
        pillow = observe("pillow")
        target = observe("target")
        self.assertEqual(target, pillow)
        self.assertEqual(
            pillow["slice"],
            {
                "error": "TypeError",
                "message": "sequence index must be integer, not 'slice'",
            },
        )
        self.assertEqual(
            pillow["past_end"],
            {"error": "IndexError", "message": "image index out of range"},
        )
        self.assertEqual(pillow["banded_first"], {"value": 10})
        self.assertEqual(
            [item["mode"] for item in pillow["banded_modes"]],
            ["1", "L", "P", "L", "L"],
        )
        self.assertEqual(pillow["banded_modes"][2]["values"], [23, 23])
        self.assertEqual(
            pillow["paletted_snapshot"],
            {"retained": [23, 23], "current": [91, 23]},
        )
        self.assertEqual(
            pillow["banded_past_end"],
            {"error": "IndexError", "message": "image index out of range"},
        )
        self.assertEqual(
            pillow["banded_slice"],
            {
                "error": "TypeError",
                "message": "sequence index must be integer, not 'slice'",
            },
        )


if __name__ == "__main__":
    unittest.main()
