#!/usr/bin/env python3
"""Operation registry — single source of truth for fixture generation and testing.

MODE LISTS are read directly from manifest.yaml (the canonical source).
Only fixture execution metadata (type, params, method overrides) lives here.

Both generate_fixtures.py and test_fixture_parity.py import this module.

Format (auto-derived from manifest + FIXTURE_META overrides):
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


def _find_manifest_modes(manifest, module_name, op_name):
    """Extract supported_modes from manifest for a given module.op combination."""
    modules = manifest.get("modules", {})
    mod = modules.get(module_name, {})
    if not mod:
        return []

    # Check class_methods, methods, functions, classes, properties
    for section in ["class_methods", "methods", "functions"]:
        for entry in mod.get(section, []):
            if entry.get("name") == op_name:
                modes = entry.get("supported_modes", [])
                return _normalize_modes(modes)

    # Classes — check class itself and its methods
    for cls in mod.get("classes", []):
        if cls.get("name") == op_name:
            modes = cls.get("supported_modes", [])
            return _normalize_modes(modes)
        # Check nested methods
        for entry in cls.get("methods", []):
            if entry.get("name") == op_name:
                modes = entry.get("supported_modes", [])
                if not modes:
                    modes = cls.get("supported_modes", [])
                return _normalize_modes(modes)

    # Properties
    for prop in mod.get("properties", []):
        if prop.get("name") == op_name:
            modes = prop.get("modes", [])
            return _normalize_modes(modes)

    # YAML anchors may resolve as lists directly
    if isinstance(modes := [], list):
        return _normalize_modes(modes)

    return []


def _normalize_modes(modes):
    """Ensure modes is a flat list of strings."""
    result = []
    for m in modes:
        if isinstance(m, list):
            result.extend([str(x) for x in m])
        else:
            result.append(str(m))
    return result


# ── Fixture execution metadata — only what CANNOT be inferred from conventions ───

FIXTURE_META = {
    # ── Image methods with non-empty params ──
    "Image.resize":      {"type": "image", "method": "resize",      "params": {"size": [50, 50]}},
    "Image.crop":         {"type": "image", "method": "crop",         "params": {"box": [25, 25, 75, 75]}},
    "Image.rotate":       {"type": "image", "method": "rotate",       "params": {"angle": 90}},
    "Image.transpose":    {"type": "image", "method": "transpose",    "params": {"method": 0}},
    "Image.thumbnail":    {"type": "image", "method": "thumbnail",    "params": {"size": [50, 50]}},
    "Image.quantize":     {"type": "image", "method": "quantize",     "params": {"colors": 16}},
    "Image.filter":       {"type": "image", "method": "filter",       "params": {"filter": "BLUR"}},
    "Image.convert":      {"type": "image", "method": "convert",      "params": {"mode": "__CONVERT_TO__"}},
    "Image.getpixel":     {"type": "value", "method": "getpixel",     "params": {"xy": [50, 50]}},
    "Image.getcolors":    {"type": "value", "method": "getcolors",    "params": {"maxcolors": 256}},
    "Image.getdata":      {"type": "value", "method": "getdata",      "params": {"band": None}},
    "Image.getchannel":   {"type": "image", "method": "getchannel",   "params": {"channel": 0}},
    "Image.seek":         {"type": "value", "method": "seek",         "params": {"frame": 0}},
    "Image.draft":        {"type": "image", "method": "draft",        "params": {"mode": "RGB", "size": [50, 50]}},
    "Image.putalpha":     {"type": "image", "method": "putalpha",     "params": {"alpha": 128}},
    "Image.putpixel":     {"type": "image", "method": "putpixel",     "params": {"xy": [50, 50], "value": [255, 255, 255, 255]}},
    "Image.putdata":      {"type": "image", "method": "putdata",      "params": {"data": [128]}},
    "Image.reduce":       {"type": "image", "method": "reduce",       "params": {"factor": 2}},
    "Image.effect_spread":{"type": "image", "method": "effect_spread","params": {"distance": 2}},
    "Image.transform":    {"type": "image", "method": "transform",    "params": {"size": (50, 50), "method": 0, "data": [1, 0, 0, 0, 1, 0]}},
    "Image.remap_palette":{"type": "image", "method": "remap_palette","params": {"dest_map": [0, 1]}},
    "Image.point":        {"type": "image", "method": "point",        "params": {"lut": list(range(256))}},
    "Image.paste":        {"type": "dual",  "method": "paste",        "params": {"box": [0, 0]}},
    "Image.putpalette":   {"type": "image", "method": "putpalette",   "params": {"data": list(range(48))}},
    "Image.has_transparency_data": {"type": "value", "method": "has_transparency_data", "params": {}},
    "Image.save":             {"type": "value", "method": "save",             "params": {"format": "PNG"}},
    "Image.close":            {"type": "value", "method": "close",            "params": {}},
    "Image.toqimage":         {"type": "value", "method": "toqimage",         "params": {}},
    "Image.toqpixmap":        {"type": "value", "method": "toqpixmap",        "params": {}},

    # ── ImageChops ──
    "ImageChops.constant": {"type": "image", "method": "constant", "params": {"value": 128}},
    "ImageChops.offset":   {"type": "image", "method": "offset",   "params": {"xoffset": 5, "yoffset": 5}},
    "ImageChops.invert":   {"type": "image", "method": "invert",   "params": {}},
    "ImageChops.duplicate":{"type": "image", "method": "copy",     "params": {}},
    "ImageChops.logical_and": {"type": "dual", "prep": "convert('1', dither='NONE')"},
    "ImageChops.logical_or":  {"type": "dual", "prep": "convert('1', dither='NONE')"},
    "ImageChops.logical_xor": {"type": "dual", "prep": "convert('1', dither='NONE')"},

    # ── ImageOps ──
    "ImageOps.autocontrast": {"type": "image", "method": "autocontrast", "params": {"cutoff": 0}},
    "ImageOps.posterize":    {"type": "image", "method": "posterize",    "params": {"bits": 4}},
    "ImageOps.solarize":     {"type": "image", "method": "solarize",     "params": {"threshold": 128}},
    "ImageOps.grayscale":    {"type": "image", "method": "convert",      "params": {}},
    "ImageOps.colorize":     {"type": "image", "method": "colorize",     "params": {"black": "black", "white": "white"}},
    "ImageOps.expand":       {"type": "image", "method": "expand",       "params": {"border": 5}},
    "ImageOps.crop":         {"type": "image", "method": "crop",         "params": {"border": 5}},
    "ImageOps.scale":        {"type": "image", "method": "scale",        "params": {"factor": 0.5}},
    "ImageOps.contain":      {"type": "image", "method": "contain",      "params": {"size": [25, 25]}},
    "ImageOps.cover":        {"type": "image", "method": "cover",        "params": {"size": [25, 25]}},
    "ImageOps.fit":          {"type": "image", "method": "fit",          "params": {"size": [25, 25]}},
    "ImageOps.pad":          {"type": "image", "method": "pad",          "params": {"size": [25, 25]}},
    "ImageOps.exif_transpose": {"type": "image", "method": "exif_transpose", "params": {}},
    "ImageOps.deform":         {"type": "image", "method": "deform",         "params": {"deformer": "__SIMPLE__"}},

    # ── ImageEnhance ──
    "ImageEnhance.Brightness": {"type": "enhance", "name": "Brightness", "params": {"factor": 1.5}},
    "ImageEnhance.Color":      {"type": "enhance", "name": "Color",      "params": {"factor": 1.5}},
    "ImageEnhance.Contrast":   {"type": "enhance", "name": "Contrast",   "params": {"factor": 1.5}},
    "ImageEnhance.Sharpness":  {"type": "enhance", "name": "Sharpness",  "params": {"factor": 1.5}},

    # ── ImageFilter with params ──
    "ImageFilter.BoxBlur":      {"type": "filter", "name": "BoxBlur",      "params": {"radius": 2}},
    "ImageFilter.GaussianBlur": {"type": "filter", "name": "GaussianBlur", "params": {"radius": 2}},
    "ImageFilter.UnsharpMask":  {"type": "filter", "name": "UnsharpMask",  "params": {"radius": 2, "percent": 150, "threshold": 3}},
    "ImageFilter.MaxFilter":    {"type": "filter", "name": "MaxFilter",    "params": {"size": 3}},
    "ImageFilter.MinFilter":    {"type": "filter", "name": "MinFilter",    "params": {"size": 3}},
    "ImageFilter.MedianFilter": {"type": "filter", "name": "MedianFilter", "params": {"size": 3}},
    "ImageFilter.ModeFilter":   {"type": "filter", "name": "ModeFilter",   "params": {"size": 3}},
    "ImageFilter.RankFilter":   {"type": "filter", "name": "RankFilter",   "params": {"size": 3, "rank": 2}},
    "ImageFilter.Kernel":       {"type": "filter", "name": "Kernel",       "params": {"size": [3, 3], "kernel": [1,1,1,1,1,1,1,1,1], "scale": 9, "offset": 0}},
    "ImageFilter.Color3DLUT":   {"type": "filter", "name": "Color3DLUT",   "params": {"size": 17, "table": "__IDENTITY_LUT__", "channels": 3}},

    # ── ImageModule ──
    "ImageModule.new":            {"type": "module", "function": "Image.new",            "params": {"mode": "RGB", "size": [100, 100], "color": 0}},
    "ImageModule.open":           {"type": "module", "function": "Image.open",           "params": {}},
    "ImageModule.frombytes":      {"type": "module", "function": "Image.frombytes",      "params": {}},
    "ImageModule.blend":          {"type": "dual",   "function": "Image.blend",          "params": {"alpha": 0.5}},
    "ImageModule.composite":      {"type": "dual",   "function": "Image.composite",      "params": {}},
    "ImageModule.merge":          {"type": "module", "function": "Image.merge",          "params": {}},
    "ImageModule.eval":           {"type": "module", "function": "Image.eval",           "params": {}},
    "ImageModule.alpha_composite":{"type": "image",  "method": "alpha_composite",        "params": {"fg_alpha": 128}},
    "ImageModule.effect_noise":   {"type": "module", "function": "Image.effect_noise",   "params": {"size": [100, 100], "sigma": 10.0}},

    # ── ImageColor ──
    "ImageColor.getcolor": {"type": "value", "function": "ImageColor.getcolor", "params": {"color": "red", "mode": "RGB"}},
    "ImageColor.getrgb":   {"type": "value", "function": "ImageColor.getrgb",   "params": {"color": "red"}},

    # ── ImagePalette ──
    "ImagePalette.getcolor": {"type": "value", "function": "ImagePalette.getcolor", "params": {"color": [255, 0, 0]}},

    # ── ImageStat ──
    "ImageStat.Stat": {"type": "value", "function": "ImageStat.Stat", "params": {}},

    # ── ImageSequence ──
    "ImageSequence.Iterator":   {"type": "value", "function": "ImageSequence.Iterator",   "params": {}},
    "ImageSequence.all_frames": {"type": "value", "function": "ImageSequence.all_frames", "params": {}},

    # ── ImageDraw ──
    "ImageDraw.line":              {"type": "draw", "draw": "line",              "params": {"xy": [[10, 10], [40, 40]], "fill": 200}},
    "ImageDraw.circle":            {"type": "draw", "draw": "circle",            "params": {"xy": [25, 25], "radius": 15, "fill": 200}},
    "ImageDraw.rectangle":         {"type": "draw", "draw": "rectangle",         "params": {"xy": [10, 10, 40, 40], "outline": 200}},
    "ImageDraw.ellipse":           {"type": "draw", "draw": "ellipse",           "params": {"xy": [10, 10, 40, 40], "outline": 200}},
    "ImageDraw.polygon":           {"type": "draw", "draw": "polygon",           "params": {"xy": [[10, 10], [40, 10], [25, 40]], "outline": 200}},
    "ImageDraw.arc":               {"type": "draw", "draw": "arc",               "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200}},
    "ImageDraw.chord":             {"type": "draw", "draw": "chord",             "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200}},
    "ImageDraw.pieslice":          {"type": "draw", "draw": "pieslice",          "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200}},
    "ImageDraw.point":             {"type": "draw", "draw": "point",             "params": {"xy": [25, 25], "fill": 200}},
    "ImageDraw.regular_polygon":   {"type": "draw", "draw": "regular_polygon",   "params": {"bounding_circle": [25, 25, 15], "n_sides": 5, "fill": 200}},
    "ImageDraw.rounded_rectangle": {"type": "draw", "draw": "rounded_rectangle", "params": {"xy": [10, 10, 40, 40], "radius": 5, "outline": 200}},
    "ImageDraw.bitmap":            {"type": "draw", "draw": "bitmap",            "params": {"xy": [5, 5], "fill": 200}},
    "ImageDraw.text":              {"type": "draw", "draw": "text",              "params": {"xy": [5, 5], "text": "Hello", "fill": 200}},
    "ImageDraw.multiline_text":    {"type": "draw", "draw": "multiline_text",    "params": {"xy": [5, 5], "text": "Hello", "fill": 200}},
    "ImageDraw.textlength":        {"type": "draw", "draw": "textlength",        "params": {"text": "Hello"}},
    "ImageDraw.getfont":           {"type": "draw", "draw": "getfont",           "params": {}},
}


# ── Convention-based defaults ────────────────────────────────────────────────────

def _default_meta(module_name, op_name):
    """Derive fixture metadata from naming conventions when not in FIXTURE_META."""
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

def _module_name_for_op(op_full_name):
    """Map 'ImageOps.invert' → ('ImageOps', 'invert')."""
    # Handle compound names like 'ImageFilter.BLUR' or 'ImageSequence.Iterator'
    for prefix in ["ImageFilter", "ImageEnhance", "ImageChops", "ImageOps",
                   "ImageDraw", "ImageColor", "ImagePalette", "ImageFont",
                   "ImageStat", "ImageSequence", "ImageModule"]:
        if op_full_name.startswith(prefix + "."):
            return prefix, op_full_name[len(prefix) + 1:]
    # Default: Image.<method> or Image.property
    if op_full_name.startswith("Image."):
        return "Image", op_full_name[6:]
    return "Image", op_full_name


def _collect_manifest_ops(manifest):
    """Walk the manifest and yield (module_name, op_name, modes, status, targets) for every implemented entry."""
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
    """Build REGISTRY dict from manifest.yaml + FIXTURE_META overrides.

    1. Start with FIXTURE_META entries (custom params)
    2. Auto-derive any remaining manifest ops not in FIXTURE_META
    3. Modes always come from manifest.yaml
    """
    manifest = _load_manifest()
    registry = {}

    # Pass 1: FIXTURE_META entries
    for op_full_name, meta in FIXTURE_META.items():
        module_name, op_name = _module_name_for_op(op_full_name)
        modes = _find_manifest_modes(manifest, module_name, op_name)
        if not modes:
            modes = meta.get("modes", ["L", "RGB"])
        entry = dict(meta)
        entry["modes"] = modes
        registry[op_full_name] = entry

    # Pass 2: Auto-derive remaining implemented manifest ops
    for mod_name, op_name, modes in _collect_manifest_ops(manifest):
        op_full_name = f"{mod_name}.{op_name}"
        if op_full_name in registry:
            continue  # already in FIXTURE_META
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
