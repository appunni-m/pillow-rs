#!/usr/bin/env python3
"""Parse manifest.yaml into a flat function list for benchmarking."""
import json
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent.parent

GPU_APPLICABLE_OPS = {
    # Pixel-parallel -- GPU accelerates
    "resize", "thumbnail", "convert", "filter", "point", "quantize", "reduce",
    "autocontrast", "equalize", "invert", "posterize", "solarize", "colorize",
    "add", "add_modulo", "blend", "darker", "difference", "hard_light",
    "lighter", "logical_and", "logical_or", "logical_xor", "multiply",
    "overlay", "screen", "soft_light", "subtract", "subtract_modulo",
    # Enhance (factor-based ops benefit from GPU parallelization)
    "Brightness", "Color", "Contrast", "Sharpness",
    # Geometry -- partial GPU
    "crop", "rotate", "transpose", "transform", "paste",
}

PRIORITY_OPS = [
    "open_save", "resize", "crop", "rotate", "transpose", "thumbnail",
    "to_bytes", "new", "paste", "paste_mask", "paste_color", "pipeline",
]


def load_manifest(path=None):
    if path is None:
        path = ROOT / "manifest.yaml"
    with open(path) as f:
        return yaml.safe_load(f)


def extract_functions(manifest):
    """Return list of {module, name, full_name, status, gpu_applicable}."""
    funcs = []
    for mod, mod_def in manifest.get("modules", {}).items():
        # Class methods and methods
        for key in ("class_methods", "methods", "functions"):
            for item in mod_def.get(key, []):
                if not isinstance(item, dict):
                    continue
                name = item.get("name", "")
                status = item.get("status", "stub")
                if status == "stub":
                    continue
                full = f"{mod}.{name}"
                gpu = name in GPU_APPLICABLE_OPS
                funcs.append({
                    "module": mod,
                    "name": name,
                    "full_name": full,
                    "status": status,
                    "gpu_applicable": gpu,
                })
        # Handle classes (filters, enhancers, fonts)
        for cls in mod_def.get("classes", []):
            if not isinstance(cls, dict):
                continue
            cls_name = cls.get("name", "")
            cls_status = cls.get("status", "stub")
            if cls_status == "stub":
                continue
            # Class itself as a function
            gpu = cls_name in GPU_APPLICABLE_OPS
            funcs.append({
                "module": mod,
                "name": cls_name,
                "full_name": f"{mod}.{cls_name}",
                "status": cls_status,
                "gpu_applicable": gpu,
            })
            # Class methods
            for m in cls.get("methods", []):
                m_name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                funcs.append({
                    "module": mod,
                    "name": m_name,
                    "full_name": f"{mod}.{cls_name}.{m_name}",
                    "status": cls_status,
                    "gpu_applicable": m_name in GPU_APPLICABLE_OPS,
                })
    return funcs


def sort_by_priority(funcs):
    """Sort: priority ops first (puhu order), then alpha by module.name."""
    priority_map = {name: i for i, name in enumerate(PRIORITY_OPS)}

    def sort_key(f):
        p = priority_map.get(f["name"], 999)
        return (p, f["module"], f["name"])

    return sorted(funcs, key=sort_key)


def main():
    manifest = load_manifest()
    funcs = extract_functions(manifest)
    funcs = sort_by_priority(funcs)
    if "--json" in sys.argv:
        print(json.dumps(funcs, indent=2))
    else:
        for f in funcs:
            gpu = "GPU" if f["gpu_applicable"] else "   "
            print(f"[{gpu}] {f['full_name']}")
        print(f"\nTotal: {len(funcs)} functions")


if __name__ == "__main__":
    main()
