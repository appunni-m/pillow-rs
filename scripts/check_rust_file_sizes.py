#!/usr/bin/env python3
"""Enforce the first-party Rust file-size ratchet."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASELINE_PATH = ROOT / "scripts" / "rust_size_baseline.json"
PACKAGES = ("pillow-rs", "pillow-rs-py", "pillow-rs-js")
EXCLUDED_PARTS = {".git", "target", "build", "vendor", "node_modules", "generated"}
FILE_LIMIT = 1_000
FILE_REVIEW_LINE = 800


def rust_sources() -> list[Path]:
    paths: list[Path] = []
    for package in PACKAGES:
        package_root = ROOT / package
        if not package_root.is_dir():
            continue
        for path in package_root.rglob("*.rs"):
            relative = path.relative_to(ROOT)
            if any(part in EXCLUDED_PARTS for part in relative.parts):
                continue
            paths.append(path)
    return sorted(set(paths))


def physical_line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").splitlines())


def load_baseline() -> dict[str, object]:
    if not BASELINE_PATH.exists():
        return {"format_version": 1, "file_lines": {}, "function_lines": []}
    data = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    if data.get("format_version") != 1:
        raise ValueError(f"unsupported baseline format in {BASELINE_PATH}")
    return data


def write_baseline(data: dict[str, object]) -> None:
    BASELINE_PATH.write_text(
        json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="record current over-limit files as reviewed legacy entries",
    )
    args = parser.parse_args()

    try:
        baseline = load_baseline()
        counts = {
            path.relative_to(ROOT).as_posix(): physical_line_count(path)
            for path in rust_sources()
        }
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Rust file-size check could not read its inputs: {error}", file=sys.stderr)
        return 2

    if args.update_baseline:
        baseline["file_lines"] = {
            path: count for path, count in sorted(counts.items()) if count > FILE_LIMIT
        }
        write_baseline(baseline)
        print(
            f"Recorded {len(baseline['file_lines'])} current Rust files over "
            f"{FILE_LIMIT} lines in {BASELINE_PATH.relative_to(ROOT)}."
        )
        return 0

    file_baseline = baseline.get("file_lines", {})
    if not isinstance(file_baseline, dict):
        print("file_lines baseline must be a JSON object", file=sys.stderr)
        return 2

    errors: list[str] = []
    reports: list[str] = []
    legacy_count = 0
    review_count = 0

    for path, count in sorted(counts.items()):
        if count <= FILE_LIMIT:
            if path in file_baseline:
                errors.append(
                    f"stale baseline entry for {path}: it is now at {count} lines "
                    f"(limit {FILE_LIMIT}); run checkstyle-update-baseline to retire it"
                )
            if count >= FILE_REVIEW_LINE:
                review_count += 1
                reports.append(f"review: {path} has {count} lines (review at {FILE_REVIEW_LINE})")
            continue

        previous = file_baseline.get(path)
        if previous is None:
            errors.append(f"new over-limit file: {path} has {count} lines (limit {FILE_LIMIT})")
            reports.append(f"new violation: {path} has {count} lines (limit {FILE_LIMIT})")
        elif not isinstance(previous, int):
            errors.append(f"invalid baseline entry for {path}: expected an integer line count")
        elif count > previous:
            errors.append(
                f"oversized file grew: {path} has {count} lines, baseline is {previous}"
            )
            reports.append(f"growth: {path} has {count} lines (baseline {previous})")
        else:
            legacy_count += 1
            reports.append(
                f"baseline: {path} has {count} lines (limit {FILE_LIMIT}; ceiling {previous})"
            )

    for path in sorted(set(file_baseline) - set(counts)):
        errors.append(
            f"stale baseline entry for removed source {path}; "
            "run checkstyle-update-baseline to retire it"
        )

    print(
        f"Rust file-size check: {len(counts)} files; {legacy_count} legacy over-limit "
        f"files are at or below baseline; {review_count} files are at least "
        f"{FILE_REVIEW_LINE} lines."
    )
    for report in reports:
        print(f"  {report}")
    if errors:
        print("Rust file-size ratchet violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
