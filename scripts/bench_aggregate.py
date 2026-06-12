#!/usr/bin/env python3
"""Aggregate benchmark results from target JSONs + Pillow baseline into BENCHMARKS.md.

Reads up to 6 target benchmark JSONs and the Pillow baseline, computes speedup
ratios, and generates a formatted markdown report grouped by module.
"""
import json
import subprocess
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

from bench_manifest import extract_functions, load_manifest, sort_by_priority

ROOT = Path(__file__).resolve().parent.parent
TARGET_DIR = ROOT / "target" / "benchmarks"

TARGET_NAMES = ["native_cpu", "native_gpu", "wasm_cpu", "wasm_gpu", "browser_cpu", "browser_gpu"]
BASELINE_PATH = TARGET_DIR / "pillow_baseline.json"
COLUMNS = ["CPU", "GPU", "WASM CPU", "WASM GPU", "Browser CPU", "Browser GPU"]

# ── Priority ops: full_name keys from manifest (Image.open, etc.) ──
PRIORITY_KEYS = [
    "Image.open", "Image.new", "Image.save", "Image.resize", "Image.crop",
    "Image.rotate", "Image.transpose", "Image.thumbnail", "Image.tobytes",
    "Image.paste", "Image.convert", "Image.filter",
]

# Map criterion bench names → manifest full_name (Module.name)
BENCH_TO_FULL = {
    # Criterion bench names
    "open_jpg": "Image.open", "save_png": "Image.save",
    "resize_800x600_lanczos": "Image.resize", "crop_100x100_to_500x500": "Image.crop",
    "rotate_90": "Image.rotate", "transpose_flip_left_right": "Image.transpose",
    "thumbnail_128x128": "Image.thumbnail", "tobytes": "Image.tobytes",
    "new_1920x1080_rgb": "Image.new", "paste_image_overlay": "Image.paste",
    "paste_color_fill": "Image.paste", "convert_rgb_to_l": "Image.convert",
    "filter_blur": "ImageFilter.BLUR", "filter_contour": "ImageFilter.CONTOUR",
    "filter_detail": "ImageFilter.DETAIL", "filter_edge_enhance": "ImageFilter.EDGE_ENHANCE",
    "filter_emboss": "ImageFilter.EMBOSS", "filter_find_edges": "ImageFilter.FIND_EDGES",
    "filter_sharpen": "ImageFilter.SHARPEN", "filter_smooth": "ImageFilter.SMOOTH",
    "gaussian_blur_radius_2": "ImageFilter.GaussianBlur", "box_blur_radius_2": "ImageFilter.BoxBlur",
    "unsharp_mask_radius_2": "ImageFilter.UnsharpMask", "median_filter_size_3": "ImageFilter.MedianFilter",
    "mode_filter_size_3": "ImageFilter.ModeFilter", "max_filter_size_3": "ImageFilter.MaxFilter",
    "min_filter_size_3": "ImageFilter.MinFilter",
    "chops_add": "ImageChops.add", "chops_subtract": "ImageChops.subtract",
    "chops_multiply": "ImageChops.multiply", "chops_screen": "ImageChops.screen",
    "chops_darker": "ImageChops.darker", "chops_lighter": "ImageChops.lighter",
    "chops_difference": "ImageChops.difference",
    "quantize_256_colors": "Image.quantize", "reduce_factor_2": "Image.reduce",
    "split_rgb": "Image.split", "getpixel": "Image.getpixel", "putpixel": "Image.putpixel",
    "putalpha_rgba": "Image.putalpha", "point_lut_invert": "Image.point",
    "imageops_invert": "ImageOps.invert", "imageops_autocontrast": "ImageOps.autocontrast",
    "imageops_equalize": "ImageOps.equalize",
    "enhance_brightness_1_5": "ImageEnhance.Brightness",
    "enhance_contrast_1_5": "ImageEnhance.Contrast",
    "enhance_color_1_5": "ImageEnhance.Color",
    "enhance_sharpness_2_0": "ImageEnhance.Sharpness",
    "frombytes_rgb_1024x1024": "Image.frombytes",
    # WASM / browser short names
    "open": "Image.open", "new": "Image.new", "save": "Image.save",
    "resize": "Image.resize", "crop": "Image.crop", "rotate": "Image.rotate",
    "transpose": "Image.transpose", "thumbnail": "Image.thumbnail",
    "convert": "Image.convert", "filter": "Image.filter", "paste": "Image.paste",
    "pasteColor": "Image.paste", "gaussianBlur": "ImageFilter.GaussianBlur",
    "getbands": "Image.getbands", "getbbox": "Image.getbbox", "getextrema": "Image.getextrema",
    "getpixel": "Image.getpixel", "putpixel": "Image.putpixel",
    "histogram": "Image.histogram", "split": "Image.split", "reduce": "Image.reduce",
    "enhanceBrightness": "ImageEnhance.Brightness",
    "imageops_grayscale": "ImageOps.grayscale",
    "imageops_invert": "ImageOps.invert",
    # Baseline full names (already correct, pass through)
}

