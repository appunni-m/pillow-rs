#!/usr/bin/env python3
"""Generate bench_spec.json from manifest.yaml — manifest is the source of truth.

Reads every function from manifest.yaml, assigns benchmark parameters
based on the function's signature, supported modes, and param_variants.
Outputs bench_spec.json for use by all benchmark targets.
"""

import json, sys, yaml
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent.parent

# ── Parameter mappings from manifest signatures ──
# For each function, determine: input image, operation, params

DEFAULT_INPUT = "ref_2k"
NONPERF_FUNCTIONS = {
    "getbands", "getbbox", "getcolors", "getdata", "getextrema", "getpixel",
    "getprojection", "histogram", "entropy", "getchannel", "getexif", "getim",
    "getpalette", "getxmp", "get_child_images", "get_flattened_data",
    "load", "seek", "tell", "verify", "show", "close", "copy",
    "getcolor", "getrgb", "tobytes", "to_bytes",
}
GPU_APPLICABLE = {
    "resize", "thumbnail", "convert", "filter", "point", "quantize", "reduce",
    "autocontrast", "equalize", "invert", "posterize", "solarize", "colorize",
    "add", "add_modulo", "blend", "darker", "difference", "hard_light",
    "lighter", "logical_and", "logical_or", "logical_xor", "multiply",
    "overlay", "screen", "soft_light", "subtract", "subtract_modulo",
    "Brightness", "Color", "Contrast", "Sharpness",
    "crop", "rotate", "transpose", "transform", "paste",
}

# Param defaults based on function name — derived from Pillow baseline
FUNC_PARAMS = {
    "resize": {"size": [800, 600], "filter": "LANCZOS"},
    "crop": {"box": [100, 100, 1100, 900]},
    "rotate": {"angle": 45, "resample": "BICUBIC", "expand": True},
    "transpose": {"method": "FLIP_LEFT_RIGHT"},
    "thumbnail": {"size": [256, 256], "filter": "LANCZOS"},
    "convert": {"mode": "L"},
    "paste": {"src_w": 512, "src_h": 512, "src_color": [255,0,0], "box": [100,100]},
    "filter": {"name": "BLUR"},
    "quantize": {"colors": 16},
    "reduce": {"factor": 2},
    "point": {"lut": "invert"},
    "effect_spread": {"distance": 3},
    "posterize": {"bits": 3},
    "solarize": {"threshold": 128},
    "enhance_brightness": {"factor": 1.5},
    "enhance_contrast": {"factor": 1.5},
    "enhance_color": {"factor": 1.5},
    "enhance_sharpness": {"factor": 2.0},
    "getchannel": {"channel": 0},
    "getcolors": {"maxcolors": 256},
    "getpixel": {"xy": [50, 50]},
    "getcolor": {"color": "red", "mode": "RGB"},
    "getrgb": {"color": "red"},
    "paste_mask": {"src_w": 512, "src_h": 512, "src_color": [255,0,0], "box": [100,100], "mask": True},
    "paste_color": {"src_color": [255,0,0], "box": [100,100,900,700]},
}

# Pipeline chain — the 5-op benchmark everyone measures
PIPELINE_CHAIN = [
    {"op": "resize", "params": {"size": [800,600], "filter": "LANCZOS"}},
    {"op": "crop", "params": {"box": [100,100,500,500]}},
    {"op": "convert", "params": {"mode": "L"}},
    {"op": "rotate", "params": {"angle": 90}},
    {"op": "filter", "params": {"name": "BLUR"}},
]


