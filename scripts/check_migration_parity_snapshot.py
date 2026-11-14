#!/usr/bin/env python3
"""Verify the versioned recipe and generated migration-parity input snapshot."""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
from pathlib import Path

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "pillow-rs" / "tests" / "fixtures"
LOCK = FIXTURES / "input-corpus.lock"
PYTHON_VERSION = "3.12.10"
RECIPE_FILES = (
    ROOT / "scripts" / "build_migration_parity_inputs.py",
    ROOT / "scripts" / "validate_migration_parity_contract.py",
    ROOT / "requirements-ci.txt",
    FIXTURES / "manifest.yaml",
)
RECIPE_MODULES = (
    "channel_inputs.py",
    "display_inputs.py",
    "gpu_batch_inputs.py",
    "mode_size_matrix.py",
    "pipeline_contract.py",
    "quantize_inputs.py",
    "target_output_contracts.py",
)
CORPUS_ROOTS = (FIXTURES / "inputs", FIXTURES / "assets" / "generated")
LOCK_SCHEMA = "migration-parity/input-snapshot-lock@1"


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _files_under(root: Path, *, excluded_dirs: frozenset[str] = frozenset()):
    if not root.exists():
        return []
    if root.is_symlink() or not root.is_dir():
        raise SystemExit(f"expected a regular directory: {root.relative_to(ROOT)}")
    files = []
    for current, directories, names in os.walk(root, followlinks=False):
        current_path = Path(current)
        directories[:] = sorted(
            name for name in directories if name not in excluded_dirs
        )
        for name in directories:
            if (current_path / name).is_symlink():
                raise SystemExit(f"symlink in input snapshot: {current_path / name}")
        for name in names:
            path = current_path / name
            if path.is_symlink() or not path.is_file():
                raise SystemExit(f"non-regular file in input snapshot: {path}")
            files.append(path)
    return sorted(files, key=lambda path: path.relative_to(ROOT).as_posix())


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _recipe_files() -> list[Path]:
    files = list(RECIPE_FILES)
    modules_root = ROOT / "scripts" / "migration_parity"
    files.extend(modules_root / name for name in RECIPE_MODULES)
    files.extend(_files_under(FIXTURES / "assets", excluded_dirs=frozenset({"generated"})))
    missing = [path for path in files if not path.is_file()]
    if missing:
        raise SystemExit("missing recipe input(s): " + ", ".join(map(str, missing)))
    return sorted(set(files), key=lambda path: path.relative_to(ROOT).as_posix())


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _frame(digest, value: bytes) -> None:
    digest.update(len(value).to_bytes(8, "big"))
    digest.update(value)


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _recipe_digest() -> str:
    digest = hashlib.sha256()
    _frame(digest, LOCK_SCHEMA.encode())
    _frame(digest, f"python={PYTHON_VERSION}".encode())
    for path in _recipe_files():
        content_digest, size = _sha256_file(path)
        _frame(digest, path.relative_to(ROOT).as_posix().encode())
        _frame(digest, size.to_bytes(8, "big"))
        _frame(digest, bytes.fromhex(content_digest))
    return digest.hexdigest()


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _corpus_files() -> list[Path]:
    files = []
    for root in CORPUS_ROOTS:
        files.extend(_files_under(root))
    return sorted(files, key=lambda path: path.relative_to(FIXTURES).as_posix())


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _corpus_snapshot(*, include_content: bool) -> dict[str, str | int]:
    files = _corpus_files()
    paths_digest = hashlib.sha256()
    tree_digest = hashlib.sha256()
    total_bytes = 0
    for path in files:
        relative = path.relative_to(FIXTURES).as_posix()
        size = path.stat().st_size
        total_bytes += size
        _frame(paths_digest, relative.encode())
        _frame(paths_digest, size.to_bytes(8, "big"))
        if include_content:
            content_digest, actual_size = _sha256_file(path)
            if actual_size != size:
                raise SystemExit(f"file changed while hashing: {relative}")
            if relative.startswith("assets/generated/"):
                if Path(relative).stem != content_digest:
                    raise SystemExit(f"generated asset is not content-addressed: {relative}")
            _frame(tree_digest, relative.encode())
            _frame(tree_digest, size.to_bytes(8, "big"))
            _frame(tree_digest, bytes.fromhex(content_digest))
    return {
        "file_count": len(files),
        "total_bytes": total_bytes,
        "paths_sha256": paths_digest.hexdigest(),
        "tree_sha256": tree_digest.hexdigest() if include_content else "",
    }


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _read_lock() -> dict[str, str]:
    try:
        entries = [line.split("=", 1) for line in LOCK.read_text().splitlines()]
    except OSError as exc:
        raise SystemExit(f"cannot read {LOCK.relative_to(ROOT)}: {exc}") from exc
    if any(len(entry) != 2 for entry in entries):
        raise SystemExit(f"malformed snapshot lock: {LOCK.relative_to(ROOT)}")
    lock = dict(entries)
    expected = {
        "schema",
        "producer_python",
        "recipe_sha256",
        "paths_sha256",
        "tree_sha256",
        "file_count",
        "total_bytes",
    }
    if len(lock) != len(entries) or set(lock) != expected:
        raise SystemExit(f"unexpected snapshot lock fields: {LOCK.relative_to(ROOT)}")
    if lock["schema"] != LOCK_SCHEMA or lock["producer_python"] != PYTHON_VERSION:
        raise SystemExit(f"unsupported snapshot lock version: {LOCK.relative_to(ROOT)}")
    return lock


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _verify_recipe(lock: dict[str, str]) -> None:
    actual = _recipe_digest()
    if actual != lock["recipe_sha256"]:
        raise SystemExit(
            "input recipe changed since the corpus lock was recorded; regenerate the "
            "inputs and update pillow-rs/tests/fixtures/input-corpus.lock"
        )


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def _write_lock() -> None:
    snapshot = _corpus_snapshot(include_content=True)
    values = {
        "schema": LOCK_SCHEMA,
        "producer_python": PYTHON_VERSION,
        "recipe_sha256": _recipe_digest(),
        "paths_sha256": snapshot["paths_sha256"],
        "tree_sha256": snapshot["tree_sha256"],
        "file_count": snapshot["file_count"],
        "total_bytes": snapshot["total_bytes"],
    }
    content = "".join(f"{key}={value}\n" for key, value in values.items())
    temporary = LOCK.with_suffix(".lock.tmp")
    temporary.write_text(content)
    temporary.replace(LOCK)
    print(f"updated {LOCK.relative_to(ROOT)}")


# VERIFY: source=reviewed@2026-11-14; parity=n/a; benchmark=n/a
def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--write-lock", action="store_true")
    group.add_argument("--recipe-only", action="store_true")
    group.add_argument("--metadata-only", action="store_true")
    args = parser.parse_args()
    if args.write_lock:
        _write_lock()
        return
    lock = _read_lock()
    _verify_recipe(lock)
    if args.recipe_only:
        print("input recipe matches the versioned corpus lock")
        return
    snapshot = _corpus_snapshot(include_content=not args.metadata_only)
    for field in ("file_count", "total_bytes", "paths_sha256"):
        if str(snapshot[field]) != lock[field]:
            raise SystemExit(f"input snapshot {field} differs from the versioned lock")
    if not args.metadata_only and snapshot["tree_sha256"] != lock["tree_sha256"]:
        raise SystemExit("input snapshot content differs from the versioned lock")
    mode = "metadata" if args.metadata_only else "content"
    print(f"{mode} input snapshot matches the versioned corpus lock")


if __name__ == "__main__":
    main()
