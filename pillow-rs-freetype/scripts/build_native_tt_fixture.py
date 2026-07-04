#!/usr/bin/env python3
"""Generate the native_tt_default fixture from vendored FreeType.

This wrapper exists for the plan's historical command name.  New fixture
families should call `scripts/build_ft_fixture.py` directly.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    script = ROOT / "scripts/build_ft_fixture.py"
    return subprocess.run(
        [
            sys.executable,
            str(script),
            "--family",
            "native_tt_default",
            *sys.argv[1:],
        ],
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
