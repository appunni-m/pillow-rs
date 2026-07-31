#!/usr/bin/env python3
"""Shared benchmark operation definitions — single source of truth.

Every harness (Pillow baseline, Rust CPU, WASM, browser) references the
same function definitions. One function = one section = easy to find.
Add or modify a benchmark in ONE place.

Usage:
  python scripts/bench_spec.py --list              # List all functions
  python scripts/bench_spec.py --list --json        # As JSON
  python scripts/bench_spec.py --group filters      # Just filter functions
  python scripts/bench_spec.py --group priority     # Just priority ops
  python scripts/bench_spec.py --only resize,crop   # Specific functions
"""

import json, sys
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent

# ── Input references (existing images only — no new images) ──

INPUTS = {
    "ref_1k":   {"file": "scripts/bench_reference_images/ref_1k.png",        "w": 1024, "h": 1024, "mode": "RGBA"},
    "ref_2k":   {"file": "scripts/bench_reference_images/ref_2k.jpg",        "w": 2048, "h": 1536, "mode": "RGB"},
    "ref_gray": {"file": "scripts/bench_reference_images/ref_grayscale.png", "w": 1024, "h": 1024, "mode": "L"},
}

# ── Benchmark function definitions ──
# One entry per function. Each entry: {name, group, input, params, description}
# This is the SINGLE SOURCE OF TRUTH. All harnesses use these definitions.

BENCH_FUNCTIONS = [
    # ═══ Priority Operations (Tier 1 — puhu parity) ═══
    {"name": "open",          "group": "priority", "input": "ref_1k",  "params": {},                                  "desc": "Image.open() — decode PNG from bytes"},
    {"name": "new",           "group": "priority", "input": None,      "params": {"mode": "RGB", "w": 1920, "h": 1080, "color": (255,0,0,255)}, "desc": "Image.new() — create blank image"},
    {"name": "save",          "group": "priority", "input": "ref_1k",  "params": {"format": "PNG"},                    "desc": "Image.save() — encode to PNG bytes"},
    {"name": "resize",        "group": "priority", "input": "ref_2k",  "params": {"w": 800, "h": 600, "filter": "LANCZOS"}, "desc": "Image.resize() — 2048→800×600 LANCZOS"},
    {"name": "crop",          "group": "priority", "input": "ref_2k",  "params": {"l": 100, "t": 100, "r": 500, "b": 500}, "desc": "Image.crop() — 400×400 subregion"},
    {"name": "rotate",        "group": "priority", "input": "ref_2k",  "params": {"angle": 90},                       "desc": "Image.rotate() — 90° rotation"},
    {"name": "transpose",     "group": "priority", "input": "ref_1k",  "params": {"method": "FLIP_LEFT_RIGHT"},       "desc": "Image.transpose() — horizontal flip"},
    {"name": "thumbnail",     "group": "priority", "input": "ref_1k",  "params": {"w": 200, "h": 200},                "desc": "Image.thumbnail() — 200×200 thumbnail"},
    {"name": "to_bytes",      "group": "priority", "input": "ref_1k",  "params": {},                                  "desc": "Image.tobytes() — raw pixel bytes"},
    {"name": "paste",         "group": "priority", "input": "ref_2k",  "params": {"src_w": 800, "src_h": 600, "x": 100, "y": 100}, "desc": "Image.paste() — 800×600 image onto 2048×1536"},
    {"name": "pasteColor",    "group": "priority", "input": "ref_2k",  "params": {"r": 255, "g": 0, "b": 0, "a": 255, "x": 100, "y": 100, "w": 800, "h": 600}, "desc": "Image.paste() — color fill 800×600 onto 2048×1536"},
    {"name": "convert",       "group": "priority", "input": "ref_1k",  "params": {"mode": "L"},                        "desc": "Image.convert() — RGBA→L"},

    # ═══ Filters ═══
    {"name": "filter_blur",           "group": "filters", "input": "ref_1k", "params": {"filter": "BLUR"},            "desc": "Image.filter(BLUR) — 3×3 box blur"},
    {"name": "filter_contour",        "group": "filters", "input": "ref_1k", "params": {"filter": "CONTOUR"},         "desc": "Image.filter(CONTOUR)"},
    {"name": "filter_detail",         "group": "filters", "input": "ref_1k", "params": {"filter": "DETAIL"},          "desc": "Image.filter(DETAIL)"},
    {"name": "filter_edge_enhance",   "group": "filters", "input": "ref_1k", "params": {"filter": "EDGE_ENHANCE"},    "desc": "Image.filter(EDGE_ENHANCE)"},
    {"name": "filter_emboss",         "group": "filters", "input": "ref_1k", "params": {"filter": "EMBOSS"},          "desc": "Image.filter(EMBOSS)"},
    {"name": "filter_sharpen",        "group": "filters", "input": "ref_1k", "params": {"filter": "SHARPEN"},         "desc": "Image.filter(SHARPEN)"},
    {"name": "filter_smooth",         "group": "filters", "input": "ref_1k", "params": {"filter": "SMOOTH"},          "desc": "Image.filter(SMOOTH)"},
    {"name": "gaussian_blur",         "group": "filters", "input": "ref_1k", "params": {"radius": 3.0},               "desc": "ImageFilter.GaussianBlur(3.0)"},

    # ═══ Channel Operations ═══
    {"name": "invert",        "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.invert()"},
    {"name": "autocontrast",  "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageOps.autocontrast()"},
    {"name": "equalize",      "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageOps.equalize()"},
    {"name": "add",           "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.add()"},
    {"name": "subtract",      "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.subtract()"},
    {"name": "multiply",      "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.multiply()"},
    {"name": "screen",        "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.screen()"},
    {"name": "darker",        "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.darker()"},
    {"name": "lighter",       "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.lighter()"},
    {"name": "difference",    "group": "chops", "input": "ref_1k", "params": {},                                  "desc": "ImageChops.difference()"},

    # ═══ Enhance ═══
    {"name": "enhance_brightness", "group": "enhance", "input": "ref_1k", "params": {"factor": 1.5},               "desc": "ImageEnhance.Brightness(1.5)"},
    {"name": "enhance_contrast",   "group": "enhance", "input": "ref_1k", "params": {"factor": 1.5},               "desc": "ImageEnhance.Contrast(1.5)"},
    {"name": "enhance_color",      "group": "enhance", "input": "ref_1k", "params": {"factor": 1.5},               "desc": "ImageEnhance.Color(1.5)"},
    {"name": "enhance_sharpness",  "group": "enhance", "input": "ref_1k", "params": {"factor": 2.0},               "desc": "ImageEnhance.Sharpness(2.0)"},

    # ═══ Misc ═══
    {"name": "getpixel",      "group": "misc", "input": "ref_1k", "params": {"x": 100, "y": 100},                  "desc": "Image.getpixel()"},
    {"name": "putpixel",      "group": "misc", "input": None,     "params": {"w": 100, "h": 100},                   "desc": "Image.putpixel() — 10k pixels"},
    {"name": "split",         "group": "misc", "input": "ref_1k", "params": {},                                    "desc": "Image.split() — split channels"},
    {"name": "getbands",      "group": "misc", "input": "ref_1k", "params": {},                                    "desc": "Image.getbands()"},
    {"name": "getbbox",       "group": "misc", "input": "ref_1k", "params": {},                                    "desc": "Image.getbbox()"},
    {"name": "getextrema",    "group": "misc", "input": "ref_1k", "params": {},                                    "desc": "Image.getextrema()"},
    {"name": "histogram",     "group": "misc", "input": "ref_1k", "params": {},                                    "desc": "Image.histogram()"},
    {"name": "reduce",        "group": "misc", "input": "ref_1k", "params": {"factor": 2},                         "desc": "Image.reduce(2)"},
    {"name": "quantize",      "group": "misc", "input": "ref_1k", "params": {"colors": 256},                       "desc": "Image.quantize(256)"},
]


