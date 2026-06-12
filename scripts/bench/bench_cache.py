#!/usr/bin/env python3
"""Incremental cache: compute cache keys, detect which functions need re-benchmarking.

Usage:
  python scripts/bench/bench_cache.py --check
  python scripts/bench/bench_cache.py --update <target> <func> <mean_ms> <std_ms>
  python scripts/bench/bench_cache.py --init
"""
import hashlib
import json
import sys
from pathlib import Path

from bench_manifest import extract_functions, sort_by_priority, load_manifest

ROOT = Path(__file__).resolve().parent.parent.parent
CACHE_DIR = ROOT / "target" / "benchmarks"
CACHE_FILE = CACHE_DIR / "cache.json"

SHARED_FILES = [
    "pillow-rs-core/src/image.rs",
    "pillow-rs-core/src/lazy.rs",
    "pillow-rs-core/src/error.rs",
    "pillow-rs-core/src/color.rs",
    "pillow-rs-core/Cargo.toml",
]

# Map function name -> source file for cache key computation
FUNC_SOURCE_MAP = {
    "resize": "pillow-rs-core/src/ops/resize.rs",
    "crop": "pillow-rs-core/src/ops/crop.rs",
    "rotate": "pillow-rs-core/src/ops/rotate.rs",
    "convert": "pillow-rs-core/src/ops/convert.rs",
    "transpose": "pillow-rs-core/src/ops/transpose.rs",
    "paste": "pillow-rs-core/src/ops/paste.rs",
    "filter": "pillow-rs-core/src/ops/filter.rs",
    "split": "pillow-rs-core/src/ops/split.rs",
    "quantize": "pillow-rs-core/src/ops/quantize.rs",
    "reduce": "pillow-rs-core/src/ops/transform.rs",
    "thumbnail": "pillow-rs-core/src/ops/resize.rs",
    "open": "pillow-rs-core/src/format.rs",
    "save": "pillow-rs-core/src/format.rs",
    "to_bytes": "pillow-rs-core/src/image.rs",
    "new": "pillow-rs-core/src/image.rs",
    "autocontrast": "pillow-rs-core/src/ops/imageops.rs",
    "equalize": "pillow-rs-core/src/ops/imageops.rs",
    "invert": "pillow-rs-core/src/ops/chops.rs",
    "posterize": "pillow-rs-core/src/ops/imageops.rs",
    "solarize": "pillow-rs-core/src/ops/imageops.rs",
    "grayscale": "pillow-rs-core/src/ops/imageops.rs",
    "flip": "pillow-rs-core/src/ops/imageops.rs",
    "mirror": "pillow-rs-core/src/ops/imageops.rs",
    "colorize": "pillow-rs-core/src/ops/imageops.rs",
    "contain": "pillow-rs-core/src/ops/imageops.rs",
    "cover": "pillow-rs-core/src/ops/imageops.rs",
    "fit": "pillow-rs-core/src/ops/imageops.rs",
    "pad": "pillow-rs-core/src/ops/imageops.rs",
    "scale": "pillow-rs-core/src/ops/imageops.rs",
    "expand": "pillow-rs-core/src/ops/imageops.rs",
    "point": "pillow-rs-core/src/ops/imageops.rs",
    "putalpha": "pillow-rs-core/src/ops/imageops.rs",
    "putpixel": "pillow-rs-core/src/ops/imageops.rs",
    "getpixel": "pillow-rs-core/src/ops/imageops.rs",
    "getbbox": "pillow-rs-core/src/ops/imageops.rs",
    "getcolors": "pillow-rs-core/src/ops/imageops.rs",
    "getextrema": "pillow-rs-core/src/ops/imageops.rs",
    "histogram": "pillow-rs-core/src/ops/analysis.rs",
    "entropy": "pillow-rs-core/src/ops/analysis.rs",
    "getprojection": "pillow-rs-core/src/ops/analysis.rs",
    "effect_spread": "pillow-rs-core/src/ops/imageops.rs",
    "alpha_composite": "pillow-rs-core/src/ops/paste.rs",
    "add": "pillow-rs-core/src/ops/chops.rs",
    "subtract": "pillow-rs-core/src/ops/chops.rs",
    "multiply": "pillow-rs-core/src/ops/chops.rs",
    "darker": "pillow-rs-core/src/ops/chops.rs",
    "lighter": "pillow-rs-core/src/ops/chops.rs",
    "difference": "pillow-rs-core/src/ops/chops.rs",
    "screen": "pillow-rs-core/src/ops/chops.rs",
    "overlay": "pillow-rs-core/src/ops/chops.rs",
    "hard_light": "pillow-rs-core/src/ops/chops.rs",
    "soft_light": "pillow-rs-core/src/ops/chops.rs",
    "add_modulo": "pillow-rs-core/src/ops/chops.rs",
    "subtract_modulo": "pillow-rs-core/src/ops/chops.rs",
    "logical_and": "pillow-rs-core/src/ops/chops.rs",
    "logical_or": "pillow-rs-core/src/ops/chops.rs",
    "logical_xor": "pillow-rs-core/src/ops/chops.rs",
    "constant": "pillow-rs-core/src/ops/chops.rs",
    "duplicate": "pillow-rs-core/src/ops/chops.rs",
    "offset": "pillow-rs-core/src/ops/chops.rs",
    "Brightness": "pillow-rs-core/src/ops/enhance.rs",
    "Color": "pillow-rs-core/src/ops/enhance.rs",
    "Contrast": "pillow-rs-core/src/ops/enhance.rs",
    "Sharpness": "pillow-rs-core/src/ops/enhance.rs",
}


