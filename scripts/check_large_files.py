#!/usr/bin/env python3
"""Reject newly staged or committed files above the repository size limit."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
APPROVALS_FILE = ROOT / "scripts/check_large_files.py"
LIMIT_BYTES = 1 * 1024 * 1024

# Reviewed exact-path approvals: path -> (maximum bytes, reason).
APPROVED_FILES: dict[str, tuple[int, str]] = {
    "scripts/build_migration_parity_inputs.py": (
        2_500_000,
        "Maintained deterministic generator for the fixed parity and benchmark inputs.",
    ),
    "docs/PERFORMANCE_CAMPAIGN.md": (
        2_500_000,
        "Maintained campaign decisions and handoff history; keep its size capped.",
    ),
    "pillow-rs/tests/fixtures/inputs/benchmark/pipeline-operations.json": (
        8_500_000,
        "Fixed public benchmark workload corpus required by the benchmark runner.",
    ),
}


def git(*args: str) -> bytes:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(detail or f"git {' '.join(args)} failed")
    return result.stdout


def changed_staged_blobs() -> list[tuple[str, str]]:
    changed = set(
        path.decode("utf-8", errors="surrogateescape")
        for path in git(
            "diff", "--cached", "--name-only", "--diff-filter=ACMRT", "--no-renames", "-z"
        ).split(b"\0")
        if path
    )
    if not changed:
        return []

    blobs: dict[str, str] = {}
    for entry in git("ls-files", "--stage", "-z").split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", maxsplit=1)
        mode, object_id, stage = metadata.split()
        if stage == b"0" and mode != b"160000":
            path = raw_path.decode("utf-8", errors="surrogateescape")
            blobs[path] = object_id.decode("ascii")
    return [(path, blobs[path]) for path in sorted(changed) if path in blobs]


def changed_revision_blobs(revision: str) -> list[tuple[str, str]]:
    changed = [
        path.decode("utf-8", errors="surrogateescape")
        for path in git(
            "diff", "--name-only", "--diff-filter=ACMRT", "--no-renames", "-z", revision
        ).split(b"\0")
        if path
    ]
    endpoint = revision.rsplit("..", maxsplit=1)[-1]
    return [(path, f"{endpoint}:{path}") for path in sorted(set(changed))]


def blob_size(object_spec: str) -> int | None:
    result = subprocess.run(
        ["git", "cat-file", "-s", object_spec],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        return None
    return int(result.stdout.strip())


def check(changes: list[tuple[str, str]], approved: dict[str, tuple[int, str]]) -> int:
    rejected: list[tuple[str, int, str]] = []
    for path, object_spec in changes:
        size = blob_size(object_spec)
        if size is None or size < LIMIT_BYTES:
            continue
        approval = approved.get(path)
        if approval is None:
            rejected.append((path, size, "no exact-path approval"))
        elif size > approval[0]:
            rejected.append((path, size, f"exceeds approved ceiling of {approval[0]} bytes"))

    if not rejected:
        print(f"Large-file check passed ({len(changes)} changed paths checked; limit {LIMIT_BYTES} bytes).")
        return 0

    print("Large-file check failed. Files at or above 1 MiB need an exact-path approval:", file=sys.stderr)
    for path, size, reason in rejected:
        print(f"  {size:>12,} bytes  {path} ({reason})", file=sys.stderr)
    print(
        f"Review the file, then add its path, maximum byte size, and reason to {APPROVALS_FILE.relative_to(ROOT)}.",
        file=sys.stderr,
    )
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--staged", action="store_true", help="check added/modified files in the Git index")
    mode.add_argument("--revision", help="check added/modified files in a Git diff range, e.g. HEAD^1..HEAD")
    args = parser.parse_args()

    try:
        if args.revision:
            changes = changed_revision_blobs(args.revision)
        else:
            changes = changed_staged_blobs()
        return check(changes, APPROVED_FILES)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Large-file check could not run: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
