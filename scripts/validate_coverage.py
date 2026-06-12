#!/usr/bin/env python3
"""Manifest-driven coverage validator. Exit 0 = complete, Exit 1 = gaps found.

Scans manifest.yaml for expected (op, mode, target, variant) tuples.
Scans Python test files for @pytest.mark.covers markers.
Scans JS test files for @covers JSDoc tags.
Diffs expected vs actual. Prints gap report.
"""
import sys, yaml, re
from pathlib import Path
from collections import namedtuple

ROOT = Path(__file__).parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"

CoveragePoint = namedtuple("CoveragePoint", ["op", "mode", "target", "variant"])

# ── Manifest ─────────────────────────────────────────────────────

FILTER_CLASSES = {
    "BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
    "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE",
    "GaussianBlur", "BoxBlur", "UnsharpMask", "Kernel",
    "MaxFilter", "MinFilter", "MedianFilter", "ModeFilter",
    "RankFilter", "Color3DLUT",
}

ENHANCE_CLASSES = {"Brightness", "Color", "Contrast", "Sharpness"}

FILTER_MODES = ["L", "LA", "RGB", "RGBA"]
ENHANCE_MODES = ["L", "RGB", "RGBA"]

ALL_MODES = ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"]


def load_manifest():
    with open(MANIFEST_PATH) as f:
        return yaml.safe_load(f)


def build_expected(manifest):
    """Build set of all expected CoveragePoints from manifest."""
    expected = set()

    for mod_name, mod_def in manifest.get("modules", {}).items():
        # --- class_methods, methods, functions ---
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict):
                    continue
                if item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = item.get("supported_modes", [])
                targets = item.get("supported_targets", ["cpu"])
                variants = ["default"]  # Variant is documentation-only for now
                if not modes:
                    # Mode-independent (fonts, palettes, metadata)
                    for target in targets:
                        for variant in variants:
                            expected.add(CoveragePoint(op_name, "", target, variant))
                else:
                    # Generate mode-specific entries
                    for mode in modes:
                        for target in targets:
                            for variant in variants:
                                expected.add(CoveragePoint(op_name, mode, target, variant))
                    # Also generate a mode-less entry for markers that don't specify mode
                    for target in targets:
                        for variant in variants:
                            expected.add(CoveragePoint(op_name, "", target, variant))

        # --- classes (ImageFilter, ImageEnhance, ImageFont, etc.) ---
        for cls in mod_def.get("classes", []):
            if not isinstance(cls, dict):
                continue
            cls_name = cls.get("name", "")
            cls_targets = cls.get("supported_targets", ["cpu"])

            if cls_name in FILTER_CLASSES:
                op_name = f"{mod_name}.{cls_name}"
                for mode in FILTER_MODES:
                    for target in cls_targets:
                        expected.add(CoveragePoint(op_name, mode, target, "default"))
                # Mode-less entry for backward compatibility
                for target in cls_targets:
                    expected.add(CoveragePoint(op_name, "", target, "default"))

            elif cls_name in ENHANCE_CLASSES:
                op_name = f"{mod_name}.{cls_name}"
                for mode in ENHANCE_MODES:
                    for target in cls_targets:
                        expected.add(CoveragePoint(op_name, mode, target, "default"))
                # Mode-less entry for backward compatibility
                for target in cls_targets:
                    expected.add(CoveragePoint(op_name, "", target, "default"))

            elif cls_name == "Stat":
                props = ["extrema", "count", "sum", "sum2", "mean", "median", "rms", "var", "stddev"]
                for prop in props:
                    op_name = f"{mod_name}.Stat.{prop}"
                    for target in cls_targets:
                        expected.add(CoveragePoint(op_name, "", target, "default"))

            elif cls_name == "Iterator":
                op_name = f"{mod_name}.Iterator"
                for target in cls_targets:
                    expected.add(CoveragePoint(op_name, "", target, "default"))

            elif cls_name in ("FreeTypeFont", "ImageFont"):
                for method in cls.get("methods", []):
                    if isinstance(method, dict) and method.get("status", cls.get("status")) == "implemented":
                        op_name = f"{mod_name}.{cls_name}.{method['name']}"
                        for target in cls_targets:
                            expected.add(CoveragePoint(op_name, "", target, "default"))

            elif cls.get("status") == "implemented":
                # Other class types
                op_name = f"{mod_name}.{cls_name}"
                cls_modes = cls.get("supported_modes", [])
                if not cls_modes:
                    for target in cls_targets:
                        expected.add(CoveragePoint(op_name, "", target, "default"))
                else:
                    for mode in cls_modes:
                        for target in cls_targets:
                            expected.add(CoveragePoint(op_name, mode, target, "default"))

        # --- properties ---
        for prop in mod_def.get("properties", []):
            if isinstance(prop, dict):
                op_name = f"{mod_name}.{prop['name']}"
                prop_modes = prop.get("modes", [])
                if not prop_modes:
                    expected.add(CoveragePoint(op_name, "", "cpu", "default"))
                else:
                    for mode in prop_modes:
                        expected.add(CoveragePoint(op_name, mode, "cpu", "default"))

    return expected


