#!/usr/bin/env python3
"""Build compact autohint fonts that exercise script-selection coverage."""

from __future__ import annotations

import struct
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphCoordinates
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "autohint"

UNITS_PER_EM = 1000

SCRIPT_PROBES: list[tuple[str, int]] = [
    ("adlm", 0x1E90C),
    ("arab", 0x0627),
    ("armn", 0x0531),
    ("avst", 0x10B00),
    ("bamu", 0xA6A7),
    ("beng", 0x0987),
    ("buhd", 0x1750),
    ("cakm", 0x11103),
    ("cans", 0x15DC),
    ("cari", 0x102A7),
    ("cher", 0x13C6),
    ("copt", 0x2C8C),
    ("cprt", 0x1080D),
    ("cyrl", 0x0411),
    ("deva", 0x0908),
    ("dsrt", 0x10402),
    ("ethi", 0x1200),
    ("geor", 0x10D2),
    ("geok", 0x10B1),
    ("glag", 0x2C05),
    ("goth", 0x10332),
    ("grek", 0x0393),
    ("gujr", 0x0AA4),
    ("guru", 0x0A07),
    ("hebr", 0x05D1),
    ("kali", 0xA905),
    ("khmr", 0x1781),
    ("khms", 0x19E0),
    ("knda", 0x0C87),
    ("lao", 0x0EB2),
    ("latb", 0x2080),
    ("latp", 0x2070),
    ("latn", 0x006F),
    ("limb", 0x1900),
    ("lisu", 0xA4E1),
    ("mlym", 0x0D12),
    ("medf", 0x16E40),
    ("mong", 0x1833),
    ("mymr", 0x1001),
    ("nkoo", 0x07D0),
    ("olck", 0x1C5B),
    ("orkh", 0x10C17),
    ("orya", 0x0B13),
    ("osge", 0x104BE),
    ("osma", 0x10486),
    ("rohg", 0x10D03),
    ("saur", 0xA89C),
    ("shaw", 0x10455),
    ("sinh", 0x0D89),
    ("sund", 0x1B8B),
    ("sylo", 0xA807),
    ("taml", 0x0B89),
    ("tavt", 0xAA86),
    ("telu", 0x0C07),
    ("tfng", 0x2D54),
    ("thai", 0x0E1A),
    ("tibt", 0x0F40),
    ("vaii", 0xA5CD),
    ("hani", 0x4ED6),
]

# ASCII digits are global autohinter metrics probes, not script probes.  Keep
# them after the script glyphs so existing fixture glyph indices remain stable.
DIGIT_WIDTH_PROBES: list[tuple[str, int, int]] = [
    ("digit_zero_wide", 0x0030, 620),
    ("digit_one_narrow", 0x0031, 520),
]

STANDARD_CHARS: dict[str, int] = {
    "adlm": 0x1E90C,
    "arab": 0x0644,
    "armn": 0x057D,
    "avst": 0x10B1A,
    "bamu": 0xA6C1,
    "beng": 0x09E6,
    "buhd": 0x174B,
    "cakm": 0x11124,
    "cans": 0x144C,
    "cari": 0x102AB,
    "cher": 0x13A4,
    "copt": 0x2C9E,
    "cprt": 0x10805,
    "cyrl": 0x043E,
    "deva": 0x0920,
    "dsrt": 0x10404,
    "ethi": 0x12D0,
    "geok": 0x10B6,
    "geor": 0x10D8,
    "glag": 0x2C15,
    "goth": 0x10334,
    "grek": 0x03BF,
    "gujr": 0x0A9F,
    "guru": 0x0A20,
    "hani": 0x7530,
    "hebr": 0x05DD,
    "kali": 0xA90D,
    "khmr": 0x17E0,
    "khms": 0x19E1,
    "knda": 0x0CE6,
    "lao": 0x0ED0,
    "latb": 0x2092,
    "latn": 0x006F,
    "latp": 0x1D52,
    "limb": 0x006F,
    "lisu": 0xA4F3,
    "medf": 0x16E61,
    "mlym": 0x0D20,
    "mong": 0x1842,
    "mymr": 0x101D,
    "nkoo": 0x07CB,
    "olck": 0x1C5B,
    "orkh": 0x10C17,
    "orya": 0x006F,
    "osge": 0x104C2,
    "osma": 0x10486,
    "rohg": 0x10D30,
    "saur": 0xA89D,
    "shaw": 0x10474,
    "sinh": 0x0DA7,
    "sund": 0x1BB0,
    "sylo": 0x006F,
    "taml": 0x0BE6,
    "tavt": 0xAA92,
    "telu": 0x0C66,
    "tfng": 0x2D54,
    "thai": 0x0E32,
    "tibt": 0x006F,
    "vaii": 0xA613,
}

# Keep a few real blue-string characters mapped to each compact probe glyph.
# The aliases make metrics initialization walk script-specific blue strings
# without adding one glyph per Unicode character.
SCRIPT_BLUE_ALIASES: dict[str, tuple[int, ...]] = {
    "cyrl": (
        0x0411,
        0x0412,
        0x0415,
        0x041E,
        0x0421,
        0x042D,
        0x0435,
        0x0437,
        0x043E,
        0x0441,
        0x0443,
        0x0444,
        0x0445,
        0x0448,
    ),
    "grek": (
        0x0393,
        0x0398,
        0x03A9,
        0x03B1,
        0x03B2,
        0x03B3,
        0x03B4,
        0x03B5,
        0x03B8,
        0x03BF,
        0x03C1,
        0x03C3,
        0x03C4,
        0x03C6,
        0x03C7,
        0x03C8,
        0x03C9,
    ),
    "latn": (
        0x0043,
        0x0045,
        0x0048,
        0x004C,
        0x004F,
        0x0051,
        0x0053,
        0x0054,
        0x0055,
        0x005A,
        0x0062,
        0x0063,
        0x0064,
        0x0065,
        0x0066,
        0x0067,
        0x0068,
        0x0069,
        0x006A,
        0x006B,
        0x006E,
        0x006F,
        0x0070,
        0x0071,
        0x0072,
        0x0073,
        0x0075,
        0x0076,
        0x0078,
        0x0079,
        0x007A,
    ),
}


def empty_glyph():
    return TTGlyphPen(None).glyph()