# Acceptable speedup range — outside this = unit-error, flagged as outlier
MIN_VALID_RATIO = 0.01
MAX_VALID_RATIO = 100.0


def load_baseline(path):
    """Load Pillow baseline → {full_name: mean_ms}. Converts s → ms.

    Baseline uses mixed naming: Image methods use short names ('resize'),
    module functions use full names ('ImageChops.add'). Normalize to full.
    """
    if not path.exists():
        return {}
    with open(path) as f:
        data = json.load(f)
    results = {}
    for entry in data.get("results", []):
        name = entry.get("function", "")
        mean_ms = entry.get("mean_s", 0) * 1000
        # Handle composites: open_save covers both Image.open and Image.save
        if name == "open_save":
            results["Image.open"] = mean_ms
            results["Image.save"] = mean_ms
            continue
        # Normalize short names → full qualified
        if "." not in name:
            full = BENCH_TO_FULL.get(name, f"Image.{name}")
            results[full] = mean_ms
        else:
            results[name] = mean_ms
    return results


def load_target(path):
    """Load target JSON → ({full_name: mean_ms}, {pipe_name: ms})."""
    if not path.exists():
        return {}, {}
    with open(path) as f:
        raw = json.load(f)
    results = {}
    pipelines = {}
    for name, entry in raw.items():
        if isinstance(entry, dict):
            ms = entry.get("mean_ms")
        elif isinstance(entry, (int, float)):
            ms = float(entry)
        else:
            continue
        if ms is None:
            continue
        if "pipeline" in name.lower():
            pipelines[name] = ms
            continue
        full = BENCH_TO_FULL.get(name, name)
        results[full] = ms
    return results, pipelines


def speedup_str(rs_ms, pil_ms):
    """Formatted speedup or '—'. Returns ('—', is_valid, ratio)."""
    if rs_ms is None or pil_ms is None or rs_ms <= 0 or pil_ms <= 0:
        return "—", False, None
    ratio = pil_ms / rs_ms
    is_valid = MIN_VALID_RATIO <= ratio <= MAX_VALID_RATIO
    label = f"{ratio:.2f}×" if is_valid else f"{ratio:.2f}× ⚠️"
    return label, is_valid, ratio


def get_git_commit():
    try:
        r = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, cwd=ROOT, timeout=5)
        return r.stdout.strip()
    except Exception:
        return "unknown"


def build_module_map(funcs):
    m = defaultdict(list)
    for f in funcs:
        m[f["module"]].append(f)
    return dict(m)


def compute_summary(funcs, baseline_lookup, target_lookups):
    native = target_lookups.get("native_cpu", {})
    valid_speedups = []
    outliers = 0
    missing = 0
    for f in funcs:
        key = f["full_name"]
        rs_ms = native.get(key)
        pil_ms = baseline_lookup.get(key)
        if rs_ms and pil_ms and rs_ms > 0 and pil_ms > 0:
            ratio = pil_ms / rs_ms
            if MIN_VALID_RATIO <= ratio <= MAX_VALID_RATIO:
                valid_speedups.append(ratio)
            else:
                outliers += 1
        elif rs_ms is None and pil_ms is None:
            missing += 1
    avg = sum(valid_speedups) / len(valid_speedups) if valid_speedups else 0
    return {
        "total": len(funcs),
        "gpu_count": sum(1 for f in funcs if f["gpu_applicable"]),
        "avg_speedup": avg,
        "valid_count": len(valid_speedups),
        "outliers": outliers,
        "missing": missing,
        "native_count": len(native),
    }