def generate_spec():
    manifest = yaml.safe_load(open(ROOT / "manifest.yaml"))
    ops = []
    seen = set()

    def add_op(name, op_type, params, category, input_img=DEFAULT_INPUT, gpu=False):
        if name in seen:
            return
        seen.add(name)
        ops.append({
            "name": name,
            "input": input_img,
            "op": op_type,
            "params": params,
            "category": category,
            "gpu_applicable": gpu or op_type in GPU_APPLICABLE or name.split(".")[-1] in GPU_APPLICABLE,
            "wasm_available": op_type not in ("effect_spread", "getexif", "getim", "getxmp", "get_child_images", "get_flattened_data", "putpalette"),
            "browser_available": op_type not in ("show", "effect_spread"),
        })

    # Walk all modules
    for mod_name, mod_def in manifest.get("modules", {}).items():
        # Class methods
        for key in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(key, []):
                if not isinstance(item, dict):
                    continue
                name = item.get("name", "")
                full = f"{mod_name}.{name}"
                status = item.get("status", "stub")
                if status == "stub":
                    continue

                params = FUNC_PARAMS.get(name, {})
                op_type = name
                cat = "nonperf" if name in NONPERF_FUNCTIONS else "perf"
                inp = DEFAULT_INPUT
                if name == "effect_spread":
                    inp = "synthetic_512"
                elif name in ("putalpha", "alpha_composite", "apply_transparency"):
                    inp = "ref_1k"
                elif name in ("autocontrast", "equalize"):
                    inp = "ref_gray"

                gpu = name in GPU_APPLICABLE
                add_op(full, op_type, params, cat, inp, gpu)

        # Classes (ImageFilter, ImageEnhance, ImageFont, etc.)
        for cls in mod_def.get("classes", []):
            if not isinstance(cls, dict):
                continue
            cls_name = cls.get("name", "")
            cls_status = cls.get("status", "stub")
            if cls_status == "stub":
                continue

            # Class itself (e.g., ImageFilter.BLUR)
            cls_full = f"{mod_name}.{cls_name}"
            cls_params = FUNC_PARAMS.get(cls_name, {})
            if not cls_params:
                cls_params = {"name": cls_name}
            cls_cat = "nonperf" if cls_name in NONPERF_FUNCTIONS else "perf"
            add_op(cls_full, cls_name.lower(), cls_params, cls_cat, DEFAULT_INPUT, cls_name in GPU_APPLICABLE)

            # Class methods (e.g., FreeTypeFont.getbbox)
            for m in cls.get("methods", []):
                m_name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                m_full = f"{mod_name}.{cls_name}.{m_name}"
                m_params = FUNC_PARAMS.get(m_name, {})
                m_cat = "nonperf"
                add_op(m_full, m_name, m_params, m_cat, DEFAULT_INPUT, False)

    # Add pipeline
    add_op("pipeline_20_st", "pipeline", {"chain": PIPELINE_CHAIN}, "perf", DEFAULT_INPUT, False)

    # Write spec
    spec = {
        "_description": "Generated from manifest.yaml — single source of truth for ALL benchmarks",
        "_targets": ["python_cpu", "wasm_cpu", "wasm_gpu", "browser_cpu", "browser_gpu", "native_gpu"],
        "inputs": {
            "ref_2k": {"file": "scripts/bench_reference_images/ref_2k.jpg", "w": 2048, "h": 1536, "mode": "RGB"},
            "ref_1k": {"file": "scripts/bench_reference_images/ref_1k.png", "w": 1024, "h": 1024, "mode": "RGBA"},
            "ref_gray": {"file": "scripts/bench_reference_images/ref_grayscale.png", "w": 1024, "h": 1024, "mode": "L"},
            "synthetic_512": {"w": 512, "h": 512, "mode": "RGB", "color": [128, 128, 128, 255]},
        },
        "operations": ops,
    }

    out_path = ROOT / "scripts" / "bench_spec.json"
    with open(out_path, "w") as f:
        json.dump(spec, f, indent=2)

    # Stats
    perf = sum(1 for o in ops if o["category"] == "perf")
    nonperf = sum(1 for o in ops if o["category"] == "nonperf")
    gpu = sum(1 for o in ops if o["gpu_applicable"])
    wasm = sum(1 for o in ops if o["wasm_available"])
    print(f"Generated {out_path}")
    print(f"  {len(ops)} operations ({perf} perf, {nonperf} non-perf)")
    print(f"  {gpu} GPU-applicable, {wasm} WASM-available")
    return spec


if __name__ == "__main__":
    generate_spec()
