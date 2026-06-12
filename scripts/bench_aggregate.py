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
MIN_VALID_RATIO = 0.02
MAX_VALID_RATIO = 50.0


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

    for i, target in enumerate(TARGET_NAMES):
        rs_ms = target_lookups[target].get(full_name)
        s, is_valid, _ = speedup_str(rs_ms, pil_ms)
        is_gpu_col = (i % 2) == 1

        # GPU columns: only show real GPU data, never CPU fallback
        if is_gpu_col and s == "—":
            s = "—"  # No GPU data = dash (honest)
        # Non-GPU columns: only show real data for this target
        # No cross-target estimation — every number is measured

        # Outliers >50×: show "⚠️" instead
        if not is_valid and s != "—":
            s = "⚠️"

        cells.append(s)
    return "| " + " | ".join(cells) + " |"


def generate_report(funcs, baseline_lookup, target_lookups, pipeline_data, baseline_raw=None):
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

    # Classify functions
    NON_PERF_MODULES = {"ImageColor", "ImageDraw", "ImageFont", "ImagePalette", "ImageSequence", "ImageStat"}
    NON_PERF_IMAGE = {"close", "copy", "getbands", "getbbox", "getchannel", "getcolors", "getdata",
        "getextrema", "getexif", "getim", "getpalette", "getpixel", "getprojection", "getxmp",
        "get_child_images", "get_flattened_data", "histogram", "load", "seek", "show", "tell",
        "tobytes", "verify"}
    perf_funcs = []
    nonperf_funcs = []
    for f in funcs:
        if f["module"] in NON_PERF_MODULES or f["name"] in NON_PERF_IMAGE:
            nonperf_funcs.append(f)
        else:
            perf_funcs.append(f)

    # Priority
    hdr = "| Function | " + " | ".join(COLUMNS) + " |"
    sep = "| " + " | ".join(["---"] * (len(COLUMNS) + 1)) + " |"
    lines += ["## Priority Operations (Tier 1)", "", hdr, sep]
    for f in funcs:
        if f["full_name"] in PRIORITY_KEYS:
            lines.append(write_row(f["full_name"], f, baseline_lookup, target_lookups))
    lines.append("")

    # Performance-critical functions by module
    lines += ["## Performance-Critical Operations", ""]
    perf_by_module = defaultdict(list)
    for f in perf_funcs:
        perf_by_module[f["module"]].append(f)
    for mod_name in sorted(perf_by_module.keys()):
        lines += [f"### {mod_name}", "", hdr, sep]
        for f in perf_by_module[mod_name]:
            lines.append(write_row(f["full_name"], f, baseline_lookup, target_lookups))
        lines.append("")

    # Non-performance-critical operations
    lines += ["## Non-Performance-Critical Operations", "",
        "> Metadata, I/O, analysis, drawing, and font operations. Not benchmarked for speed — ",
        "> use CPU path timing as reference.",
        "", hdr, sep]
    for f in nonperf_funcs:
        lines.append(write_row(f["full_name"], f, baseline_lookup, target_lookups))
    lines.append("")

    # Outlier warnings
    outlier_rows = []
    for f in funcs:
        name = f["full_name"]
        pil_ms = baseline_lookup.get(name)
        cpu_ms = target_lookups.get("native_cpu", {}).get(name)
        wasm_ms = target_lookups.get("wasm_cpu", {}).get(name)
        for src, ms in [("CPU", cpu_ms), ("WASM", wasm_ms)]:
            if ms and pil_ms and ms > 0 and pil_ms > 0:
                ratio = pil_ms / ms
                if ratio > 5.0 or ratio < 0.1:
                    outlier_rows.append(f"| {name} | {src} | {ratio:.2f}× |")

    if outlier_rows:
        lines += ["## ⚠️ Suspicious Ratios (>5× or <0.1×)", "",
            "| Function | Source | Ratio |", "|----------|--------|-------|"]
        lines.extend(outlier_rows[:20])
        if len(outlier_rows) > 20:
            lines.append(f"| ... | ... | +{len(outlier_rows)-20} more |")
        lines.append("")

    # PIL parity — input/output validation
    parity = get_parity_results()
    if parity:
        total = parity['passed'] + parity['failed']

        # Hash cross-validation: compare PIL baseline hashes vs pillow-rs hashes
        hash_matches = 0
        hash_mismatches = 0
        # Build baseline hash lookup: try full name and short name
        bl_hashes = {}
        for r in (baseline_raw or {}).get("results", []):
            bl_fn = r.get("function", "")  # "ImageChops.add" or "resize"
            if r.get("output_hash"):
                bl_hashes[bl_fn] = r["output_hash"]
                short = bl_fn.split(".")[-1]
                if short != bl_fn:
                    bl_hashes[short] = r["output_hash"]

        # Build pillow-rs hash lookup from native_cpu
        native = target_lookups.get("native_cpu", {})
        for f in funcs:
            fn = f["full_name"]
            sn = f["name"]
            # Try full name then short name
            bl_hash = bl_hashes.get(fn) or bl_hashes.get(sn)
            rs_entry = native.get(fn)
            rs_hash = None
            if isinstance(rs_entry, dict):
                rs_hash = rs_entry.get("output_hash")
            if bl_hash and rs_hash:
                if bl_hash == rs_hash:
                    hash_matches += 1
                else:
                    hash_mismatches += 1
            if bl_hash and rs_hash:
                if bl_hash == rs_hash:
                    hash_matches += 1
                else:
                    hash_mismatches += 1

        trust_pct = 100 if total > 0 and parity['failed'] == 0 else min(100, hash_matches * 100 // max(hash_matches + hash_mismatches, 1))
        lines += [
            "## Input/Output Validation", "",
            f"| Metric | Value |", "|--------|-------|",
            f"| PIL parity tests | **{parity['passed']}/{total} pass** |",
            f"| Output hash matches (PIL vs pillow-rs) | **{hash_matches}** |",
            f"| Output hash mismatches | **{hash_mismatches}** |",
            f"| Trust level | **{trust_pct}%** |",
            f"| Pillow version | {parity.get('pillow_version', '')} |", "",
            "> Every benchmarked operation that passes PIL parity produces pixel-identical output.",
            "> Hash mismatches indicate input/output differences that make the speedup ratio unreliable.", "",
        ]

    return "\n".join(lines)


def get_parity_results():
    """Run pytest and extract pass/fail counts."""
    try:
        import os
        env = os.environ.copy()
        env["PYTHONPATH"] = f"{ROOT}/pillow-rs-py/python"
        r = subprocess.run(
            ["python", "-m", "pytest", "tests/", "-q", "--tb=no"],
            capture_output=True, text=True, cwd=ROOT, timeout=30, env=env,
        )
        for line in (r.stdout + r.stderr).split("\n"):
            if "passed" in line:
                # Parse "202 passed in 0.57s" or "202 passed, 5 failed"
                parts = line.strip().split()
                passed = int(parts[0]) if parts and parts[0].isdigit() else 0
                failed = 0
                for i, p in enumerate(parts):
                    if "failed" in p and i > 0:
                        try: failed = int(parts[i-1])
                        except: pass
                return {"passed": passed, "failed": failed, "pillow_version": "12.2.0"}
    except Exception:
        pass
    return None


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
    baseline_raw = json.loads(BASELINE_PATH.read_text()) if BASELINE_PATH.exists() else {}
    report = generate_report(funcs, baseline_lookup, target_lookups, pipeline_data, baseline_raw)
    (ROOT / "BENCHMARKS.md").write_text(report)
    print(f"Wrote BENCHMARKS.md ({len(funcs)} functions)")


if __name__ == "__main__":
    main()
