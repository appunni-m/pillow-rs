#!/usr/bin/env python3
"""Build shared SFNT name-table fixture fonts for public API inputs."""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable
from fontTools.ttLib.tables._n_a_m_e import NameRecord


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
PUBLIC_API_INPUTS = FIXTURE_ROOT / "inputs" / "public-api"
FONT_ROOT = FIXTURE_ROOT / "input" / "fonts"
BASE_FONT = FONT_ROOT / "DejaVuSans.ttf"
SHARED_FONT = FONT_ROOT / "generated" / "sfnt-name-records.ttf"
CONSTANTS_RS = ROOT / "src" / "ffi" / "generated_constants.rs"

ENCODING_CHARMAP_IDS = {
    "FT_ENCODING_APPLE_ROMAN": (1, 0),
    "FT_ENCODING_MS_SYMBOL": (3, 0),
    "FT_ENCODING_SJIS": (3, 2),
    "FT_ENCODING_MS_SJIS": (3, 2),
    "FT_ENCODING_PRC": (3, 3),
    "FT_ENCODING_GB2312": (3, 3),
    "FT_ENCODING_MS_GB2312": (3, 3),
    "FT_ENCODING_BIG5": (3, 4),
    "FT_ENCODING_MS_BIG5": (3, 4),
    "FT_ENCODING_WANSUNG": (3, 5),
    "FT_ENCODING_MS_WANSUNG": (3, 5),
    "FT_ENCODING_JOHAB": (3, 6),
    "FT_ENCODING_MS_JOHAB": (3, 6),
}


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
        if value in {"any", "-"}:
            return None
        try:
            return int(value, 0)
        except ValueError:
            return constants.get(value)
    return None


def codepoint_value(value: object, constants: dict[str, int]) -> int | None:
    if isinstance(value, str) and value.startswith("U+"):
        try:
            return int(value[2:], 16)
        except ValueError:
            return None
    return int_value(value, constants)


def field_value(params: dict[str, object], field: str, constants: dict[str, int]) -> int | None:
    sources = []
    match = params.get("match")
    if isinstance(match, dict):
        sources.append(match)
    sources.append(params)
    keys = (
        f"{field}_value",
        field,
        f"{field}_symbol",
    )
    for source in sources:
        for key in keys:
            if key in source:
                value = int_value(source[key], constants)
                if value is not None:
                    return value
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


def collect_records(
    constants: dict[str, int],
) -> tuple[list[tuple[int, int, int, int, str]], dict[tuple[int, int], set[int]], set[str]]:
    records: list[tuple[int, int, int, int, str]] = []
    cmap_records: dict[tuple[int, int], set[int]] = {}
    asset_refs: set[str] = set()
    operations = {
        "ftsnames.get_sfnt_name_by_record",
        "ftsnames.get_sfnt_name_group",
        "sfnt.get_sfnt_name",
        "sfnt.get_name",
        "sfnt.mac_encoding_record",
        "charmap.get_char_index",
    }
    for path in sorted(PUBLIC_API_INPUTS.glob("*.json")):
        data = json.loads(path.read_text())
        for case in data.get("cases", []):
            if case.get("operation") in {"freetype.inspect_charmaps", "freetype.charmap_ownership"}:
                ref = font_ref(case)
                if ref and ref.startswith("fonts/charmap/"):
                    asset_refs.add(ref)
            if case.get("operation") not in operations:
                if case.get("operation") == "freetype.select_charmap":
                    ref = font_ref(case)
                    if ref:
                        asset_refs.add(ref)
                    params = case.get("inputs", {}).get("params", {})
                    if not isinstance(params, dict):
                        continue
                    encoding = params.get("encoding")
                    if isinstance(encoding, str) and encoding in ENCODING_CHARMAP_IDS:
                        codepoints = params.get("probe_char_codes") or params.get("post_probe_chars")
                        values = set()
                        if isinstance(codepoints, list):
                            for codepoint in codepoints:
                                value = codepoint_value(codepoint, constants)
                                if value is not None:
                                    values.add(value)
                        if not values:
                            values.add(0x41)
                        cmap_records.setdefault(ENCODING_CHARMAP_IDS[encoding], set()).update(values)
                continue
            ref = font_ref(case)
            if not ref:
                continue
            asset_refs.add(ref)
            params = case.get("inputs", {}).get("params", {})
            if not isinstance(params, dict):
                params = {}
            platform_id = field_value(params, "platform_id", constants)
            encoding_id = field_value(params, "encoding_id", constants)
            language_id = field_value(params, "language_id", constants)
            name_id = field_value(params, "name_id", constants)

            if platform_id is None:
                platform_id = 1 if "/mac-" in ref or "mac_" in case.get("case_id", "") else 3
            if encoding_id is None:
                encoding_id = 0 if platform_id == 1 else 1
            if language_id is None:
                language_id = 0 if platform_id == 1 else 0x0409
            if name_id is None:
                name_id = 4

            label = case.get("case_id", "sfnt-name-record")
            records.append((platform_id, encoding_id, language_id, name_id, str(label)))
            if case.get("operation") in {"sfnt.mac_encoding_record", "charmap.get_char_index"}:
                codepoints = params.get("codepoints")
                values = set()
                if isinstance(codepoints, list):
                    for codepoint in codepoints:
                        value = codepoint_value(codepoint, constants)
                        if value is not None:
                            values.add(value)
                if not values:
                    values.add(0x41)
                cmap_records.setdefault((platform_id, encoding_id), set()).update(values)
    return records, cmap_records, asset_refs


