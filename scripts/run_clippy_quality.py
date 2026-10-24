#!/usr/bin/env python3
"""Run Clippy and ratchet existing functions over the hard line limit."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

from check_rust_file_sizes import (
    legacy_owner_errors,
    reconcile_legacy_owners,
)


ROOT = Path(__file__).resolve().parents[1]
BASELINE_PATH = ROOT / "scripts" / "rust_size_baseline.json"
FUNCTION_HARD_LIMIT = 100
FUNCTION_REVIEW_LINE = 60
TOO_MANY_LINES = "clippy::too_many_lines"
LINE_COUNT_RE = re.compile(r"too many lines \((\d+)/(\d+)\)")


def lint_code(message: dict[str, object]) -> str | None:
    code = message.get("code")
    return str(code.get("code")) if isinstance(code, dict) else None


def normalize_signature(span: dict[str, object]) -> str:
    pieces = span.get("text", [])
    if not isinstance(pieces, list):
        return ""
    return " ".join(
        " ".join(str(piece.get("text", "")).split())
        for piece in pieces
        if isinstance(piece, dict)
    ).strip()


def source_path(file_name: str) -> str:
    path = Path(file_name)
    if not path.is_absolute():
        path = ROOT / path
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def function_metrics(messages: list[dict[str, object]]) -> list[dict[str, object]]:
    locations: dict[tuple[str, str], dict[int, int]] = defaultdict(dict)
    for message in messages:
        if lint_code(message) != TOO_MANY_LINES:
            continue
        match = LINE_COUNT_RE.search(str(message.get("message", "")))
        span = next(
            (item for item in message.get("spans", []) if item.get("is_primary")), None
        )
        if match is None or span is None:
            raise ValueError("could not read a Clippy too_many_lines diagnostic")
        count = int(match.group(1))
        path = source_path(str(span.get("file_name", "")))
        signature = normalize_signature(span)
        if not signature:
            raise ValueError(f"missing function signature in Clippy diagnostic for {path}")
        line = int(span.get("line_start", 0))
        locations[(path, signature)][line] = max(
            count, locations[(path, signature)].get(line, 0)
        )

    metrics: list[dict[str, object]] = []
    for (path, signature), starts in sorted(locations.items()):
        for occurrence, (line, count) in enumerate(sorted(starts.items()), start=1):
            metrics.append(
                {
                    "path": path,
                    "signature": signature,
                    "occurrence": occurrence,
                    "lines": count,
                    "line_start": line,
                }
            )
    return metrics


def metric_key(metric: dict[str, object]) -> tuple[str, str, int]:
    return (
        str(metric["path"]),
        str(metric["signature"]),
        int(metric["occurrence"]),
    )


def stale_function_baseline_errors(
    current: list[dict[str, object]],
    function_baseline: dict[tuple[str, str, int], int],
) -> list[str]:
    current_keys = {metric_key(metric) for metric in current}
    return [
        f"stale function baseline entry for {path} {signature} occurrence {occurrence}; "
        "run checkstyle-update-baseline to retire it"
        for path, signature, occurrence in sorted(set(function_baseline) - current_keys)
    ]


def function_ratchet_errors(
    over_limit: list[dict[str, object]],
    function_baseline: dict[tuple[str, str, int], int],
    owner_map: object = None,
) -> tuple[list[str], int]:
    errors: list[str] = []
    legacy_count = 0
    for metric in over_limit:
        key = metric_key(metric)
        count = int(metric["lines"])
        previous = function_baseline.get(key)
        location = f"{metric['path']}:{metric['line_start']} {metric['signature']}"
        record = owner_map.get(str(metric["path"]), {}) if isinstance(owner_map, dict) else {}
        if not isinstance(record, dict):
            record = {}
        owner = record.get("owner", "missing")
        task = record.get("reduction_task", "missing")
        if previous is None:
            errors.append(
                f"new over-limit function: {location} has {count} lines "
                f"(limit {FUNCTION_HARD_LIMIT}; owner {owner}; task {task})"
            )
        elif count > previous:
            errors.append(
                f"over-limit function grew: {location} has {count} lines "
                f"(limit {FUNCTION_HARD_LIMIT}; baseline {previous}; owner {owner}; task {task})"
            )
        else:
            legacy_count += 1
    return errors, legacy_count


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


def render_clippy(command: list[str], result: subprocess.CompletedProcess[str]) -> tuple[list[dict[str, object]], int]:
    messages: list[dict[str, object]] = []
    warning_groups: dict[str, list[object]] = defaultdict(list)
    warning_counts: dict[str, int] = defaultdict(int)
    for raw in result.stdout.splitlines():
        try:
            item = json.loads(raw)
        except json.JSONDecodeError:
            print(raw)
            continue
        if item.get("reason") != "compiler-message":
            continue
        message = item.get("message", {})
        if lint_code(message) == TOO_MANY_LINES:
            messages.append(message)
            continue
        if message.get("level") != "warning":
            rendered = message.get("rendered")
            if rendered:
                print(rendered, file=sys.stderr, end="")
            continue

        code = lint_code(message) or "unclassified warning"
        warning_counts[code] += 1
        if len(warning_groups[code]) < 2:
            spans = message.get("spans", [])
            primary = next((span for span in spans if span.get("is_primary")), None)
            location = ""
            if primary:
                location = f"{primary.get('file_name', '<unknown>')}:{primary.get('line_start', '?')}: "
            summary = " ".join(str(message.get("message", "")).split())
            warning_groups[code].append(f"{location}{summary}")

    if warning_counts:
        total = sum(warning_counts.values())
        print(f"Rust Clippy: {total} existing warning diagnostics across {len(warning_counts)} lint categories.")
        for code, count in sorted(warning_counts.items()):
            print(f"  {code}: {count}")
            for example in warning_groups[code]:
                print(f"    {example}")
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    if result.returncode:
        print(f"Clippy failed with exit code {result.returncode}: {' '.join(command)}", file=sys.stderr)
    return messages, result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package",
        help="limit Clippy to one Cargo package (for example, pillow-rs)",
    )
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="record current functions over the hard limit as reviewed legacy entries",
    )
    args = parser.parse_args()

    cargo = os.environ.get("CARGO", "cargo")
    command = [cargo, "clippy"]
    command += ["--package", args.package] if args.package else ["--workspace"]
    command += ["--all-targets", "--all-features", "--message-format=json", "--locked"]
    command += ["--", "-D", "deprecated"]

    try:
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
        baseline = load_baseline()
        messages, clippy_status = render_clippy(command, result)
        metrics = function_metrics(messages)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Clippy quality check failed: {error}", file=sys.stderr)
        return 2

    # A failed Cargo/Clippy run may emit partial diagnostics. Do not compare
    # that incomplete function inventory with the checked-in baseline or write
    # it back as a reviewed update.
    if clippy_status != 0:
        return clippy_status

    over_limit = [metric for metric in metrics if int(metric["lines"]) > FUNCTION_HARD_LIMIT]
    if args.update_baseline:
        file_baseline = baseline.get("file_lines", {})
        if not isinstance(file_baseline, dict):
            print("file_lines baseline must be a JSON object", file=sys.stderr)
            return 2
        function_lines = [
            {key: metric[key] for key in ("path", "signature", "occurrence", "lines")}
            for metric in over_limit
        ]
        active_paths = set(file_baseline)
        active_paths.update(str(metric["path"]) for metric in over_limit)
        baseline["function_lines"] = function_lines
        owner_errors, retired_count = reconcile_legacy_owners(baseline, active_paths)
        if owner_errors:
            print("Rust size ownership metadata is invalid:", file=sys.stderr)
            for error in owner_errors:
                print(f"  {error}", file=sys.stderr)
            return 2
        write_baseline(baseline)
        print(
            f"Recorded {len(over_limit)} current functions over {FUNCTION_HARD_LIMIT} lines "
            f"in {BASELINE_PATH.relative_to(ROOT)}; retired "
            f"{retired_count} inactive ownership entries."
        )
        return clippy_status

    raw_baseline = baseline.get("function_lines", [])
    if not isinstance(raw_baseline, list):
        print("function_lines baseline must be a JSON array", file=sys.stderr)
        return 2
    function_baseline = {metric_key(item): int(item["lines"]) for item in raw_baseline}

    file_baseline = baseline.get("file_lines", {})
    if not isinstance(file_baseline, dict):
        print("file_lines baseline must be a JSON object", file=sys.stderr)
        return 2

    errors = stale_function_baseline_errors(over_limit, function_baseline)
    owner_map = baseline.get("legacy_owners")
    ratchet_errors, legacy_count = function_ratchet_errors(
        over_limit, function_baseline, owner_map
    )
    errors.extend(ratchet_errors)
    active_owner_paths = set(file_baseline)
    active_owner_paths.update(str(metric["path"]) for metric in over_limit)
    errors.extend(legacy_owner_errors(owner_map, active_owner_paths))

    if clippy_status == 0:
        print(
            f"Rust function-size report: {len(metrics)} Clippy warnings over "
            f"{FUNCTION_REVIEW_LINE} lines; {legacy_count} legacy functions over "
            f"{FUNCTION_HARD_LIMIT} lines are at or below baseline."
        )
        for metric in sorted(metrics, key=lambda item: int(item["lines"]), reverse=True)[:5]:
            print(
                f"  review: {metric['path']}:{metric['line_start']} has "
                f"{metric['lines']} lines: {metric['signature']}"
            )
        if isinstance(owner_map, dict):
            for metric in over_limit:
                key = metric_key(metric)
                previous = function_baseline.get(key)
                if previous is None or int(metric["lines"]) > previous:
                    continue
                record = owner_map.get(str(metric["path"]), {})
                if not isinstance(record, dict):
                    record = {}
                print(
                    f"  baseline: {metric['path']}:{metric['line_start']} has "
                    f"{metric['lines']} lines (limit {FUNCTION_HARD_LIMIT}; ceiling {previous}; "
                    f"owner {record.get('owner', 'missing')}; "
                    f"task {record.get('reduction_task', 'missing')}): {metric['signature']}"
                )
    if errors:
        print("Rust function-size ratchet violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    return clippy_status


if __name__ == "__main__":
    raise SystemExit(main())
