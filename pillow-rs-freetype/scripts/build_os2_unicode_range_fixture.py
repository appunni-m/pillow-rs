#!/usr/bin/env python3
"""Build shared OS/2 Unicode-range fixture fonts for public API inputs."""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
PUBLIC_API_INPUTS = FIXTURE_ROOT / "inputs" / "public-api"
FONT_ROOT = FIXTURE_ROOT / "input" / "fonts"
BASE_FONT = FONT_ROOT / "DejaVuSans.ttf"
SHARED_FONT = FONT_ROOT / "generated" / "os2-unicode-ranges.ttf"
CONSTANTS_RS = ROOT / "src" / "ffi" / "generated_constants.rs"


def load_constants() -> dict[str, int]:
    constants: dict[str, int] = {}
    pattern = re.compile(r"pub const ([A-Za-z0-9_]+): i64 = (-?\d+);")
    for line in CONSTANTS_RS.read_text().splitlines():
        match = pattern.match(line)
        if match:
            constants[match.group(1)] = int(match.group(2))
    return constants


def int_value(value: object, constants: dict[str, int]) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        text = value.strip()
        if text.startswith("U+"):
            return int(text[2:], 16)
        try:
            return int(text, 0)
        except ValueError:
            return constants.get(text)
    return None


def font_ref(case: dict[str, object]) -> str | None:
    inputs = case.get("inputs")
    if not isinstance(inputs, dict):
        return None
    assets = inputs.get("assets")
    if not isinstance(assets, dict):
        return None
    font = assets.get("font")
    if not isinstance(font, dict):
        return None
    ref = font.get("path") or font.get("id")
    return ref if isinstance(ref, str) else None


def field_index(field: str | None, bit: int | None) -> int | None:
    if field:
        match = re.fullmatch(r"ulUnicodeRange([1-4])", field)
        if match:
            return int(match.group(1)) - 1
    if bit is not None:
        return bit // 32
    return None


def field_mask(params: dict[str, object], constants: dict[str, int]) -> tuple[int, int] | None:
    sources: list[dict[str, object]] = []
    expected_set_bit = params.get("expected_set_bit")
    if isinstance(expected_set_bit, dict):
        sources.append(expected_set_bit)
    os2_range = params.get("os2_unicode_range")
    if isinstance(os2_range, dict):
        sources.append(os2_range)
    sources.append(params)

    field: str | None = None
    bit: int | None = None
    mask: int | None = None
    for source in sources:
        field = field or source.get("field") or source.get("expected_field")
        if not isinstance(field, str):
            field = None
        for key in ("os2_absolute_bit", "expected_global_bit", "global_bit"):
            value = int_value(source.get(key), constants)
            if value is not None:
                bit = value
                break
        for key in ("mask_value", "expected_mask_from_header", "expected_value_from_pinned_header"):
            value = int_value(source.get(key), constants)
            if value is not None:
                mask = value
                break
        if mask is None:
            symbol = source.get("mask_symbol") or source.get("symbol")
            mask = int_value(symbol, constants)
    index = field_index(field, bit)
    if index is None or not (0 <= index < 4) or mask is None:
        return None
    return index, mask


def codepoints(params: dict[str, object], constants: dict[str, int]) -> set[int]:
    values = params.get("representative_codepoints") or params.get("codepoints") or []
    if not isinstance(values, list):
        return set()
    parsed = set()
    for value in values:
        item = int_value(value, constants)
        if item is not None:
            parsed.add(item)
    return parsed


def collect_requirements(constants: dict[str, int]) -> tuple[list[int], set[int], set[str]]:
    ranges = [0, 0, 0, 0]
    cmap_codepoints: set[int] = set()
    asset_refs: set[str] = set()
    for path in sorted(PUBLIC_API_INPUTS.glob("*.json")):
        data = json.loads(path.read_text())
        for case in data.get("cases", []):
            if case.get("operation") != "sfnt.get_os2_unicode_ranges":
                continue
            ref = font_ref(case)
            if ref:
                asset_refs.add(ref)
            params = case.get("inputs", {}).get("params", {})
            if not isinstance(params, dict):
                params = {}
            mask = field_mask(params, constants)
            if mask:
                index, value = mask
                ranges[index] |= value
            cmap_codepoints.update(codepoints(params, constants))
    return ranges, cmap_codepoints, asset_refs


def ensure_cmap_subtable(font: TTFont, platform_id: int, encoding_id: int, fmt: int) -> CmapSubtable:
    cmap = font["cmap"]
    for table in cmap.tables:
        if table.platformID == platform_id and table.platEncID == encoding_id and table.format == fmt:
            return table
    table = CmapSubtable.newSubtable(fmt)
    table.platformID = platform_id
    table.platEncID = encoding_id
    table.language = 0
    table.cmap = {}
    cmap.tables.append(table)
    return table


def build_shared_font(ranges: list[int], cmap_codepoints: set[int]) -> None:
    SHARED_FONT.parent.mkdir(parents=True, exist_ok=True)
    font = TTFont(BASE_FONT)
    os2 = font["OS/2"]
    os2.ulUnicodeRange1 = ranges[0]
    os2.ulUnicodeRange2 = ranges[1]
    os2.ulUnicodeRange3 = ranges[2]
    os2.ulUnicodeRange4 = ranges[3]

    glyph_name = "A" if "A" in font.getGlyphOrder() else font.getGlyphOrder()[1]
    bmp_cmap = ensure_cmap_subtable(font, 3, 1, 4)
    full_cmap = ensure_cmap_subtable(font, 3, 10, 12)
    for codepoint in sorted(cmap_codepoints):
        if codepoint <= 0xFFFF:
            bmp_cmap.cmap[codepoint] = glyph_name
        full_cmap.cmap[codepoint] = glyph_name
    font.save(SHARED_FONT)


def link_assets(asset_refs: set[str]) -> None:
    for ref in sorted(asset_refs):
        if not ref.startswith("input/fonts/"):
            continue
        target = FIXTURE_ROOT / ref
        if target.exists():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        rel = os.path.relpath(SHARED_FONT, target.parent)
        target.symlink_to(rel)


def main() -> None:
    constants = load_constants()
    ranges, cmap_codepoints, asset_refs = collect_requirements(constants)
    build_shared_font(ranges, cmap_codepoints)
    link_assets(asset_refs)
    print(f"wrote {SHARED_FONT.relative_to(ROOT)} with ranges={ranges}")
    print(f"mapped {len(cmap_codepoints)} representative codepoints")
    print(f"ensured {len(asset_refs)} OS/2 Unicode-range fixture asset paths")


if __name__ == "__main__":
    main()