def rectangle_glyph(left: int, bottom: int, right: int, top: int):
    pen = TTGlyphPen(None)
    pen.moveTo((left, bottom))
    pen.lineTo((left, top))
    pen.lineTo((right, top))
    pen.lineTo((right, bottom))
    pen.closePath()
    return pen.glyph()


def rectangles_glyph(rects: list[tuple[int, int, int, int]]):
    pen = TTGlyphPen(None)
    for left, bottom, right, top in rects:
        pen.moveTo((left, bottom))
        pen.lineTo((left, top))
        pen.lineTo((right, top))
        pen.lineTo((right, bottom))
        pen.closePath()
    return pen.glyph()


def extreme_rectangle_glyph():
    """Full signed-16-bit bounds with individually encodable glyf deltas."""
    pen = TTGlyphPen(None)
    pen.moveTo((-32768, -32768))
    pen.lineTo((-32768, -1))
    pen.lineTo((-32768, 32766))
    pen.lineTo((-32768, 32767))
    pen.lineTo((-1, 32767))
    pen.lineTo((32766, 32767))
    pen.lineTo((32767, 32767))
    pen.lineTo((32767, 0))
    pen.lineTo((32767, -32767))
    pen.lineTo((32767, -32768))
    pen.lineTo((0, -32768))
    pen.lineTo((-32767, -32768))
    pen.lineTo((-32768, -32768))
    pen.closePath()
    return pen.glyph()


def stacked_contour_glyph():
    """Three vertically separated contours for the double-top adjustment path."""
    return rectangles_glyph(
        [
            (100, 0, 500, 500),
            (180, 540, 420, 600),
            (210, 660, 390, 700),
        ]
    )


def top_tilde_glyph(extra_top: bool = False):
    contours = [
        (100, 0, 500, 500),
        # A compact tilde contour: the middle on-curve point is flanked by
        # off-curve controls at the same y so the Latin autohinter measures it
        # as a tilde wave, not as a plain accent rectangle.
        [
            (140, 620, True),
            (190, 580, False),
            (240, 580, True),
            (310, 580, False),
            (370, 620, True),
            (430, 540, True),
        ],
    ]
    if extra_top:
        contours.append((210, 660, 390, 700))
    return mixed_contour_glyph(contours)


def top_tilde_centering_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 616, True),
                (190, 574, False),
                (240, 574, True),
                (310, 574, False),
                (370, 616, True),
                (430, 536, True),
            ],
            (210, 664, 390, 704),
        ]
    )


def top_tilde_measure_zero_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 620, True),
                (190, 580, False),
                (240, 580, True),
                (310, 580, False),
                (370, 540, True),
                (430, 560, True),
            ],
        ]
    )


def top_tilde_flat_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 560, True),
                (430, 560, True),
            ],
        ]
    )


def top_tilde_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 560, True),
                (235, 560, True),
                (335, 560, True),
                (430, 560, True),
            ],
        ]
    )


def horizontal_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 500, True),
                (240, 500, True),
                (380, 500, True),
                (520, 500, True),
            ],
        ]
    )


def bottom_tilde_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 80, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, False),
                (370, 80, True),
                (430, 0, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_measure_zero_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 80, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, False),
                (370, 0, True),
                (430, 20, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_flat_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 60, True),
                (430, 60, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 60, True),
                (235, 60, True),
                (335, 60, True),
                (430, 60, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tall_accent_glyph():
    return rectangles_glyph(
        [
            (100, 0, 500, 500),
            (190, -620, 410, -80),
        ]
    )


def top_and_bottom_accent_glyph():
    return mixed_contour_glyph(
        [
            [
                (190, -90, True),
                (410, -90, True),
                (410, -30, True),
                (190, -30, True),
            ],
            (100, 0, 500, 500),
            [
                (210, 550, True),
                (390, 550, True),
                (390, 610, True),
                (210, 610, True),
            ],
        ]
    )


def disjoint_top_accent_glyph():
    return rectangles_glyph(
        [
            (80, 0, 300, 500),
            (430, 548, 530, 588),
        ]
    )


def serif_m_symmetry_glyph():
    """Three serifed stems with 12 horizontal-dimension edges."""
    return rectangles_glyph(
        [
            (100, 0, 150, 500),
            (70, 0, 180, 120),
            (70, 380, 180, 500),
            (300, 0, 350, 500),
            (270, 0, 380, 120),
            (270, 380, 380, 500),
            (500, 0, 550, 500),
            (470, 0, 580, 120),
            (470, 380, 580, 500),
        ]
    )


def serif_overlap_break_glyph():
    """Serifed stem with an intermediate vertical edge sharing the serif range."""
    return rectangles_glyph(
        [
            (100, 0, 150, 500),
            (70, 0, 180, 120),
            (85, 30, 95, 90),
        ]
    )


def latin_wide_segment_filter_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 0, True),
                (140, 600, True),
                (260, 600, True),
                (220, 0, True),
            ],
        ]
    )


def latin_vertical_cusp_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 400, True),
                (100, 500, True),
                (100, 0, False),
                (100, 490, True),
                (300, 490, True),
                (300, 400, True),
            ],
        ]
    )


def latin_blue_delta_round_glyph():
    return ring_glyph(90, 0, 510, 530, 190, 120, 410, 400)


def nonreciprocal_chain_glyph():
    # U+51A1: two major-direction segments share one opposite segment so CJK
    # link cleanup sees a non-reciprocal chain and assigns a serif fallback.
    pen = TTGlyphPen(None)
    pen.moveTo((20, 20))
    pen.lineTo((20, 220))
    pen.lineTo((60, 220))
    pen.lineTo((60, 460))
    pen.lineTo((80, 460))
    pen.lineTo((80, 20))
    pen.closePath()
    return pen.glyph()


def cjk_same_position_minor_edge_glyph():
    """Clockwise major and counter-clockwise minor edges at the same x."""
    pen = TTGlyphPen(None)

    # The larger clockwise contour fixes the glyph's TrueType orientation, so
    # its left edge is the horizontal axis' major-direction segment.
    pen.moveTo((40, 20))
    pen.lineTo((40, 220))
    pen.lineTo((320, 220))
    pen.lineTo((320, 20))
    pen.closePath()

    # Reverse the smaller contour.  Its closing left edge is minor-direction
    # and shares x=40 with the major edge above.
    pen.moveTo((40, 300))
    pen.lineTo((80, 300))
    pen.lineTo((80, 400))
    pen.lineTo((40, 400))
    pen.closePath()
    return pen.glyph()


