#!/usr/bin/env python3
"""Operation registry — fixture metadata derived from manifest.yaml.

MODE LISTS are read directly from manifest.yaml (the canonical source).
Call style metadata (type, params, method overrides) is derived from naming
conventions via _default_meta(), not from a hardcoded dict.

Format:
    "Module.function": {
        "type": "image" | "dual" | "filter" | "draw" | "value" | "module" | "enhance",
        "method": "resize",           # PIL method/function to call
        "params": {"size": [50, 50]}, # Default parameters
        "modes": [...],               # From manifest.yaml supported_modes
        "prep": "...",                # Optional input prep
    }
"""

import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(ROOT))

import yaml


# ── Load manifest ────────────────────────────────────────────────────────────────

def _load_manifest():
    with open(ROOT / "manifest.yaml") as f:
        return yaml.safe_load(f)


def _normalize_modes(modes):
    """Ensure modes is a flat list of strings."""
    result = []
    for m in modes:
        if isinstance(m, list):
            result.extend([str(x) for x in m])
        else:
            result.append(str(m))
    return result


# ── Convention-based defaults ────────────────────────────────────────────────────

def _default_meta(module_name, op_name):
    """Derive fixture metadata from naming conventions."""
    # Image instance methods
    if module_name == "Image":
        return {"type": "value" if _is_value_op(op_name) else "image",
                "method": op_name, "params": {}}
    # Image properties
    if module_name == "ImageProperties":
        return {"type": "value", "property": op_name, "params": {}}

    # ImageFilter built-in constants (no params)
    if module_name == "ImageFilter":
        return {"type": "filter", "name": op_name, "params": {}}

    # ImageChops dual-input ops
    if module_name == "ImageChops":
        single_image_ops = {"invert", "duplicate", "constant", "offset"}
        if op_name in single_image_ops:
            return {"type": "image", "method": op_name, "params": {}}
        return {"type": "dual", "params": {}}

    # ImageOps instance methods
    if module_name == "ImageOps":
        return {"type": "image", "method": op_name, "params": {}}

    # ImageEnhance
    if module_name == "ImageEnhance":
        return {"type": "enhance", "name": op_name, "params": {"factor": 1.5}}

    # ImageDraw
    if module_name == "ImageDraw":
        return {"type": "draw", "draw": op_name, "params": {}}

    # ImageModule
    if module_name == "ImageModule":
        return {"type": "module", "function": f"Image.{op_name}", "params": {}}

    # Value-returning modules
    for mod_name in ["ImageColor", "ImagePalette", "ImageFont", "ImageStat", "ImageSequence"]:
        if module_name == mod_name:
            return {"type": "value", "function": f"{mod_name}.{op_name}", "params": {}}

    return None


def _is_value_op(name):
    """Return True if an Image method returns a non-image value."""
    return name in {
        "tobytes", "split", "getbands", "getbbox", "getextrema", "histogram",
        "getpixel", "getcolors", "getdata", "getprojection", "entropy",
        "load", "verify", "seek", "tell", "tobitmap", "has_transparency_data",
        "getexif", "getim", "getpalette", "getxmp", "get_flattened_data",
        "get_child_images", "apply_transparency",
        "close", "save",
    }


# ── Build registry ──────────────────────────────────────────────────────────────

def _collect_manifest_ops(manifest):
    """Walk the manifest and yield (module_name, op_name, modes) for every implemented entry."""
    modules = manifest.get("modules", {})
    for mod_name, mod_data in modules.items():
        # Functions / class_methods / methods
        for section in ["class_methods", "methods", "functions"]:
            for entry in mod_data.get(section, []):
                name = entry.get("name")
                status = entry.get("status", "")
                modes = entry.get("supported_modes", [])
                if name and status == "implemented":
                    yield mod_name, name, _normalize_modes(modes)

        # Classes — the class itself AND its methods
        for cls in mod_data.get("classes", []):
            cls_name = cls.get("name")
            cls_status = cls.get("status", "")
            cls_modes = cls.get("supported_modes", [])
            if cls_name and cls_status == "implemented":
                yield mod_name, cls_name, _normalize_modes(cls_modes)
            for entry in cls.get("methods", []):
                name = entry.get("name")
                status = entry.get("status", "")
                modes = entry.get("supported_modes", [])
                if not modes:
                    modes = cls_modes
                # Include methods even if ignored (they need registry entries)
                if name:
                    yield mod_name, name, _normalize_modes(modes)


def build_registry():
    """Build REGISTRY dict from manifest.yaml.

    Modes always come from manifest.yaml. Call style metadata
    is no longer needed — that's handled by engine.get_call_style().
    """
    manifest = _load_manifest()
    registry = {}

    for mod_name, op_name, modes in _collect_manifest_ops(manifest):
        op_full_name = f"{mod_name}.{op_name}"
        meta = _default_meta(mod_name, op_name)
        if meta is None:
            continue
        if not modes:
            modes = ["L", "RGB"]
        meta["modes"] = modes
        registry[op_full_name] = meta

    return registry


# ── Public API ───────────────────────────────────────────────────────────────────

REGISTRY = build_registry()
