#!/usr/bin/env python3
"""Report actual Node/browser WASM parity gaps from large result artifacts.

The JS parity artifacts intentionally retain the complete source and target
workflow records, so a full run can be too large for ``json.load``.  This
reporter streams only the ``comparisons`` array and writes a compact manifest
of failed case IDs grouped by the operation and the kind of work required.

``summary.not_run`` is the only pending count.  A target ``NotImplementedError``
is an executed parity failure, not a pending case; it is listed separately so
the missing facade can be implemented without confusing it with runner or
transport incompleteness.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"
CHUNK_SIZE = 1024 * 1024


def _fill(handle: Any, buffer: bytearray) -> bool:
    chunk = handle.read(CHUNK_SIZE)
    if not chunk:
        return False
    buffer.extend(chunk)
    return True


def _find_array_start(handle: Any, key: str, buffer: bytearray) -> None:
    needle = json.dumps(key, separators=(",", ":")).encode("utf-8")
    while True:
        position = buffer.find(needle)
        if position >= 0:
            cursor = position + len(needle)
            while True:
                while cursor < len(buffer) and buffer[cursor] in b" \t\r\n":
                    cursor += 1
                if cursor < len(buffer):
                    break
                if not _fill(handle, buffer):
                    break
            if cursor >= len(buffer) or buffer[cursor] != ord(":"):
                raise ValueError(f"top-level JSON member {key!r} has no colon")
            cursor += 1
            while True:
                while cursor < len(buffer) and buffer[cursor] in b" \t\r\n":
                    cursor += 1
                if cursor < len(buffer):
                    break
                if not _fill(handle, buffer):
                    break
            if cursor >= len(buffer) or buffer[cursor] != ord("["):
                raise ValueError(f"JSON member {key!r} is not an array")
            del buffer[: cursor + 1]
            return
        # Keep enough overlap to find a key split across two chunks.
        keep = max(len(needle) - 1, 1)
        if len(buffer) > keep:
            del buffer[:-keep]
        if not _fill(handle, buffer):
            raise ValueError(f"JSON member {key!r} was not found")


def _iter_array_values(path: Path, key: str) -> Iterable[Any]:
    """Yield values from one large top-level JSON array without loading it."""

    with path.open("rb") as handle:
        buffer = bytearray()
        _find_array_start(handle, key, buffer)
        while True:
            while True:
                while buffer and buffer[0] in b" \t\r\n,":
                    del buffer[0]
                if buffer:
                    break
                if not _fill(handle, buffer):
                    raise ValueError(f"unterminated JSON array {key!r}")
            if buffer[0] == ord("]"):
                return

            item = bytearray()
            cursor = 0
            depth = 0
            started = False
            in_string = False
            escaped = False
            while True:
                if cursor >= len(buffer):
                    if not _fill(handle, buffer):
                        raise ValueError(f"unterminated JSON value in {key!r}")
                value = buffer[cursor]
                item.append(value)
                cursor += 1
                if in_string:
                    if escaped:
                        escaped = False
                    elif value == ord("\\"):
                        escaped = True
                    elif value == ord('"'):
                        in_string = False
                    continue
                if value == ord('"'):
                    in_string = True
                elif value in (ord("{"), ord("[")):
                    started = True
                    depth += 1
                elif value in (ord("}"), ord("]")):
                    depth -= 1
                    if started and depth == 0:
                        break
            del buffer[:cursor]
            yield json.loads(item)


def _summary(path: Path) -> dict[str, int]:
    """Read the small summary object from the end of a large artifact."""

    with path.open("rb") as handle:
        handle.seek(max(0, path.stat().st_size - 2 * CHUNK_SIZE))
        tail = handle.read().decode("utf-8")
    marker = '"summary"'
    position = tail.rfind(marker)
    if position < 0:
        raise ValueError(f"summary was not found near the end of {path}")
    colon = tail.find(":", position + len(marker))
    if colon < 0:
        raise ValueError(f"summary in {path} has no colon")
    value, _end = json.JSONDecoder().raw_decode(tail[colon + 1 :].lstrip())
    if not isinstance(value, dict):
        raise ValueError(f"summary in {path} is not an object")
    fields = ("selected", "executed", "passed", "failed", "not_run", "infrastructure_errors")
    if any(type(value.get(field)) is not int for field in fields):
        raise ValueError(f"summary in {path} is missing integer fields")
    return {field: value[field] for field in fields}


def _case_index(manifest_path: Path) -> dict[str, tuple[str, str]]:
    sys.path.insert(0, str(ROOT / "scripts"))
    from run_migration_parity import load_cases, load_manifest

    manifest = load_manifest(manifest_path)
    cases, _inputs = load_cases(manifest, case_ids=None, surface=None)
    return {case["case_id"]: (case["surface"], case["operation"]) for case in cases}


def _target_state(comparison: dict[str, Any]) -> str:
    observations = comparison.get("target_observations", [])
    if not observations:
        target = comparison.get("target", {})
        observations = target.get("observations", []) if isinstance(target, dict) else []
    errors = [
        observation.get("error", {})
        for observation in observations
        if isinstance(observation, dict) and observation.get("status") == "error"
    ]
    if any(error.get("class") == "NotImplementedError" for error in errors if isinstance(error, dict)):
        return "target_not_implemented"
    if any(
        isinstance(observation, dict) and observation.get("status") == "not_run"
        for observation in observations
    ):
        return "workflow_dependency_not_run"
    if errors:
        return "target_error"
    return "result_mismatch"


def _compact_records(path: Path) -> Iterable[dict[str, Any]]:
    """Yield only fields needed for grouping.

    jq is used when available because the retained artifacts contain large
    image byte arrays.  The fallback keeps the reporter self-contained for
    environments without jq, at the cost of a slower Python scan.
    """

    jq = shutil.which("jq")
    if jq is not None:
        expression = (
            ".comparisons[] | {"
            "case_id: .case_id, "
            "outcome: .outcome, "
            "diffs: [.diffs[] | {kind: .kind, path: .path}], "
            "target_observations: [.target.observations[] | "
            "{step_id: .step_id, status: .status, "
            "error: (if .status == \"error\" then .error else null end)}]"
            "}"
        )
        process = subprocess.Popen(
            [jq, "-c", expression, str(path)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert process.stdout is not None
        for raw_line in process.stdout:
            value = json.loads(raw_line)
            if not isinstance(value, dict):
                raise ValueError(f"jq emitted a non-object comparison for {path}")
            yield value
        stderr = process.stderr.read().decode("utf-8", errors="replace") if process.stderr else ""
        returncode = process.wait()
        if returncode != 0:
            raise RuntimeError(f"jq failed for {path}: {stderr[-1000:]}")
        return

    for comparison in _iter_array_values(path, "comparisons"):
        target = comparison.get("target", {})
        observations = target.get("observations", []) if isinstance(target, dict) else []
        yield {
            "case_id": comparison.get("case_id"),
            "outcome": comparison.get("outcome"),
            "diffs": [
                {"kind": diff.get("kind"), "path": diff.get("path")}
                for diff in comparison.get("diffs", [])
                if isinstance(diff, dict)
            ],
            "target_observations": [
                {
                    "step_id": observation.get("step_id"),
                    "status": observation.get("status"),
                    "error": observation.get("error")
                    if observation.get("status") == "error"
                    else None,
                }
                for observation in observations
                if isinstance(observation, dict)
            ],
        }


def _target_stream_digest(path: Path) -> str | None:
    """Hash the semantic target stream, ignoring opaque WASM handle pointers."""

    jq = shutil.which("jq")
    if jq is None:
        digest = hashlib.sha256()
        for comparison in _iter_array_values(path, "comparisons"):
            target = comparison.get("target") if isinstance(comparison, dict) else None
            digest.update(
                json.dumps(
                    _scrub_handle_pointers(target),
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=True,
                ).encode("utf-8")
            )
            digest.update(b"\n")
        return digest.hexdigest()
    expression = (
        "def scrub: if type == \"object\" then "
        "with_entries(select(.key != \"__wbg_ptr\") | .value |= scrub) "
        "elif type == \"array\" then map(scrub) else . end; "
        ".comparisons[] | .target | scrub"
    )
    process = subprocess.Popen(
        [jq, "-S", "-c", expression, str(path)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdout is not None
    digest = hashlib.sha256()
    for chunk in iter(lambda: process.stdout.read(CHUNK_SIZE), b""):
        digest.update(chunk)
    stderr = process.stderr.read().decode("utf-8", errors="replace") if process.stderr else ""
    returncode = process.wait()
    if returncode != 0:
        raise RuntimeError(f"jq target digest failed for {path}: {stderr[-1000:]}")
    return digest.hexdigest()


def _scrub_handle_pointers(value: Any) -> Any:
    """Remove process-local wasm-bindgen pointer fields for the fallback."""

    if isinstance(value, dict):
        return {
            key: _scrub_handle_pointers(item)
            for key, item in value.items()
            if key != "__wbg_ptr"
        }
    if isinstance(value, list):
        return [_scrub_handle_pointers(item) for item in value]
    return value


def _summarize(path: Path, case_index: dict[str, tuple[str, str]]) -> dict[str, Any]:
    summary = _summary(path)
    categories: collections.Counter[str] = collections.Counter()
    operation_counts: collections.Counter[tuple[str, str]] = collections.Counter()
    group_cases: dict[tuple[str, str, str], list[str]] = collections.defaultdict(list)
    group_diff_kinds: dict[tuple[str, str, str], collections.Counter[str]] = collections.defaultdict(collections.Counter)
    target_errors: collections.Counter[tuple[str, str]] = collections.Counter()
    seen: set[str] = set()

    for comparison in _compact_records(path):
        if not isinstance(comparison, dict):
            raise ValueError(f"comparison in {path} is not an object")
        case_id = comparison.get("case_id")
        if not isinstance(case_id, str):
            raise ValueError(f"comparison in {path} has no case_id")
        if case_id in seen:
            raise ValueError(f"duplicate comparison case_id in {path}: {case_id}")
        seen.add(case_id)
        if comparison.get("outcome") != "fail":
            continue
        surface, operation = case_index.get(case_id, ("<unknown>", "<unknown>"))
        category = _target_state(comparison)
        categories[category] += 1
        operation_counts[(surface, operation)] += 1
        group_key = (category, surface, operation)
        group_cases[group_key].append(case_id)
        for diff in comparison.get("diffs", []):
            if isinstance(diff, dict) and isinstance(diff.get("kind"), str):
                group_diff_kinds[group_key][diff["kind"]] += 1
        for observation in comparison.get("target_observations", []):
            if not isinstance(observation, dict) or observation.get("status") != "error":
                continue
            error = observation.get("error", {})
            if isinstance(error, dict):
                target_errors[(str(error.get("class", "")), str(error.get("message", "")))] += 1

    if len(seen) != summary["executed"]:
        raise ValueError(
            f"{path}: summary.executed={summary['executed']} but comparisons={len(seen)}"
        )
    groups = []
    for rank, (group_key, case_ids) in enumerate(
        sorted(
            group_cases.items(),
            key=lambda item: (-len(item[1]), item[0]),
        ),
        start=1,
    ):
        category, surface, operation = group_key
        groups.append(
            {
                "rank": rank,
                "category": category,
                "surface": surface,
                "operation": operation,
                "count": len(case_ids),
                "case_ids": sorted(case_ids),
                "diff_kinds": dict(group_diff_kinds[group_key].most_common()),
            }
        )

    return {
        "path": str(path),
        "summary": summary,
        "categories": dict(categories.most_common()),
        "operation_counts": [
            {"surface": surface, "operation": operation, "count": count}
            for (surface, operation), count in sorted(
                operation_counts.items(), key=lambda item: (-item[1], item[0])
            )
        ],
        "groups": groups,
        "target_errors": [
            {"class": error_class, "message": message, "count": count}
            for (error_class, message), count in target_errors.most_common(80)
        ],
        "target_stream_sha256": _target_stream_digest(path),
    }


def _markdown(report: dict[str, Any]) -> str:
    node = report["node"]
    browser = report["browser"]
    lines = [
        "# Node/browser WASM parity gap manifest",
        "",
        "This report distinguishes selected cases, completed comparisons, actual parity failures, and pending cases.",
        "",
        f"- Common scope: **{node['summary']['selected']} selected**",
        f"- Node: **{node['summary']['executed']} executed**, {node['summary']['passed']} passed, {node['summary']['failed']} failed, {node['summary']['not_run']} pending, {node['summary']['infrastructure_errors']} infrastructure errors",
        f"- Browser: **{browser['summary']['executed']} executed**, {browser['summary']['passed']} passed, {browser['summary']['failed']} failed, {browser['summary']['not_run']} pending, {browser['summary']['infrastructure_errors']} infrastructure errors",
        f"- Node/browser semantic target result streams identical (opaque WASM pointers ignored): **{report['node_browser_target_stream_identical']}**",
        "",
        "## What is actually pending",
        "",
        f"**{node['summary']['not_run']} cases**. Pending means a selected case has no completed target comparison (`summary.not_run`). A facade `NotImplementedError` is not pending: the case ran and failed parity.",
        "",
        "## Failure categories",
        "",
        "| Category | Cases | Meaning / next owner |",
        "| --- | ---: | --- |",
        "| `result_mismatch` | %d | Completed target result differs; fix core behavior or metadata, keeping bindings thin. |" % node["categories"].get("result_mismatch", 0),
        "| `target_error` | %d | Target reached a public error path whose class/message/result differs; fix boundary validation or core error parity. |" % node["categories"].get("target_error", 0),
        "| `target_not_implemented` | %d | JS/WASM facade or an underlying public operation is missing; add a thin export only when core already owns the behavior. |" % node["categories"].get("target_not_implemented", 0),
        "| `workflow_dependency_not_run` | %d | A later observation was blocked by an earlier target failure; fix the first target error, not the dependent observation. |" % node["categories"].get("workflow_dependency_not_run", 0),
        "",
        "## Ordered operation failures",
        "",
        "The complete case IDs are in the JSON manifest; each row below is ordered by failing-case count.",
        "",
        "| Rank | Public operation | Cases |",
        "| ---: | --- | ---: |",
    ]
    for rank, item in enumerate(node["operation_counts"], start=1):
        lines.append(
            f"| {rank} | `{item['surface']}.{item['operation']}` | {item['count']} |"
        )
    lines.extend(
        [
            "",
            "## Interpretation",
            "",
            "Node and browser use the same manifest, workflow payload, and JS adapter. They therefore should have the same selected and executed denominators. Opaque WASM handle pointers are host-process addresses and are intentionally ignored when checking semantic target equivalence. A capability difference belongs in the separate WebGPU/WGSL lane; it must not silently remove public parity cases.",
            "",
            "The JSON `groups` array is the actionable backlog. Use `case_ids` for incremental runs and preserve the Python oracle as the behavioral authority. `target_errors` lists the most frequent target error signatures.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node", type=Path, required=True)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--json-out", type=Path, required=True)
    parser.add_argument("--markdown-out", type=Path, required=True)
    args = parser.parse_args()

    case_index = _case_index(args.manifest.resolve())
    node = _summarize(args.node.resolve(), case_index)
    browser = _summarize(args.browser.resolve(), case_index)
    node_digest = node["target_stream_sha256"]
    browser_digest = browser["target_stream_sha256"]
    identical = node_digest is not None and node_digest == browser_digest
    report = {
        "schema": "migration-parity/js-wasm-gap-manifest@1",
        "manifest": str(args.manifest.resolve().relative_to(ROOT)),
        "pending_definition": "summary.not_run: selected cases without a completed target comparison",
        "node_browser_scope_identical": node["summary"]["selected"] == browser["summary"]["selected"],
        "node_browser_target_stream_identical": identical,
        "node": node,
        "browser": browser,
    }
    args.json_out.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.json_out.resolve().write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown_out.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.markdown_out.resolve().write_text(_markdown(report), encoding="utf-8")
    print(
        json.dumps(
            {
                "selected": node["summary"]["selected"],
                "node_executed": node["summary"]["executed"],
                "browser_executed": browser["summary"]["executed"],
                "node_failed": node["summary"]["failed"],
                "browser_failed": browser["summary"]["failed"],
                "pending": node["summary"]["not_run"],
                "target_stream_identical": identical,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
