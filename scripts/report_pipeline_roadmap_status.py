#!/usr/bin/env python3
"""Generate a machine-readable status view for the image pipeline roadmap.

The roadmap remains the human-reviewed authority. This report only indexes its
IDs and joins them with the latest maintained benchmark evidence; it does not
infer that an item is closed from a timing result and never changes a
denominator.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ROADMAP = ROOT / "docs" / "image-pipeline-performance-roadmap.md"
DEFAULT_RESULT = ROOT / "build" / "migration-parity" / "benchmark-result.json"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "pipeline-roadmap-status.json"

sys.path.insert(0, str(Path(__file__).resolve().parent))
from report_pipeline_benchmark_coverage import (  # noqa: E402
    DEFAULT_INPUT,
    report as report_workload_coverage,
)


ITEM_RE = re.compile(r"^### FIL-(\d+) — (.+)$", re.MULTILINE)
PRIORITY_RE = re.compile(r"^Priority: (.+)$", re.MULTILINE)
STATUS_RE = re.compile(r"^Status: (.+)$", re.MULTILINE)


def normalize_status(raw: str | None) -> str:
    if raw is None:
        return "proposed"
    lowered = raw.lower()
    if lowered.startswith("closed"):
        return "closed"
    if lowered.startswith("in progress"):
        return "in progress"
    if lowered.startswith("implemented"):
        return "implemented"
    if lowered.startswith("verified"):
        return "verified"
    if lowered.startswith("rejected"):
        return "rejected"
    return "proposed"


def parse_items(text: str) -> list[dict[str, Any]]:
    matches = list(ITEM_RE.finditer(text))
    items: list[dict[str, Any]] = []
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        body = text[match.end() : end]
        priority = PRIORITY_RE.search(body)
        status = STATUS_RE.search(body)
        items.append(
            {
                "id": f"FIL-{int(match.group(1)):02d}",
                "number": int(match.group(1)),
                "title": match.group(2).strip(),
                "priority": priority.group(1).strip() if priority else None,
                "status": normalize_status(status.group(1).strip() if status else None),
                "status_text": status.group(1).strip() if status else None,
            }
        )
    return items


def file_timestamp(path: Path) -> str | None:
    if not path.is_file():
        return None
    return datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc).isoformat()


def build_report(result_path: Path) -> dict[str, Any]:
    roadmap_text = ROADMAP.read_text(encoding="utf-8")
    items = parse_items(roadmap_text)
    expected = [f"FIL-{number:02d}" for number in range(1, 65)]
    ids = [str(item["id"]) for item in items]
    duplicates = sorted(item for item, count in Counter(ids).items() if count != 1)
    missing = sorted(set(expected) - set(ids))
    unexpected = sorted(set(ids) - set(expected))

    coverage: dict[str, Any] | None = None
    if result_path.is_file():
        coverage = report_workload_coverage(DEFAULT_INPUT, result_path)

    status_counts = Counter(str(item["status"]) for item in items)
    output: dict[str, Any] = {
        "schema": "pillow-rs/pipeline-roadmap-status@1",
        "roadmap": str(ROADMAP.relative_to(ROOT)),
        "roadmap_status": (
            roadmap_text.splitlines()[2].removeprefix("Status: ").strip()
            if len(roadmap_text.splitlines()) > 2
            else None
        ),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "items_total": len(items),
        "expected_items_total": len(expected),
        "missing_ids": missing,
        "unexpected_ids": unexpected,
        "duplicate_ids": duplicates,
        "status_counts": dict(sorted(status_counts.items())),
        "closed_ids": [item["id"] for item in items if item["status"] == "closed"],
        "open_ids": [item["id"] for item in items if item["status"] != "closed"],
        "items": items,
        "evidence": {
            "benchmark_result": str(result_path.relative_to(ROOT))
            if result_path.is_file()
            else None,
            "benchmark_result_mtime": file_timestamp(result_path),
            "roadmap_mtime": file_timestamp(ROADMAP),
            "coverage": coverage,
        },
    }
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--result", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    result_path = args.result.resolve()
    output_path = args.output.resolve()
    document = build_report(result_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")

    coverage = document["evidence"]["coverage"]
    errors: list[str] = []
    if document["items_total"] != document["expected_items_total"]:
        errors.append("roadmap does not contain all FIL-01..FIL-64 items")
    if document["missing_ids"] or document["unexpected_ids"] or document["duplicate_ids"]:
        errors.append("roadmap FIL IDs are missing, unexpected, or duplicated")
    if args.check and coverage is None:
        errors.append(f"benchmark result does not exist: {result_path}")
    if args.check and coverage is not None:
        if coverage["operation_coverage_percent"] != 100.0:
            errors.append("PipelineOp benchmark input coverage is not 100 percent")
        if coverage["context_missing_workloads"]:
            errors.append("benchmark workloads are missing required context")
        if coverage["duplicate_workload_ids"]:
            errors.append("benchmark workload IDs are duplicated")

    print(
        json.dumps(
            {
                "items_total": document["items_total"],
                "closed": len(document["closed_ids"]),
                "open": len(document["open_ids"]),
                "operation_coverage_percent": (
                    coverage["operation_coverage_percent"] if coverage else None
                ),
                "output": str(output_path.relative_to(ROOT)),
                "errors": errors,
            },
            sort_keys=True,
        )
    )
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
