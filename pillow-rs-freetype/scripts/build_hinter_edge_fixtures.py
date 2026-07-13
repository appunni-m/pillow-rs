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
    return program_from_bytes(b"")


def program_from_bytes(bytecode: bytes) -> Program:
    program = Program()
    program.fromBytecode(bytecode)
    return program


def write_empty_fpgm() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = empty_program()
    save_font("hinter-empty-fpgm.ttf", font)


def write_prep_definitions() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    # Existing prep sets INSTCTRL, then these no-output definitions exercise
    # range-0 FDEF and IDEF scanning without changing glyph points.
    prep += bytes.fromhex("b0 02 2c b0 01 21 2d")
    prep += bytes.fromhex("b0 84 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-definitions.ttf", font)


def write_prep_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    prep += bytes.fromhex("b0 84 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-idef.ttf", font)


def write_fpgm_loopcall() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b1 02 01 2a"))
    save_font("hinter-fpgm-loopcall.ttf", font)


def write_fpgm_nested_fdef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    fpgm = font["fpgm"].program.getBytecode()
    # Define an unused nested FDEF to exercise FreeType's Nested_DEFS error.
    fpgm += bytes.fromhex("b0 00 2c b0 00 2c b0 01 21 2d 2d")
    font["fpgm"].program = program_from_bytes(fpgm)
    save_font("hinter-fpgm-nested-fdef.ttf", font)


def write_fpgm_idef_opcode_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b8 01 00 89"))
    save_font("hinter-fpgm-idef-opcode-overflow.ttf", font)


def write_fpgm_nested_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 84 89 b0 85 89 2d"))
    save_font("hinter-fpgm-nested-idef.ttf", font)


def write_fpgm_unterminated_fdef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 00 2c"))
    save_font("hinter-fpgm-unterminated-fdef.ttf", font)


def write_fpgm_unterminated_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 84 89 b0 01 21"))
    save_font("hinter-fpgm-unterminated-idef.ttf", font)


def main() -> None:
    write_empty_fpgm()
    write_prep_definitions()
    write_prep_idef()
    write_fpgm_loopcall()
    write_fpgm_nested_fdef()
    write_fpgm_idef_opcode_overflow()
    write_fpgm_nested_idef()
    write_fpgm_unterminated_fdef()
    write_fpgm_unterminated_idef()


if __name__ == "__main__":
    main()
