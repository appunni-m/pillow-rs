#!/usr/bin/env python3
"""Check that the abi3 Python facade imports on the selected interpreter."""

from __future__ import annotations

import importlib
import sys


MODULES = (
    "pillow_rs",
    "pillow_rs.imagedraw",
    "pillow_rs.operations",
)


def main() -> int:
    for module in MODULES:
        importlib.import_module(module)
    print(f"Python facade imports passed on {sys.version.split()[0]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
