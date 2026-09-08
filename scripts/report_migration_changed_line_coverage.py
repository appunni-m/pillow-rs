#!/usr/bin/env python3
"""Attribute changed Rust lines to a fresh, source-bound LCOV measurement.

WGSL and lines without DA records stay explicitly unmeasured. This report
does not infer executable lines from LLVM segment starts or claim GPU shader
line coverage from host-side dispatch coverage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def lcov_line_hits(text: str) -> dict[str, dict[int, int]]:
    files: dict[str, dict[int, int]] = {}
    current: dict[int, int] | None = None
    for line in text.splitlines():
        if line.startswith("SF:"):
            path = Path(line[3:])
            if path.is_absolute():
                path = path.resolve().relative_to(ROOT)
            current = files.setdefault(path.as_posix(), {})
        elif line.startswith("DA:") and current is not None:
            number, hits, *_ = line[3:].split(",")
            current[int(number)] = max(current.get(int(number), 0), int(hits))
        elif line == "end_of_record":
            current = None
    return files


def changed_lines(diff: str) -> set[int]:
    lines: set[int] = set()
    for match in re.finditer(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", diff, re.MULTILINE):
        start = int(match[1])
        count = int(match[2]) if match[2] is not None else 1
        lines.update(range(start, start + count))
    return lines


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True)


def report_changes(report: Path, base: str) -> dict:
    data = report.read_bytes()
    receipt = json.loads(report.with_name(report.name + ".context.json").read_text())
    if receipt["report_sha256"] != hashlib.sha256(data).hexdigest():
        raise ValueError("coverage receipt does not match report bytes")
    if receipt["test_status"] != "passed":
        raise ValueError("coverage execution did not pass")
    for path, expected in receipt["source_hashes"].items():
        if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != expected:
            raise ValueError(f"coverage source changed: {path}")
    hits = lcov_line_hits(data.decode("utf-8"))
    paths = git("diff", "--name-only", "-z", base, "--", "pillow-rs/src", "pillow-rs-py/src").split("\0")
    files = []
    for path in filter(None, paths):
        if not (ROOT / path).is_file():
            continue
        diff = git("diff", "--no-ext-diff", "--no-color", "--unified=0", base, "--", path)
        changed = changed_lines(diff)
        if not changed:
            continue
        measured = changed & hits.get(path, {}).keys()
        covered = {line for line in measured if hits[path][line] > 0}
        files.append({
            "path": path,
            "changed_lines": len(changed),
            "measured_changed_lines": len(measured),
            "covered_changed_lines": len(covered),
            "uncovered_lines": sorted(measured - covered),
            "without_line_records": sorted(changed - measured),
        })
    return {
        "schema": "migration-parity/changed-line-coverage@1",
        "base_revision": git("rev-parse", base).strip(),
        "measured_revision": receipt["recorded_revision"],
        "report_sha256": receipt["report_sha256"],
        "build_id": receipt["build_id"],
        "source": "matches_receipt",
        "scope": receipt["scope"],
        "test_status": receipt["test_status"],
        "summary": {
            key: sum(item[key] for item in files)
            for key in ("changed_lines", "measured_changed_lines", "covered_changed_lines")
        },
        "limitations": [
            "Lines without LCOV DA records include comments, declarations, and uninstrumented code; they are not counted as covered.",
            "WGSL execution is proven separately by exact parity and native backend receipts; LLVM does not instrument shader lines.",
            "Coverage execution is target-only; live-oracle parity is verified separately.",
        ],
        "files": files,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--base", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = report_changes(args.report, args.base)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result["summary"], sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
