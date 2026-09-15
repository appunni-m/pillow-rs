#!/usr/bin/env python3
"""Stage only absent PyPI files and reject conflicting immutable artifacts."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import urllib.error
import urllib.request


def missing_files(archives: list[Path], metadata: dict | None) -> list[Path]:
    if not archives:
        raise ValueError("no Python release artifacts")
    published = {}
    for item in (metadata or {}).get("urls", []):
        name = item.get("filename")
        if not isinstance(name, str) or name in published:
            raise ValueError("invalid or duplicated PyPI file metadata")
        published[name] = item.get("digests", {}).get("sha256")
    missing = []
    for archive in archives:
        if not archive.is_file() or archive.is_symlink():
            raise ValueError(f"not a regular release file: {archive}")
        if archive.name not in published:
            missing.append(archive)
        elif hashlib.sha256(archive.read_bytes()).hexdigest() != published[archive.name]:
            raise ValueError(f"PyPI artifact differs from the verified bundle: {archive.name}")
    return missing


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--archive-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if not args.version or any(c not in "0123456789." for c in args.version):
        raise SystemExit("expected a numeric release version")
    archives = sorted(args.archive_dir.glob("*.whl")) + sorted(args.archive_dir.glob("*.tar.gz"))
    for archive in archives:
        if not archive.name.startswith((f"pillow_rs-{args.version}-", f"pillow_rs-{args.version}.tar.gz", f"pillow-rs-{args.version}.tar.gz")):
            raise SystemExit(f"unexpected package/version: {archive.name}")
    try:
        with urllib.request.urlopen(f"https://pypi.org/pypi/pillow-rs/{args.version}/json", timeout=30) as response:
            metadata = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        metadata = None
    selected = missing_files(archives, metadata)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise SystemExit("PyPI staging directory must be empty")
    for archive in selected:
        shutil.copyfile(archive, args.output_dir / archive.name)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"missing={'true' if selected else 'false'}\n")
    print(f"PyPI: {len(archives) - len(selected)} identical files already published; {len(selected)} files staged")


if __name__ == "__main__":
    main()
