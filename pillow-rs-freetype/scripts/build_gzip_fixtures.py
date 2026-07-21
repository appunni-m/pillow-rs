#!/usr/bin/env python3
"""Build deterministic gzip/zlib payload fixtures for public ftgzip parity."""

from __future__ import annotations

import gzip
import json
from pathlib import Path
import zlib


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tests" / "fixtures" / "compressed" / "gzip"


PAYLOADS = {
    "small_text": b"fontdone gzip parity fixture\nsmall text payload\n",
    "empty": b"",
}


def write_if_changed(path: Path, data: bytes) -> None:
    if path.exists() and path.read_bytes() == data:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    manifest_payloads = []
    for payload_id, raw in PAYLOADS.items():
        stem = payload_id.replace("_", "-")
        raw_path = OUT / f"{stem}.raw"
        gzip_path = OUT / f"{stem}.gz"
        zlib_path = OUT / f"{stem}.zlib"
        write_if_changed(raw_path, raw)
        write_if_changed(gzip_path, gzip.compress(raw, compresslevel=9, mtime=0))
        write_if_changed(zlib_path, zlib.compress(raw, level=9))
        manifest_payloads.append(
            {
                "id": payload_id,
                "raw": f"compressed/gzip/{stem}.raw",
                "gzip": f"compressed/gzip/{stem}.gz",
                "zlib_wrapped": f"compressed/gzip/{stem}.zlib",
            }
        )

    manifest = {
        "version": 1,
        "source": "scripts/build_gzip_fixtures.py",
        "payloads": manifest_payloads,
    }
    manifest_path = OUT / "small-text-and-empty-payloads.json"
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(manifest_path, encoded)


if __name__ == "__main__":
    main()
