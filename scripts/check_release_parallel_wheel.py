#!/usr/bin/env python3
"""Install both Python wheels in isolation and verify extension selection."""
from __future__ import annotations

import argparse
from email.parser import BytesParser
import os
from pathlib import Path
import re
import subprocess
import tempfile
import venv
import zipfile

from check_release_licenses import ROOT, verify_archive_license
from release_versions import python_version, runtime_version


def canonical_project_name(name: str) -> str:
    """Apply PyPI's case and separator normalization to a metadata name."""
    return re.sub(r"[-_.]+", "-", name).lower()


def validate_project_name(metadata, expected: str) -> None:
    actual = metadata.get("Name")
    if not isinstance(actual, str) or canonical_project_name(actual) != expected:
        raise ValueError(f"wheel project name {actual!r} does not match {expected!r}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-dir", type=Path, required=True)
    args = parser.parse_args()
    wheel_dir = args.wheel_dir.resolve()
    standard = sorted(wheel_dir.glob("pillow_rs-*.whl"))
    parallel = sorted(wheel_dir.glob("pillow_rs_parallel-*.whl"))
    if len(standard) != 1 or len(parallel) != 1:
        raise SystemExit(
            "expected one standard and one parallel wheel for this host; "
            f"standard={standard}, parallel={parallel}"
        )

    def metadata(wheel: Path):
        with zipfile.ZipFile(wheel) as archive:
            metadata_file = next(name for name in archive.namelist() if name.endswith(".dist-info/METADATA"))
            return BytesParser().parsebytes(archive.read(metadata_file))

    expected = runtime_version()
    normalized = python_version(expected)
    standard_metadata = metadata(standard[0])
    parallel_metadata = metadata(parallel[0])
    validate_project_name(standard_metadata, "pillow-rs")
    validate_project_name(parallel_metadata, "pillow-rs-parallel")
    if standard_metadata.get("Version") != normalized:
        raise SystemExit("standard wheel version does not match the Python package version")
    if standard_metadata.get_all("Provides-Extra") is None or "parallel" not in standard_metadata.get_all("Provides-Extra"):
        raise SystemExit("standard wheel does not advertise its parallel extra")
    if not any(
        requirement.startswith(f"pillow-rs-parallel=={normalized}")
        and "extra == 'parallel'" in requirement
        for requirement in standard_metadata.get_all("Requires-Dist", [])
    ):
        raise SystemExit("standard wheel extra does not require the matching companion package")
    if parallel_metadata.get("Version") != normalized:
        raise SystemExit("parallel wheel version does not match the standard package version")
    if not any(
        requirement.startswith(f"pillow-rs=={normalized}")
        for requirement in parallel_metadata.get_all("Requires-Dist", [])
    ):
        raise SystemExit("parallel wheel does not depend on the matching standard package")

    license_bytes = (ROOT / "LICENSE").read_bytes()
    for wheel in (*standard, *parallel):
        verify_archive_license(wheel, license_bytes)

    with tempfile.TemporaryDirectory(prefix="pillow-rs-parallel-wheel-consumer-") as directory:
        root = Path(directory)
        venv.EnvBuilder(with_pip=True).create(root / "venv")
        python = root / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")

        subprocess.run(
            [str(python), "-m", "pip", "install", "--no-deps", str(standard[0])],
            check=True,
            cwd=root,
        )
        subprocess.run(
            [str(python), "-I", "-c", '''
from importlib.util import find_spec
from pathlib import Path
import pillow_rs
assert pillow_rs.parallel_feature_enabled() is False
assert Path(pillow_rs._core.__file__).name.startswith("_core.")
assert find_spec("pillow_rs._core_parallel") is None
'''],
            check=True,
            cwd=root,
        )

        extra_root = root / "parallel-extra"
        extra_root.mkdir()
        venv.EnvBuilder(with_pip=True).create(extra_root / "venv")
        extra_python = extra_root / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run(
            [
                str(extra_python),
                "-m",
                "pip",
                "install",
                "--no-index",
                "--find-links",
                str(wheel_dir),
                f"pillow-rs[parallel]=={normalized}",
            ],
            check=True,
            cwd=extra_root,
        )
        subprocess.run(
            [str(extra_python), "-I", "-c", '''
import pillow_rs
assert pillow_rs.parallel_feature_enabled() is True
print("pip extra selected:", pillow_rs._core.__file__)
'''],
            check=True,
            cwd=extra_root,
        )

        subprocess.run(
            [str(python), "-m", "pip", "install", "--no-deps", str(parallel[0])],
            check=True,
            cwd=root,
        )
        subprocess.run(
            [str(python), "-I", "-c", '''
from importlib.metadata import version
from pathlib import Path
import sys
from PIL import Image
import pillow_rs
assert pillow_rs.parallel_feature_enabled() is True
assert Path(pillow_rs._core.__file__).name.startswith("_core_parallel.")
assert version("pillow-rs") == version("pillow-rs-parallel") == sys.argv[1]
assert pillow_rs._core.Image is not None
image = Image.new("L", (8, 4), 37)
assert image.tobytes() == bytes([37]) * 32
print("parallel extension selected:", pillow_rs._core.__file__)
''',
             python_version(expected)],
            check=True,
            cwd=root,
        )


if __name__ == "__main__":
    main()
