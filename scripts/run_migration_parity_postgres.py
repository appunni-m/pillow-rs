"""Run Pillow parity incrementally through the generic PostgreSQL store."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys

from scripts import run_migration_parity as parity
from scripts.migration_parity.postgres_adapter import PROJECT_ID, import_manifest
from scripts.migration_parity.postgres_campaign import build_pillow_adapters
from scripts.parity_store import PostgresStore, run_target_profile

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ENV_FILE = ROOT / "build" / "parity-store" / "local.env"
DEFAULT_MAXIMUM_CASES = 128
DEFAULT_MAXIMUM_ESTIMATED_BYTES = 32 * 1024 * 1024


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    _load_env_file(args.env_file)
    profile_id = args.profile or parity.target_profile_for_backend(parity.TARGET_BACKEND)
    _validate_selection(args)

    store = PostgresStore.connect(_database_dsn())
    try:
        store.migrate()
        import_selection = args.case_id if args.limit is None else None
        manifest, specification_digest, import_summary = import_manifest(
            store, args.manifest, case_ids=import_selection
        )
        case_ids = args.case_id
        if args.limit is not None:
            with store.connection.transaction():
                selected = store.selected_case_digests(
                    PROJECT_ID,
                    specification_digest,
                    profile_id=profile_id,
                )
            case_ids = [case_id for case_id, _digest in selected[: args.limit]]
        source, target, comparator = build_pillow_adapters(
            manifest, args.manifest.resolve(), profile_id
        )
        summary = run_target_profile(
            store,
            project_id=PROJECT_ID,
            specification_digest=specification_digest,
            profile_id=profile_id,
            source=source,
            target=target,
            comparator=comparator,
            host_description=_host_description(),
            case_ids=case_ids,
            maximum_cases=args.maximum_cases,
            maximum_estimated_bytes=args.maximum_estimated_bytes,
        )
        outcomes = " ".join(
            f"{name}={count}" for name, count in summary.outcomes.items()
        )
        import_details = (
            f"import-scope={import_summary['import_scope']} "
            f"import-selected={import_summary['selected_cases']} "
            f"imported={import_summary['new_cases']} "
            f"reused={import_summary['already_imported_cases']}"
        )
        if import_summary["filtered_cases"] is not None:
            import_details += f" import-filtered={import_summary['filtered_cases']}"
        print(
            f"run={summary.run_id} profile={summary.profile_id} "
            f"cases={summary.selected_cases} batches={summary.completed_batches} "
            f"{outcomes} {import_details}"
        )
        return (
            0
            if summary.outcomes.get("fail", 0) == 0
            and summary.outcomes.get("not_run", 0) == 0
            else 1
        )
    finally:
        store.close()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=parity.DEFAULT_MANIFEST)
    parser.add_argument("--profile")
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--maximum-cases", type=int, default=DEFAULT_MAXIMUM_CASES)
    parser.add_argument(
        "--maximum-estimated-bytes",
        type=int,
        default=DEFAULT_MAXIMUM_ESTIMATED_BYTES,
    )
    parser.add_argument("--env-file", type=Path, default=DEFAULT_ENV_FILE)
    return parser


def _validate_selection(args: argparse.Namespace) -> None:
    if args.case_id and args.limit is not None:
        raise ValueError("use either --case-id or --limit, not both")
    if args.limit is not None and args.limit <= 0:
        raise ValueError("--limit must be positive")
    if args.maximum_cases <= 0 or args.maximum_estimated_bytes <= 0:
        raise ValueError("batch limits must be positive")


def _load_env_file(path: Path) -> None:
    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip())


def _database_dsn() -> str:
    configured = os.environ.get("PARITY_STORE_DSN")
    if configured:
        return configured
    required = os.environ.get("PARITY_STORE_PASSWORD")
    if not required:
        raise RuntimeError(
            "set PARITY_STORE_DSN or PARITY_STORE_PASSWORD (local defaults are in "
            f"{DEFAULT_ENV_FILE})"
        )
    try:
        from psycopg.conninfo import make_conninfo
    except ModuleNotFoundError as exc:
        raise RuntimeError("install requirements-parity-store.txt first") from exc
    return make_conninfo(
        host=os.environ.get("PARITY_STORE_HOST", "127.0.0.1"),
        port=os.environ.get("PARITY_STORE_PORT", "55432"),
        dbname=os.environ.get("PARITY_STORE_DB", "parity"),
        user=os.environ.get("PARITY_STORE_USER", "parity"),
        password=required,
    )


def _host_description() -> str:
    import platform

    return f"{platform.system()} {platform.release()} {platform.machine()}"


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, RuntimeError) as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2) from exc
