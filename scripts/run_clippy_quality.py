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

    over_limit = [metric for metric in metrics if int(metric["lines"]) > FUNCTION_HARD_LIMIT]
    if args.update_baseline:
        baseline["function_lines"] = [
            {key: metric[key] for key in ("path", "signature", "occurrence", "lines")}
            for metric in over_limit
        ]
        write_baseline(baseline)
        print(
            f"Recorded {len(over_limit)} current functions over {FUNCTION_HARD_LIMIT} lines "
            f"in {BASELINE_PATH.relative_to(ROOT)}."
        )
        return clippy_status

    raw_baseline = baseline.get("function_lines", [])
    if not isinstance(raw_baseline, list):
        print("function_lines baseline must be a JSON array", file=sys.stderr)
        return 2
    function_baseline = {metric_key(item): int(item["lines"]) for item in raw_baseline}

    errors: list[str] = []
    legacy_count = 0
    for metric in over_limit:
        key = metric_key(metric)
        count = int(metric["lines"])
        previous = function_baseline.get(key)
        location = f"{metric['path']}:{metric['line_start']} {metric['signature']}"
        if previous is None:
            errors.append(f"new over-limit function: {location} has {count} lines (limit {FUNCTION_HARD_LIMIT})")
        elif count > previous:
            errors.append(f"over-limit function grew: {location} has {count} lines, baseline is {previous}")
        else:
            legacy_count += 1

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
    if errors:
        print("Rust function-size ratchet violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    return clippy_status


if __name__ == "__main__":
    raise SystemExit(main())