def cjk_grouped_shorter_link_glyph():
    """Two grouped left segments whose second stem has the shorter link."""
    return rectangles_glyph(
        [
            (40, 20, 310, 220),
            (40, 280, 300, 480),
        ]
    )


def mixed_round_straight_edge_glyph():
    pen = TTGlyphPen(None)
    for bottom, right, top in ((20, 390, 180), (260, 410, 420)):
        pen.moveTo((100, bottom))
        pen.lineTo((100, top))
        pen.lineTo((right, top))
        pen.lineTo((right, bottom))
        pen.closePath()

    mid_x = 270
    mid_y = 260
    pen.moveTo((mid_x, 0))
    pen.qCurveTo((500, 0), (500, mid_y))
    pen.qCurveTo((500, 520), (mid_x, 520))
    pen.qCurveTo((40, 520), (40, mid_y))
    pen.qCurveTo((40, 0), (mid_x, 0))
    pen.closePath()
    pen.moveTo((250, 100))
    pen.qCurveTo((100, 100), (100, mid_y))
    pen.qCurveTo((100, 420), (250, 420))
    pen.qCurveTo((400, 420), (400, mid_y))
    pen.qCurveTo((400, 100), (250, 100))
    pen.closePath()
    return pen.glyph()


def ring_glyph(
    left: int,
    bottom: int,
    right: int,
    top: int,
    inset_left: int,
    inset_bottom: int,
    inset_right: int,
    inset_top: int,
):
    pen = TTGlyphPen(None)
    mid_x = (left + right) // 2
    mid_y = (bottom + top) // 2
    pen.moveTo((mid_x, bottom))
    pen.qCurveTo((right, bottom), (right, mid_y))
    pen.qCurveTo((right, top), (mid_x, top))
    pen.qCurveTo((left, top), (left, mid_y))
    pen.qCurveTo((left, bottom), (mid_x, bottom))
    pen.closePath()

    inset_mid_x = (inset_left + inset_right) // 2
    inset_mid_y = (inset_bottom + inset_top) // 2
    pen.moveTo((inset_mid_x, inset_bottom))
    pen.qCurveTo((inset_left, inset_bottom), (inset_left, inset_mid_y))
    pen.qCurveTo((inset_left, inset_top), (inset_mid_x, inset_top))
    pen.qCurveTo((inset_right, inset_top), (inset_right, inset_mid_y))
    pen.qCurveTo((inset_right, inset_bottom), (inset_mid_x, inset_bottom))
    pen.closePath()
    return pen.glyph()


def one_point_contour_glyph(points: list[tuple[int, int]]):
    glyph = Glyph()
    glyph.numberOfContours = len(points)
    glyph.coordinates = GlyphCoordinates(points)
    glyph.endPtsOfContours = list(range(len(points)))
    glyph.flags = bytearray([1] * len(points))
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = min(x for x, _ in points)
    glyph.xMax = max(x for x, _ in points)
    glyph.yMin = min(y for _, y in points)
    glyph.yMax = max(y for _, y in points)
    return glyph


def low_upem_duplicate_point_glyph():
    """One contour whose first segment has zero length."""
    points = [(4, 0), (4, 0), (4, 48), (44, 48), (44, 0)]
    glyph = Glyph()
    glyph.numberOfContours = 1
    glyph.coordinates = GlyphCoordinates(points)
    glyph.endPtsOfContours = [len(points) - 1]
    glyph.flags = bytearray([1] * len(points))
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = 4
    glyph.yMin = 0
    glyph.xMax = 44
    glyph.yMax = 48
    return glyph


def mixed_contour_glyph(contours: list[object]):
    coordinates: list[tuple[int, int]] = []
    end_pts: list[int] = []
    flags = bytearray()
    for contour in contours:
        if isinstance(contour, tuple):
            left, bottom, right, top = contour
            points = [
                (left, bottom, True),
                (left, top, True),
                (right, top, True),
                (right, bottom, True),
            ]
        else:
            points = contour
        for x, y, on_curve in points:
            coordinates.append((x, y))
            flags.append(1 if on_curve else 0)
        end_pts.append(len(coordinates) - 1)

    glyph = Glyph()
    glyph.numberOfContours = len(contours)
    glyph.coordinates = GlyphCoordinates(coordinates)
    glyph.endPtsOfContours = end_pts
    glyph.flags = flags
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = min(x for x, _ in coordinates)
    glyph.xMax = max(x for x, _ in coordinates)
    glyph.yMin = min(y for _, y in coordinates)
    glyph.yMax = max(y for _, y in coordinates)
    return glyph


def table_offsets(path: Path) -> dict[str, tuple[int, int]]:
    data = path.read_bytes()
    num_tables = struct.unpack(">H", data[4:6])[0]
    tables: dict[str, tuple[int, int]] = {}
    for index in range(num_tables):
        entry = 12 + index * 16
        tag = data[entry : entry + 4].decode("ascii")
        offset = struct.unpack(">L", data[entry + 8 : entry + 12])[0]
        length = struct.unpack(">L", data[entry + 12 : entry + 16])[0]
        tables[tag] = (offset, length)
    return tables


