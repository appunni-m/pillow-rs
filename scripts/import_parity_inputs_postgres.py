"""Import immutable parity inputs into PostgreSQL without running test cases."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

from scripts import run_migration_parity as parity
from scripts.migration_parity.pillow_fixture_adapter import import_manifest
from scripts.parity_store.connection import DEFAULT_ENV_FILE, database_dsn, load_env_file
from scripts.parity_store.postgres import PostgresStore


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=parity.DEFAULT_MANIFEST)
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--env-file", type=Path, default=DEFAULT_ENV_FILE)
    args = parser.parse_args(argv)
    load_env_file(args.env_file)

    store = PostgresStore.connect(database_dsn())
    try:
        store.migrate()
        _manifest, specification_digest, summary = import_manifest(
            store, args.manifest, case_ids=args.case_id
        )
    finally:
        store.close()

    print(
        f"snapshot={specification_digest.hex()} "
        f"scope={summary['import_scope']} "
        f"cases={summary['selected_cases']} "
        f"inserted={summary['new_cases']} "
        f"already-present={summary['already_imported_cases']} "
        f"documents={summary['documents']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2) from exc