def _variants_from_param_variants(param_variants):
    """Convert param_variants list to stable string keys.

    Uses simplified keys that match @pytest.mark.covers conventions.
    For most operations, just returns ['default']. For operations with
    clearly distinct parameter variants (different resample methods, etc.),
    returns the variant names.
    """
    if not param_variants:
        return ["default"]
    keys = []
    for v in param_variants:
        if not v or v == {}:
            keys.append("default")
            continue
        # For convert mode variants, use mode_<MODE> convention
        if "mode" in v and len(v) == 1:
            mode_val = v["mode"]
            if isinstance(mode_val, str):
                keys.append(f"mode_{mode_val}")
            else:
                keys.append("default")
        elif "resample" in v and "size" in v and len(v) <= 2:
            # resize with specific resample method
            resample = v.get("resample", "")
            keys.append(f"resample_{resample}" if resample else "default")
        elif "method" in v and len(v) == 1:
            # transpose method variants
            method = v["method"]
            keys.append(f"method_{method}" if isinstance(method, str) else f"variant_{method}")
        elif "angle" in v and len(v) <= 2:
            angle = v["angle"]
            keys.append(f"angle_{angle}")
        else:
            keys.append("default")
    return keys if keys else ["default"]


# ── Scanner: Python tests ────────────────────────────────────────

PYTHON_COVERS_RE = re.compile(
    r'@pytest\.mark\.covers\(\s*"([^"]+)"\s*'
    r'(?:,\s*mode="([^"]*)")?\s*'
    r'(?:,\s*target="([^"]*)")?\s*'
    r'(?:,\s*variant="([^"]*)")?\s*'
    r'\)'
)


def scan_python_tests(tests_dir):
    """Parse @pytest.mark.covers decorators from Python test files."""
    actual = set()
    for py_file in Path(tests_dir).rglob("test_*.py"):
        content = py_file.read_text()
        for match in PYTHON_COVERS_RE.finditer(content):
            op = match.group(1)
            mode = match.group(2) or ""
            target = match.group(3) or "cpu"
            variant = match.group(4) or "default"
            actual.add(CoveragePoint(op, mode, target, variant))
    return actual


# ── Scanner: JS tests ────────────────────────────────────────────

JS_COVERS_RE = re.compile(
    r'@covers\s+(\S+)\s*\n'
    r'(?:\s*\*\s*@mode\s+(\S+)\s*\n)?'
    r'(?:\s*\*\s*@target\s+(\S+)\s*\n)?'
    r'(?:\s*\*\s*@variant\s+(\S+)\s*\n)?'
)


def scan_js_tests(tests_dir):
    """Parse @covers JSDoc tags from JS test files."""
    actual = set()
    js_dir = Path(tests_dir)
    if not js_dir.exists():
        return actual
    for js_file in js_dir.rglob("*"):
        if js_file.suffix in (".js", ".mjs", ".ts") and js_file.is_file():
            content = js_file.read_text()
            for match in JS_COVERS_RE.finditer(content):
                op = match.group(1)
                mode = match.group(2) or ""
                target = match.group(3) or "wasm"
                variant = match.group(4) or "default"
                actual.add(CoveragePoint(op, mode, target, variant))
    return actual


# ── Main ─────────────────────────────────────────────────────────

def main():
    manifest = load_manifest()
    expected = build_expected(manifest)
    python_set = scan_python_tests(ROOT / "tests")
    js_set = scan_js_tests(ROOT / "pillow-rs-js" / "tests")

    actual = python_set | js_set

    # Normalize: map any non-default variant to "default" for expected matching
    normalized_actual = set()
    for ap in actual:
        normalized_actual.add(CoveragePoint(ap.op, ap.mode, ap.target, "default"))

    # Expand mode-less markers: a marker without mode covers ALL modes for that op/target/variant
    expanded_actual = set(normalized_actual)
    for ap in normalized_actual:
        if ap.mode == "":
            for ep in expected:
                if (ep.op == ap.op and ep.target == ap.target
                        and ep.variant == ap.variant and ep.mode != ""):
                    expanded_actual.add(CoveragePoint(ep.op, ep.mode, ep.target, ep.variant))

    gaps = expected - expanded_actual
    unknown = normalized_actual - expected

    if gaps:
        print(f"\n{'='*70}")
        print(f"  GAPS: {len(gaps)} missing tests")
        print(f"{'='*70}")
        by_module = {}
        for g in sorted(gaps, key=str):
            mod = g.op.split(".")[0]
            by_module.setdefault(mod, []).append(g)
        for mod, items in sorted(by_module.items()):
            print(f"\n  {mod} ({len(items)} gaps):")
            for g in sorted(items, key=str)[:30]:  # limit per module for readability
                mode_str = f" x {g.mode}" if g.mode else ""
                target_str = f" x {g.target}" if g.target != "cpu" else ""
                variant_str = f" x {g.variant}" if g.variant != "default" else ""
                print(f"    MISS  {g.op}{mode_str}{target_str}{variant_str}")
            if len(items) > 30:
                print(f"    ... and {len(items) - 30} more")
        print()

    if unknown:
        print(f"\n{'='*70}")
        print(f"  UNKNOWN: {len(unknown)} markers with no manifest match")
        print(f"{'='*70}")
        for u in sorted(unknown, key=str):
            mode_str = f" x {u.mode}" if u.mode else ""
            print(f"    EXTRA  {u.op}{mode_str} x {u.target} x {u.variant}")
        print()

    total_expected = len(expected)
    total_actual = len(actual)
    coverage_pct = (total_actual / total_expected * 100) if total_expected else 100

    print(f"  Expected: {total_expected}  |  Actual: {total_actual}  |  Coverage: {coverage_pct:.1f}%")
    print(f"  Python: {len(python_set)}  |  JS: {len(js_set)}")
    print(f"  Gaps: {len(gaps)}  |  Unknown: {len(unknown)}")

    if gaps or unknown:
        print(f"\n  ❌ VALIDATION FAILED\n")
        sys.exit(1)
    else:
        print(f"\n  ✅ VALIDATION PASSED — coverage matrix complete\n")
        sys.exit(0)


if __name__ == "__main__":
    main()
