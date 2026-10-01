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

def observe(index):
    try:
        return {"value": data[index]}
    except Exception as error:
        return {"error": type(error).__name__, "message": str(error)}

print(json.dumps({
    "first": observe(0),
    "last": observe(-1),
    "past_end": observe(2),
    "float": observe(1.5),
    "slice": observe(slice(None)),
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


if __name__ == "__main__":
    unittest.main()
