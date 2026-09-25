"""Compare font-variant source and mutable-state behavior with isolated Pillow.

Run after ``make build-parity``. No coverage or benchmark timing is collected.
"""
from __future__ import annotations

import hashlib
from io import BytesIO
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "pillow-rs-py/python"
ASSETS = ROOT / "pillow-rs/tests/fixtures/assets/font/fonts"


def observations():
    from PIL import ImageFont

    source = (ASSETS / "render-coverage.ttf").read_bytes()
    alternate = (ASSETS / "fixed-width.ttf").read_bytes()
    records = {}

    def capture(name, create):
        try:
            font = create()
            mask = font.getmask("Abc")
            records[name] = {
                "size": font.size, "name": font.getname(),
                "metrics": font.getmetrics(), "bbox": font.getbbox("Abc"),
                "length": font.getlength("Abc"), "mask_size": mask.size,
                "mask_sha256": hashlib.sha256(bytes(mask)).hexdigest(),
                "has_font_bytes": hasattr(font, "font_bytes"),
                "path_type": type(font.path).__name__,
            }
        except Exception as error:
            records[name] = {"error": type(error).__name__, "message": str(error)}

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "font.ttf"
        path.write_bytes(source)
        font = ImageFont.truetype(str(path), 20)
        capture("path_default", font.font_variant)
        font.size = 31
        capture("changed_size", font.font_variant)
        font.size = 0
        capture("changed_size_zero", font.font_variant)
        font.size = 20
        font.path = str(Path(directory) / "missing.ttf")
        capture("changed_path", font.font_variant)
        font.path = str(path)
        path.write_bytes(alternate)
        capture("replaced_path_contents", font.font_variant)
        path.unlink()
        capture("deleted_path", font.font_variant)
        capture("explicit_memory_override", lambda: font.font_variant(font=BytesIO(source)))
        path.write_bytes(source)
        path_font = ImageFont.truetype(path, 20)
        capture("pathlike_source", path_font.font_variant)

    stream = BytesIO(source)
    font = ImageFont.truetype(stream, 20)
    stream.close()
    capture("closed_memory_source", font.font_variant)
    font.size = 31
    capture("memory_changed_size", font.font_variant)
    font.path = "missing-font-source"
    capture("memory_ignores_path", font.font_variant)
    font.font_bytes = alternate
    capture("changed_public_source_bytes", font.font_variant)
    font.font_bytes = b""
    capture("empty_public_source_bytes", font.font_variant)
    font.font_bytes = b"invalid font dat"
    capture("short_public_source_bytes", font.font_variant)
    font.font_bytes = b"invalid font data"
    capture("invalid_public_source_bytes", font.font_variant)
    capture("default_font_variant", ImageFont.load_default().font_variant)

    independent_source = ImageFont.truetype(BytesIO(source), 20)
    independent_variant = independent_source.font_variant(size=31)
    capture("base_after_sized_variant", lambda: independent_source)
    capture("sized_variant_after_creation", lambda: independent_variant)
    return records


def run_side(target):
    env = os.environ.copy()
    paths = [p for p in env.get("PYTHONPATH", "").split(os.pathsep)
             if p and Path(p).resolve() != TARGET]
    if target:
        paths.insert(0, str(TARGET))
    env["PYTHONPATH"] = os.pathsep.join(paths)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    process = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--worker"],
        env=env, cwd=ROOT, capture_output=True, text=True, check=True, timeout=60,
    )
    result = json.loads(process.stdout)
    loaded_from_target = Path(result["module"]).is_relative_to(TARGET)
    if loaded_from_target != target:
        raise RuntimeError(f"wrong PIL import for {'target' if target else 'oracle'}: {result['module']}")
    return result["records"]


class FontVariantParityTests(unittest.TestCase):
    def test_source_lifetime_and_mutable_public_settings(self):
        expected, actual = run_side(False), run_side(True)
        self.assertEqual(set(expected), set(actual))
        for name in expected:
            with self.subTest(case=name):
                self.assertEqual(actual[name], expected[name])


if __name__ == "__main__":
    if sys.argv[1:] == ["--worker"]:
        import PIL
        print(json.dumps({"module": PIL.__file__, "records": observations()}, sort_keys=True))
    else:
        unittest.main()
