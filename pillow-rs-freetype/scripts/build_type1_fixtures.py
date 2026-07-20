#!/usr/bin/env python3
"""Build compact Type 1 fixtures for non-SFNT public face routes."""

from __future__ import annotations

from pathlib import Path

from fontTools.misc.psCharStrings import T1CharString
from fontTools.t1Lib import StandardEncoding, T1Font, write


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
OUT_DIR = FIXTURE_ROOT / "fonts" / "type1"
MM_OUT_DIR = FIXTURE_ROOT / "fonts" / "type1-mm"
INPUT_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1"


def charstring(program: list[object]) -> T1CharString:
    return T1CharString(program=program)


def stable_generator_header(data: bytes) -> bytes:
    lines = data.split(b"\n")
    return b"\n".join(
        b"%t1Font: (fontdone fixture)" if line.startswith(b"%t1Font: ") else line
        for line in lines
    )


def build_simple_type1(
    path: Path,
    font_name: str,
    family_name: str,
    notice: str,
    *,
    weight: str = "Regular",
    is_fixed_pitch: bool = False,
    cleartext_replacements: list[tuple[bytes, bytes]] | None = None,
) -> None:
    font = T1Font.__new__(T1Font)
    font.encoding = "ascii"
    font.font = {
        "FontName": font_name,
        "FontInfo": {
            "version": "001.000",
            "Notice": notice,
            "FullName": family_name,
            "FamilyName": family_name,
            "Weight": weight,
            "ItalicAngle": 0,
            "isFixedPitch": is_fixed_pitch,
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
    for before, after in cleartext_replacements or []:
        if before not in data:
            raise ValueError(f"missing Type 1 fixture token: {before!r}")
        data = data.replace(before, after, 1)
    path.parent.mkdir(parents=True, exist_ok=True)
    write(str(path), data, kind="PFB")


def invalidate_first_pfb_segment(path: Path) -> None:
    data = bytearray(path.read_bytes())
    if data[:2] != b"\x80\x01":
        raise ValueError("expected an ASCII PFB first segment")
    data[1] = 2
    path.write_bytes(data)


def build_adobe_mm_two_axis(path: Path) -> None:
    """Build a compact Adobe Type 1 Multiple Master descriptor fixture.

    FreeType's Type 1 MM parser reads these top-level dictionary keys in
    `src/type1/t1load.c`: `BlendAxisTypes`, `BlendDesignPositions`,
    `BlendDesignMap`, and `WeightVector`.  The glyph program is intentionally
    minimal; this fixture exists first to make Adobe MM descriptor, design
    coordinate, weight-vector, and named-instance reset API state reproducible
    through pinned C FreeType.
    """

    build_simple_type1(
        path,
        "AdobeMMTwoAxis",
        "Adobe MM Two Axis",
        "Generated for fontdone Type 1 Multiple Master API parity",
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight /Width] def\n"
                b"/BlendDesignPositions [[400 100] [900 100] [400 200] [900 200]] def\n"
                b"/BlendDesignMap [[[400 0] [900 1]] [[100 0] [200 1]]] def\n"
                b"/WeightVector [0.25 0.25 0.25 0.25] def",
            )
        ],
    )


def main() -> None:
    build_simple_type1(
        OUT_DIR / "simple-type1.pfb",
        "MinimalNonSfnt",
        "Minimal NonSFNT",
        "Generated for fontdone non-SFNT coverage",
    )
    build_simple_type1(
        OUT_DIR / "metadata-bold-invalid-bool.pfb",
        "MetadataProbe",
        "Metadata Probe",
        "Generated for fontdone Type 1 metadata coverage",
        weight="Bold",
        cleartext_replacements=[(b"/isFixedPitch false def", b"/isFixedPitch maybe def")],
    )
    build_simple_type1(
        OUT_DIR / "fixed-pitch-type1.pfb",
        "FixedPitchTypeOne",
        "Fixed Pitch Type One",
        "Generated for fontdone Type 1 fixed-pitch face-flag coverage",
        is_fixed_pitch=True,
    )
    build_simple_type1(
        OUT_DIR / "bbox-array-type1.pfb",
        "BBoxArrayTypeOne",
        "BBox Array Type One",
        "Generated for fontdone Type 1 array bbox coverage",
        cleartext_replacements=[
            (b"/FontBBox {0 0 500 700} def", b"/FontBBox [0 0 500 700] def")
        ],
    )
    invalid_segment_path = OUT_DIR / "invalid-first-segment-type1.pfb"
    build_simple_type1(
        invalid_segment_path,
        "InvalidSegmentTypeOne",
        "Invalid Segment Type One",
        "Generated for fontdone Type 1 PFB segment coverage",
    )
    invalidate_first_pfb_segment(invalid_segment_path)
    build_simple_type1(
        INPUT_OUT_DIR / "attach-afm-base.pfb",
        "AttachAfmBase",
        "Attach AFM Base",
        "Generated for fontdone Type 1 attach/patent coverage",
    )
    build_adobe_mm_two_axis(MM_OUT_DIR / "adobe-mm-two-axis.pfb")


if __name__ == "__main__":
    main()