def make_name_record(
    platform_id: int,
    encoding_id: int,
    language_id: int,
    name_id: int,
    label: str,
) -> NameRecord:
    record = NameRecord()
    record.nameID = name_id
    record.platformID = platform_id
    record.platEncID = encoding_id
    record.langID = language_id
    text = f"fontdone {label}"
    if platform_id == 3:
        record.string = text.encode("utf-16-be")
    else:
        record.string = text.encode("mac_roman", errors="replace")
    return record


def build_shared_font(
    records: list[tuple[int, int, int, int, str]], cmap_records: dict[tuple[int, int], set[int]]
) -> None:
    SHARED_FONT.parent.mkdir(parents=True, exist_ok=True)
    font = TTFont(BASE_FONT)
    name_table = font["name"]
    seen = {
        (name.platformID, name.platEncID, name.langID, name.nameID, bytes(name.string))
        for name in name_table.names
    }
    for platform_id, encoding_id, language_id, name_id, label in records:
        record = make_name_record(platform_id, encoding_id, language_id, name_id, label)
        key = (record.platformID, record.platEncID, record.langID, record.nameID, bytes(record.string))
        if key not in seen:
            name_table.names.append(record)
            seen.add(key)
    if cmap_records:
        cmap_table = font["cmap"]
        replaced = set(cmap_records)
        cmap_table.tables = [
            table
            for table in cmap_table.tables
            if (table.platformID, table.platEncID) not in replaced
        ]
        glyph_name = "A" if "A" in font.getGlyphOrder() else font.getGlyphOrder()[0]
        for (platform_id, encoding_id), codepoints in sorted(cmap_records.items()):
            table = CmapSubtable.newSubtable(12)
            table.platformID = platform_id
            table.platEncID = encoding_id
            table.language = 0
            table.cmap = {codepoint: glyph_name for codepoint in sorted(codepoints)}
            cmap_table.tables.append(table)
    font.save(SHARED_FONT)


def link_assets(asset_refs: set[str]) -> None:
    for ref in sorted(asset_refs):
        if not (ref.startswith("input/fonts/") or ref.startswith("fonts/")):
            continue
        target = FIXTURE_ROOT / ref
        if target.exists():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        rel = os.path.relpath(SHARED_FONT, target.parent)
        target.symlink_to(rel)


def main() -> None:
    constants = load_constants()
    records, cmap_records, asset_refs = collect_records(constants)
    build_shared_font(records, cmap_records)
    link_assets(asset_refs)
    print(f"wrote {SHARED_FONT.relative_to(ROOT)} with {len(records)} generated name records")
    print(f"added {len(cmap_records)} generated cmap platform/encoding records")
    print(f"ensured {len(asset_refs)} SFNT name fixture asset paths")


if __name__ == "__main__":
    main()
