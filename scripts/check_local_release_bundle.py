#!/usr/bin/env python3
"""Verify the file-backed local first-release bundle.

The local bundle is intentionally outside the tracked source tree. This
checker keeps its artifact list and checksum receipts reviewable without
contacting a package registry or a remote Git host.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tarfile
from pathlib import Path


PACKAGE_FILES = (
    "image-slash-star/image-slash-star-0.1.0.crate",
    "fontdone/fontdone-2.14.3-alpha.2.crate",
    "fontdone/fontdone-c-abi-2.14.3-alpha.2-aarch64-apple-darwin.tar.gz",
    "fontdone/fontdone-2.14.3-alpha.2.tgz",
    "pillow-rs/pillow-rs-0.1.0.crate",
    "pillow-rs/python/pillow_rs-0.1.0-cp38-abi3-macosx_11_0_arm64.whl",
    "pillow-rs/npm/pillow-rs-0.1.0.tgz",
)
GIT_BUNDLES = (
    "git/fontdone-v2.14.3-alpha.2.bundle",
    "git/image-slash-star-v0.1.0.bundle",
    "git/pillow-rs-v0.1.0.bundle",
)
RECEIPT_FILES = (
    "LOCAL_RELEASE_RECEIPT.txt",
    "README.md",
    "release-manifest.txt",
    "cargo-registry-config.toml",
)
CARGO_PACKAGE_SURFACES = (
    (
        "image-slash-star",
        "0.1.0",
        "image-slash-star/image-slash-star-0.1.0.crate",
        "im/ag/image-slash-star",
    ),
    (
        "fontdone",
        "2.14.3-alpha.2",
        "fontdone/fontdone-2.14.3-alpha.2.crate",
        "fo/nt/fontdone",
    ),
    (
        "pillow-rs",
        "0.1.0",
        "pillow-rs/pillow-rs-0.1.0.crate",
        "pi/ll/pillow-rs",
    ),
)
INTERNAL_FONTDONE_PACKAGES = ("fontdone-c-abi", "fontdone-wasm")


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


def _release_revisions(manifest: str) -> dict[str, str]:
    revisions: dict[str, str] = {}
    pattern = re.compile(r"^([a-z0-9-]+): ([0-9a-f]{40})$", re.MULTILINE)
    for name, revision in pattern.findall(manifest):
        if name in revisions:
            raise ValueError(f"release manifest lists {name} more than once")
        revisions[name] = revision
    expected_names = {surface[0] for surface in CARGO_PACKAGE_SURFACES}
    if set(revisions) != expected_names:
        missing = sorted(expected_names - set(revisions))
        extra = sorted(set(revisions) - expected_names)
        raise ValueError(f"release manifest revision set mismatch: missing={missing}, extra={extra}")
    return revisions


def _archive_member_text(path: Path, suffix: str) -> str:
    try:
        with tarfile.open(path, mode="r:gz") as archive:
            members = [member for member in archive.getmembers() if member.name.endswith(suffix)]
            if len(members) != 1:
                raise ValueError(f"{path} must contain exactly one {suffix} member")
            stream = archive.extractfile(members[0])
            if stream is None:
                raise ValueError(f"cannot read {suffix} from {path}")
            return stream.read().decode("utf-8")
    except (OSError, tarfile.TarError, UnicodeDecodeError) as exc:
        raise ValueError(f"cannot inspect Cargo archive {path}: {exc}") from exc


def _archive_revision(path: Path) -> str:
    try:
        metadata = json.loads(_archive_member_text(path, "/.cargo_vcs_info.json"))
        revision = metadata["git"]["sha1"]
    except (KeyError, TypeError, json.JSONDecodeError) as exc:
        raise ValueError(f"Cargo archive has invalid VCS metadata: {path}") from exc
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError(f"Cargo archive has invalid VCS revision: {path}")
    return revision


def _archive_lock_checksums(path: Path, names: set[str]) -> dict[str, str]:
    lockfile = _archive_member_text(path, "/Cargo.lock")
    checksums: dict[str, str] = {}
    for block in lockfile.split("[[package]]"):
        name_match = re.search(r'^name = "([^"]+)"$', block, re.MULTILINE)
        if not name_match or name_match.group(1) not in names:
            continue
        checksum_match = re.search(r'^checksum = "([0-9a-f]{64})"$', block, re.MULTILINE)
        if checksum_match:
            checksums[name_match.group(1)] = checksum_match.group(1)
    missing = sorted(names - set(checksums))
    if missing:
        raise ValueError(f"Cargo archive lockfile has no registry checksum for {missing}: {path}")
    return checksums


def _verify_registry_index(
    bundle: Path,
    name: str,
    version: str,
    index_relative: str,
    expected_checksum: str,
) -> None:
    index_path = bundle / "cargo-registry" / "index" / index_relative
    if not index_path.is_file():
        raise ValueError(f"missing local registry index entry: {index_path}")
    matches = []
    for raw_line in index_path.read_text(encoding="utf-8").splitlines():
        try:
            entry = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"invalid JSON in local registry index: {index_path}") from exc
        if entry.get("name") == name and entry.get("vers") == version:
            matches.append(entry)
    if len(matches) != 1:
        raise ValueError(
            f"local registry index must contain one {name} {version} entry, found {len(matches)}"
        )
    if matches[0].get("cksum") != expected_checksum:
        raise ValueError(
            f"local registry checksum drift for {name} {version}: "
            f"{matches[0].get('cksum')} != {expected_checksum}"
        )


def _verify_cargo_surfaces(bundle: Path, manifest: str) -> None:
    revisions = _release_revisions(manifest)
    registry = bundle / "cargo-registry"
    # The C ABI and raw-WASM Cargo packages are build targets only.  Their
    # public surfaces are the native SDK archive and npm tarball; neither may
    # accidentally become a second Cargo registry release.
    for internal_name in INTERNAL_FONTDONE_PACKAGES:
        leaked_archives = sorted(registry.glob(f"{internal_name}-*.crate"))
        leaked_index = sorted(
            path
            for path in (registry / "index").rglob(internal_name)
            if path.is_file()
        )
        if leaked_archives or leaked_index:
            raise ValueError(
                f"internal fontdone package leaked into public Cargo registry: "
                f"{internal_name}"
            )
    digests: dict[str, str] = {}
    for name, version, relative_name, index_relative in CARGO_PACKAGE_SURFACES:
        public_archive = bundle / relative_name
        registry_archive = bundle / "cargo-registry" / f"{name}-{version}.crate"
        if not registry_archive.is_file():
            raise ValueError(f"missing local registry archive: {registry_archive}")
        if public_archive.read_bytes() != registry_archive.read_bytes():
            raise ValueError(f"public and local registry archives differ for {name} {version}")
        digest = hashlib.sha256(public_archive.read_bytes()).hexdigest()
        digests[name] = digest
        _verify_registry_index(bundle, name, version, index_relative, digest)
        actual_revision = _archive_revision(public_archive)
        if actual_revision != revisions[name]:
            raise ValueError(
                f"Cargo archive revision drift for {name}: {actual_revision} != {revisions[name]}"
            )

    lock_checksums = _archive_lock_checksums(
        bundle / "pillow-rs/pillow-rs-0.1.0.crate",
        {"fontdone", "image-slash-star"},
    )
    for name in ("fontdone", "image-slash-star"):
        if lock_checksums[name] != digests[name]:
            raise ValueError(
                f"pillow-rs Cargo.lock checksum drift for {name}: "
                f"{lock_checksums[name]} != {digests[name]}"
            )


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

    _verify_cargo_surfaces(bundle, manifest)

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