def truncate_glyph_loca(path: Path, glyph_name: str, byte_len: int) -> None:
    font = TTFont(path, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    glyph_id = glyph_order.index(glyph_name)
    locations = list(font["loca"].locations)
    start = locations[glyph_id]
    locations[glyph_id + 1] = start + byte_len
    loca_format = font["head"].indexToLocFormat
    font.close()

    tables = table_offsets(path)
    loca_offset, _ = tables["loca"]
    data = bytearray(path.read_bytes())
    if loca_format == 0:
        if byte_len % 2 != 0:
            raise ValueError("short loca glyph lengths must be even")
        entry = loca_offset + (glyph_id + 1) * 2
        data[entry : entry + 2] = struct.pack(">H", locations[glyph_id + 1] // 2)
    else:
        entry = loca_offset + (glyph_id + 1) * 4
        data[entry : entry + 4] = struct.pack(">L", locations[glyph_id + 1])
    path.write_bytes(data)


def glyph_name(tag: str) -> str:
    return f"script_{tag}"


def build_script_coverage() -> None:
    glyph_order = [".notdef", "space"]
    glyph_order.extend(glyph_name(tag) for tag, _ in SCRIPT_PROBES)
    glyph_order.extend(name for name, _, _ in DIGIT_WIDTH_PROBES)
    glyph_order.append("latin_double_top")
    glyph_order.append("latin_tilde_top")
    glyph_order.append("latin_tilde_top2")
    glyph_order.append("latin_tilde_top_measure_zero")
    glyph_order.append("latin_tilde_top_flat")
    glyph_order.append("latin_tilde_top_flat_loop")
    glyph_order.append("latin_tilde_bottom")
    glyph_order.append("latin_tilde_bottom_measure_zero")
    glyph_order.append("latin_tilde_bottom_flat")
    glyph_order.append("latin_tilde_bottom_flat_loop")
    glyph_order.append("latin_bottom_tall_accent")
    glyph_order.append("latin_tilde_top2_topflag")
    glyph_order.append("latin_top_bottom_accent")
    glyph_order.append("latin_disjoint_top_accent")
    glyph_order.append("latin_serif_m_symmetry")
    glyph_order.append("latin_serif_overlap_break")
    glyph_order.append("latin_tilde_top2_centering")
    glyph_order.append("latin_vertical_cusp")
    glyph_order.append("latin_nonbase_tilde")
    glyph_order.append("latin_extreme_coordinate")

    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
    }
    cmap = {0x20: "space"}

    for index, (tag, codepoint) in enumerate(SCRIPT_PROBES):
        name = glyph_name(tag)
        width = 500 + (index % 5) * 20
        top = 480 + (index % 7) * 24
        left = 70 + (index % 3) * 10
        glyphs[name] = rectangle_glyph(left, 0, left + width, top)
        metrics[name] = (700, left)
        cmap[codepoint] = name
        standard = STANDARD_CHARS.get(tag)
        if standard is not None:
            cmap.setdefault(standard, name)
        for alias in SCRIPT_BLUE_ALIASES.get(tag, ()):
            cmap.setdefault(alias, name)

    for name, codepoint, advance in DIGIT_WIDTH_PROBES:
        glyphs[name] = rectangle_glyph(100, 0, 440, 560)
        metrics[name] = (advance, 100)
        cmap[codepoint] = name

    glyphs["latin_double_top"] = stacked_contour_glyph()
    metrics["latin_double_top"] = (700, 100)
    cmap[0x01D5] = "latin_double_top"
    glyphs["latin_tilde_top"] = top_tilde_glyph()
    metrics["latin_tilde_top"] = (700, 100)
    cmap[0x00F1] = "latin_tilde_top"
    glyphs["latin_tilde_top2"] = top_tilde_glyph(extra_top=True)
    metrics["latin_tilde_top2"] = (700, 100)
    cmap[0x1E4D] = "latin_tilde_top2"
    glyphs["latin_tilde_top_measure_zero"] = top_tilde_measure_zero_glyph()
    metrics["latin_tilde_top_measure_zero"] = (700, 100)
    cmap[0x00E3] = "latin_tilde_top_measure_zero"
    glyphs["latin_tilde_top_flat"] = top_tilde_flat_glyph()
    metrics["latin_tilde_top_flat"] = (700, 100)
    cmap[0x00D1] = "latin_tilde_top_flat"
    glyphs["latin_tilde_top_flat_loop"] = top_tilde_flat_loop_glyph()
    metrics["latin_tilde_top_flat_loop"] = (700, 100)
    cmap[0x00C3] = "latin_tilde_top_flat_loop"
    glyphs["latin_tilde_bottom"] = bottom_tilde_glyph()
    metrics["latin_tilde_bottom"] = (700, 100)
    cmap[0x1E1B] = "latin_tilde_bottom"
    glyphs["latin_tilde_bottom_measure_zero"] = bottom_tilde_measure_zero_glyph()
    metrics["latin_tilde_bottom_measure_zero"] = (700, 100)
    cmap[0x1E1A] = "latin_tilde_bottom_measure_zero"
    glyphs["latin_tilde_bottom_flat"] = bottom_tilde_flat_glyph()
    metrics["latin_tilde_bottom_flat"] = (700, 100)
    cmap[0x1E75] = "latin_tilde_bottom_flat"
    glyphs["latin_tilde_bottom_flat_loop"] = bottom_tilde_flat_loop_glyph()
    metrics["latin_tilde_bottom_flat_loop"] = (700, 100)
    cmap[0x1E74] = "latin_tilde_bottom_flat_loop"
    glyphs["latin_bottom_tall_accent"] = bottom_tall_accent_glyph()
    metrics["latin_bottom_tall_accent"] = (700, 100)
    cmap[0x0122] = "latin_bottom_tall_accent"
    glyphs["latin_tilde_top2_topflag"] = top_tilde_glyph(extra_top=True)
    metrics["latin_tilde_top2_topflag"] = (700, 100)
    cmap[0x1EAA] = "latin_tilde_top2_topflag"
    glyphs["latin_top_bottom_accent"] = top_and_bottom_accent_glyph()
    metrics["latin_top_bottom_accent"] = (700, 100)
    cmap[0x1EAD] = "latin_top_bottom_accent"
    glyphs["latin_disjoint_top_accent"] = disjoint_top_accent_glyph()
    metrics["latin_disjoint_top_accent"] = (700, 80)
    cmap[0x1E02] = "latin_disjoint_top_accent"
    glyphs["latin_serif_m_symmetry"] = serif_m_symmetry_glyph()
    metrics["latin_serif_m_symmetry"] = (700, 70)
    cmap[0x01D7] = "latin_serif_m_symmetry"
    glyphs["latin_serif_overlap_break"] = serif_overlap_break_glyph()
    metrics["latin_serif_overlap_break"] = (620, 70)
    cmap[0x0244] = "latin_serif_overlap_break"
    glyphs["latin_tilde_top2_centering"] = top_tilde_centering_glyph()
    metrics["latin_tilde_top2_centering"] = (700, 100)
    cmap[0x1EB4] = "latin_tilde_top2_centering"
    glyphs["latin_vertical_cusp"] = latin_vertical_cusp_glyph()
    metrics["latin_vertical_cusp"] = (620, 100)
    cmap[0x0245] = "latin_vertical_cusp"
    glyphs["latin_nonbase_tilde"] = top_tilde_glyph()
    metrics["latin_nonbase_tilde"] = (0, 0)
    cmap[0x0303] = "latin_nonbase_tilde"
    glyphs["latin_extreme_coordinate"] = extreme_rectangle_glyph()
    metrics["latin_extreme_coordinate"] = (1000, -32768)
    cmap[0x0246] = "latin_extreme_coordinate"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Script Coverage",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Script Coverage Regular",
            "fullName": "Autohint Script Coverage Regular",
            "psName": "AutohintScriptCoverage-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "script-coverage.ttf")


def build_arabic_standard_fallback() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "arabic_target",
        "arabic_standard_ha",
        "arabic_join_sample",
        "arabic_neutral_stem",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "arabic_target": rectangle_glyph(100, 0, 180, 680),
        "arabic_standard_ha": ring_glyph(80, 0, 520, 500, 200, 120, 400, 380),
        # afblue.dat defines U+0640 TATWEEL as the Arabic neutral zone.
        # Its lower flat establishes y=300; the target below links that
        # neutral edge to the normal Arabic bottom zone at y=0, exercising
        # FreeType's linked-blue neutral dedup in aflatin.c:4276-4290.
        "arabic_join_sample": rectangle_glyph(80, 300, 520, 340),
        "arabic_neutral_stem": rectangle_glyph(100, 0, 500, 300),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "arabic_target": (420, 100),
        "arabic_standard_ha": (620, 80),
        "arabic_join_sample": (620, 80),
        "arabic_neutral_stem": (620, 100),
    }
    cmap = {
        0x20: "space",
        # U+0644 LAM, the first pinned standard character, is intentionally
        # absent.  U+062D HAH is the next candidate in afscript.h.
        0x062D: "arabic_standard_ha",
        0x0628: "arabic_neutral_stem",
        0x0640: "arabic_join_sample",
    }
    for codepoint in (0x0627, 0x0625, 0x0643, 0x0637, 0x0638, 0x062A, 0x062B):
        cmap[codepoint] = "arabic_target"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Arabic Standard Fallback",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Arabic Standard Fallback Regular",
            "fullName": "Autohint Arabic Standard Fallback Regular",
            "psName": "AutohintArabicStandardFallback-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "arabic-standard-fallback.ttf")


