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
LEGACY_MM_OUT_DIR = FIXTURE_ROOT / "fonts" / "mm"
INPUT_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1"
INPUT_AUX_OUT_DIR = FIXTURE_ROOT / "input" / "aux" / "type1"
INPUT_ENCODING_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1-encoding"
INPUT_MM_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1-mm"


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
    private_overrides: dict[str, object] | None = None,
    cleartext_replacements: list[tuple[bytes, bytes]] | None = None,
) -> None:
    private_dict = {
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
    }
    private_dict.update(private_overrides or {})
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
        "Private": private_dict,
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


def build_mm_blend_fontinfo_private(path: Path) -> None:
    """Build the declared Type 1 MM fixture for private blend-table parity.

    The public rows under `t1tables.get_ps_font_private_mm_blend` need a
    Multiple Master face with populated Private-dictionary fields, not just the
    descriptor-only MM fixture used by `ftmm`.  Keep this source-backed so the
    eventual `FT_Get_PS_Font_Private`/`FT_Get_PS_Font_Value` route can compare
    pinned C and Rust against a reproducible same input.
    """

    build_simple_type1(
        path,
        "MMBlendPrivate",
        "MM Blend Private",
        "Generated for fontdone Type 1 MM private blend parity",
        private_overrides={
            "BlueValues": [-20, 0, 480, 500],
            "OtherBlues": [-250, -230],
            "FamilyBlues": [-15, 0, 470, 490],
            "FamilyOtherBlues": [-260, -240],
            "BlueScale": 0.047,
            "BlueShift": 9,
            "StdHW": [42],
            "StdVW": [83],
            "StemSnapH": [38, 42, 46],
            "StemSnapV": [78, 83, 91],
            "ForceBold": True,
        },
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


def build_mm_underline_blend_fixture(
    path: Path,
    font_name: str,
    family_name: str,
    underline_key: str,
    values: list[int],
) -> None:
    """Build Type 1 MM FontInfo underline-array fixtures.

    FreeType parses scalar FontInfo arrays in MM fonts into
    `blend->font_infos[1..]` (`src/type1/t1load.c:t1_load_keyword` via
    `ps_parser_load_field`).  The public `FT_Get_PS_Font_Info` record still
    exposes the base face FontInfo value; these fixtures pin that C behavior
    while proving the blend dictionary array is present in the source font.
    """

    array = " ".join(str(value) for value in values).encode()
    build_simple_type1(
        path,
        font_name,
        family_name,
        f"Generated for fontdone Type 1 MM {underline_key} blend parity",
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight /Width] def\n"
                b"/BlendDesignPositions [[400 100] [900 100] [400 200] [900 200]] def\n"
                b"/BlendDesignMap [[[400 0] [900 1]] [[100 0] [200 1]]] def\n"
                b"/WeightVector [1 0 0 0] def\n"
                + f"/{underline_key} [".encode()
                + array
                + b"] def",
            )
        ],
    )


def build_non_mm_force_bold(path: Path) -> None:
    """Build the declared non-MM ForceBold control for Type 1 private parity."""

    build_simple_type1(
        path,
        "NonMMForceBold",
        "Non MM Force Bold",
        "Generated for fontdone Type 1 ForceBold private control parity",
        private_overrides={"ForceBold": True},
    )


def build_font_value_populated(path: Path) -> None:
    """Build the declared FT_Get_PS_Font_Value selector-matrix fixture."""

    build_simple_type1(
        path,
        "FontValuePopulated",
        "Font Value Populated",
        "Generated for fontdone Type 1 font-value selector parity",
        private_overrides={
            "BlueValues": [-20, 0, 480, 500],
            "StdHW": [42],
            "StdVW": [83],
        },
    )


def build_encoding_fixture(path: Path, font_name: str, family_name: str, encoding: bytes) -> None:
    """Build a Type 1 fixture with a specific clear-text Encoding object."""

    build_simple_type1(
        path,
        font_name,
        family_name,
        f"Generated for fontdone {family_name} encoding parity",
        cleartext_replacements=[(b"/Encoding StandardEncoding def", encoding)],
    )


