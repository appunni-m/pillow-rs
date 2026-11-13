"""Create local-only PostgreSQL credentials without overwriting existing ones."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import secrets
import sys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("build/parity-store/local.env"),
    )
    args = parser.parse_args(argv)
    path = args.output
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    contents = (
        "PARITY_STORE_DB=parity\n"
        "PARITY_STORE_USER=parity\n"
        f"PARITY_STORE_PASSWORD={secrets.token_hex(32)}\n"
        "PARITY_STORE_PORT=55432\n"
    )
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        print(f"Kept existing local credentials at {path}")
        return 0
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        output.write(contents)
    print(f"Created local credentials at {path}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except OSError as exc:
        print(f"could not create local parity-store credentials: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
