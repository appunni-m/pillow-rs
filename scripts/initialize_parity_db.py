"""Start-up helper that applies parity-store migrations to PostgreSQL."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

from scripts.parity_store.connection import DEFAULT_ENV_FILE, database_dsn, load_env_file
from scripts.parity_store.postgres import PostgresStore


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--env-file", type=Path, default=DEFAULT_ENV_FILE)
    args = parser.parse_args(argv)
    load_env_file(args.env_file)

    store = PostgresStore.connect(database_dsn())
    try:
        store.migrate()
    finally:
        store.close()
    print("Parity database schema is current; existing rows and volume are retained.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2) from exc
