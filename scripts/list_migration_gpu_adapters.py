#!/usr/bin/env python3
"""List native WGPU adapters available to the built pillow-rs extension."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "pillow-rs-py" / "python"))

ADAPTER_FIELDS = (
    "selection_index",
    "name",
    "vendor",
    "device",
    "device_type",
    "backend",
    "driver",
    "driver_info",
)


def adapter_inventory() -> list[dict[str, Any]]:
    """Return the extension's WGPU adapter inventory as labeled records."""

    from pillow_rs import _core

    query = getattr(_core, "_migration_gpu_adapter_inventory", None)
    if not callable(query):
        raise RuntimeError(
            "the built pillow-rs extension does not expose GPU adapter inventory"
        )
    inventory = []
    for row in query():
        if not isinstance(row, (tuple, list)) or len(row) != len(ADAPTER_FIELDS):
            raise RuntimeError("GPU adapter inventory returned a malformed row")
        record = dict(zip(ADAPTER_FIELDS, row, strict=True))
        if not isinstance(record["selection_index"], int):
            raise RuntimeError("GPU adapter inventory omitted an adapter index")
        inventory.append(record)
    return inventory


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--indices",
        action="store_true",
        help="print adapter indices as a space-separated list",
    )
    args = parser.parse_args()
    try:
        inventory = adapter_inventory()
    except Exception as exc:
        parser.exit(2, f"{exc}\n")
    if not inventory:
        parser.exit(1, "no native WGPU adapters are available\n")
    if args.indices:
        print(" ".join(str(item["selection_index"]) for item in inventory))
    else:
        print(json.dumps(inventory, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
