#!/usr/bin/env python3
"""Summarize coverage_matrix_tests failure ID files.

This is a developer triage tool. It consumes the `/tmp/pillow_failure_ids.txt`
files emitted by `tests/coverage_matrix_tests.rs` and writes a Markdown report.
It never reads or rewrites fixtures, thresholds, baselines, or Rust output.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


STAGE_RE = re.compile(
    r"^(?P<row_id>\S+) stage=(?P<stage>.*?)(?="
    r" actual=| actual_sha=| error=| raw=|$)"
)
ACTUAL_EXPECTED_RE = re.compile(r" actual=(?P<actual>\{.*\}) expected=(?P<expected>\{.*\})")
WIDTH_DELTA_RE = re.compile(r"width_delta=(-?\d+)")
HEIGHT_DELTA_RE = re.compile(r"height_delta=(-?\d+)")
MAX_DIFF_RE = re.compile(r" max=(\d+)")
DIFFS_RE = re.compile(r" diffs=(\d+)")


@dataclass(frozen=True)
class Failure:
    row_id: str
    stage: str
    line: str
    font: str
    ppem: str
    codepoint: str
    operation: str
    actual: object | None = None
    expected: object | None = None


def parse_lane_arg(value: str) -> tuple[str, Path]:
    if "=" not in value:
        raise argparse.ArgumentTypeError("lane must be NAME=/path/to/failure_ids.txt")
    name, path = value.split("=", 1)
    name = name.strip()
    if not name:
        raise argparse.ArgumentTypeError("lane name must not be empty")
    return name, Path(path)


def operation_suffix(row_id: str) -> str:
    for suffix in (
        "_metrics_only_metrics_only",
        "_outline_cbox_outline_cbox",
        "_getmask",
        "_getbbox",
        "_getmetrics",
        "_getlength",
        "_getname",
    ):
        if row_id.endswith(suffix):
            return suffix
    return ""


def parse_row_id(row_id: str) -> tuple[str, str, str, str]:
    suffix = operation_suffix(row_id)
    stem = row_id[: -len(suffix)] if suffix else row_id
    parts = stem.rsplit("_", 2)
    if len(parts) == 3 and parts[1].isdigit() and parts[2].isdigit():
        return parts[0], parts[1], parts[2], suffix.strip("_") or "unknown"
    return row_id, "?", "?", suffix.strip("_") or "unknown"


def parse_actual_expected(line: str) -> tuple[object | None, object | None]:
    match = ACTUAL_EXPECTED_RE.search(line)
    if not match:
        return None, None
    try:
        return json.loads(match.group("actual")), json.loads(match.group("expected"))
    except json.JSONDecodeError:
        return None, None


def parse_failure_line(line: str) -> Failure | None:
    line = line.strip()
    if not line:
        return None
    match = STAGE_RE.match(line)
    if not match:
        return None
    row_id = match.group("row_id")
    font, ppem, codepoint, operation = parse_row_id(row_id)
    actual, expected = parse_actual_expected(line)
    return Failure(
        row_id=row_id,
        stage=match.group("stage").strip(),
        line=line,
        font=font,
        ppem=ppem,
        codepoint=codepoint,
        operation=operation,
        actual=actual,
        expected=expected,
    )


def read_failures(path: Path) -> list[Failure]:
    failures: list[Failure] = []
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, 1):
            failure = parse_failure_line(line)
            if failure is None:
                print(
                    f"warning: {path}:{line_no}: could not parse failure line",
                    file=sys.stderr,
                )
                continue
            failures.append(failure)
    return failures


def flatten_json(value: object, prefix: str = "") -> dict[str, object]:
    if isinstance(value, dict):
        result: dict[str, object] = {}
        for key, child in value.items():
            child_prefix = f"{prefix}.{key}" if prefix else str(key)
            result.update(flatten_json(child, child_prefix))
        return result
    return {prefix: value}


def differing_fields(failure: Failure) -> list[tuple[str, object, object]]:
    if failure.actual is None or failure.expected is None:
        return []
    actual = flatten_json(failure.actual)
    expected = flatten_json(failure.expected)
    fields = sorted(set(actual) | set(expected))
    return [
        (field, actual.get(field), expected.get(field))
        for field in fields
        if actual.get(field) != expected.get(field)
    ]


def numeric_delta(actual: object, expected: object) -> int | None:
    if isinstance(actual, int) and isinstance(expected, int):
        return actual - expected
    return None


def counter_top(counter: collections.Counter[str], limit: int) -> list[tuple[str, int]]:
    return counter.most_common(limit)


def line_int(pattern: re.Pattern[str], line: str) -> int | None:
    match = pattern.search(line)
    return int(match.group(1)) if match else None


def native_bucket(failure: Failure) -> str:
    if failure.stage == "bitmap placement":
        wd = line_int(WIDTH_DELTA_RE, failure.line)
        hd = line_int(HEIGHT_DELTA_RE, failure.line)
        return f"bitmap dimensions differ wd={wd} hd={hd}"
    if failure.stage == "pixel coverage":
        max_diff = line_int(MAX_DIFF_RE, failure.line)
        diffs = line_int(DIFFS_RE, failure.line)
        if max_diff == 255:
            return "pixel coverage hard differences max=255"
        if max_diff is not None and diffs is not None and max_diff <= 9 and diffs <= 32:
            return "pixel coverage small sparse differences"
        return "pixel coverage soft differences"
    return f"{failure.stage}/other failure"


def outline_bucket(failure: Failure) -> str:
    diffs = differing_fields(failure)
    deltas = {field: numeric_delta(actual, expected) for field, actual, expected in diffs}
    numeric = {field: delta for field, delta in deltas.items() if delta is not None}

    x_outline = [
        delta
        for field, delta in numeric.items()
        if field.startswith(("outline_bbox_26_6.x_", "outline_cbox_26_6.x_"))
    ]
    y_outline = [
        delta
        for field, delta in numeric.items()
        if field.startswith(("outline_bbox_26_6.y_", "outline_cbox_26_6.y_"))
    ]
    bitmap_y = [
        delta for field, delta in numeric.items() if field.startswith("bitmap_pixels.y_")
    ]
    bitmap_x = [
        delta for field, delta in numeric.items() if field.startswith("bitmap_pixels.x_")
    ]

    if x_outline and all(abs(delta) == 1 for delta in x_outline):
        return "x cbox/bbox off by 1 subpixel unit"
    if bitmap_y and all(abs(delta) == 1 for delta in bitmap_y) and not bitmap_x:
        return "vertical gridline off by 1px"
    if any(abs(delta) >= 256 for delta in y_outline):
        return "large vertical bbox drift >=4px"
    return "mixed/small bbox mismatch"


def write_counts(lines: list[str], title: str, counts: collections.Counter[str], limit: int) -> None:
    lines.append(f"- {title}:")
    if not counts:
        lines.append("  - none")
        return
    for key, value in counter_top(counts, limit):
        lines.append(f"  - {key}: {value}")


def sample_lines(samples: dict[str, list[str]], limit: int) -> list[str]:
    lines: list[str] = []
    for name, ids in samples.items():
        lines.append(f"  - {name}:")
        for row_id in ids[:limit]:
            lines.append(f"    - {row_id}")
    return lines


def summarize_generic(name: str, failures: list[Failure], top: int) -> list[str]:
    stage_counts = collections.Counter(f.stage for f in failures)
    font_counts = collections.Counter(f.font for f in failures)
    ppem_counts = collections.Counter(f.ppem for f in failures)
    lines = [f"## {name}", f"- Failures: {len(failures)}"]
    write_counts(lines, "By stage", stage_counts, top)
    write_counts(lines, "Top font families", font_counts, top)
    write_counts(lines, "By ppem", ppem_counts, top)
    return lines


def summarize_metrics(failures: list[Failure], top: int) -> list[str]:
    field_counts: collections.Counter[str] = collections.Counter()
    single_field_counts: collections.Counter[str] = collections.Counter()
    delta_samples: dict[str, list[str]] = collections.defaultdict(list)

    for failure in failures:
        diffs = differing_fields(failure)
        for field, actual, expected in diffs:
            field_counts[field] += 1
            delta = numeric_delta(actual, expected)
            if delta is not None and len(delta_samples[field]) < 5:
                delta_samples[field].append(
                    f"{failure.row_id}: actual={actual} expected={expected} delta={delta}"
                )
        if len(diffs) == 1:
            single_field_counts[diffs[0][0]] += 1

    lines = ["## metrics_only mismatch shape"]
    write_counts(lines, "Top differing fields", field_counts, top)
    write_counts(lines, "Single-field-only failures", single_field_counts, top)
    lines.append("- Sample field deltas:")
    if not delta_samples:
        lines.append("  - none")
    for field, samples in sorted(delta_samples.items()):
        lines.append(f"  - {field}:")
        for sample in samples:
            lines.append(f"    - {sample}")
    return lines


def summarize_outline(failures: list[Failure], top: int) -> list[str]:
    buckets: collections.Counter[str] = collections.Counter()
    bucket_samples: dict[str, list[str]] = collections.defaultdict(list)
    field_counts: collections.Counter[str] = collections.Counter()
    delta_counts: collections.Counter[str] = collections.Counter()

    for failure in failures:
        bucket = outline_bucket(failure)
        buckets[bucket] += 1
        if len(bucket_samples[bucket]) < 12:
            bucket_samples[bucket].append(failure.row_id)
        for field, actual, expected in differing_fields(failure):
            field_counts[field] += 1
            delta = numeric_delta(actual, expected)
            if delta is not None:
                delta_counts[f"{field} delta {delta}"] += 1

    lines = ["## outline_cbox mismatch shape"]
    write_counts(lines, "Buckets", buckets, top)
    write_counts(lines, "Top differing fields", field_counts, top)
    write_counts(lines, "Most common numeric field deltas", delta_counts, top)
    lines.append("- Bucket samples:")
    lines.extend(sample_lines(bucket_samples, 12))
    return lines


def summarize_native(failures: list[Failure], top: int) -> list[str]:
    buckets: collections.Counter[str] = collections.Counter(native_bucket(f) for f in failures)
    lines = ["## native_tt_default mismatch shape"]
    write_counts(lines, "Buckets", buckets, top)
    return lines


def lane_specific_summary(name: str, failures: list[Failure], top: int) -> list[str]:
    if name == "metrics_only":
        return summarize_metrics(failures, top)
    if name == "outline_cbox":
        return summarize_outline(failures, top)
    if name == "native_tt_default":
        return summarize_native(failures, top)
    return []


def report(lanes: list[tuple[str, Path]], source_commit: str, top: int) -> str:
    loaded = [(name, path, read_failures(path)) for name, path in lanes]
    lines = [
        "# FreeType Parity Failure Classification",
        "",
        f"Source commit: {source_commit}" if source_commit else "Source commit: not recorded",
        "Runtime constraints: pure Rust only; no FFI/native FreeType runtime path; "
        "do not edit fixtures, thresholds, or harnesses to hide failures.",
        "",
        "Input failure files:",
    ]
    for name, path, failures in loaded:
        lines.append(f"- {name}: `{path}` ({len(failures)} failures)")
    lines.append("")

    for index, (name, _, failures) in enumerate(loaded):
        if index:
            lines.append("")
        lines.extend(summarize_generic(name, failures, top))

    for name, _, failures in loaded:
        extra = lane_specific_summary(name, failures, top)
        if extra:
            lines.append("")
            lines.extend(extra)

    lines.append("")
    lines.append("This report is assignment triage only. It is not a parity gate and does not")
    lines.append("change expected output, thresholds, or fixture data.")
    return "\n".join(lines) + "\n"


def default_lanes() -> list[tuple[str, Path]]:
    candidates = [
        ("native_tt_default", Path("/tmp/native_tt_default_failure_ids.txt")),
        ("metrics_only", Path("/tmp/metrics_only_failure_ids.txt")),
        ("outline_cbox", Path("/tmp/outline_cbox_failure_ids.txt")),
    ]
    return [(name, path) for name, path in candidates if path.exists()]


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--lane",
        action="append",
        type=parse_lane_arg,
        help="Lane input as NAME=/path/to/failure_ids.txt. Repeat for multiple lanes.",
    )
    parser.add_argument("--output", type=Path, help="Write Markdown report to this path.")
    parser.add_argument("--source-commit", default="", help="Commit or branch under analysis.")
    parser.add_argument("--top", type=int, default=15, help="Number of top counts to print.")
    args = parser.parse_args(list(argv) if argv is not None else None)

    lanes = args.lane or default_lanes()
    if not lanes:
        parser.error("no lanes provided; use --lane NAME=/path/to/failure_ids.txt")
    for _, path in lanes:
        if not path.exists():
            parser.error(f"failure file does not exist: {path}")

    text = report(lanes, args.source_commit, args.top)
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
