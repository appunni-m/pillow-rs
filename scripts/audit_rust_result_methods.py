#!/usr/bin/env python3
"""Generate an audit of Rust functions and Result-returning fallible APIs.

The output is intentionally mechanical: every detected `fn` item in the tracked
Rust workspace is listed with its return shape and simple fallibility signals.
This is not a Rust parser; it is a conservative inventory used to drive manual
review and migration work.
"""

from __future__ import annotations

import csv
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/generated/rust-method-result-audit.tsv"


FN_RE = re.compile(
    r"(?P<prefix>(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]+\"\s+)?)"
    r"fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*",
    re.MULTILINE,
)


@dataclass
class FunctionRecord:
    path: Path
    line: int
    name: str
    visibility: str
    scope: str
    return_type: str
    returns_result: bool
    signals: list[str]
    classification: str


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def find_matching_brace(text: str, open_index: int) -> int:
    depth = 0
    in_line_comment = False
    in_block_comment = 0
    in_string = False
    in_char = False
    escaped = False
    for i in range(open_index, len(text)):
        c = text[i]
        n = text[i + 1] if i + 1 < len(text) else ""
        if in_line_comment:
            if c == "\n":
                in_line_comment = False
            continue
        if in_block_comment:
            if c == "/" and n == "*":
                in_block_comment += 1
            elif c == "*" and n == "/":
                in_block_comment -= 1
            continue
        if in_string:
            if escaped:
                escaped = False
            elif c == "\\":
                escaped = True
            elif c == '"':
                in_string = False
            continue
        if in_char:
            if escaped:
                escaped = False
            elif c == "\\":
                escaped = True
            elif c == "'":
                in_char = False
            continue
        if c == "/" and n == "/":
            in_line_comment = True
            continue
        if c == "/" and n == "*":
            in_block_comment += 1
            continue
        if c == '"':
            in_string = True
            continue
        if c == "'":
            in_char = True
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
    return len(text)


def signature_return(text: str, fn_start: int, body_start: int) -> str:
    signature = " ".join(text[fn_start:body_start].split())
    if "->" not in signature:
        return "()"
    return signature.split("->", 1)[1].strip()


def fallibility_signals(body: str) -> list[str]:
    checks = [
        ("question_operator", "?"),
        ("unwrap", ".unwrap("),
        ("expect", ".expect("),
        ("panic", "panic!("),
        ("todo", "todo!("),
        ("unimplemented", "unimplemented!("),
        ("fs_io", "std::fs::"),
        ("command_spawn", "Command::"),
        ("checked_arithmetic", "checked_"),
        ("map_err", ".map_err("),
        ("ok_or", ".ok_or"),
        ("result_ctor", "Err("),
    ]
    return [name for name, needle in checks if needle in body]


def classify(return_type: str, signals: list[str]) -> str:
    returns_result = "Result" in return_type
    if returns_result:
        return "ok_result"
    if not signals:
        return "likely_infallible"
    if any(signal in signals for signal in ["panic", "todo", "unimplemented"]):
        return "review_panic_path"
    if any(signal in signals for signal in ["question_operator", "map_err", "ok_or", "result_ctor"]):
        return "parser_review"
    if any(signal in signals for signal in ["unwrap", "expect", "fs_io", "command_spawn", "checked_arithmetic"]):
        return "review_non_result_fallible"
    return "review"


def test_cfg_ranges(text: str) -> list[tuple[int, int]]:
    ranges: list[tuple[int, int]] = []
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+\w+\s*\{", text):
        open_index = text.find("{", match.end() - 1)
        if open_index != -1:
            ranges.append((match.start(), find_matching_brace(text, open_index)))
    return ranges


def file_scope(path: Path) -> str | None:
    parts = path.relative_to(ROOT).parts
    if "tests" in parts:
        return "test"
    if "examples" in parts:
        return "example"
    if "bench-rust" in parts:
        return "bench"
    return None


def function_scope(
    path: Path,
    text: str,
    fn_start: int,
    test_ranges: list[tuple[int, int]],
) -> str:
    if scope := file_scope(path):
        return scope
    if any(start <= fn_start <= end for start, end in test_ranges):
        return "test"
    prefix = text[max(0, fn_start - 256) : fn_start]
    if re.search(r"#\s*\[\s*(?:tokio::)?test(?:\([^]]*\))?\s*\]\s*$", prefix):
        return "test"
    return "production"


def iter_functions(path: Path) -> list[FunctionRecord]:
    text = path.read_text()
    test_ranges = test_cfg_ranges(text)
    records: list[FunctionRecord] = []
    for match in FN_RE.finditer(text):
        brace = text.find("{", match.end())
        semi = text.find(";", match.end())
        if brace == -1 or (semi != -1 and semi < brace):
            body = ""
            body_end = semi if semi != -1 else match.end()
        else:
            body_end = find_matching_brace(text, brace)
            body = text[brace:body_end + 1]
        return_type = signature_return(text, match.start(), brace if brace != -1 else body_end)
        signals = fallibility_signals(body)
        visibility = "pub" if match.group("prefix").strip().startswith("pub") else "private"
        scope = function_scope(path, text, match.start(), test_ranges)
        records.append(
            FunctionRecord(
                path=path.relative_to(ROOT),
                line=line_number(text, match.start()),
                name=match.group("name"),
                visibility=visibility,
                scope=scope,
                return_type=return_type,
                returns_result="Result" in return_type,
                signals=signals,
                classification=classify(return_type, signals),
            )
        )
    return records


def tracked_rust_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "*.rs", "**/*.rs"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    paths = sorted({ROOT / line for line in result.stdout.splitlines() if line})
    return [path for path in paths if path.is_file()]


def main() -> None:
    records: list[FunctionRecord] = []
    for path in tracked_rust_files():
        records.extend(iter_functions(path))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT.open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(
            [
                "path",
                "line",
                "name",
                "visibility",
                "scope",
                "return_type",
                "returns_result",
                "classification",
                "signals",
            ]
        )
        for record in records:
            writer.writerow(
                [
                    str(record.path),
                    record.line,
                    record.name,
                    record.visibility,
                    record.scope,
                    record.return_type,
                    str(record.returns_result).lower(),
                    record.classification,
                    ",".join(record.signals),
                ]
            )
    print(f"wrote {len(records)} function records to {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
