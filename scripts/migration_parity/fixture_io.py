"""Read logical fixture paths from raw files or deterministic gzip snapshots."""

from __future__ import annotations

import gzip
import hashlib
import json
from pathlib import Path
from typing import Any, BinaryIO, TextIO


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def stored_path(path: Path) -> Path:
    """Prefer the compressed representation for one logical fixture path."""
    if path.name.endswith(".gz"):
        return path
    compressed = path.with_name(path.name + ".gz")
    if compressed.is_file():
        return compressed
    return path


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def exists(path: Path) -> bool:
    """Return whether a logical raw or compressed fixture is available."""
    return stored_path(path).is_file()


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def open_binary(path: Path) -> BinaryIO:
    """Open a logical fixture path, streaming gzip content when present."""
    physical = stored_path(path)
    if physical.name.endswith(".gz"):
        return gzip.open(physical, "rb")
    return physical.open("rb")


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def open_text(path: Path, *, encoding: str = "utf-8") -> TextIO:
    """Open a logical text fixture path, streaming gzip content when present."""
    physical = stored_path(path)
    if physical.name.endswith(".gz"):
        return gzip.open(physical, "rt", encoding=encoding)
    return physical.open("r", encoding=encoding)


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def read_bytes(path: Path) -> bytes:
    """Read the uncompressed bytes represented by a fixture path."""
    with open_binary(path) as stream:
        return stream.read()


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def read_text(path: Path, *, encoding: str = "utf-8") -> str:
    """Read the uncompressed text represented by a fixture path."""
    with open_text(path, encoding=encoding) as stream:
        return stream.read()


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def read_json(path: Path) -> Any:
    """Decode one generated JSON fixture from raw or compressed storage."""
    with open_text(path) as stream:
        return json.load(stream)


# VERIFY: source=reviewed@2026-11-14; parity=pending; benchmark=n/a
def sha256(path: Path) -> tuple[bytes, int]:
    """Hash the logical uncompressed bytes and return their size."""
    digest = hashlib.sha256()
    size = 0
    with open_binary(path) as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    return digest.digest(), size
