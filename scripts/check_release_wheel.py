#!/usr/bin/env python3
"""Install and exercise a release wheel outside the checkout and oracle env."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import venv

from check_docs_examples import fenced_examples


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--wheel-dir", type=Path)
    inputs.add_argument("--sdist", type=Path)
    args = parser.parse_args()
    if args.sdist:
        package = args.sdist.resolve()
        if not package.is_file() or not package.name.endswith(".tar.gz"):
            raise SystemExit("expected the source distribution archive")
    else:
        wheels = sorted(args.wheel_dir.resolve().glob("*.whl"))
        if len(wheels) != 1:
            raise SystemExit(f"expected exactly one wheel for this host, got {wheels}")
        package = wheels[0]
        if "-linux_" in package.name:
            raise SystemExit("generic Linux wheels are not accepted for PyPI; build a manylinux wheel")
    with tempfile.TemporaryDirectory(prefix="pillow-rs-wheel-consumer-") as directory:
        root = Path(directory)
        venv.EnvBuilder(with_pip=True).create(root / "venv")
        python = root / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run([str(python), "-m", "pip", "install", "--no-deps", str(package)], check=True, cwd=root)
        subprocess.run([str(python), "-I", "-c", '''
from importlib.metadata import version
from importlib.util import find_spec
from io import BytesIO
from pathlib import Path
import sys
from PIL import Image, ImageOps
import PIL
import pillow_rs._core
assert find_spec("RSPIL") is None, "retired Python namespace must not ship"
assert Path(PIL.__file__).resolve().is_relative_to(Path(sys.prefix).resolve())
im = Image.new("RGB", (3, 2), (255, 12, 34))
assert im.tobytes() == bytes([255, 12, 34]) * 6
out = BytesIO()
im.save(out, format="PNG")
out.seek(0)
loaded = Image.open(out)
assert loaded.size == (3, 2)
assert loaded.tobytes() == im.tobytes()
assert ImageOps.mirror(loaded).tobytes() == im.tobytes()
assert loaded.resize((6, 4)).size == (6, 4)
print("installed PIL replacement:", version("pillow-rs"), PIL.__file__)
'''], check=True, cwd=root)
        checkout = Path(__file__).resolve().parent.parent
        for source in ("README.md", "docs/PYTHON.md"):
            for index, example in enumerate(fenced_examples(checkout / source, "python")):
                subprocess.run([str(python), "-I", "-c", example], check=True, cwd=root)
                print(f"Executed installed-wheel example: {source}, block {index + 1}")


if __name__ == "__main__":
    main()