def build_cjk_empty_standard() -> None:
    glyph_order = [".notdef", "space", "hani_empty"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_empty": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_empty": (700, 0),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_empty",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Empty Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Empty Standard Regular",
            "fullName": "Autohint CJK Empty Standard Regular",
            "psName": "AutohintCJKEmptyStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-empty-standard.ttf")


def build_latin_small_ignore() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_x",
        "latin_c",
        "latin_oslash",
        "latin_small_top",
        "latin_g_cedilla",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
        "latin_x": rectangles_glyph(
            [
                (120, 0, 240, 520),
                (360, 0, 480, 520),
            ]
        ),
        "latin_c": rectangles_glyph(
            [
                (90, 0, 210, 520),
                (210, 0, 520, 90),
                (210, 430, 520, 520),
            ]
        ),
        # Keep U+00F8 on a unique glyph index so the adjustment database lookup
        # reaches AF_IGNORE_SMALL_TOP | AF_IGNORE_SMALL_BOTTOM for this row.
        "latin_oslash": ring_glyph(90, -40, 510, 560, 190, 100, 410, 420),
        "latin_small_top": ring_glyph(90, 0, 510, 560, 190, 120, 410, 430),
        "latin_g_cedilla": rectangles_glyph(
            [
                (90, 0, 520, 560),
                (220, -70, 360, -20),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
        "latin_x": (620, 120),
        "latin_c": (620, 90),
        "latin_oslash": (620, 90),
        "latin_small_top": (620, 90),
        "latin_g_cedilla": (620, 90),
    }
    cmap = {
        0x20: "space",
        0x0063: "latin_c",
        0x006F: "latin_o",
        0x0078: "latin_x",
        0x00F8: "latin_oslash",
        0x0188: "latin_small_top",
        0x0122: "latin_g_cedilla",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Small Ignore",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Small Ignore Regular",
            "fullName": "Autohint Latin Small Ignore Regular",
            "psName": "AutohintLatinSmallIgnore-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-small-ignore.ttf")


def build_latin_remaining_topology() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_capital_blue",
        "latin_wide_segment_filter",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_capital_blue": rectangle_glyph(100, 0, 500, 520),
        "latin_wide_segment_filter": latin_wide_segment_filter_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_capital_blue": (620, 100),
        "latin_wide_segment_filter": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x0043: "latin_capital_blue",
        0x0045: "latin_capital_blue",
        0x0048: "latin_capital_blue",
        0x004C: "latin_capital_blue",
        0x004F: "latin_capital_blue",
        0x0051: "latin_capital_blue",
        0x0053: "latin_capital_blue",
        0x0054: "latin_capital_blue",
        0x0055: "latin_capital_blue",
        0x005A: "latin_capital_blue",
        0x0243: "latin_wide_segment_filter",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Remaining Topology",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Remaining Topology Regular",
            "fullName": "Autohint Latin Remaining Topology Regular",
            "psName": "AutohintLatinRemainingTopology-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-remaining-topology.ttf")


