#!/usr/bin/env python3
"""Generate the specification and evidence views for migration parity.

The specification page is derived only from the manifest and indexed inputs.
The evidence pages consume the strict aggregate status report and label every
statement as measured, declared, or not proven.  No generated page is an input
to a lane producer.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_STATUS = ROOT / "build" / "migration-parity" / "status-report.json"
DEFAULT_OUTPUT_DIR = ROOT / "docs" / "generated"
GENERATOR = "scripts/generate_migration_parity_docs.py@1"

sys.path.insert(0, str(Path(__file__).resolve().parent))
from aggregate_migration_parity import (  # noqa: E402
    load_inputs,
    operation_records,
    sha256,
)
from run_migration_parity import load_manifest  # noqa: E402
from validate_migration_parity_result import status_report as validate_status  # noqa: E402


def relative(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT.resolve()).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def marker(manifest: dict[str, Any], manifest_path: Path) -> str:
    return "\n".join(
        (
            f"generator: {GENERATOR}",
            f"manifest_path: {relative(manifest_path)}",
            f"manifest_schema: {manifest['schema']}",
            f"manifest_sha256: {sha256(manifest_path)}",
        )
    )


def specification(manifest: dict[str, Any], manifest_path: Path, inputs: dict[str, Any]) -> str:
    operations = operation_records(manifest)
    requirements = sum(len(operation.get("requirements", [])) for _, operation in operations)
    lines = [
        "# Migration parity public contract",
        "",
        "This is the generated specification view. It contains declared public",
        "contract and indexed input mappings only; it contains no measured result.",
        "",
        "```yaml",
        marker(manifest, manifest_path),
        "statement_status: declared",
        "```",
        "",
        "## Scope",
        "",
        f"- Scope: `{manifest['scope']['id']}` (`{manifest['scope']['mode']}`)",
        f"- Oracle: `{manifest['oracles'][0]['name']} {manifest['oracles'][0]['version']}`",
        f"- Target profiles: {', '.join(f'`{item["id"]}`' for item in manifest['target_profiles'])}",
        f"- Public surfaces: {len(manifest['surfaces'])}",
        f"- Operations: {len(operations)}",
        f"- Requirements: {requirements}",
        f"- Indexed parity cases: {len(inputs['cases'])}",
        f"- Indexed coverage plans: {len(inputs['plans'])}",
        f"- Indexed benchmark workloads: {len(inputs['workloads'])}",
        "",
        "## Declared operations",
        "",
        "| Surface | Operation | Kind | Source path | Target path | Requirements |",
        "| --- | --- | --- | --- | --- | ---: |",
    ]
    for surface, operation in operations:
        target = next(
            (item for item in operation.get("targets", []) if item["target_id"] == "pillow-rs-python"),
            {"path": "—"},
        )
        lines.append(
            f"| `{surface['id']}` | `{operation['id']}` | `{operation['kind']}` | "
            f"`{operation['source']['path']}` | `{target['path']}` | "
            f"{len(operation.get('requirements', []))} |"
        )
    lines.extend(
        (
            "",
            "## Lane inputs",
            "",
            "The manifest index is closed: only the following indexed documents are",
            "inputs to the corresponding lane. Results and documentation are not",
            "accepted as input truth.",
            "",
            "| Lane | Documents |",
            "| --- | ---: |",
            f"| parity | {len(manifest['input_index']['parity'])} |",
            f"| coverage | {len(manifest['input_index']['coverage'])} |",
            f"| benchmark | {len(manifest['input_index']['benchmark'])} |",
            "",
        )
    )
    return "\n".join(lines)


def evidence_page(
    title: str,
    lane: str,
    manifest: dict[str, Any],
    manifest_path: Path,
    status: dict[str, Any],
) -> str:
    completeness = [item for item in status["completeness"] if item["dimension"].startswith(lane)]
    if lane == "parity":
        completeness = [item for item in status["completeness"] if item["dimension"] == "parity_outcome"]
    elif lane == "coverage":
        completeness = [item for item in status["completeness"] if item["dimension"].endswith("_coverage") or item["dimension"] == "coverage_input_mapping"]
    else:
        completeness = [item for item in status["completeness"] if item["dimension"] in {"benchmark_input_mapping", "benchmark_budget_outcome"}]
    operations = [item for item in status["operations"] if item[lane]["applicability"] != "not_applicable"]
    outcomes = Counter(item[lane]["outcome"] for item in operations)
    evidence_ids = sorted(
        {
            item[lane]["evidence_id"]
            for item in operations
            if item[lane]["evidence_id"] is not None
        }
    )
    lines = [
        f"# {title}",
        "",
        "This is a generated evidence view. It never changes the manifest or",
        "lane inputs, and it does not turn missing evidence into a pass.",
        "",
        "```yaml",
        marker(manifest, manifest_path),
        f"lane: {lane}",
        "```",
        "",
        "## Evidence state",
        "",
        f"- Compatible evidence IDs: {', '.join(f'`{item}`' for item in evidence_ids) if evidence_ids else 'none'}",
        f"- Operation outcomes: {', '.join(f'{key}={outcomes[key]}' for key in sorted(outcomes)) if outcomes else 'none'}",
        f"- Stale/incompatible artifacts: {len(status['stale_or_incompatible_evidence'])}",
        "",
        "| Dimension | Target profile | Covered | Total | Evidence ID |",
        "| --- | --- | ---: | ---: | --- |",
    ]
    for item in completeness:
        lines.append(
            f"| `{item['dimension']}` | `{item['target_profile'] or 'all'}` | "
            f"{item['numerator']} | {item['denominator']} | "
            f"`{item['evidence_id'] or 'not_proven'}` |"
        )
    lines.extend(
        (
            "",
            "## Interpretation",
            "",
            "- `pass` and measured counts are evidence from a compatible run.",
            "- `not_proven` means the specification exists but the required fresh",
            "  evidence is absent, stale, dirty, or not ingested.",
            "- Static operation support is not a substitute for live parity.",
            "",
        )
    )
    return "\n".join(lines)


def run(args: argparse.Namespace) -> int:
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    inputs = load_inputs(manifest)
    status = json.loads(args.status.resolve().read_text(encoding="utf-8"))
    validate_status(status)
    if status["manifest"]["sha256"] != sha256(manifest_path):
        raise ValueError("status report does not match the active manifest")
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    files = {
        output_dir / "migration-parity-public-contract.md": specification(manifest, manifest_path, inputs),
        output_dir / "migration-parity-status.md": evidence_page("Migration parity status", "parity", manifest, manifest_path, status),
        output_dir / "migration-coverage-status.md": evidence_page("Migration coverage status", "coverage", manifest, manifest_path, status),
        output_dir / "migration-benchmark-status.md": evidence_page("Migration benchmark status", "benchmark", manifest, manifest_path, status),
    }
    for path, content in files.items():
        path.write_text(content.rstrip() + "\n", encoding="utf-8")
    print(json.dumps({"generated": [relative(path) for path in files]}, sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--status", type=Path, default=DEFAULT_STATUS)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
