#!/usr/bin/env python3
"""Build FreeType-path fixture matrices from pinned FreeType C.

The C helper is the source of truth for every generated row.  It records the
exact load flags, render mode, glyph index, metrics, bboxes, bitmap placement,
and raw bitmap bytes for each glyph.

Examples:
  python3 scripts/build_ft_fixture.py --family force_autohint
  python3 scripts/build_ft_fixture.py --family no_hinting
  python3 scripts/build_ft_fixture.py --all-small
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FONT_DIR = ROOT / "tests/fixtures/input/fonts_autohint"
FIXTURE_DIR = ROOT / "tests/fixtures"
BUILD_DIR = ROOT / "freetype/build"
RAW_BASE = FIXTURE_DIR / "outputs"
DEFAULT_REF_BIN = Path(os.environ.get("FT_REF_BIN", "/tmp/gen_refs_v4"))

FULL_SIZES = (10, 20)
NATIVE_TT_SIZES = (10, 12, 16, 20, 24)
ASCII_CODEPOINTS = tuple(range(33, 127))
SMALL_SIZES = (10, 20)
SMALL_CODEPOINTS = (33, 65, 103, 109)
FULL_FAMILY = "force_autohint"
FAMILIES = (
    "native_tt_default",
    "force_autohint",
    "no_hinting",
    "metrics_only",
    "outline_cbox",
    "render_mono",
    "render_lcd",
)

FONT_MAP = {
    "DejaVuSans-ExtraLight": "DejaVuSans-ExtraLight.ttf",
    "DejaVuSans-Oblique": "DejaVuSans-Oblique.ttf",
    "DejaVuSansMono": "DejaVuSansMono.ttf",
    "DejaVuSerif-Bold": "DejaVuSerif-Bold.ttf",
    "DejaVuSerif-Italic": "DejaVuSerif-Italic.ttf",
    "LiberationSans-Regular": "LiberationSans-Regular.ttf",
    "LiberationSansNarrow-Bold": "LiberationSansNarrow-Bold.ttf",
    "NotoSans-Bold": "NotoSans-Bold.ttf",
}


def load_inventory() -> dict[str, dict]:
    path = FIXTURE_DIR / "font_inventory.json"
    with path.open() as fh:
        return json.load(fh)["fonts"]


def build_ref_bin(output: Path) -> None:
    cmd = [
        "cc",
        "-I",
        str(ROOT / "freetype/include"),
        str(ROOT / "scripts/gen_ft_refs.c"),
        "-L",
        str(BUILD_DIR),
        "-lfreetype",
        "-lm",
        "-o",
        str(output),
    ]
    subprocess.run(cmd, check=True)


def run_ref(ref_bin: Path, font_path: Path, cp: int, size: int, family: str) -> dict | None:
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(BUILD_DIR)
    result = subprocess.run(
        [str(ref_bin), "--json", str(font_path), f"{cp:04X}", str(size), family],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        env=env,
    )
    data = json.loads(result.stdout)
    if data.get("status") == "missing_glyph":
        return None
    return data


def run_face_ref(ref_bin: Path, font_path: Path, size: int) -> dict:
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(BUILD_DIR)
    result = subprocess.run(
        [str(ref_bin), "--face-json", str(font_path), str(size)],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        env=env,
    )
    return json.loads(result.stdout)


def operation_for_family(family: str) -> str:
    if family == "metrics_only":
        return "metrics_only"
    if family == "outline_cbox":
        return "outline_cbox"
    return "getmask"


def full_inventory_selection() -> tuple[dict[str, str], tuple[int, ...], dict[str, dict[str, list[int]]]]:
    inventory = load_inventory()
    fonts = {name: info["path"] for name, info in sorted(inventory.items())}
    codepoints_by_font = {
        name: {script: cps for script, cps in info["scripts"].items()}
        for name, info in sorted(inventory.items())
    }
    return fonts, FULL_SIZES, codepoints_by_font


def bitmap_bbox_array(bbox: dict) -> list[int]:
    pixels = bbox["bitmap_pixels"]
    return [pixels["x_min"], pixels["y_min"], pixels["x_max"], pixels["y_max"]]


def glyph_row_id(family: str, font_name: str, size: int, cp: int, op: str) -> str:
    if family == "native_tt_default":
        return f"{font_name}_{size}_{cp}_{op}"
    return f"{font_name}_{size}_{cp}_{family}_{op}"


def row_from_ref(family: str, font_name: str, size: int, cp: int, ref: dict, raw_dir: Path) -> dict:
    op = operation_for_family(family)
    row_id = glyph_row_id(family, font_name, size, cp, op)
    raw_bytes = bytes.fromhex(ref["raw_pixels"])

    bitmap = ref["bitmap"]
    bbox = ref["bbox"]
    row = {
        "id": row_id,
        "fixture_family": family,
        "generator": ref["generator"],
        "freetype_version": ref["freetype_version"],
        "load_flags": ref["load_flags"],
        "load_flags_value": ref["load_flags_value"],
        "render_mode": ref["render_mode"],
        "font": font_name,
        "font_file": ref["font"],
        "size_pt": size,
        "codepoint": cp,
        "glyph_index": ref["glyph_index"],
        "char": chr(cp) if cp <= 0x10FFFF else "",
        "operation": op,
        "status": "active",
        "metrics": ref["metrics"],
        "bbox": bbox,
        "bitmap": bitmap,
        "bitmap_placement": {"left": bitmap["left"], "top": bitmap["top"]},
        "raw_pixels": ref["raw_pixels"],
        "ref_sha256": hashlib.sha256(raw_bytes).hexdigest(),
        "ref_size": [bitmap["width"], bitmap["rows"]],
    }
    if op == "getmask":
        raw_name = f"{row_id}.bin"
        raw_path = raw_dir / raw_name
        raw_path.write_bytes(raw_bytes)
        row["ref_raw"] = str(raw_path.relative_to(FIXTURE_DIR))
        row["ref_value"] = bbox["bitmap_pixels"]
    elif op == "metrics_only":
        row["ref_value"] = ref["metrics"]
    else:
        row["ref_value"] = bbox
    return row


def rows_from_ref(family: str, font_name: str, size: int, cp: int, ref: dict, raw_dir: Path) -> list[dict]:
    row = row_from_ref(family, font_name, size, cp, ref, raw_dir)
    if family != "native_tt_default" or row["operation"] != "getmask":
        return [row]

    bbox_row = dict(row)
    bbox_row["id"] = glyph_row_id(family, font_name, size, cp, "getbbox")
    bbox_row["operation"] = "getbbox"
    bbox_row["ref_value"] = bitmap_bbox_array(row["bbox"])
    bbox_row.pop("ref_raw", None)
    return [row, bbox_row]


def native_face_rows(ref_bin: Path, font_name: str, font_path: Path, sizes: tuple[int, ...]) -> list[dict]:
    rows = []
    for size in sizes:
        ref = run_face_ref(ref_bin, font_path, size)
        common = {
            "fixture_family": "native_tt_default",
            "generator": "scripts/build_ft_fixture.py + scripts/gen_ft_refs.c",
            "freetype_version": ref["freetype_version"],
            "load_flags": ["FT_LOAD_RENDER"],
            "load_flags_value": 4,
            "render_mode": "FT_RENDER_MODE_NORMAL",
            "font": font_name,
            "font_file": ref["font"],
            "size_pt": size,
            "codepoint": 0,
            "glyph_index": 0,
            "char": "",
            "status": "active",
            "metrics": {},
            "bbox": {},
            "bitmap": {},
            "bitmap_placement": {},
            "raw_pixels": "",
            "script": "latn",
        }
        rows.append({
            **common,
            "id": f"{font_name}_{size}_getmetrics",
            "operation": "getmetrics",
            "ref_value": ref["metrics"],
        })
        rows.append({
            **common,
            "id": f"{font_name}_{size}_getname",
            "operation": "getname",
            "ref_value": ref["name"],
        })
        rows.append({
            **common,
            "id": f"{font_name}_{size}_getlength_hello",
            "operation": "getlength",
            "ref_value": ref["length_hello"],
        })
    return rows


def build_family(
    family: str,
    ref_bin: Path,
    small: bool,
    output: Path | None,
) -> Path:
    raw_dir = RAW_BASE / f"raws_{family}"
    raw_dir.mkdir(parents=True, exist_ok=True)
    for stale_raw in raw_dir.glob("*.bin"):
        stale_raw.unlink()
    rows = []

    if small:
        fonts = {"DejaVuSans-ExtraLight": FONT_MAP["DejaVuSans-ExtraLight"]}
        sizes = SMALL_SIZES
        codepoints_by_font = {"DejaVuSans-ExtraLight": {"latn": list(SMALL_CODEPOINTS)}}
    elif family == FULL_FAMILY:
        fonts, sizes, codepoints_by_font = full_inventory_selection()
    elif family == "native_tt_default":
        fonts = FONT_MAP
        sizes = NATIVE_TT_SIZES
        codepoints_by_font = {
            name: {"latn": list(ASCII_CODEPOINTS)} for name in sorted(FONT_MAP)
        }
    else:
        fonts, sizes, codepoints_by_font = full_inventory_selection()

    for font_name, font_file in fonts.items():
        font_path = FONT_DIR / font_file
        if not font_path.exists():
            print(f"SKIP {font_name}: {font_file} not found", file=sys.stderr)
            continue
        if family == "native_tt_default":
            rows.extend(native_face_rows(ref_bin, font_name, font_path, sizes))
        for script, codepoints in codepoints_by_font[font_name].items():
            for size in sizes:
                for cp in codepoints:
                    ref = run_ref(ref_bin, font_path, cp, size, family)
                    if ref is None:
                        continue
                    for row in rows_from_ref(family, font_name, size, cp, ref, raw_dir):
                        row["script"] = script
                        rows.append(row)

    matrix_path = output or (FIXTURE_DIR / f"{family}_matrix.json")
    matrix = {
        "version": "6.0.0",
        "fixture_family": family,
        "generator": "scripts/build_ft_fixture.py + scripts/gen_ft_refs.c",
        "source": "pinned FreeType C",
        "font_source": "fonts_autohint",
        "load_flags": rows[0]["load_flags"] if rows else [],
        "render_mode": rows[0]["render_mode"] if rows else "none",
        "assert_pixel_parity": True,
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": len(rows),
            "fonts": len({row["font"] for row in rows}),
            "sizes": sorted({row["size_pt"] for row in rows}),
            "glyphs": len({(row["font"], row["codepoint"]) for row in rows}),
        },
    }
    with matrix_path.open("w") as fh:
        json.dump(matrix, fh, indent=2, ensure_ascii=False)
    print(f"{family}: {len(rows)} rows -> {matrix_path}", file=sys.stderr)
    return matrix_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", choices=FAMILIES)
    parser.add_argument("--all-small", action="store_true")
    parser.add_argument("--small", action="store_true")
    parser.add_argument("--ref-bin", type=Path, default=DEFAULT_REF_BIN)
    parser.add_argument("--build-ref-bin", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    if args.build_ref_bin:
        build_ref_bin(args.ref_bin)
    if not args.ref_bin.exists():
        print(f"missing FreeType reference helper: {args.ref_bin}", file=sys.stderr)
        print("run with --build-ref-bin after building pinned FreeType", file=sys.stderr)
        return 1

    families = FAMILIES if args.all_small else (args.family,)
    if not families or families == (None,):
        parser.error("--family or --all-small is required")

    for family in families:
        build_family(family, args.ref_bin, args.small or args.all_small, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
