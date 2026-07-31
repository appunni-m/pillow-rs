#!/usr/bin/env python3
"""Verify deterministic regeneration of all active migration inputs."""

from __future__ import annotations

import tempfile
from pathlib import Path

from build_migration_parity_inputs import (
    DEFAULT_MANIFEST,
    FIXTURE_ROOT,
    build_inputs,
    load_manifest,
)


def main() -> None:
    manifest = load_manifest(DEFAULT_MANIFEST)
    active_root = FIXTURE_ROOT
    with tempfile.TemporaryDirectory(prefix="migration-parity-inputs-") as directory:
        generated_root = Path(directory)
        build_inputs(manifest, generated_root, FIXTURE_ROOT / "assets")
        for lane, relative_paths in manifest["input_index"].items():
            for relative in relative_paths:
                active = active_root / relative
                generated = generated_root / relative
                if active.read_bytes() != generated.read_bytes():
                    raise SystemExit(
                        f"input drift in {lane}: {relative} differs from generator"
                    )
    print("migration parity inputs reproduce exactly")


if __name__ == "__main__":
    main()
