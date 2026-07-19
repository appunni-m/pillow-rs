#!/usr/bin/env python3
"""Generate deterministic malformed BDF fixtures for constructor error parity."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"


VALID_PREFIX = """STARTFONT 2.1
FONT PillowRsMalformedBDF
SIZE 8 75 75
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES 2
FONT_ASCENT 6
FONT_DESCENT 2
ENDPROPERTIES
CHARS 1
"""

VALID_GLYPH = """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
18
24
42
7E
42
42
00
ENDCHAR
ENDFONT
"""


def write_fixture(relative: str, text: str) -> None:
    path = FIXTURE_ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("ascii"))


def main() -> None:
    write_fixture(
        "generated/bdf/corrupted-header.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\n",
    )
    write_fixture(
        "generated/bdf/corrupted-glyphs.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
18
""",
    )
    write_fixture(
        "generated/bdf/bbx-too-big.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 70000 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_startfont_field.bdf",
        "FONT PillowRsMalformedBDF\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_font_field.bdf",
        "STARTFONT 2.1\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_size_field.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_fontboundingbox_field.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\nSIZE 8 75 75\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_startchar_field.bdf",
        VALID_PREFIX + "ENCODING 65\nENDFONT\n",
    )
    write_fixture(
        "fixtures/assets/bdf/nested_startchar_before_endchar.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
STARTCHAR B
ENCODING 66
BBX 8 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_encoding_field.bdf",
        VALID_PREFIX
        + """STARTCHAR A
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "fixtures/assets/bdf/missing_bbx_field.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
BITMAP
00
ENDCHAR
ENDFONT
""",
    )


if __name__ == "__main__":
    main()
