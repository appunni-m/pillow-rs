#!/usr/bin/env python3
"""Build compact TrueType bytecode edge fixtures from the hinter matrix."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph
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


def write_empty_glyph_iup() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    # This body waives v40 compatibility, moves pp1, then invokes IUP[x].
    # C's empty-glyph shortcut ignores the entire body before simple-glyph
    # instruction parsing, so none of those phantom mutations may be observed.
    # FontTools normally compiles a zero-contour `Glyph` to a zero-length glyf
    # record, so preserve the valid raw header, instruction length, and program.
    font["glyf"]["empty"] = Glyph(
        bytes.fromhex(
            "00 00 00 00 00 00 00 00 00 00"
            " 00 09 b1 04 03 8e b1 00 40 48 31"
        )
    )
    save_font("hinter-empty-glyph-iup.ttf", font)


def write_invalid_contour_endpoints() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    # Two contours both end at point zero.  C `TT_Load_Simple_Glyph` rejects
    # the second endpoint before reading the otherwise complete point record.
    font["glyf"]["empty"] = Glyph(
        bytes.fromhex(
            "00 02 00 00 00 00 00 00 00 00"
            " 00 00 00 00"
            " 00 02 30 31"
            " 31"
        )
    )
    save_font("hinter-invalid-contour-endpoints.ttf", font)


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


def write_prep_redefine_defs() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    # FreeType allows definitions in prep.  Redefine existing FDEF 1 and IDEF
    # 0x8F so this stays within the font's maxp definition budgets.
    prep += bytes.fromhex("b0 01 2c b0 01 21 2d")
    prep += bytes.fromhex("b0 8f 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-redefine-defs.ttf", font)


def write_fpgm_loopcall() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b1 02 01 2a"))
    save_font("hinter-fpgm-loopcall.ttf", font)


def write_fpgm_loopcall_redefinition() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # The first LOOPCALL iteration jumps into PUSHB payload bytes that redefine
    # FDEF 1.  FreeType's call record points at that mutable definition record,
    # so the second iteration runs the new WCVTP body and writes CVT 0 to 1px.
    # The outer FDEF scanner treats the embedded definition as push data and
    # therefore accepts this deliberately broken but public-reachable program.
    font["fpgm"].program = program_from_bytes(
        bytes.fromhex("b0 01 2c b0 02 1c b7 b0 01 2c b1 00 40 44 2d 2d")
    )
    font["prep"].program = program_from_bytes(bytes.fromhex("b1 02 01 2a"))
    # MIAP point 0 to CVT 0 so the redefined second iteration changes geometry.
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b1 00 00 3e"))
    save_font("hinter-fpgm-loopcall-redefinition.ttf", font)


def write_called_fpgm_instctrl() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # A prep-initiated CALL switches curRange to fpgm, but C's Ins_INSTCTRL
    # validates iniRange and therefore accepts selector 1 under pedantic mode.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 01 2c b1 00 01 8e 2d"))
    font["prep"].program = program_from_bytes(bytes.fromhex("b0 01 2b"))
    save_font("hinter-called-fpgm-instctrl.ttf", font)


def write_fpgm_call_errors() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # This single font keeps CALL/LOOPCALL error coverage compact.  Its fpgm
    # defines FDEF 1 as a self-recursive body; function 0 and -1 remain invalid
    # references while glyph 24 reaches FreeType's call-stack overflow guard.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 01 2c b0 01 2b 2d"))
    font["glyf"][".notdef"].program = program_from_bytes(bytes.fromhex("b8 ff ff 2b"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 00 2b"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("b1 01 00 2a"))
    font["glyf"]["scanType0"].program = program_from_bytes(bytes.fromhex("b0 01 2b"))
    save_font("hinter-fpgm-call-errors.ttf", font)


def write_execution_too_long_loop() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # C `TT_RunIns` stops this negative JMPR loop with Execution_Too_Long.
    # The bytecode lands back at the PUSHW so the operand stack stays bounded.
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b8 ff fd 1c"))
    save_font("hinter-execution-too-long-loop.ttf", font)


def write_fpgm_fdef_index_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # FDEF 256 is beyond the fixed TT_DefRecord array range.  FreeType rejects
    # it before scanning a function body, so no glyph points are needed.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b8 01 00 2c"))
    save_font("hinter-fpgm-fdef-index-overflow.ttf", font)


def write_idef_recursive_depth() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Redefine the existing ADJUST IDEF opcode with a body that calls itself.
    # FreeType bails out through its IDEF call-stack guard instead of looping.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 8f 89 8f 2d"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("8f"))
    save_font("hinter-idef-recursive-depth.ttf", font)


def write_storage_cvt_reference_errors() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # maxp declares two storage and two CVT entries.  C's RS/WS/RCVT/WCVTP
    # handlers ignore index 9 in normal mode and return Invalid_Reference when
    # the same public glyph load enables FT_LOAD_PEDANTIC.
    font["glyf"][".notdef"].program = program_from_bytes(bytes.fromhex("b1 09 01 42"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 09 43"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("b1 09 20 44"))
    font["glyf"]["scanType0"].program = program_from_bytes(bytes.fromhex("b0 09 45"))
    # INSTCTRL selector 1 is valid only in prep.  A glyph-range use is ignored
    # normally and reports Invalid_Reference under FT_LOAD_PEDANTIC.
    font["glyf"]["scanType2"].program = program_from_bytes(bytes.fromhex("b1 01 01 8e"))
    font["glyf"]["idefCall"].program = program_from_bytes(bytes.fromhex("b1 09 20 70"))
    # Invalid selector 4 and invalid value 1 for selector 2 are both ignored
    # normally and report Invalid_Reference under FT_LOAD_PEDANTIC.
    font["glyf"]["untouchPoint"].program = program_from_bytes(bytes.fromhex("b1 00 04 8e"))
    font["glyf"]["superRoundMatrix"].program = program_from_bytes(bytes.fromhex("b1 01 02 8e"))
    # At 20 ppem, delta base 9 makes 0xB8 applicable.  CVT index 9 isolates
    # DELTAC's normal no-op / pedantic Invalid_Reference split.
    font["glyf"]["stackStateMatrix"].program = program_from_bytes(bytes.fromhex("b2 b8 09 01 73"))
    # Keep the pedantic SHP proof independent from the branch-edge program's
    # earlier invalid-reference probes.
    font["glyf"]["instructionControl"].program = program_from_bytes(bytes.fromhex("b0 09 32"))
    save_font("hinter-storage-cvt-reference-errors.ttf", font)


def write_fpgm_nested_fdef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    fpgm = font["fpgm"].program.getBytecode()
    # Redefine existing FDEF 1 so maxp budgets are already satisfied, then put
    # a nested FDEF in the body to exercise FreeType's Nested_DEFS error.
    fpgm += bytes.fromhex("b0 01 2c b0 00 2c b0 01 21 2d 2d")
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
    write_empty_glyph_iup()
    write_invalid_contour_endpoints()
    write_prep_definitions()
    write_prep_idef()
    write_prep_redefine_defs()
    write_fpgm_loopcall()
    write_fpgm_loopcall_redefinition()
    write_called_fpgm_instctrl()
    write_fpgm_call_errors()
    write_execution_too_long_loop()
    write_fpgm_fdef_index_overflow()
    write_idef_recursive_depth()
    write_storage_cvt_reference_errors()
    write_fpgm_nested_fdef()
    write_fpgm_idef_opcode_overflow()
    write_fpgm_nested_idef()
    write_fpgm_unterminated_fdef()
    write_fpgm_unterminated_idef()


if __name__ == "__main__":
    main()