def build_latin_width_clusters() -> None:
    glyph_order = [".notdef", "space", "latin_o_width_clusters"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o_width_clusters": rectangles_glyph(
            [
                (60, 0, 100, 520),
                (180, 0, 260, 520),
                (340, 0, 470, 520),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o_width_clusters": (620, 60),
    }
    cmap = {
        0x20: "space",
        0x006F: "latin_o_width_clusters",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Width Clusters",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Width Clusters Regular",
            "fullName": "Autohint Latin Width Clusters Regular",
            "psName": "AutohintLatinWidthClusters-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-width-clusters.ttf")


def build_latin_many_widths() -> None:
    stems = []
    x = 40
    for i in range(18):
        width = 24 + i * 4
        stems.append((x, 0, x + width, 520))
        x += width + 34

    glyph_order = [".notdef", "space", "latin_o_many_widths"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o_many_widths": rectangles_glyph(stems),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o_many_widths": (x + 40, 40),
    }
    cmap = {
        0x20: "space",
        0x006F: "latin_o_many_widths",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Many Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Many Widths Regular",
            "fullName": "Autohint Latin Many Widths Regular",
            "psName": "AutohintLatinManyWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-many-widths.ttf")


def build_cjk_blue_edge_cases() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "blue_empty",
        "blue_two_points",
        "blue_degenerate",
        "top_flat",
        "bottom_fill",
        "bottom_flat",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        "blue_empty": empty_glyph(),
        "blue_two_points": one_point_contour_glyph([(210, 40), (330, 180)]),
        "blue_degenerate": one_point_contour_glyph([(160, 40), (260, 120), (360, 200)]),
        "top_flat": rectangle_glyph(110, 20, 580, 220),
        "bottom_fill": rectangle_glyph(120, 0, 560, 360),
        "bottom_flat": rectangle_glyph(120, -80, 560, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "blue_empty": (700, 0),
        "blue_two_points": (700, 210),
        "blue_degenerate": (700, 160),
        "top_flat": (700, 110),
        "bottom_fill": (700, 120),
        "bottom_flat": (700, 120),
    }
    cmap = {
        0x20: "space",
        0x4E2A: "bottom_fill",
        0x4E3B: "bottom_flat",
        0x4EBA: "blue_two_points",
        0x4ED6: "blue_empty",
        0x4EEC: "blue_degenerate",
        0x519B: "top_flat",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Blue Edge Cases",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Blue Edge Cases Regular",
            "fullName": "Autohint CJK Blue Edge Cases Regular",
            "psName": "AutohintCJKBlueEdgeCases-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-blue-edge-cases.ttf")


def build_latin_blue_edge_cases() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_A",
        "blue_empty",
        "blue_degenerate",
        "blue_flat_loop",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
        "latin_A": rectangles_glyph(
            [(100, 0, 180, 680), (420, 0, 500, 680), (180, 300, 420, 380)]
        ),
        "blue_empty": empty_glyph(),
        "blue_degenerate": one_point_contour_glyph([(180, 640)]),
        "blue_flat_loop": horizontal_flat_loop_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
        "latin_A": (700, 100),
        "blue_empty": (600, 0),
        "blue_degenerate": (600, 180),
        "blue_flat_loop": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_A",
        0x6F: "latin_o",
        0x54: "blue_empty",
        0x48: "blue_degenerate",
        0x45: "blue_flat_loop",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Edge Cases",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Edge Cases Regular",
            "fullName": "Autohint Latin Blue Edge Cases Regular",
            "psName": "AutohintLatinBlueEdgeCases-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-edge-cases.ttf")


def build_cjk_malformed_blue() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "bottom_fill_malformed",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        "bottom_fill_malformed": rectangle_glyph(120, 0, 560, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "bottom_fill_malformed": (700, 120),
    }
    cmap = {
        0x20: "space",
        0x4E2A: "bottom_fill_malformed",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Malformed Blue",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Malformed Blue Regular",
            "fullName": "Autohint CJK Malformed Blue Regular",
            "psName": "AutohintCJKMalformedBlue-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / "cjk-malformed-blue.ttf"
    font.save(path)
    truncate_glyph_loca(path, "bottom_fill_malformed", 2)


def build_cjk_tiny_stem() -> None:
    glyph_order = [".notdef", "space", "hani_tiny_stem"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_tiny_stem": rectangle_glyph(100, 0, 120, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_tiny_stem": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_tiny_stem",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Tiny Stem",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Tiny Stem Regular",
            "fullName": "Autohint CJK Tiny Stem Regular",
            "psName": "AutohintCJKTinyStem-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-tiny-stem.ttf")


def build_cjk_snap_below_standard() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_snap_below",
        "hani_snap_far_below",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_snap_below": rectangle_glyph(100, 0, 190, 560),
        "hani_snap_far_below": rectangle_glyph(100, 0, 140, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_snap_below": (700, 100),
        "hani_snap_far_below": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x4E1E: "hani_snap_far_below",
        0x4ED6: "hani_snap_below",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Snap Below Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Snap Below Standard Regular",
            "fullName": "Autohint CJK Snap Below Standard Regular",
            "psName": "AutohintCJKSnapBelowStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-snap-below-standard.ttf")


def build_cjk_multi_width_snap() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_snap_width"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangles_glyph(
            [
                (80, 0, 140, 560),
                (260, 0, 390, 560),
            ]
        ),
        "hani_snap_width": rectangle_glyph(80, 0, 140, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 80),
        "hani_snap_width": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_snap_width",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Multi Width Snap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Multi Width Snap Regular",
            "fullName": "Autohint CJK Multi Width Snap Regular",
            "psName": "AutohintCJKMultiWidthSnap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-multi-width-snap.ttf")


def build_cjk_quantized_widths() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_probe"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        # Keep the input order non-monotonic so the public route covers both
        # FreeType's insertion sort and its unusual cluster-boundary handling.
        "hani_standard": rectangles_glyph(
            [
                (40, 0, 70, 560),
                (120, 0, 128, 560),
                (180, 0, 199, 560),
            ]
        ),
        "hani_probe": rectangle_glyph(80, 0, 90, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 40),
        "hani_probe": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_probe",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Quantized Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Quantized Widths Regular",
            "fullName": "Autohint CJK Quantized Widths Regular",
            "psName": "AutohintCJKQuantizedWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-quantized-widths.ttf")


def build_cjk_many_widths() -> None:
    glyph_order = [".notdef", "space", "hani_many_widths"]
    rects: list[tuple[int, int, int, int]] = []
    x = 40
    for width in range(8, 29):
        rects.append((x, 0, x + width, 560))
        x += width + 8

    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_many_widths": rectangles_glyph(rects),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_many_widths": (900, 40),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_many_widths",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Many Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Many Widths Regular",
            "fullName": "Autohint CJK Many Widths Regular",
            "psName": "AutohintCJKManyWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-many-widths.ttf")


