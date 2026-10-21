#!/usr/bin/env python3
"""Ratchet legacy Rust source-policy findings without hiding existing debt."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
PUBLIC_PALETTE_STRING_APIS = {
    "palette_getcolor_append",
    "palette_getcolor_validate",
    "palette_getcolor_validate_input",
}
TEST_MODULE = re.compile(r"^\s*#\[cfg\(test\)\]")
FUNCTION_DECLARATION = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b"
)
CONTRACTS = {
    "raw_image_construction": re.compile(
        r"ImageLuma8.*from_raw|ImageRgb8.*from_raw|ImageRgba8.*from_raw|ImageLumaA8.*from_raw"
    ),
    "encoded_mode_flags": re.compile(
        r"has_gb\s*=\s*mode\s*[>=]|has_a\s*=\s*mode\s*[=!|]"
    ),
    "string_error_types": re.compile(r"Result<[^,]*,\s*String\s*>"),
}


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    contract: str
    text: str


def production_lines(path: str, text: str) -> list[tuple[int, str]]:
    """Return non-test source lines with original line numbers."""
    if path.endswith("_tests.rs"):
        return []
    lines = text.splitlines()
    test_starts = [index for index, line in enumerate(lines) if TEST_MODULE.match(line)]
    if test_starts:
        lines = lines[: test_starts[-1]]
    return list(enumerate(lines, start=1))


def scan_source(path: str, text: str) -> list[Finding]:
    findings: list[Finding] = []
    function_name: str | None = None
    for line_number, line in production_lines(path, text):
        declaration = FUNCTION_DECLARATION.match(line)
        if declaration:
            function_name = declaration.group(1)
        # Keep the established exemption for comments while avoiding fixture
        # and unit-test matches through production_lines above.
        if "//" in line:
            continue
        for contract, pattern in CONTRACTS.items():
            if not pattern.search(line):
                continue
            if (
                contract == "string_error_types"
                and path == "pillow-rs/src/color.rs"
                and function_name in PUBLIC_PALETTE_STRING_APIS
            ):
                continue
            findings.append(Finding(path, line_number, contract, line.strip()))
    return findings


def current_findings(root: Path = ROOT) -> list[Finding]:
    findings: list[Finding] = []
    source_root = root / "pillow-rs" / "src"
    for path in sorted(source_root.rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        findings.extend(scan_source(relative, path.read_text(encoding="utf-8")))
    return findings


def baseline_findings(ref: str, root: Path = ROOT) -> list[Finding]:
    paths = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", ref, "--", "pillow-rs/src"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    findings: list[Finding] = []
    for path in paths:
        if not path.endswith(".rs"):
            continue
        content = subprocess.run(
            ["git", "show", f"{ref}:{path}"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        findings.extend(scan_source(path, content))
    return findings


def growth(
    baseline: list[Finding], current: list[Finding]
) -> dict[tuple[str, str], tuple[int, int]]:
    baseline_counts: dict[tuple[str, str], int] = {}
    current_counts: dict[tuple[str, str], int] = {}
    for finding in baseline:
        key = (finding.path, finding.contract)
        baseline_counts[key] = baseline_counts.get(key, 0) + 1
    for finding in current:
        key = (finding.path, finding.contract)
        current_counts[key] = current_counts.get(key, 0) + 1
    return {
        key: (baseline_counts.get(key, 0), count)
        for key, count in current_counts.items()
        if count > baseline_counts.get(key, 0)
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--baseline-ref",
        default="HEAD",
        help="Git revision whose per-file findings define the existing debt baseline",
    )
    args = parser.parse_args(argv)
    try:
        baseline = baseline_findings(args.baseline_ref)
        current = current_findings()
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"Source-contract check could not read baseline {args.baseline_ref!r}: {exc}", file=sys.stderr)
        return 2

    baseline_counts = {name: 0 for name in CONTRACTS}
    current_counts = {name: 0 for name in CONTRACTS}
    for finding in baseline:
        baseline_counts[finding.contract] += 1
    for finding in current:
        current_counts[finding.contract] += 1
    print("Source-contract findings (production code only; test fixtures excluded):")
    for contract in CONTRACTS:
        print(
            f"  {contract}: {current_counts[contract]} current / "
            f"{baseline_counts[contract]} baseline"
        )

    violations = growth(baseline, current)
    if violations:
        print("Source-contract ratchet violations:", file=sys.stderr)
        for (path, contract), (before, after) in sorted(violations.items()):
            print(f"  {path}: {contract} grew from {before} to {after}", file=sys.stderr)
            for finding in current:
                if finding.path == path and finding.contract == contract:
                    print(f"    {finding.line}: {finding.text}", file=sys.stderr)
        return 1
    print(f"No source-contract category grew relative to {args.baseline_ref}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
