#!/usr/bin/env python3
"""Build compact TrueType bytecode edge fixtures from the hinter matrix."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "glyf"


def save_font(name: str, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def empty_program() -> Program:
    program = Program()
    program.fromBytecode(b"")
    return program


def write_empty_fpgm() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = empty_program()
    save_font("hinter-empty-fpgm.ttf", font)


def main() -> None:
    write_empty_fpgm()


if __name__ == "__main__":
    main()