def build_cjk_wide_stem_snap() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_wide_stem"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_wide_stem": rectangle_glyph(80, 0, 250, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_wide_stem": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_wide_stem",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Wide Stem Snap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Wide Stem Snap Regular",
            "fullName": "Autohint CJK Wide Stem Snap Regular",
            "psName": "AutohintCJKWideStemSnap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-wide-stem-snap.ttf")


def build_cjk_round_stem_light() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_round_ring",
        "hani_mixed_round_straight",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_round_ring": ring_glyph(80, 20, 520, 460, 180, 120, 420, 360),
        "hani_mixed_round_straight": mixed_round_straight_edge_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_round_ring": (700, 80),
        "hani_mixed_round_straight": (700, 40),
    }
    cmap = {
        0x20: "space",
        0x51A2: "hani_round_ring",
        0x51A3: "hani_mixed_round_straight",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Round Stem Light",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Round Stem Light Regular",
            "fullName": "Autohint CJK Round Stem Light Regular",
            "psName": "AutohintCJKRoundStemLight-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-round-stem-light.ttf")


def build_cjk_duplicate_edge() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_duplicate_edge",
        "hani_nonreciprocal_chain",
        "hani_leading_skip",
        "hani_serif_conflict",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_duplicate_edge": rectangles_glyph(
            [
                (40, 20, 80, 220),
                (40, 260, 320, 460),
            ]
        ),
        "hani_nonreciprocal_chain": nonreciprocal_chain_glyph(),
        "hani_leading_skip": rectangles_glyph(
            [
                (20, 20, 30, 22),
                (80, 20, 130, 460),
            ]
        ),
        "hani_serif_conflict": rectangles_glyph(
            [
                (80, 20, 130, 460),
                (190, 20, 230, 460),
                (60, 20, 130, 55),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_duplicate_edge": (700, 40),
        "hani_nonreciprocal_chain": (700, 20),
        "hani_leading_skip": (700, 20),
        "hani_serif_conflict": (700, 60),
    }
    cmap = {
        0x20: "space",
        0x519E: "hani_duplicate_edge",
        0x51A0: "hani_serif_conflict",
        0x51A1: "hani_nonreciprocal_chain",
        0x51A4: "hani_leading_skip",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Duplicate Edge",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Duplicate Edge Regular",
            "fullName": "Autohint CJK Duplicate Edge Regular",
            "psName": "AutohintCJKDuplicateEdge-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-duplicate-edge.ttf")


def build_cjk_remaining_branches() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_minor_same_position",
        "hani_grouped_shorter_link",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_minor_same_position": cjk_same_position_minor_edge_glyph(),
        "hani_grouped_shorter_link": cjk_grouped_shorter_link_glyph(),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_minor_same_position": (700, 40),
        "hani_grouped_shorter_link": (700, 40),
    }
    cmap = {
        0x20: "space",
        0x51B0: "hani_minor_same_position",
        0x51B1: "hani_grouped_shorter_link",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Remaining Branches",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Remaining Branches Regular",
            "fullName": "Autohint CJK Remaining Branches Regular",
            "psName": "AutohintCJKRemainingBranches-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-remaining-branches.ttf")


