#!/usr/bin/env python3
"""Build compact Type 1 fixtures for non-SFNT public face routes."""

from __future__ import annotations

from pathlib import Path

from fontTools.misc.psCharStrings import T1CharString
from fontTools.t1Lib import StandardEncoding, T1Font, write


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
OUT_DIR = FIXTURE_ROOT / "fonts" / "type1"
INPUT_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1"


def charstring(program: list[object]) -> T1CharString:
    return T1CharString(program=program)


def stable_generator_header(data: bytes) -> bytes:
    lines = data.split(b"\n")
    return b"\n".join(
        b"%t1Font: (fontdone fixture)" if line.startswith(b"%t1Font: ") else line
        for line in lines
    )


def build_simple_type1(path: Path, font_name: str, family_name: str, notice: str) -> None:
    font = T1Font.__new__(T1Font)
    font.encoding = "ascii"
    font.font = {
        "FontName": font_name,
        "FontInfo": {
            "version": "001.000",
            "Notice": notice,
            "FullName": family_name,
            "FamilyName": family_name,
            "Weight": "Regular",
            "ItalicAngle": 0,
            "isFixedPitch": False,
            "UnderlinePosition": -100,
            "UnderlineThickness": 50,
        },
        "Encoding": StandardEncoding,
        "PaintType": 0,
        "FontType": 1,
        "FontMatrix": [0.001, 0, 0, 0.001, 0, 0],
        "FontBBox": (0, 0, 500, 700),
        "Private": {
            "BlueValues": [],
            "OtherBlues": [],
            "FamilyBlues": [],
            "FamilyOtherBlues": [],
            "BlueScale": 0.039625,
            "BlueShift": 7,
            "BlueFuzz": 1,
            "StdHW": [50],
            "StdVW": [80],
            "ForceBold": False,
            "LanguageGroup": 0,
            "password": 5839,
            "lenIV": 4,
            "RD": "-|",
            "ND": "|-",
            "NP": "|",
            "Subrs": [],
        },
        "CharStrings": {
            ".notdef": charstring([500, 0, "hsbw", "endchar"]),
            "A": charstring(
                [
                    500,
                    0,
                    "hsbw",
                    50,
                    0,
                    "rmoveto",
                    0,
                    700,
                    "rlineto",
                    400,
                    0,
                    "rlineto",
                    0,
                    -700,
                    "rlineto",
                    "closepath",
                    "endchar",
                ]
            ),
        },
    }
    data = stable_generator_header(font.getData())
    path.parent.mkdir(parents=True, exist_ok=True)
    write(str(path), data, kind="PFB")


def main() -> None:
    build_simple_type1(
        OUT_DIR / "simple-type1.pfb",
        "MinimalNonSfnt",
        "Minimal NonSFNT",
        "Generated for fontdone non-SFNT coverage",
    )
    build_simple_type1(
        INPUT_OUT_DIR / "attach-afm-base.pfb",
        "AttachAfmBase",
        "Attach AFM Base",
        "Generated for fontdone Type 1 attach/patent coverage",
    )


if __name__ == "__main__":
    main()
