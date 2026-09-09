#!/usr/bin/env python3
"""Verify the file-backed local first-release bundle.

The local bundle is intentionally outside the tracked source tree. This
checker keeps its artifact list and checksum receipts reviewable without
contacting a package registry or a remote Git host.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from pathlib import Path


PACKAGE_FILES = (
    "image-slash-star/image-slash-star-0.1.0.crate",
    "fontdone/fontdone-2.14.3-alpha.1.crate",
    "fontdone/fontdone-c-abi-2.14.3-alpha.1.crate",
    "fontdone/fontdone-wasm-2.14.3-alpha.1.crate",
    "fontdone/fontdone-2.14.3-alpha.1.tgz",
    "pillow-rs/pillow-rs-0.1.0.crate",
    "pillow-rs/python/pillow_rs-0.1.0-cp38-abi3-macosx_11_0_arm64.whl",
    "pillow-rs/npm/pillow-rs-0.1.0.tgz",
)
GIT_BUNDLES = (
    "git/fontdone-v2.14.3-alpha.1.bundle",
    "git/image-slash-star-v0.1.0.bundle",
    "git/pillow-rs-v0.1.0.bundle",
)
RECEIPT_FILES = (
    "LOCAL_RELEASE_RECEIPT.txt",
    "README.md",
    "release-manifest.txt",
    "cargo-registry-config.toml",
)


def _checksum_file(path: Path, expected: str) -> None:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    actual = digest.hexdigest()
    if actual != expected:
        raise ValueError(f"checksum mismatch for {path}: {actual} != {expected}")


def _verify_checksums(bundle: Path, checksum_name: str, relative_dir: Path) -> int:
    checksum_path = bundle / relative_dir / checksum_name
    if not checksum_path.is_file():
        raise ValueError(f"missing checksum manifest: {checksum_path}")
    checked = 0
    seen: set[str] = set()
    for raw_line in checksum_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line:
            continue
        try:
            expected, relative_name = line.split(maxsplit=1)
        except ValueError as exc:
            raise ValueError(f"malformed checksum line in {checksum_path}: {line!r}") from exc
        if relative_name.startswith("./"):
            relative_name = relative_name[2:]
        if relative_name in seen:
            raise ValueError(f"duplicate checksum entry in {checksum_path}: {relative_name}")
        seen.add(relative_name)
        artifact = bundle / relative_dir / relative_name
        if not artifact.is_file():
            raise ValueError(f"checksum entry points to missing file: {artifact}")
        if len(expected) != hashlib.sha256().digest_size * 2:
            raise ValueError(f"invalid SHA-256 digest in {checksum_path}: {expected!r}")
        _checksum_file(artifact, expected)
        checked += 1
    if not checked:
        raise ValueError(f"checksum manifest is empty: {checksum_path}")
    return checked


def _verify_bundle(path: Path) -> None:
    try:
        result = subprocess.run(
            ["git", "bundle", "verify", str(path)],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        raise ValueError("git is required to verify release bundles") from exc
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise ValueError(f"git bundle verification failed for {path}: {detail}")


def verify(bundle: Path) -> tuple[int, int, int]:
    if not bundle.is_dir():
        raise ValueError(f"local release bundle does not exist: {bundle}")

    for relative_name in RECEIPT_FILES:
        path = bundle / relative_name
        if not path.is_file():
            raise ValueError(f"missing release receipt file: {path}")

    manifest = (bundle / "release-manifest.txt").read_text(encoding="utf-8")
    for relative_name in (*PACKAGE_FILES, *GIT_BUNDLES):
        if relative_name not in manifest:
            raise ValueError(f"release manifest does not list {relative_name}")

    package_count = _verify_checksums(bundle, "SHA256SUMS", Path("."))
    benchmark_count = _verify_checksums(bundle, "SHA256SUMS", Path("benchmarks"))

    for relative_name in (*PACKAGE_FILES, *GIT_BUNDLES):
        path = bundle / relative_name
        if not path.is_file():
            raise ValueError(f"release artifact is missing: {path}")

    for relative_name in GIT_BUNDLES:
        _verify_bundle(bundle / relative_name)

    receipt = (bundle / "LOCAL_RELEASE_RECEIPT.txt").read_text(encoding="utf-8")
    if "local-only" not in receipt.lower():
        raise ValueError("local release receipt does not identify the bundle as local-only")
    return package_count, len(GIT_BUNDLES), benchmark_count


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bundle-dir",
        type=Path,
        default=Path("dist/release-local"),
        help="path to the local first-release bundle (default: dist/release-local)",
    )
    args = parser.parse_args()
    try:
        package_count, bundle_count, benchmark_count = verify(args.bundle_dir)
    except ValueError as exc:
        print(f"local release bundle check failed: {exc}", file=sys.stderr)
        return 1
    print(
        "local release bundle OK: "
        f"{package_count} artifact checksum entries, {bundle_count} Git bundles, "
        f"{benchmark_count} benchmark entries"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