def get_functions(groups=None, only=None):
    """Filter functions by group(s) and/or name(s)."""
    funcs = BENCH_FUNCTIONS
    if groups:
        group_set = set(groups.split(","))
        funcs = [f for f in funcs if f["group"] in group_set]
    if only:
        name_set = set(only.split(","))
        funcs = [f for f in funcs if f["name"] in name_set]
    return funcs


def get_input(name):
    """Get input image info for a benchmark function."""
    return INPUTS.get(name)


def input_for_function(func):
    """Get the input spec for a function."""
    inp_name = func.get("input")
    if inp_name is None:
        return None
    return INPUTS.get(inp_name)


if __name__ == "__main__":
    funcs = BENCH_FUNCTIONS
    if "--group" in sys.argv:
        idx = sys.argv.index("--group")
        funcs = get_functions(groups=sys.argv[idx + 1])
    if "--only" in sys.argv:
        idx = sys.argv.index("--only")
        funcs = get_functions(only=sys.argv[idx + 1])

    if "--json" in sys.argv:
        output = {"inputs": INPUTS, "functions": funcs}
        print(json.dumps(output, indent=2))
    else:
        groups_seen = []
        for f in funcs:
            if f["group"] != (groups_seen[-1] if groups_seen else None):
                groups_seen.append(f["group"])
                print(f"\n═══ {f['group'].upper()} ═══")
            print(f"  {f['name']:<30} | {f['input'] or 'synthetic':<8} | {f['desc']}")
        print(f"\n{len(funcs)} functions")
