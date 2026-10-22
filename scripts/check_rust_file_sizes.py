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
EXCLUDED_PARTS = {".git", "target", "vendor", "node_modules", "generated"}
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


def function_baseline_paths(entries: object) -> set[str]:
    if not isinstance(entries, list):
        raise ValueError("function_lines baseline must be a JSON array")
    paths: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise ValueError(f"function_lines entry {index} must be a JSON object")
        path = entry.get("path")
        if not isinstance(path, str) or not path:
            raise ValueError(f"function_lines entry {index} must include a path")
        paths.add(path)
    return paths


def legacy_owner_errors(owner_map: object, active_paths: set[str]) -> list[str]:
    if not isinstance(owner_map, dict):
        return ["legacy_owners must be a JSON object"]

    errors: list[str] = []
    mapped_paths = set(owner_map)
    missing = sorted(active_paths - mapped_paths)
    stale = sorted(mapped_paths - active_paths)
    if missing:
        errors.append(f"legacy_owners is missing active paths: {', '.join(missing)}")
    if stale:
        errors.append(
            "legacy_owners contains retired paths; run checkstyle-update-baseline: "
            + ", ".join(stale)
        )

    for path in sorted(active_paths & mapped_paths):
        record = owner_map[path]
        if not isinstance(record, dict):
            errors.append(f"legacy_owners entry for {path} must be a JSON object")
            continue
        for field in ("owner", "reduction_task", "reason"):
            value = record.get(field)
            if not isinstance(value, str) or not value.strip():
                errors.append(f"legacy_owners entry for {path} needs a nonempty {field}")
        task = record.get("reduction_task")
        if isinstance(task, str):
            repository, separator, issue = task.partition("#")
            if (
                not separator
                or not repository.strip()
                or not issue.strip()
                or "#" in issue
                or any(character.isspace() for character in task)
            ):
                errors.append(
                    f"legacy_owners entry for {path} needs a qualified Kata reduction_task"
                )
    return errors


def reconcile_legacy_owners(
    baseline: dict[str, object], active_paths: set[str]
) -> tuple[list[str], int]:
    owner_map = baseline.get("legacy_owners")
    if not isinstance(owner_map, dict):
        return ["legacy_owners must be a JSON object"], 0

    active_map = {
        path: owner_map[path] for path in sorted(active_paths) if path in owner_map
    }
    errors = legacy_owner_errors(active_map, active_paths)
    if errors:
        return errors, 0
    retired_count = len(owner_map) - len(active_map)
    baseline["legacy_owners"] = active_map
    return [], retired_count


def file_ratchet_errors(
    counts: dict[str, int],
    file_baseline: dict[str, object],
    owner_map: object = None,
) -> list[str]:
    errors: list[str] = []
    for path, count in sorted(counts.items()):
        if count <= FILE_LIMIT:
            if path in file_baseline:
                errors.append(
                    f"stale baseline entry for {path}: it is now at {count} lines "
                    f"(limit {FILE_LIMIT}); run checkstyle-update-baseline to retire it"
                )
            continue

        previous = file_baseline.get(path)
        record = owner_map.get(path, {}) if isinstance(owner_map, dict) else {}
        if not isinstance(record, dict):
            record = {}
        owner = record.get("owner", "missing")
        task = record.get("reduction_task", "missing")
        if previous is None:
            errors.append(
                f"new over-limit file: {path} has {count} lines "
                f"(limit {FILE_LIMIT}; owner {owner}; task {task})"
            )
        elif not isinstance(previous, int):
            errors.append(f"invalid baseline entry for {path}: expected an integer line count")
        elif count > previous:
            errors.append(
                f"oversized file grew: {path} has {count} lines "
                f"(limit {FILE_LIMIT}; baseline {previous}; owner {owner}; task {task})"
            )

    for path in sorted(set(file_baseline) - set(counts)):
        errors.append(
            f"stale baseline entry for removed source {path}; "
            "run checkstyle-update-baseline to retire it"
        )
    return errors


def load_baseline() -> dict[str, object]:
    if not BASELINE_PATH.exists():
        return {
            "format_version": 1,
            "file_lines": {},
            "function_lines": [],
            "legacy_owners": {},
        }
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
        file_baseline = {
            path: count for path, count in sorted(counts.items()) if count > FILE_LIMIT
        }
        try:
            function_paths = function_baseline_paths(baseline.get("function_lines", []))
        except ValueError as error:
            print(f"Rust size baseline is invalid: {error}", file=sys.stderr)
            return 2
        active_paths = set(file_baseline) | function_paths
        owner_errors, retired_count = reconcile_legacy_owners(baseline, active_paths)
        if owner_errors:
            print("Rust size ownership metadata is invalid:", file=sys.stderr)
            for error in owner_errors:
                print(f"  {error}", file=sys.stderr)
            return 2
        baseline["file_lines"] = file_baseline
        write_baseline(baseline)
        print(
            f"Recorded {len(baseline['file_lines'])} current Rust files over "
            f"{FILE_LIMIT} lines in {BASELINE_PATH.relative_to(ROOT)}; "
            f"retired {retired_count} inactive ownership entries."
        )
        return 0

    file_baseline = baseline.get("file_lines", {})
    if not isinstance(file_baseline, dict):
        print("file_lines baseline must be a JSON object", file=sys.stderr)
        return 2

    try:
        function_paths = function_baseline_paths(baseline.get("function_lines", []))
    except ValueError as error:
        print(f"Rust size baseline is invalid: {error}", file=sys.stderr)
        return 2

    over_limit_paths = {path for path, count in counts.items() if count > FILE_LIMIT}
    owner_errors = legacy_owner_errors(
        baseline.get("legacy_owners"), over_limit_paths | function_paths
    )
    errors = file_ratchet_errors(counts, file_baseline, baseline.get("legacy_owners")) + owner_errors
    reports: list[str] = []
    legacy_count = 0
    review_count = 0
    owner_map = baseline.get("legacy_owners", {})

    for path, count in sorted(counts.items()):
        if count <= FILE_LIMIT:
            if count >= FILE_REVIEW_LINE:
                review_count += 1
                reports.append(f"review: {path} has {count} lines (review at {FILE_REVIEW_LINE})")
            continue

        previous = file_baseline.get(path)
        record = owner_map.get(path, {}) if isinstance(owner_map, dict) else {}
        owner = record.get("owner", "missing") if isinstance(record, dict) else "missing"
        task = record.get("reduction_task", "missing") if isinstance(record, dict) else "missing"
        if previous is None:
            reports.append(f"new violation: {path} has {count} lines (limit {FILE_LIMIT})")
        elif not isinstance(previous, int):
            continue
        elif count > previous:
            reports.append(
                f"growth: {path} has {count} lines (limit {FILE_LIMIT}; baseline {previous}; "
                f"owner {owner}; task {task})"
            )
        else:
            legacy_count += 1
            reports.append(
                f"baseline: {path} has {count} lines (limit {FILE_LIMIT}; ceiling {previous}; "
                f"owner {owner}; task {task})"
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
