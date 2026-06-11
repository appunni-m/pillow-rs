#!/usr/bin/env python3
"""Aggregate benchmark results from target JSONs + Pillow baseline into BENCHMARKS.md.

Reads up to 6 target benchmark JSONs and the Pillow baseline, computes speedup
ratios, and generates a formatted markdown report grouped by module.
"""

import json
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import yaml

from bench_manifest import extract_functions, load_manifest, sort_by_priority

ROOT = Path(__file__).resolve().parent.parent
TARGET_DIR = ROOT / "target" / "benchmarks"

TARGET_NAMES = [
    "native_cpu",
    "native_gpu",
    "wasm_cpu",
    "wasm_gpu",
    "browser_cpu",
    "browser_gpu",
]

BASELINE_PATH = TARGET_DIR / "pillow_baseline.json"

COLUMNS = ["CPU", "GPU", "WASM CPU", "WASM GPU", "Browser CPU", "Browser GPU"]

PRIORITY_OP_NAMES = [
    "open_save", "resize", "crop", "rotate", "transpose", "thumbnail",
    "to_bytes", "new", "paste", "paste_mask", "paste_color", "pipeline",
]

# Map criterion benchmark names → manifest function names
BENCH_NAME_MAP = {
    "open_jpg": "open",
    "save_png": "save",
    "resize_800x600_lanczos": "resize",
    "crop_100x100_to_500x500": "crop",
    "rotate_90": "rotate",
    "transpose_flip_left_right": "transpose",
    "thumbnail_128x128": "thumbnail",
    "tobytes": "to_bytes",
    "new_1920x1080_rgb": "new",
    "paste_image_overlay": "paste",
    "paste_color_fill": "paste",
    "convert_rgb_to_l": "convert",
    "filter_blur": "filter",
    "filter_contour": "filter",
    "filter_detail": "filter",
    "filter_edge_enhance": "filter",
    "filter_emboss": "filter",
    "filter_find_edges": "filter",
    "filter_sharpen": "filter",
    "filter_smooth": "filter",
    "gaussian_blur_radius_2": "GaussianBlur",
    "box_blur_radius_2": "BoxBlur",
    "unsharp_mask_radius_2": "UnsharpMask",
    "median_filter_size_3": "MedianFilter",
    "mode_filter_size_3": "ModeFilter",
    "max_filter_size_3": "MaxFilter",
    "min_filter_size_3": "MinFilter",
    "chops_add": "add",
    "chops_subtract": "subtract",
    "chops_multiply": "multiply",
    "chops_screen": "screen",
    "chops_darker": "darker",
    "chops_lighter": "lighter",
    "chops_difference": "difference",
    "quantize_256_colors": "quantize",
    "reduce_factor_2": "reduce",
    "split_rgb": "split",
    "getpixel": "getpixel",
    "putpixel": "putpixel",
    "putalpha_rgba": "putalpha",
    "point_lut_invert": "point",
    "imageops_invert": "invert",
    "imageops_autocontrast": "autocontrast",
    "imageops_equalize": "equalize",
    "enhance_brightness_1_5": "Brightness",
    "enhance_contrast_1_5": "Contrast",
    "enhance_color_1_5": "Color",
    "enhance_sharpness_2_0": "Sharpness",
    "frombytes_rgb_1024x1024": "frombytes",
}


def load_baseline(path):
    """Load Pillow baseline, normalize to {func_name: mean_ms}."""
    if not path.exists():
        return {}
    with open(path) as f:
        data = json.load(f)
    results = {}
    for entry in data.get("results", []):
        name = entry["function"].split(".")[-1]  # "ImageChops.add" -> "add"
        mean_ms = entry.get("mean_s", 0) * 1000  # seconds -> milliseconds
        results[name] = mean_ms
    return results


def load_target(path):
    """Load a target benchmark JSON as {func_name: mean_ms}.

    Expected format: {func_name: {mean_ms: float, std_ms: float}}
    If the file does not exist, returns empty dict.
    Applies BENCH_NAME_MAP to normalize criterion names → manifest names.
    Also returns raw pipeline results separately.
    """
    if not path.exists():
        return {}, {}
    with open(path) as f:
        raw = json.load(f)
    results = {}
    pipelines = {}
    for func_name, entry in raw.items():
        if isinstance(entry, dict) and "mean_ms" in entry:
            ms = entry["mean_ms"]
        elif isinstance(entry, (int, float)):
            ms = float(entry)
        else:
            ms = None
        if ms is None:
            continue
        # Check for pipeline benchmarks
        if "pipeline" in func_name:
            pipelines[func_name] = ms
            continue
        # Apply name mapping
        mapped = BENCH_NAME_MAP.get(func_name, func_name)
        results[mapped] = ms
    return results, pipelines


def speedup_str(rs_ms, pil_ms):
    """Return formatted speedup string or '—'."""
    if rs_ms is None or pil_ms is None or rs_ms <= 0 or pil_ms <= 0:
        return "—"
    ratio = pil_ms / rs_ms
    return f"{ratio:.2f}×"