def build_type1_encoding_fixtures() -> None:
    custom_array = (
        b"/Encoding 256 array\n"
        b"0 1 255 {1 index exch /.notdef put} for\n"
        b"dup 65 /A put\n"
        b"readonly def"
    )
    variants = [
        (
            INPUT_ENCODING_OUT_DIR / "custom-array.pfb",
            "EncodingCustomArray",
            "Encoding Custom Array",
            custom_array,
        ),
        (
            INPUT_ENCODING_OUT_DIR / "standard.pfb",
            "EncodingStandard",
            "Encoding Standard",
            b"/Encoding StandardEncoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "isolatin1.pfb",
            "EncodingISOLatin1",
            "Encoding ISO Latin 1",
            b"/Encoding ISOLatin1Encoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "expert.pfb",
            "EncodingExpert",
            "Encoding Expert",
            b"/Encoding ExpertEncoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "no-recognized-encoding.pfb",
            "EncodingNone",
            "Encoding None",
            b"/Encoding UnknownEncoding def",
        ),
        (
            OUT_DIR / "custom-encoding-array.pfb",
            "EncodingCustomArray",
            "Encoding Custom Array",
            custom_array,
        ),
        (
            OUT_DIR / "standard-encoding.pfb",
            "EncodingStandard",
            "Encoding Standard",
            b"/Encoding StandardEncoding def",
        ),
        (
            OUT_DIR / "isolatin1-encoding.pfb",
            "EncodingISOLatin1",
            "Encoding ISO Latin 1",
            b"/Encoding ISOLatin1Encoding def",
        ),
        (
            OUT_DIR / "expert-encoding.pfb",
            "EncodingExpert",
            "Encoding Expert",
            b"/Encoding ExpertEncoding def",
        ),
    ]
    for path, font_name, family_name, encoding in variants:
        build_encoding_fixture(path, font_name, family_name, encoding)


def build_attach_afm_fixture(path: Path) -> None:
    """Build matching AFM data for the generated attach AFM Type 1 face.

    FreeType 2.14.3 parses Type 1 auxiliary AFM data through
    `src/type1/t1afm.c:T1_Read_Metrics` and `src/psaux/afmparse.c`.
    TrackKern records have degree, minimum point size, minimum kern, maximum
    point size, and maximum kern fields.  Keep the values intentionally simple
    and non-zero so later `FT_Attach_File`, `FT_Attach_Stream`, and
    `FT_Get_Track_Kerning` rows can prove observable attachment behavior
    instead of only proving that a file opened.
    """

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "StartFontMetrics 4.1",
                "Comment Generated for fontdone Type 1 attach/track parity",
                "FontName AttachAfmBase",
                "FullName Attach AFM Base",
                "FamilyName Attach AFM Base",
                "Weight Regular",
                "ItalicAngle 0",
                "IsFixedPitch false",
                "FontBBox 0 0 500 700",
                "UnderlinePosition -100",
                "UnderlineThickness 50",
                "StartCharMetrics 2",
                "C -1 ; WX 500 ; N .notdef ; B 0 0 0 0 ;",
                "C 65 ; WX 500 ; N A ; B 50 0 450 700 ;",
                "EndCharMetrics",
                "StartKernData",
                "StartTrackKern 3",
                "TrackKern -1 8 -30 72 -90",
                "TrackKern 0 8 0 72 0",
                "TrackKern 1 8 20 72 80",
                "EndTrackKern",
                "StartKernPairs 1",
                "KPX A A -25",
                "EndKernPairs",
                "EndKernData",
                "EndFontMetrics",
                "",
            ]
        ),
        encoding="ascii",
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
    build_attach_afm_fixture(INPUT_AUX_OUT_DIR / "attach-afm-base.afm")
    build_simple_type1(
        INPUT_OUT_DIR / "track-kern-base.pfb",
        "AttachAfmBase",
        "Attach AFM Base",
        "Generated for fontdone Type 1 track-kerning coverage",
    )
    build_attach_afm_fixture(INPUT_AUX_OUT_DIR / "track-kern-base.afm")
    build_font_value_populated(INPUT_OUT_DIR / "font-value-populated.pfb")
    build_adobe_mm_two_axis(MM_OUT_DIR / "adobe-mm-two-axis.pfb")
    build_adobe_mm_two_axis(LEGACY_MM_OUT_DIR / "adobe-multiple-master.pfb")
    build_mm_blend_fontinfo_private(OUT_DIR / "mm-blend-fontinfo-private.pfb")
    build_mm_underline_blend_fixture(
        INPUT_MM_OUT_DIR / "underline-position.pfb",
        "MMUnderlinePosition",
        "MM Underline Position",
        "UnderlinePosition",
        [-111, -222, -333, -444],
    )
    build_mm_underline_blend_fixture(
        INPUT_MM_OUT_DIR / "underline-thickness.pfb",
        "MMUnderlineThickness",
        "MM Underline Thickness",
        "UnderlineThickness",
        [11, 22, 33, 44],
    )
    build_non_mm_force_bold(OUT_DIR / "non-mm-force-bold.pfb")
    build_type1_encoding_fixtures()


if __name__ == "__main__":
    main()