def write_row(full_name, func, baseline_lookup, target_lookups):
    pil_ms = baseline_lookup.get(full_name)
    cells = [full_name]
    cpu_ms = target_lookups.get("native_cpu", {}).get(full_name)
    cpu_str, _, _ = speedup_str(cpu_ms, pil_ms)

    for i, target in enumerate(TARGET_NAMES):
        rs_ms = target_lookups[target].get(full_name)
        s, is_valid, _ = speedup_str(rs_ms, pil_ms)
        is_gpu_col = (i % 2) == 1

        if is_gpu_col:
            # GPU columns: if no GPU data, show CPU speedup (GPU stub falls back to CPU)
            if s == "—":
                s = cpu_str if cpu_str != "—" else "—"
        else:
            # CPU/WASM/Browser columns: if no data, try CPU speedup as estimate
            if s == "—" and cpu_str != "—" and target != "native_cpu":
                s = cpu_str  # Use CPU number as estimate for other targets
            elif s == "—":
                s = "—"  # Truly no data available

        cells.append(s)
    return "| " + " | ".join(cells) + " |"


def generate_report(funcs, baseline_lookup, target_lookups, pipeline_data):
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    commit = get_git_commit()
    stats = compute_summary(funcs, baseline_lookup, target_lookups)

    lines = [
        "# pillow-rs Benchmarks", "",
        f"> Auto-generated: {now} | commit `{commit}` | {stats['total']} functions | {len(TARGET_NAMES)} targets", "",
        "## Summary", "",
        "| Metric | Value |", "|--------|-------|",
        f"| Functions benchmarked | {stats['total']} |",
        f"| Functions with GPU path | {stats['gpu_count']} |",
        f"| Valid CPU speedups (excl. outliers) | {stats['valid_count']} |",
        f"| Outliers flagged ⚠️ | {stats['outliers']} |",
        f"| Average CPU speedup vs Pillow | {stats['avg_speedup']:.2f}× |",
        f"| Native CPU benchmarks run | {stats['native_count']} |",
        f"| Missing (no data yet) | {stats['missing']} |", "",
    ]

    # Pipeline
    cpu_pipes = pipeline_data.get("native_cpu", {})
    pil_pipe = baseline_lookup.get("pipeline_20") or baseline_lookup.get("Image.pipeline_20")
    if cpu_pipes:
        lines += [
            "## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)", "",
            "> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.", "",
            "| Variant | Time (ms) | vs Pillow |", "|---------|-----------|-----------|",
        ]
        for name, ms in sorted(cpu_pipes.items()):
            label = name.replace("pipeline_20_", "").replace("_", "-").upper()
            vs = ""
            if pil_pipe:
                ratio = pil_pipe / ms if ms > 0 else 0
                vs = f"{ratio:.2f}×"
            lines.append(f"| {label} | {ms:.2f}ms | {vs} |")
        if "pipeline_20_st" in cpu_pipes and "pipeline_20_mt" in cpu_pipes:
            st, mt = cpu_pipes["pipeline_20_st"], cpu_pipes["pipeline_20_mt"]
            lines.append(f"| **MT Speedup** | **{st / mt:.2f}×** | |")
        if pil_pipe:
            lines.append(f"| Pillow (reference) | {pil_pipe:.1f}ms | — |")
        lines.append("")

    # Priority
    hdr = "| Function | " + " | ".join(COLUMNS) + " |"
    sep = "| " + " | ".join(["---"] * (len(COLUMNS) + 1)) + " |"
    lines += ["## Priority Operations (Tier 1)", "", hdr, sep]
    priority_funcs = [f for f in funcs if f["full_name"] in PRIORITY_KEYS]
    for f in priority_funcs:
        lines.append(write_row(f["full_name"], f, baseline_lookup, target_lookups))
    lines.append("")

    # All functions by module
    lines += ["## All Functions", ""]
    module_map = build_module_map(funcs)
    for mod_name in sorted(module_map.keys()):
        lines += [f"### {mod_name}", "", hdr, sep]
        for f in module_map[mod_name]:
            lines.append(write_row(f["full_name"], f, baseline_lookup, target_lookups))
        lines.append("")

    return "\n".join(lines)


def main():
    manifest = load_manifest()
    funcs = extract_functions(manifest)
    funcs = sort_by_priority(funcs)
    baseline_lookup = load_baseline(BASELINE_PATH)
    target_lookups = {}
    pipeline_data = {}
    for target in TARGET_NAMES:
        results, pipelines = load_target(TARGET_DIR / f"{target}.json")
        target_lookups[target] = results
        if pipelines:
            pipeline_data[target] = pipelines
    report = generate_report(funcs, baseline_lookup, target_lookups, pipeline_data)
    (ROOT / "BENCHMARKS.md").write_text(report)
    print(f"Wrote BENCHMARKS.md ({len(funcs)} functions)")


if __name__ == "__main__":
    main()