def get_git_commit():
    """Return short commit hash or 'unknown'."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=ROOT, timeout=5,
        )
        return result.stdout.strip()
    except Exception:
        return "unknown"


def build_module_map(funcs):
    """Group functions by module. Returns {module_name: [func_dict, ...]}."""
    modules = {}
    for f in funcs:
        module = f["module"]
        modules.setdefault(module, []).append(f)
    return modules


def build_target_data(baseline, funcs):
    """Load all existing target JSONs and return lookup data structures.

    Returns:
        baseline_lookup: {func_name: pil_mean_ms}
        target_lookups: {target_name: {func_name: rs_mean_ms}}
        pipeline_data: {target_name: {bench_name: mean_ms}}
    """
    baseline_lookup = load_baseline(BASELINE_PATH)

    target_lookups = {}
    pipeline_data = {}
    for target in TARGET_NAMES:
        path = TARGET_DIR / f"{target}.json"
        results, pipelines = load_target(path)
        target_lookups[target] = results
        if pipelines:
            pipeline_data[target] = pipelines

    return baseline_lookup, target_lookups, pipeline_data


def compute_summary_stats(funcs, baseline_lookup, target_lookups):
    """Compute summary statistics from available data."""
    total = len(funcs)
    gpu_count = sum(1 for f in funcs if f["gpu_applicable"])

    # Speedups only from native_cpu (always available if present)
    native_data = target_lookups.get("native_cpu", {})
    speedups = []
    for f in funcs:
        name = f["name"]
        rs_ms = native_data.get(name)
        pil_ms = baseline_lookup.get(name)
        if rs_ms is not None and pil_ms is not None and rs_ms > 0 and pil_ms > 0:
            speedups.append(pil_ms / rs_ms)

    avg_cpu_speedup = sum(speedups) / len(speedups) if speedups else 0

    return {
        "total": total,
        "gpu_count": gpu_count,
        "avg_cpu_speedup": avg_cpu_speedup,
        "functions_benchmarked": total,
    }


def write_row(func, baseline_lookup, target_lookups):
    """Build a markdown table row for one function."""
    name = func["name"]
    pil_ms = baseline_lookup.get(name)
    cells = [name]
    for i, target in enumerate(TARGET_NAMES):
        rs_ms = target_lookups[target].get(name)
        # Show real numbers for native_cpu/native_gpu on any function,
        # but for WASM/browser targets, only show for priority ops
        s = speedup_str(rs_ms, pil_ms)
        # Determine if GPU column should show:
        # Columns 1, 3, 5 (0-indexed) are GPU columns
        is_gpu_col = (i % 2) == 1
        if is_gpu_col and not func["gpu_applicable"]:
            s = "—"  # forced — for non-GPU functions
        cells.append(s)
    return "| " + " | ".join(cells) + " |"


def generate_report(funcs, baseline_lookup, target_lookups, pipeline_data):
    """Generate the full BENCHMARKS.md content."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    commit = get_git_commit()
    stats = compute_summary_stats(funcs, baseline_lookup, target_lookups)

    lines = []
    lines.append("# pillow-rs Benchmarks")
    lines.append("")
    lines.append(
        f"> Auto-generated: {now} | commit {commit} | "
        f"{stats['functions_benchmarked']} functions | {len(TARGET_NAMES)} targets"
    )
    lines.append("")

    # Summary section
    lines.append("## Summary")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("|--------|-------|")
    lines.append(f"| Functions benchmarked | {stats['total']} |")
    lines.append(f"| Functions with GPU path | {stats['gpu_count']} |")
    lines.append(f"| Average CPU speedup vs Pillow | {stats['avg_cpu_speedup']:.2f}× |")
    lines.append("")

    # --- Pipeline Benchmarks ---
    cpu_pipes = pipeline_data.get("native_cpu", {})
    if cpu_pipes:
        lines.append("## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)")
        lines.append("")
        lines.append("> Chaining 20 image operations end-to-end. Measures scheduling overhead, coherence, and clone avoidance.")
        lines.append("")
        lines.append("| Variant | Time (ms) |")
        lines.append("|---------|-----------|")
        for name, ms in sorted(cpu_pipes.items()):
            label = name.replace("pipeline_20_", "").replace("_", "-").upper()
            lines.append(f"| {label} | {ms:.2f}ms |")
        if "pipeline_20_st" in cpu_pipes and "pipeline_20_mt" in cpu_pipes:
            st = cpu_pipes["pipeline_20_st"]
            mt = cpu_pipes["pipeline_20_mt"]
            speedup = st / mt if mt > 0 else 0
            lines.append(f"| **MT Speedup** | **{speedup:.2f}×** |")
        lines.append("")

    # --- Priority Operations ---
    lines.append("## Priority Operations (Tier 1)")
    lines.append("")
    header = "| Function | " + " | ".join(COLUMNS) + " |"
    sep = "| " + " | ".join(["---"] * (len(COLUMNS) + 1)) + " |"
    lines.append(header)
    lines.append(sep)

    priority_funcs = [f for f in funcs if f["name"] in PRIORITY_OP_NAMES]
    for f in priority_funcs:
        lines.append(write_row(f, baseline_lookup, target_lookups))
    lines.append("")

    # --- All Functions by Module ---
    lines.append("## All Functions")
    lines.append("")

    module_map = build_module_map(funcs)
    for module_name in sorted(module_map.keys()):
        lines.append(f"### {module_name}")
        lines.append("")
        header = "| Function | " + " | ".join(COLUMNS) + " |"
        sep = "| " + " | ".join(["---"] * (len(COLUMNS) + 1)) + " |"
        lines.append(header)
        lines.append(sep)

        for f in module_map[module_name]:
            lines.append(write_row(f, baseline_lookup, target_lookups))
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

    output_path = ROOT / "BENCHMARKS.md"
    output_path.write_text(report)
    print(f"Wrote {output_path} ({len(funcs)} functions)")


if __name__ == "__main__":
    main()
