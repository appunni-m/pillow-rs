"""Local configuration helpers for the reusable parity input store."""

from __future__ import annotations

import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ENV_FILE = ROOT / "build" / "parity-store" / "local.env"


def load_env_file(path: Path) -> None:
    """Load simple KEY=VALUE entries without replacing process configuration."""

    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip())


def database_dsn() -> str:
    """Resolve a configured DSN or the local Docker connection fields."""

    configured = os.environ.get("PARITY_STORE_DSN")
    if configured:
        return configured
    password = os.environ.get("PARITY_STORE_PASSWORD")
    if not password:
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
        password=password,
    )
