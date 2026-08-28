#!/usr/bin/env python3
"""Collect Python source coverage for the pinned Pillow oracle.

The target coverage lanes instrument pillow-rs.  This audit lane runs the same
manifest-selected public workflows against Pillow itself and measures only
Pillow's Python package with coverage.py.  Pillow's native C extension is not
line-instrumented by coverage.py and is therefore reported separately as an
unmeasured implementation layer rather than being mistaken for Python source
coverage.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from typing import Any

import coverage

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "pillow-oracle-coverage-result.json"
DEFAULT_REPORT = ROOT / "target" / "coverage" / "migration-parity-pillow.json"
DEFAULT_DATA = ROOT / "target" / "coverage" / ".migration-parity-pillow"
ORACLE_VERSION = "12.2.0"

sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_coverage import (  # noqa: E402
    coverage_case_failed,
    coverage_not_applicable_operations,
    load_coverage_plans,
    scope_coverage_plans,
)
from run_migration_parity import (  # noqa: E402
    build_operation_index,
    load_cases,
    load_manifest,
    run_case,
)


def pillow_root() -> tuple[Path, str]:
    spec = importlib.util.find_spec("PIL")
    if spec is None or not spec.submodule_search_locations:
        raise RuntimeError("Pillow is not importable in the selected Python environment")
    import PIL

    version = str(getattr(PIL, "__version__", ""))
    if version != ORACLE_VERSION:
        raise RuntimeError(
            f"Pillow oracle version {version!r}, expected {ORACLE_VERSION}"
        )
    return Path(next(iter(spec.submodule_search_locations))).resolve(), version


def select_public_parity_cases(
    cases_by_id: dict[str, dict[str, Any]],
    *,
    same_parity_corpus: bool,
    case_ids: list[str] | None,
    operation: str | None,
    exclude_case_ids: list[str] | None,
    excluded_operations: set[tuple[str, str]],
) -> set[str]:
    """Select the corpus used by the oracle coverage lane.

    Indexed coverage plans are useful for attributing requirements, but they
    are not the public-input corpus: nuanced parity cases may intentionally be
    kept outside a plan.  The same-parity-corpus mode therefore starts from
    every active parity case and applies only the explicit user scope.  This
    keeps Pillow coverage comparable with the canonical parity runner.
    """

    requested = set(case_ids or [])
    excluded = set(exclude_case_ids or [])
    missing_requested = requested - cases_by_id.keys()
    if missing_requested:
        raise ValueError(
            f"coverage selects missing parity cases: {sorted(missing_requested)[:5]}"
        )
    missing_excluded = excluded - cases_by_id.keys()
    if missing_excluded:
        raise ValueError(
            f"coverage excludes missing parity cases: {sorted(missing_excluded)[:5]}"
        )

    if same_parity_corpus:
        selected = {
            case_id
            for case_id, case in cases_by_id.items()
            if (case.get("surface"), case.get("operation")) not in excluded_operations
        }
    else:
        # Preserve the indexed-plan behavior for callers that need the old
        # bounded audit explicitly.
        selected = {
            case_id
            for case_id, case in cases_by_id.items()
            if (case.get("surface"), case.get("operation")) not in excluded_operations
        }

    if operation is not None:
        selected = {
            case_id for case_id in selected if case_id.startswith(f"{operation}.")
        }
    if requested:
        selected &= requested
    selected -= excluded
    if not selected:
        scope = operation or ", ".join(sorted(requested)) or "public parity corpus"
        raise ValueError(f"oracle coverage scope selected no active parity cases: {scope}")
    return selected


def run(args: argparse.Namespace) -> int:
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    plans, plan_paths = load_coverage_plans(manifest)
    cases, case_inputs = load_cases(manifest, case_ids=None, surface=None)
    cases_by_id = {case["case_id"]: case for case in cases}
    excluded_operations = coverage_not_applicable_operations(manifest)
    if args.same_parity_corpus:
        # ``not_applicable`` controls source-coverage attribution, not whether
        # the public workflow exists.  The reverse lane must execute the same
        # public corpus as the Rust and WASM lanes, including Qt-only methods;
        # those methods may produce a normal Pillow public error in this
        # environment and remain visible as a parity case.
        selected_ids = select_public_parity_cases(
            cases_by_id,
            same_parity_corpus=True,
            case_ids=args.case_id,
            operation=args.operation,
            exclude_case_ids=args.exclude_case_id,
            excluded_operations=set(),
        )
        # Plans remain useful as an attribution index, but are not allowed to
        # decide which public workflows execute in this mode.
        selected_plan_ids = {
            case_id
            for plan in plans
            for case_id in plan["selectors"]["parity_case_ids"]
            if case_id in selected_ids
        }
        plans = [
            plan
            for plan in plans
            if any(case_id in selected_ids for case_id in plan["selectors"]["parity_case_ids"])
        ]
        plans_selected_from_corpus = len(selected_plan_ids)
    else:
        plans, selected_ids = scope_coverage_plans(
            plans,
            cases_by_id,
            case_ids=set(args.case_id) if args.case_id else None,
            operation=args.operation,
            exclude_case_ids=set(args.exclude_case_id) if args.exclude_case_id else None,
            excluded_operations=excluded_operations,
        )
        plans_selected_from_corpus = len(selected_ids)
    if not plans or not selected_ids:
        raise ValueError("oracle coverage scope selected no public workflows")

    source_root, oracle_version = pillow_root()
    coverage_report_path = args.coverage_report.resolve()
    coverage_data_path = args.coverage_data.resolve()
    output_path = args.output.resolve()
    coverage_report_path.parent.mkdir(parents=True, exist_ok=True)
    coverage_data_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    cov = coverage.Coverage(
        data_file=str(coverage_data_path),
        branch=True,
        source=[str(source_root)],
    )
    case_failures: list[str] = []
    infrastructure_errors: list[dict[str, str]] = []
    with tempfile.TemporaryDirectory(prefix="migration-pillow-coverage-") as temporary:
        cov.erase()
        cov.start()
        try:
            operation_index = build_operation_index(manifest)
            for case_id in sorted(selected_ids):
                try:
                    result = run_case(
                        "source",
                        cases_by_id[case_id],
                        operation_index,
                        Path(temporary),
                    )
                except BaseException as exc:
                    infrastructure_errors.append(
                        {
                            "case_id": case_id,
                            "kind": type(exc).__name__,
                            "message": str(exc),
                        }
                    )
                    continue
                if coverage_case_failed(result["observations"]):
                    case_failures.append(case_id)
        finally:
            cov.stop()
            cov.save()

    cov.json_report(outfile=str(coverage_report_path), pretty_print=True)
    report = json.loads(coverage_report_path.read_text(encoding="utf-8"))
    totals: dict[str, Any] = report.get("totals", {})
    input_paths = sorted(
        set(plan_paths.values())
        | {case_inputs[case_id] for case_id in selected_ids}
    )
    result = {
        "schema": "pillow-rs/pillow-oracle-coverage@1",
        "oracle": {
            "name": "Pillow",
            "version": oracle_version,
            "runtime": "CPython 3.12",
            "source_root": str(source_root),
            "native_extension_note": (
                "coverage.py measures Pillow Python files; native C extension code "
                "is not included in this report"
            ),
        },
        "manifest": {
            "path": str(manifest_path.relative_to(ROOT)),
            "input_paths": input_paths,
        },
        "execution": {
            "plans_selected": len(plans),
            "plan_case_ids_selected": plans_selected_from_corpus,
            "selection_mode": (
                "same_public_parity_corpus"
                if args.same_parity_corpus
                else "indexed_coverage_plans"
            ),
            "cases_selected": len(selected_ids),
            "cases_executed": len(selected_ids) - len(infrastructure_errors),
            "cases_with_incomplete_workflow": len(case_failures),
            "infrastructure_errors": infrastructure_errors,
            "incomplete_case_ids": case_failures,
        },
        "coverage": {
            "collector": "coverage.py",
            "version": coverage.__version__,
            "report": str(coverage_report_path.relative_to(ROOT)),
            "totals": totals,
        },
    }
    output_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result["execution"], sort_keys=True))
    print(json.dumps(totals, sort_keys=True))
    return 2 if infrastructure_errors else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--operation")
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--exclude-case-id", action="append")
    parser.add_argument(
        "--same-parity-corpus",
        action="store_true",
        help="run every active public parity case, including cases outside coverage plans",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--coverage-report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--coverage-data", type=Path, default=DEFAULT_DATA)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