def build_digit_notdef_cmap() -> None:
    glyph_order = [".notdef", "space", "latin_o"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
    }
    cmap = {
        0x20: "space",
        # Exercise FreeType's digit-width scan case where a cmap-covered digit
        # still resolves to glyph 0.
        0x30: ".notdef",
        0x6F: "latin_o",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Digit Notdef Cmap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Digit Notdef Cmap Regular",
            "fullName": "Autohint Digit Notdef Cmap Regular",
            "psName": "AutohintDigitNotdefCmap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "digit-notdef-cmap.ttf")


def build_out_of_range_cmap_coverage() -> None:
    glyph_order = [".notdef", "space", "latin_O"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_O": ring_glyph(90, 0, 510, 680, 190, 120, 410, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_O": (620, 90),
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap({0x20: "space", 0x4F: "latin_O"})
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Out Of Range Cmap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Out Of Range Cmap Regular",
            "fullName": "Autohint Out Of Range Cmap Regular",
            "psName": "AutohintOutOfRangeCmap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    # Keep U+004F valid so the public load succeeds. U+0304 maps explicitly to
    # glyph zero, and U+0D00 shares its glyph with Latin so the later Malayalam
    # non-base scan must skip it. The other format-12 mappings intentionally
    # exceed maxp.numGlyphs=3: pinned FreeType returns those cmap GIDs, then
    # afglobal.c skips them while assigning script, digit, and non-base coverage.
    mappings = [
        (0x0020, 1),
        (0x0030, 254),
        (0x004F, 2),
        (0x006F, 255),
        (0x0303, 253),
        (0x0304, 0),
        (0x0D00, 2),
    ]
    groups = b"".join(
        struct.pack(">III", codepoint, codepoint, glyph_id)
        for codepoint, glyph_id in mappings
    )
    subtable = struct.pack(">HHIII", 12, 0, 16 + len(groups), 0, len(mappings)) + groups
    cmap_data = struct.pack(">HHHHI", 0, 1, 3, 10, 12) + subtable
    cmap = DefaultTable("cmap")
    cmap.data = cmap_data
    font.font["cmap"] = cmap

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "out-of-range-cmap-coverage.ttf")


def build_latin_standard_fallbacks() -> None:
    fallback_cases = [
        (
            "latin-missing-standard.ttf",
            "Autohint Latin Missing Standard",
            {
                ".notdef": rectangle_glyph(80, -120, 520, 720),
                "space": empty_glyph(),
                "latin_A": rectangle_glyph(100, 0, 540, 680),
            },
            {
                ".notdef": (600, 80),
                "space": (300, 0),
                "latin_A": (700, 100),
            },
            {
                0x20: "space",
                0x41: "latin_A",
            },
            [".notdef", "space", "latin_A"],
        ),
        (
            "latin-empty-standard.ttf",
            "Autohint Latin Empty Standard",
            {
                ".notdef": rectangle_glyph(80, -120, 520, 720),
                "space": empty_glyph(),
                "latin_o_empty": empty_glyph(),
                "latin_A": rectangle_glyph(100, 0, 540, 680),
            },
            {
                ".notdef": (600, 80),
                "space": (300, 0),
                "latin_o_empty": (620, 0),
                "latin_A": (700, 100),
            },
            {
                0x20: "space",
                0x41: "latin_A",
                0x6F: "latin_o_empty",
            },
            [".notdef", "space", "latin_o_empty", "latin_A"],
        ),
    ]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for filename, family, glyphs, metrics, cmap, glyph_order in fallback_cases:
        font = FontBuilder(UNITS_PER_EM, isTTF=True)
        font.setupGlyphOrder(glyph_order)
        font.setupCharacterMap(cmap)
        font.setupGlyf(glyphs)
        font.setupHorizontalMetrics(metrics)
        font.setupHorizontalHeader(ascent=820, descent=-220)
        font.setupNameTable(
            {
                "familyName": family,
                "styleName": "Regular",
                "uniqueFontIdentifier": f"{family} Regular",
                "fullName": f"{family} Regular",
                "psName": family.replace(" ", "") + "-Regular",
                "version": "Version 1.0",
            }
        )
        font.setupOS2(
            sTypoAscender=820,
            sTypoDescender=-220,
            usWinAscent=820,
            usWinDescent=220,
        )
        font.setupPost()

        head = font.font["head"]
        head.created = 0
        head.modified = 0
        font.font.recalcTimestamp = False
        font.save(OUT_DIR / filename)


def build_latin_malformed_standard() -> None:
    glyph_order = [".notdef", "space", "latin_A", "latin_o_malformed"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_A": rectangle_glyph(100, 0, 540, 680),
        "latin_o_malformed": rectangle_glyph(90, 0, 510, 520),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_A": (700, 100),
        "latin_o_malformed": (620, 90),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_A",
        0x6F: "latin_o_malformed",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Malformed Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Malformed Standard Regular",
            "fullName": "Autohint Latin Malformed Standard Regular",
            "psName": "AutohintLatinMalformedStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / "latin-malformed-standard.ttf"
    font.save(path)
    truncate_glyph_loca(path, "latin_o_malformed", 2)


def build_latin_blue_delta() -> None:
    glyph_order = [".notdef", "space", "latin_flat_cap", "latin_round_cap"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_flat_cap": rectangle_glyph(100, 0, 540, 500),
        "latin_round_cap": latin_blue_delta_round_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_flat_cap": (700, 100),
        "latin_round_cap": (700, 90),
    }
    cmap = {0x20: "space"}
    for codepoint in (0x54, 0x48, 0x45, 0x5A):
        cmap[codepoint] = "latin_flat_cap"
    for codepoint in (0x4F, 0x43, 0x51, 0x53):
        cmap[codepoint] = "latin_round_cap"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Delta",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Delta Regular",
            "fullName": "Autohint Latin Blue Delta Regular",
            "psName": "AutohintLatinBlueDelta-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-delta.ttf")


def build_latin_blue_overlap() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_cap_flat",
        "latin_cap_round_high",
        "latin_small_high",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_cap_flat": rectangle_glyph(100, 0, 540, 500),
        "latin_cap_round_high": ring_glyph(90, 0, 550, 700, 190, 120, 450, 560),
        "latin_small_high": rectangle_glyph(110, 0, 520, 650),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_cap_flat": (700, 100),
        "latin_cap_round_high": (700, 90),
        "latin_small_high": (620, 110),
    }
    cmap = {0x20: "space"}
    for codepoint in (0x54, 0x48, 0x45, 0x5A, 0x4C):
        cmap[codepoint] = "latin_cap_flat"
    for codepoint in (0x4F, 0x43, 0x51, 0x53):
        cmap[codepoint] = "latin_cap_round_high"
    for codepoint in (0x78, 0x7A, 0x6E, 0x72, 0x6F, 0x65, 0x73, 0x63):
        cmap[codepoint] = "latin_small_high"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Overlap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Overlap Regular",
            "fullName": "Autohint Latin Blue Overlap Regular",
            "psName": "AutohintLatinBlueOverlap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-overlap.ttf")


def build_latin_low_upem() -> None:
    units_per_em = 64
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_small_flat",
        "latin_cap_duplicate",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(4, -8, 36, 48),
        "space": empty_glyph(),
        "latin_o": ring_glyph(4, 0, 44, 36, 12, 8, 36, 28),
        "latin_small_flat": rectangle_glyph(6, 0, 40, 32),
        "latin_cap_duplicate": low_upem_duplicate_point_glyph(),
    }
    metrics = {
        ".notdef": (52, 4),
        "space": (24, 0),
        "latin_o": (52, 4),
        "latin_small_flat": (48, 6),
        "latin_cap_duplicate": (52, 4),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_cap_duplicate",
        0x6F: "latin_o",
    }
    for codepoint in (0x54, 0x48, 0x45, 0x5A, 0x4C):
        cmap[codepoint] = "latin_cap_duplicate"
    for codepoint in (0x78, 0x7A, 0x6E, 0x72):
        cmap[codepoint] = "latin_small_flat"
    for codepoint in (0x65, 0x73, 0x63):
        cmap[codepoint] = "latin_o"

    font = FontBuilder(units_per_em, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=52, descent=-12)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Low UPEM",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Low UPEM Regular",
            "fullName": "Autohint Latin Low UPEM Regular",
            "psName": "AutohintLatinLowUPEM-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=52,
        sTypoDescender=-12,
        usWinAscent=52,
        usWinDescent=12,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-low-upem.ttf")


def main() -> None:
    build_script_coverage()
    build_arabic_standard_fallback()
    build_cjk_empty_standard()
    build_latin_small_ignore()
    build_latin_remaining_topology()
    build_latin_width_clusters()
    build_latin_many_widths()
    build_cjk_blue_edge_cases()
    build_latin_blue_edge_cases()
    build_cjk_malformed_blue()
    build_cjk_tiny_stem()
    build_cjk_snap_below_standard()
    build_cjk_multi_width_snap()
    build_cjk_quantized_widths()
    build_cjk_many_widths()
    build_cjk_wide_stem_snap()
    build_cjk_round_stem_light()
    build_cjk_duplicate_edge()
    build_cjk_remaining_branches()
    build_digit_notdef_cmap()
    build_out_of_range_cmap_coverage()
    build_latin_standard_fallbacks()
    build_latin_malformed_standard()
    build_latin_blue_delta()
    build_latin_blue_overlap()
    build_latin_low_upem()


if __name__ == "__main__":
    main()
