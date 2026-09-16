#!/usr/bin/env python3
"""Require the unchanged project license in sources and release archives."""
from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def verify_archive_license(archive: Path, expected: bytes) -> None:
    if archive.suffix == ".whl":
        with zipfile.ZipFile(archive) as package:
            licenses = [package.read(name) for name in package.namelist()
                        if PurePosixPath(name).name == "LICENSE"]
    else:
        with tarfile.open(archive) as package:
            licenses = [package.extractfile(member).read() for member in package.getmembers()
                        if member.isfile() and PurePosixPath(member.name).name == "LICENSE"]
    if expected not in licenses:
        raise ValueError(f"{archive.name} does not contain the complete project LICENSE")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archives", nargs="*", type=Path)
    parser.add_argument("--crate", action="store_true")
    args = parser.parse_args()
    expected = (ROOT / "LICENSE").read_bytes()
    for directory in ("pillow-rs", "pillow-rs-py", "pillow-rs-js"):
        if (ROOT / directory / "LICENSE").read_bytes() != expected:
            raise ValueError(f"{directory}/LICENSE differs from the project LICENSE")
    if args.crate:
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=ROOT, text=True))
        core = next(package for package in metadata["packages"] if package["name"] == "pillow-rs")
        args.archives.append(Path(metadata["target_directory"]) / "package"
                             / f"pillow-rs-{core['version']}.crate")
    for archive in args.archives:
        verify_archive_license(archive, expected)
    print(f"Project license verified in all source packages and {len(args.archives)} archives")


if __name__ == "__main__":
    main()