def file_hash(path: Path) -> str:
    """SHA-256 of file contents, or empty string if missing."""
    full = ROOT / path
    if not full.exists():
        return ""
    return hashlib.sha256(full.read_bytes()).hexdigest()


def compute_cache_key(func_name: str) -> str:
    """Compute cache key for a function.

    Combines hashes of:
    - Shared files (image.rs, lazy.rs, error.rs, color.rs, Cargo.toml)
    - Function-specific source file (from FUNC_SOURCE_MAP)
    - Reference image hash
    """
    parts = []
    for sf in SHARED_FILES:
        parts.append(file_hash(Path(sf)))
    src_file = FUNC_SOURCE_MAP.get(func_name)
    if src_file:
        parts.append(file_hash(Path(src_file)))
    else:
        # Try candidates for unknown functions
        for candidate in [
            "pillow-rs-core/src/ops/filter.rs",
            "pillow-rs-core/src/ops/chops.rs",
            "pillow-rs-core/src/ops/enhance.rs",
            "pillow-rs-core/src/ops/imageops.rs",
            "pillow-rs-core/src/ops/convert.rs",
            "pillow-rs-core/src/ops/mod.rs",
        ]:
            h = file_hash(Path(candidate))
            if h:
                parts.append(h)
                break
    ref_img = ROOT / "scripts" / "bench_reference_images" / "ref_2k.jpg"
    if ref_img.exists():
        parts.append(hashlib.sha256(ref_img.read_bytes()).hexdigest())
    return hashlib.sha256("".join(parts).encode()).hexdigest()


def load_cache() -> dict:
    """Load cache from disk, return empty dict if not present."""
    if CACHE_FILE.exists():
        with open(CACHE_FILE) as f:
            return json.load(f)
    return {}


def save_cache(cache: dict):
    """Save cache to disk, creating parent directories."""
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    with open(CACHE_FILE, "w") as f:
        json.dump(cache, f, indent=2)


def check_stale() -> list:
    """Return list of function names whose cache is stale."""
    cache = load_cache()
    manifest = load_manifest()
    funcs = extract_functions(manifest)
    stale = []
    for func in funcs:
        name = func["name"]
        new_key = compute_cache_key(name)
        cached = cache.get(name, {})
        if cached.get("key") != new_key:
            stale.append(name)
    return stale


def update_cache(func_name: str, target: str, mean_ms: float, std_ms: float):
    """Update cache entry after benchmarking a function on a target."""
    cache = load_cache()
    key = compute_cache_key(func_name)
    if func_name not in cache:
        cache[func_name] = {"key": key, "targets": {}}
    cache[func_name]["key"] = key
    cache[func_name]["targets"][target] = {
        "mean_ms": round(mean_ms, 4),
        "std_ms": round(std_ms, 4),
    }
    save_cache(cache)


if __name__ == "__main__":
    if "--check" in sys.argv:
        stale = check_stale()
        if stale:
            print(f"STALE: {len(stale)} functions need re-benchmarking:")
            for s in stale:
                print(f"  - {s}")
        else:
            print("FRESH: all functions up-to-date")
    elif "--update" in sys.argv:
        target = sys.argv[2]
        func = sys.argv[3]
        mean = float(sys.argv[4])
        std = float(sys.argv[5])
        update_cache(func, target, mean, std)
        print(f"Updated cache: {func}/{target} = {mean}ms")
    elif "--init" in sys.argv:
        save_cache({})
        print("Initialized empty cache")
    else:
        print("Usage: bench_cache.py --check | --update <target> <func> <mean> <std> | --init")
