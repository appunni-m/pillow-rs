#!/usr/bin/env python3
"""Build small source-backed variable fonts for FTMM future parity rows."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.varLib import instancer


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "variable" / "compact-variable.ttf"
MVAR_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "variation" / "mvar-vertical-metrics.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "variable"


def save_font(path: Path, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=True)


def write_inter_wght() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Keep a real variable font, but pin the width axis.  This leaves the
    # source-backed `wght` axis, fvar named instances, gvar, and HVAR data for
    # the single-axis FTMM coordinate rows.
    font = instancer.instantiateVariableFont(font, {"wdth": 100.0}, inplace=False)
    save_font(OUT_DIR / "inter-wght.ttf", font)


def write_compact_alias(name: str) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    save_font(OUT_DIR / name, font)


def write_mvar_alias(name: str) -> None:
    font = TTFont(MVAR_FONT, recalcTimestamp=False)
    save_font(OUT_DIR / name, font)


def main() -> None:
    write_inter_wght()
    write_compact_alias("multi-axis-named-instances.ttf")
    write_compact_alias("named-instances-wght-wdth.ttf")
    write_compact_alias("named-instance-missing-psid.ttf")
    write_compact_alias("gvar-hvar-wght.ttf")
    write_mvar_alias("mvar-hvar-vvar.ttf")


if __name__ == "__main__":
    main()
