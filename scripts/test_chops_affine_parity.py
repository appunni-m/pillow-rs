"""Check Chops Python argument boundaries against isolated live Pillow.

Run after ``make build-parity``. Invalid literals cannot be represented by the
migration manifest's declared parameter value types, so retain them here.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "pillow-rs-py/python"


class IntegerOffset:
    def __index__(self):
        return 7


def observations(backend):
    from PIL import Image, ImageChops

    if backend != "pillow":
        import pillow_rs
        for name in ("cpu", "simd", "gpu"):
            pillow_rs.disable_backend(name)
        if not pillow_rs.enable_backend(backend):
            raise RuntimeError(f"backend unavailable: {backend}")
    records = {}
    offsets = (0.0, 0.5, None, "1", 2**31, -2**31 - 1,
               2**100, True, -2**31, 2**31 - 1, IntegerOffset())
    for operation in ("add", "subtract"):
        for index, offset in enumerate(offsets):
            try:
                result = getattr(ImageChops, operation)(
                    Image.new("L", (16, 1), 17), Image.new("L", (16, 1), 31),
                    offset=offset,
                )
                record = {"mode": result.mode, "size": result.size,
                          "bytes": result.tobytes().hex()}
            except Exception as error:
                record = {"error": type(error).__name__, "message": str(error)}
            records[f"{operation}-offset-{index}"] = record
    return records


def run_side(backend):
    env = os.environ.copy()
    paths = [p for p in env.get("PYTHONPATH", "").split(os.pathsep)
             if p and Path(p).resolve() != TARGET]
    if backend != "pillow":
        paths.insert(0, str(TARGET))
    env["PYTHONPATH"] = os.pathsep.join(paths)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    process = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--worker", backend],
        env=env, cwd=ROOT, capture_output=True, text=True, check=True, timeout=60,
    )
    result = json.loads(process.stdout)
    if Path(result["module"]).is_relative_to(TARGET) != (backend != "pillow"):
        raise RuntimeError(f"wrong PIL import for {backend}: {result['module']}")
    return result["records"]


class ChopsAffineParityTests(unittest.TestCase):
    def test_integer_offset_protocol_and_bounds(self):
        expected = run_side("pillow")
        for backend in ("cpu", "simd", "gpu"):
            actual = run_side(backend)
            self.assertEqual(set(actual), set(expected))
            for name in expected:
                with self.subTest(backend=backend, case=name):
                    self.assertEqual(actual[name], expected[name])


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--worker":
        import PIL
        print(json.dumps({"module": PIL.__file__, "records": observations(sys.argv[2])},
                         sort_keys=True))
    else:
        unittest.main()
