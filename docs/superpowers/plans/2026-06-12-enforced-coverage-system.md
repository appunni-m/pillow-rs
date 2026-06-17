# Enforced Coverage System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement manifest-driven enforced coverage that makes it impossible to miss a test or write a wrong test across Python and JS targets.

**Architecture:** `manifest.yaml` is the single source of truth defining expected (op × mode × target × variant) tuples. `@pytest.mark.covers` / `@covers` JSDoc on every parity test. `scripts/validate_coverage.py` diffs expected vs actual, exits 1 on gap. Pytest collection hook rejects untracked tests at collection time.

**Tech Stack:** Python 3.10+ (pytest, PyYAML), Rust (cargo test), Node.js (Jest/Vitest), WASM (wasm-pack), Puppeteer (browser GPU tests)

---

### Task 1: Manifest — add `supported_targets` field

**Files:**
- Modify: `manifest.yaml` (add supported_targets to every implemented operation)

- [ ] **Step 1: Add supported_targets to manifest.yaml**

Add `supported_targets: [cpu]` to every implemented operation. For operations that already have GPU/WASM support, set to `[cpu, gpu, wasm]`. For I/O operations (open, save) set to `[cpu]` only. For PipelineOps, set to `[cpu, gpu, wasm, wasm_gpu]`.

The default is `[cpu]` — GPU and WASM are opt-in. Edit manifest.yaml and add this field to each operation. Here is the complete list of operations organized by target support:

**CPU-only (no GPU benefit):**
`Image.open`, `Image.save`, `Image.close`, `Image.seek`, `Image.tell`, `Image.verify`, `Image.load`, `Image.draft`, `Image.tobitmap`, `Image.remap_palette`, `Image.frombytes`, `Image.getexif`, `Image.getim`, `Image.getpalette`, `Image.getxmp`, `Image.putpalette`, `Image.show`, `Image.get_child_images`, `Image.get_flattened_data`, `Image.apply_transparency`, `ImagePalette.*` (all), `ImageFont.*` (all), `ImageStat.*` (all), `ImageSequence.*` (all), `ImageColor.*` (all)

**CPU + GPU (PipelineOps, pixel operations):**
`Image.new`, `Image.resize`, `Image.crop`, `Image.rotate`, `Image.transpose`, `Image.convert`, `Image.paste`, `Image.filter`, `Image.copy`, `Image.split`, `Image.getbands`, `Image.thumbnail`, `Image.tobytes`, `Image.alpha_composite`, `Image.getbbox`, `Image.getchannel`, `Image.getcolors`, `Image.getdata`, `Image.getextrema`, `Image.getpixel`, `Image.getprojection`, `Image.histogram`, `Image.point`, `Image.putalpha`, `Image.putdata`, `Image.putpixel`, `Image.quantize`, `Image.reduce`, `Image.transform`, `Image.effect_spread`, `Image.entropy`, all `ImageModule.*`, all `ImageDraw.*`, all `ImageFilter.*`, all `ImageEnhance.*`, all `ImageOps.*`, all `ImageChops.*`

The `supported_targets` field is a list in each YAML operation entry:
```yaml
- name: resize
  supported_modes: [L, LA, RGB, RGBA, 1, P]
  supported_targets: [cpu, gpu, wasm, wasm_gpu]
```

- [ ] **Step 2: Verify manifest parses correctly**

```bash
python -c "import yaml; yaml.safe_load(open('manifest.yaml')); print('OK')"
```
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add manifest.yaml
git commit -m "feat: add supported_targets field to manifest.yaml
All implemented operations now carry supported_targets. CPU-only ops
(metadata, fonts, I/O) default to [cpu]. Pixel ops get [cpu, gpu, wasm, wasm_gpu]."
```

---

### Task 2: Build `scripts/validate_coverage.py`

**Files:**
- Create: `scripts/validate_coverage.py`

- [ ] **Step 1: Write the validation script**

```python
#!/usr/bin/env python3
"""Manifest-driven coverage validator. Exit 0 = complete, Exit 1 = gaps found.

Scans manifest.yaml for expected (op, mode, target, variant) tuples.
Scans Python test files for @pytest.mark.covers markers.
Scans JS test files for @covers JSDoc tags.
Diffs expected vs actual. Prints gap report.
"""
import sys, yaml, re, ast, os
from pathlib import Path
from collections import namedtuple

ROOT = Path(__file__).parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"

CoveragePoint = namedtuple("CoveragePoint", ["op", "mode", "target", "variant"])

def load_manifest():
    with open(MANIFEST_PATH) as f:
        return yaml.safe_load(f)

def build_expected(manifest):
    """Build set of all expected CoveragePoints from manifest."""
    expected = set()
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict):
                    continue
                if item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = item.get("supported_modes", [])
                targets = item.get("supported_targets", ["cpu"])
                variants = item.get("param_variants", [{}])
                if not modes:
                    # Mode-independent operations (fonts, palettes, etc.)
                    # Use empty string as sentinel for "no mode"
                    modes = [""]
                for mode in modes:
                    for target in targets:
                        for variant in variants:
                            variant_key = _variant_to_key(variant)
                            expected.add(CoveragePoint(op_name, mode, target, variant_key))
        # Handle classes (ImageFilter, ImageEnhance, ImageFont, ImageStat, ImageSequence)
        for cls in mod_def.get("classes", []):
            if not isinstance(cls, dict):
                continue
            if cls.get("status") != "implemented":
                continue
            cls_name = cls.get("name", "")
            # Each class method gets its own entry or the class itself is the operation
            if cls_name in ("BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
                            "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE",
                            "GaussianBlur", "BoxBlur", "UnsharpMask", "Kernel",
                            "MaxFilter", "MinFilter", "MedianFilter", "ModeFilter",
                            "RankFilter", "Color3DLUT"):
                op_name = f"ImageFilter.{cls_name}"
                targets = cls.get("supported_targets", ["cpu"])
                for mode in ["L", "LA", "RGB", "RGBA"]:
                    for target in targets:
                        expected.add(CoveragePoint(op_name, mode, target, "default"))
            elif cls_name in ("Brightness", "Color", "Contrast", "Sharpness"):
                op_name = f"ImageEnhance.{cls_name}"
                targets = cls.get("supported_targets", ["cpu"])
                for mode in ["L", "RGB", "RGBA"]:
                    for target in targets:
                        expected.add(CoveragePoint(op_name, mode, target, "default"))
            elif cls_name == "Stat":
                for prop in ["extrema", "count", "sum", "sum2", "mean", "median", "rms", "var", "stddev"]:
                    op_name = f"ImageStat.Stat.{prop}"
                    expected.add(CoveragePoint(op_name, "", "cpu", "default"))
            elif cls_name == "Iterator":
                expected.add(CoveragePoint("ImageSequence.Iterator", "", "cpu", "default"))
            # FreeTypeFont and ImageFont methods
            for method in cls.get("methods", []):
                if isinstance(method, dict):
                    m_name = method.get("name", "")
                    op_name = f"ImageFont.{cls_name}.{m_name}"
                    if method.get("status", cls.get("status")) == "implemented":
                        expected.add(CoveragePoint(op_name, "", "cpu", "default"))
        # Handle properties
        for prop in mod_def.get("properties", []):
            if isinstance(prop, dict):
                op_name = f"{mod_name}.{prop['name']}"
                p_modes = prop.get("modes", [])
                if not p_modes:
                    expected.add(CoveragePoint(op_name, "", "cpu", "default"))
                else:
                    for mode in p_modes:
                        expected.add(CoveragePoint(op_name, mode, "cpu", "default"))
    return expected

def _variant_to_key(variant):
    """Convert a variant dict to a stable string key."""
    if not variant or variant == {}:
        return "default"
    parts = []
    for k in sorted(variant.keys()):
        v = variant[k]
        if isinstance(v, list):
            v = "x".join(str(x) for x in v)
        parts.append(f"{k}={v}")
    return "_".join(parts)

def scan_python_tests(tests_dir):
    """Parse @pytest.mark.covers decorators from Python test files."""
    actual = set()
    pattern = re.compile(
        r'@pytest\.mark\.covers\(\s*"([^"]+)"\s*'
        r'(?:,\s*mode="([^"]*)")?\s*'
        r'(?:,\s*target="([^"]*)")?\s*'
        r'(?:,\s*variant="([^"]*)")?\s*'
        r'\)'
    )
    for py_file in Path(tests_dir).rglob("test_*.py"):
        content = py_file.read_text()
        for match in pattern.finditer(content):
            op = match.group(1)
            mode = match.group(2) or ""
            target = match.group(3) or "cpu"
            variant = match.group(4) or "default"
            actual.add(CoveragePoint(op, mode, target, variant))
    return actual

def scan_js_tests(tests_dir):
    """Parse @covers JSDoc tags from JS test files."""
    actual = set()
    if not Path(tests_dir).exists():
        return actual
    pattern = re.compile(
        r'@covers\s+(\S+)\s*\n'
        r'(?:\s*\*\s*@mode\s+(\S+)\s*\n)?'
        r'(?:\s*\*\s*@target\s+(\S+)\s*\n)?'
        r'(?:\s*\*\s*@variant\s+(\S+)\s*\n)?'
    )
    for js_file in Path(tests_dir).rglob("*.{js,mjs,ts}"):
        content = js_file.read_text()
        for match in pattern.finditer(content):
            op = match.group(1)
            mode = match.group(2) or ""
            target = match.group(3) or "wasm"
            variant = match.group(4) or "default"
            actual.add(CoveragePoint(op, mode, target, variant))
    return actual

def main():
    manifest = load_manifest()
    expected = build_expected(manifest)
    python_set = scan_python_tests(ROOT / "tests")
    js_set = scan_js_tests(ROOT / "pillow-rs-js" / "tests")

    actual = python_set | js_set
    gaps = expected - actual
    unknown = actual - expected

    if gaps:
        print(f"\n{'='*70}")
        print(f"  GAPS: {len(gaps)} missing tests")
        print(f"{'='*70}")
        # Group by module
        by_module = {}
        for g in sorted(gaps):
            mod = g.op.split(".")[0]
            by_module.setdefault(mod, []).append(g)
        for mod, items in sorted(by_module.items()):
            print(f"\n  {mod} ({len(items)} gaps):")
            for g in items:
                mode_str = f" × {g.mode}" if g.mode else ""
                target_str = f" × {g.target}" if g.target != "cpu" else ""
                variant_str = f" × {g.variant}" if g.variant != "default" else ""
                print(f"    MISS  {g.op}{mode_str}{target_str}{variant_str}")
        print()

    if unknown:
        print(f"\n{'='*70}")
        print(f"  UNKNOWN: {len(unknown)} markers with no manifest match")
        print(f"{'='*70}")
        for u in sorted(unknown):
            print(f"    EXTRA  {u.op} × {u.mode} × {u.target}")
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
```

- [ ] **Step 2: Run validate_coverage.py — expect many gaps**

```bash
python scripts/validate_coverage.py manifest.yaml
```
Expected: prints large gap report, exits 1 (all the current untracked and missing-mode tests)

- [ ] **Step 3: Commit**

```bash
git add scripts/validate_coverage.py
git commit -m "feat: add manifest-driven coverage validator
Scans manifest.yaml for expected (op × mode × target × variant) tuples.
Scans Python @pytest.mark.covers and JS @covers JSDoc markers.
Diffs expected vs actual, exits 1 on any gap."
```

---

### Task 3: Add pytest collection hook to conftest.py

**Files:**
- Modify: `tests/conftest.py`

- [ ] **Step 1: Add the collection hook**

Edit `tests/conftest.py`. After the `pytest_configure` function, add:

```python
def pytest_collection_modifyitems(config, items):
    """Reject any test that is missing @pytest.mark.covers or has an invalid mode."""
    manifest_path = Path(config.getoption("--manifest", default="manifest.yaml"))
    with open(manifest_path) as f:
        manifest = yaml.safe_load(f)

    # Build lookup: operation_name -> set of supported_modes
    op_modes = {}
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if isinstance(item, dict) and item.get("status") == "implemented":
                    op_key = f"{mod_name}.{item['name']}"
                    op_modes[op_key] = set(item.get("supported_modes", []))
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict) and cls.get("status") == "implemented":
                cls_name = cls.get("name", "")
                op_key = f"{mod_name}.{cls_name}"
                op_modes[op_key] = set(cls.get("supported_modes", ["L", "RGB", "RGBA"]))
                for method in cls.get("methods", []):
                    if isinstance(method, dict):
                        m_name = method.get("name", "")
                        op_modes[f"{mod_name}.{cls_name}.{m_name}"] = set()
        for prop in mod_def.get("properties", []):
            if isinstance(prop, dict):
                op_key = f"{mod_name}.{prop['name']}"
                op_modes[op_key] = set(prop.get("modes", []))

    errors = []
    for item in items:
        marker = item.get_closest_marker('covers')
        if marker is None:
            errors.append(f"MISSING @covers: {item.nodeid}")
            continue
        op_name = marker.args[0] if marker.args else None
        if op_name is None:
            errors.append(f"EMPTY @covers: {item.nodeid}")
            continue
        # Check operation exists in manifest
        if op_name not in op_modes:
            errors.append(f"UNKNOWN op '{op_name}' in @covers: {item.nodeid}")
            continue
        # Check mode is valid for this operation
        mode = marker.kwargs.get('mode', '')
        valid_modes = op_modes[op_name]
        if mode and valid_modes and mode not in valid_modes:
            errors.append(
                f"INVALID mode '{mode}' for {op_name} (valid: {sorted(valid_modes)}): {item.nodeid}"
            )
        # Check target is valid
        target = marker.kwargs.get('target', 'cpu')
        valid_targets = {'cpu', 'gpu', 'wasm', 'wasm_gpu'}
        if target not in valid_targets:
            errors.append(
                f"INVALID target '{target}' (valid: {sorted(valid_targets)}): {item.nodeid}"
            )

    if errors:
        raise pytest.UsageError(
            "\n" + "="*70 + "\n" +
            f"  COVERAGE ENFORCEMENT: {len(errors)} error(s)\n" +
            "="*70 + "\n" +
            "\n".join(f"  • {e}" for e in errors) +
            "\n" + "="*70
        )
```

- [ ] **Step 2: Run pytest — expect collection errors**

```bash
python -m pytest tests/ --collect-only 2>&1 | head -80
```
Expected: prints MISSING @covers errors for all 74 untracked tests. Pytest exits with non-zero.

- [ ] **Step 3: Commit**

```bash
git add tests/conftest.py
git commit -m "feat: add pytest collection hook enforcing @covers markers
Every parity test MUST have @pytest.mark.covers with a valid operation
name and mode matching manifest supported_modes. Unknown ops, invalid
modes, or missing markers raise pytest.UsageError at collection time."
```

---

### Task 4: Add @covers markers to all untracked Python tests

**Files:**
- Modify: 22 test files (all except test_image_new.py and test_completion.py which are already mostly covered)

This is the largest single task. There are 74 tests missing covers markers across 22 files. The approach: add the correct `@pytest.mark.covers(...)` decorator to each test based on what the test actually exercises (read the test body to determine mode, operation, variant).

**Pattern — read test body, add marker before function:**

Before:
```python
def test_crop_grayscale_parity(PIL):
    pil_img = PIL.Image.new("L", (100, 100), 128)
    rs_img = Image.new("L", (100, 100), 128)
    pil_cropped = pil_img.crop((25, 25, 75, 75))
    rs_cropped = rs_img.crop((25, 25, 75, 75))
    assert_images_equal(rs_cropped, pil_cropped)
```

After:
```python
@pytest.mark.covers("Image.crop", mode="L", target="cpu", variant="default")
def test_crop_grayscale_parity(PIL):
    pil_img = PIL.Image.new("L", (100, 100), 128)
    rs_img = Image.new("L", (100, 100), 128)
    pil_cropped = pil_img.crop((25, 25, 75, 75))
    rs_cropped = rs_img.crop((25, 25, 75, 75))
    assert_images_equal(rs_cropped, pil_cropped)
```

Here is the complete mapping of every untracked test to its correct covers decorator:

**tests/test_image_convert.py:**
- `TestConvert::test_l_to_rgb_parity` → `@pytest.mark.covers("Image.convert", mode="L", target="cpu", variant="default")`
- `TestConvert::test_convert_chain_parity` → `@pytest.mark.covers("Image.convert", mode="RGB", target="cpu", variant="chain")`

**tests/test_image_resize.py:**
- `test_resize_upscale_parity` → `@pytest.mark.covers("Image.resize", mode="RGB", target="cpu", variant="upscale")`

**tests/test_image_crop.py:**
- `test_crop_grayscale_parity` → `@pytest.mark.covers("Image.crop", mode="L", target="cpu", variant="default")`
- `test_crop_rgba_parity` → `@pytest.mark.covers("Image.crop", mode="RGBA", target="cpu", variant="default")`

**tests/test_image_rotate_transpose.py:**
- `TestRotate::test_rotate_90_parity` → `@pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_90")`
- `TestRotate::test_rotate_180_parity` → `@pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_180")`
- `TestRotate::test_rotate_270_parity` → `@pytest.mark.covers("Image.rotate", mode="RGB", target="cpu", variant="angle_270")`
- `TestTranspose::test_transpose_parity` → `@pytest.mark.covers("Image.transpose", mode="RGB", target="cpu", variant="TRANSPOSE")`
- `TestTranspose::test_transverse_parity` → `@pytest.mark.covers("Image.transpose", mode="RGB", target="cpu", variant="TRANSVERSE")`

**tests/test_image_filter.py:**
- `test_filter_contour_works` → `@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="CONTOUR")`
- `test_filter_emboss_works` → `@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="EMBOSS")`
- `test_filter_find_edges_works` → `@pytest.mark.covers("Image.filter", mode="RGB", target="cpu", variant="FIND_EDGES")`

**tests/test_image_paste.py:**
- `test_paste_at_origin_parity` → `@pytest.mark.covers("Image.paste", mode="RGB", target="cpu", variant="origin")`

**tests/test_image_split.py:**
- `test_split_grayscale_parity` → `@pytest.mark.covers("Image.split", mode="L", target="cpu", variant="default")`
- `test_getbands_l_parity` → `@pytest.mark.covers("Image.getbands", mode="L", target="cpu", variant="default")`

**tests/test_image_analysis.py:**
- `test_getextrema_rgb_parity` → `@pytest.mark.covers("Image.getextrema", mode="RGB", target="cpu", variant="default")`

**tests/test_image_enhance_etc.py:**
- `test_reduce_parity` → `@pytest.mark.covers("Image.reduce", mode="RGB", target="cpu", variant="default")`

**tests/test_image_chops.py:**
- `test_chops_subtract_parity` → `@pytest.mark.covers("ImageChops.subtract", mode="RGB", target="cpu", variant="default")`
- `test_chops_screen_parity` → `@pytest.mark.covers("ImageChops.screen", mode="RGB", target="cpu", variant="default")`
- `test_chops_lighter_parity` → `@pytest.mark.covers("ImageChops.lighter", mode="RGB", target="cpu", variant="default")`
- `test_chops_invert_parity` → `@pytest.mark.covers("ImageChops.invert", mode="RGB", target="cpu", variant="default")`

**tests/test_image_chops_advanced.py:**
- `test_subtract_modulo_works` → `@pytest.mark.covers("ImageChops.subtract_modulo", mode="RGB", target="cpu", variant="default")`
- `test_blend_parity` → `@pytest.mark.covers("ImageChops.blend", mode="RGB", target="cpu", variant="default")`
- `test_composite_works` → `@pytest.mark.covers("ImageChops.composite", mode="RGB", target="cpu", variant="default")`

**tests/test_image_ops.py:**
- `test_ops_flip_parity` → `@pytest.mark.covers("ImageOps.flip", mode="RGB", target="cpu", variant="default")`
- `test_ops_grayscale_parity` → `@pytest.mark.covers("ImageOps.grayscale", mode="RGB", target="cpu", variant="default")`
- `test_ops_solarize_parity` → `@pytest.mark.covers("ImageOps.solarize", mode="RGB", target="cpu", variant="default")`

**tests/test_imageops_advanced.py:**
- `test_cover_parity` → `@pytest.mark.covers("ImageOps.cover", mode="RGB", target="cpu", variant="default")`
- `test_scale_parity` → `@pytest.mark.covers("ImageOps.scale", mode="RGB", target="cpu", variant="default")`

**tests/test_image_draw.py:**
- `test_draw_ellipse` → `@pytest.mark.covers("ImageDraw.ellipse", mode="RGB", target="cpu", variant="default")`
- `test_draw_polygon` → `@pytest.mark.covers("ImageDraw.polygon", mode="RGB", target="cpu", variant="default")`

**tests/test_imagedraw_advanced.py:**
- `test_draw_chord_works` → `@pytest.mark.covers("ImageDraw.chord", mode="RGB", target="cpu", variant="default")`
- `test_draw_circle_works` → `@pytest.mark.covers("ImageDraw.circle", mode="RGB", target="cpu", variant="default")`

**tests/test_image_modules.py:**
- `TestImageFilter::test_gaussian_blur_class` → `@pytest.mark.covers("ImageFilter.GaussianBlur", mode="RGB", target="cpu", variant="default")`
- `TestImageFont::test_truetype_loads_real_font` → `@pytest.mark.covers("ImageFont.truetype", target="cpu", variant="default")`
- `TestImagePalette::test_copy_palette` → `@pytest.mark.covers("ImagePalette.copy", target="cpu", variant="default")`
- `TestImageStat::test_iterator_exists` → `@pytest.mark.covers("ImageSequence.Iterator", target="cpu", variant="default")`
- `test_expand_border` → `@pytest.mark.covers("ImageOps.expand", mode="RGB", target="cpu", variant="border")`

**tests/test_imagemodule_fns.py:**
- `test_blend_parity` → `@pytest.mark.covers("ImageModule.blend", mode="RGB", target="cpu", variant="default")`
- `test_composite_works` → `@pytest.mark.covers("ImageModule.composite", mode="RGB", target="cpu", variant="default")`

**tests/test_image_color.py:**
- `test_getcolor_l_parity` → `@pytest.mark.covers("ImageColor.getcolor", mode="L", target="cpu", variant="default")`

**tests/test_image_io.py:**
- `TestOpenSave::test_save_png_roundtrip` → `@pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="png")`
- `TestOpenSave::test_save_jpeg_roundtrip` → `@pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="jpeg")`
- `TestOpenSave::test_open_bytes` → `@pytest.mark.covers("Image.open", mode="RGB", target="cpu", variant="bytes")`
- `TestThumbnail::test_thumbnail_parity` → `@pytest.mark.covers("Image.thumbnail", mode="RGB", target="cpu", variant="default")`
- `TestBookkeeping::test_close_no_error` → `@pytest.mark.covers("Image.close", mode="RGB", target="cpu", variant="default")`
- `TestBookkeeping::test_verify_no_error` → `@pytest.mark.covers("Image.verify", mode="RGB", target="cpu", variant="default")`
- `TestBookkeeping::test_seek_tell` → `@pytest.mark.covers("Image.seek", mode="RGB", target="cpu", variant="default")` (also add a second call for tell)
- `TestBookkeeping::test_load_returns` → `@pytest.mark.covers("Image.load", mode="RGB", target="cpu", variant="default")`

**tests/test_final_stubs.py:**
- `test_apply_transparency` → `@pytest.mark.covers("Image.apply_transparency", target="cpu", variant="default")`
- `test_get_child_images` → `@pytest.mark.covers("Image.get_child_images", target="cpu", variant="default")`
- `test_getexif` → `@pytest.mark.covers("Image.getexif", target="cpu", variant="default")`
- `test_getpalette` → `@pytest.mark.covers("Image.getpalette", target="cpu", variant="default")`
- `test_getxmp` → `@pytest.mark.covers("Image.getxmp", target="cpu", variant="default")`
- `test_putpalette` → `@pytest.mark.covers("Image.putpalette", target="cpu", variant="default")`
- `test_show_no_error` → `@pytest.mark.covers("Image.show", target="cpu", variant="default")`
- `test_get_flattened_data` → `@pytest.mark.covers("Image.get_flattened_data", target="cpu", variant="default")`
- `test_draw_getfont` → `@pytest.mark.covers("ImageDraw.getfont", target="cpu", variant="default")`
- `test_palette_tostring` → `@pytest.mark.covers("ImagePalette.tobytes", target="cpu", variant="default")`
- `test_load_default_imagefont` → `@pytest.mark.covers("ImageFont.load_default_imagefont", target="cpu", variant="default")`
- `test_load_path` → `@pytest.mark.covers("ImageFont.load_path", target="cpu", variant="default")`
- `test_getim_raises` → `@pytest.mark.covers("Image.getim", target="cpu", variant="default")`

**tests/test_image_advanced.py (entire file missing markers):**
- `TestAlphaComposite::test_alpha_composite_works` → `@pytest.mark.covers("Image.alpha_composite", mode="RGBA", target="cpu", variant="default")`
- `TestPoint::test_point_lut_parity` → `@pytest.mark.covers("Image.point", mode="RGB", target="cpu", variant="lut")`
- `TestEffectSpread::test_effect_spread_works` → `@pytest.mark.covers("Image.effect_spread", mode="RGB", target="cpu", variant="default")`
- `TestQuantize::test_quantize_parity` → `@pytest.mark.covers("Image.quantize", mode="RGB", target="cpu", variant="default")`
- `TestAnalysis::test_entropy_works` → `@pytest.mark.covers("Image.entropy", mode="RGB", target="cpu", variant="default")`
- `TestAnalysis::test_getcolors_works` → `@pytest.mark.covers("Image.getcolors", mode="RGB", target="cpu", variant="default")`
- `TestAnalysis::test_getdata_rgb_parity` → `@pytest.mark.covers("Image.getdata", mode="RGB", target="cpu", variant="default")`
- `TestAnalysis::test_getprojection_works` → `@pytest.mark.covers("Image.getprojection", mode="RGB", target="cpu", variant="default")`

- [ ] **Step 1: Apply all covers markers**

Go through each file listed above and add the `@pytest.mark.covers(...)` decorator to each test. Edit each file with the Edit tool, adding the decorator line directly before `def test_...` or `def test_...` inside classes.

- [ ] **Step 2: Verify collection passes**

```bash
python -m pytest tests/ --collect-only -q 2>&1 | tail -5
```
Expected: no MISSING @covers errors, pytest collects all tests successfully.

- [ ] **Step 3: Run validate_coverage.py**

```bash
python scripts/validate_coverage.py manifest.yaml
```
Expected: fewer gaps than before, but still many (mode gaps). The validate script should no longer report EXTRA markers — all markers should match manifest ops.

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "feat: add @pytest.mark.covers to all 74 untracked tests
Every parity test now carries an exact @covers marker with operation,
mode, target, and variant. Zero untracked tests remain."
```

---

### Task 5: Expand mode coverage — fill L, LA, 1, P gaps

**Files:**
- Modify: multiple test files (see list below)

This task fills ⬜ cells for modes that PIL supports but we haven't tested yet. Focused on the modes that are quickest to add: L, LA, 1, P for operations that already have RGB tests.

- [ ] **Step 1: Add L mode tests for pixel operations**

For each operation that has `L` in supported_modes and only has RGB tests, add an L-mode test. The pattern is: create an L-mode image, run the operation, compare.

Example — add to `tests/test_image_crop.py`:
```python
@pytest.mark.covers("Image.crop", mode="L", target="cpu", variant="default")
def test_crop_l_parity(PIL):
    pil_img = PIL.Image.new("L", (100, 100), 128)
    rs_img = Image.new("L", (100, 100), 128)
    pil_cropped = pil_img.crop((25, 25, 75, 75))
    rs_cropped = rs_img.crop((25, 25, 75, 75))
    assert_images_equal(rs_cropped, pil_cropped)
```

Add L-mode tests for these operations (one test per operation, add to the existing test file for that operation):
- `Image.crop` L → `tests/test_image_crop.py`
- `Image.resize` L (already exists as `test_resize_grayscale_parity` — verify it has covers marker)
- `Image.rotate` L → `tests/test_image_rotate_transpose.py`
- `Image.transpose` L → `tests/test_image_rotate_transpose.py`
- `Image.paste` L → `tests/test_image_paste.py`
- `Image.filter` L → `tests/test_image_filter.py`
- `Image.copy` L → `tests/test_image_new.py`
- `Image.split` L (already exists as `test_split_grayscale_parity` — verify marker)
- `Image.getbands` L (already exists as `test_getbands_l_parity` — verify marker)
- `Image.thumbnail` L → `tests/test_image_io.py`
- `Image.tobytes` L → `tests/test_image_new.py`
- `Image.getbbox` L → `tests/test_image_analysis.py`
- `Image.getchannel` L → `tests/test_image_enhance_etc.py`
- `Image.getpixel` L (already exists as `test_getpixel_grayscale_parity`)
- `Image.putpixel` L → `tests/test_image_analysis.py`
- `Image.close` L → `tests/test_image_io.py`
- `Image.load` L → `tests/test_image_io.py`
- `Image.point` L → `tests/test_image_advanced.py`
- `Image.putalpha` L → `tests/test_image_enhance_etc.py`
- `Image.putdata` L → `tests/test_completion.py`
- `Image.reduce` L → `tests/test_image_enhance_etc.py`
- `Image.transform` L → `tests/test_completion.py`
- `Image.effect_spread` L → `tests/test_image_advanced.py`

- [ ] **Step 2: Add LA mode tests**

LA mode (luminance + alpha) is tested only in convert. Add LA tests for operations that support it:

- `Image.resize` LA → `tests/test_image_resize.py`
- `Image.crop` LA → `tests/test_image_crop.py`
- `Image.rotate` LA → `tests/test_image_rotate_transpose.py`
- `Image.transpose` LA → `tests/test_image_rotate_transpose.py`
- `Image.filter` LA → `tests/test_image_filter.py`
- `Image.copy` LA → `tests/test_image_new.py`
- `Image.split` LA → `tests/test_image_split.py`
- `Image.getbands` LA → `tests/test_image_split.py`
- `Image.getpixel` LA → `tests/test_image_analysis.py`
- `Image.putpixel` LA → `tests/test_image_analysis.py`

- [ ] **Step 3: Add "1" mode tests**

Mode "1" (binary, threshold at 128):

- `Image.resize` 1 → `tests/test_image_resize.py`
- `Image.crop` 1 → `tests/test_image_crop.py`
- `Image.transpose` 1 → `tests/test_image_rotate_transpose.py`
- `Image.convert` 1 → `tests/test_image_convert.py`
- `Image.copy` 1 → `tests/test_image_new.py`
- `Image.getpixel` 1 → `tests/test_image_analysis.py`
- `Image.putpixel` 1 → `tests/test_image_analysis.py`

- [ ] **Step 4: Add P mode tests**

P mode (palette-based):

- `Image.resize` P → `tests/test_image_resize.py`
- `Image.crop` P → `tests/test_image_crop.py`
- `Image.transpose` P → `tests/test_image_rotate_transpose.py`
- `Image.convert` P → `tests/test_image_convert.py`
- `Image.copy` P → `tests/test_image_new.py`
- `Image.getpixel` P → `tests/test_image_analysis.py`
- `Image.putpixel` P → `tests/test_image_analysis.py`

- [ ] **Step 5: Add RGBA mode tests for operations missing them**

- `Image.crop` RGBA → `tests/test_image_crop.py`
- `Image.rotate` RGBA → `tests/test_image_rotate_transpose.py`
- `Image.transpose` RGBA → `tests/test_image_rotate_transpose.py`
- `Image.paste` RGBA → `tests/test_image_paste.py`
- `Image.filter` RGBA → `tests/test_image_filter.py`
- `Image.copy` RGBA → `tests/test_image_new.py`
- `Image.thumbnail` RGBA → `tests/test_image_io.py`
- `Image.getbbox` RGBA → `tests/test_image_analysis.py`
- `Image.getchannel` RGBA → `tests/test_image_enhance_etc.py`
- `Image.getcolors` RGBA → `tests/test_image_advanced.py`
- `Image.getdata` RGBA → `tests/test_image_advanced.py`
- `Image.getextrema` RGBA → `tests/test_image_analysis.py`
- `Image.alpha_composite` RGBA → `tests/test_image_advanced.py`
- `Image.putpixel` RGBA → `tests/test_image_analysis.py`
- `Image.quantize` RGBA → `tests/test_image_advanced.py`
- `Image.tobytes` RGBA → `tests/test_image_new.py`

- [ ] **Step 6: Run tests to verify new mode tests pass**

```bash
python -m pytest tests/ -q
```
Expected: all tests pass (new mode tests create images in the new mode and run the operation).

- [ ] **Step 7: Commit**

```bash
git add tests/
git commit -m "feat: expand mode coverage — add L, LA, 1, P, RGBA tests
Fills ⬜ gaps in mode×operation matrix for pixel operations.
Each new test creates a PIL image in the target mode, creates
an identical RSPIL image, runs the operation, and asserts pixel parity."
```

---

### Task 6: Add error-parity tests for 🔴 cells

**Files:**
- Create: `tests/test_error_parity.py`

- [ ] **Step 1: Write error-parity tests**

These tests verify that when PIL throws for an unsupported mode, pillow-rs throws the same error type with the same message.

```python
"""Error parity tests — verify pillow-rs matches PIL errors for unsupported mode×operation combos."""
import pytest
import PIL.Image as PILImage
from pillow_rs import Image
from conftest import assert_values_equal


@pytest.mark.covers("Image.resize", mode="CMYK", target="cpu", variant="error_parity")
def test_resize_cmyk_raises(PIL):
    """PIL rejects CMYK resize — we must match the error."""
    pil_img = PILImage.new("RGB", (100, 100)).convert("CMYK")
    rs_img = Image.new("RGB", (100, 100)).convert("CMYK")
    try:
        pil_img.resize((50, 50))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        rs_img.resize((50, 50))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None), \
        f"Error mismatch: PIL={pil_error} RSPIL={rs_error}"
    if pil_error is not None:
        assert pil_error[0] == rs_error[0], \
            f"Error type mismatch: PIL={pil_error[0]} RSPIL={rs_error[0]}"


@pytest.mark.covers("Image.convert", mode="CMYK", target="cpu", variant="error_parity")
def test_convert_from_rgb_to_cmyk(PIL):
    """PIL supports RGB→CMYK conversion. Verify parity."""
    pil_img = PILImage.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    pil_cmyk = pil_img.convert("CMYK")
    rs_cmyk = rs_img.convert("CMYK")
    assert_images_equal(rs_cmyk, pil_cmyk)


@pytest.mark.covers("Image.convert", mode="YCbCr", target="cpu", variant="error_parity")
def test_convert_from_rgb_to_ycbcr(PIL):
    """PIL supports RGB→YCbCr conversion. Verify parity."""
    pil_img = PILImage.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    pil_result = pil_img.convert("YCbCr")
    rs_result = rs_img.convert("YCbCr")
    assert_images_equal(rs_result, pil_result)


@pytest.mark.covers("Image.convert", mode="HSV", target="cpu", variant="error_parity")
def test_convert_from_rgb_to_hsv(PIL):
    """PIL supports RGB→HSV conversion. Verify parity."""
    pil_img = PILImage.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    pil_result = pil_img.convert("HSV")
    rs_result = rs_img.convert("HSV")
    assert_images_equal(rs_result, pil_result)


@pytest.mark.covers("Image.new", mode="CMYK", target="cpu", variant="error_parity")
def test_new_cmyk_raises(PIL):
    """Verify pillow-rs matches PIL behavior for Image.new('CMYK', ...)"""
    try:
        PILImage.new("CMYK", (100, 100))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        Image.new("CMYK", (100, 100))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None), \
        f"Error mismatch: PIL={pil_error} RSPIL={rs_error}"


@pytest.mark.covers("Image.new", mode="YCbCr", target="cpu", variant="error_parity")
def test_new_ycbcr_raises(PIL):
    try:
        PILImage.new("YCbCr", (100, 100))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        Image.new("YCbCr", (100, 100))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None)


@pytest.mark.covers("Image.new", mode="HSV", target="cpu", variant="error_parity")
def test_new_hsv_raises(PIL):
    try:
        PILImage.new("HSV", (100, 100))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        Image.new("HSV", (100, 100))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None)


@pytest.mark.covers("Image.new", mode="I", target="cpu", variant="error_parity")
def test_new_I_raises(PIL):
    try:
        PILImage.new("I", (100, 100))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        Image.new("I", (100, 100))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None)


@pytest.mark.covers("Image.new", mode="F", target="cpu", variant="error_parity")
def test_new_F_raises(PIL):
    try:
        PILImage.new("F", (100, 100))
        pil_error = None
    except Exception as e:
        pil_error = (type(e).__name__, str(e))
    try:
        Image.new("F", (100, 100))
        rs_error = None
    except Exception as e:
        rs_error = (type(e).__name__, str(e))
    assert (pil_error is None) == (rs_error is None)
```

- [ ] **Step 2: Run tests — some may fail if implementation doesn't handle mode**

```bash
python -m pytest tests/test_error_parity.py -v
```
Expected: convert tests pass (CMYK/YCbCr/HSV conversion works), new() tests may fail if implementation doesn't raise the right error.

- [ ] **Step 3: If tests fail, fix implementation**

If `Image.new("CMYK", ...)` doesn't raise the same error as PIL, edit `pillow-rs/src/image.rs` in the `Image::new()` method to add the error case.

- [ ] **Step 4: Commit**

```bash
git add tests/test_error_parity.py
git commit -m "feat: add error-parity tests for unsupported mode×operation combos
Tests verify that when PIL throws for an unsupported mode/operation
combination, pillow-rs throws the same error type. Covers CMYK, YCbCr,
HSV, I, F modes for Image.new and Image.convert."
```

---

### Task 7: Format × Mode I/O tests

**Files:**
- Modify: `tests/test_image_io.py`

- [ ] **Step 1: Add format×mode roundtrip tests**

For each format-mode combo in matrix 4M of the spec:

```python
@pytest.mark.covers("Image.open", mode="RGB", target="cpu", variant="png_roundtrip")
def test_open_save_png_rgb_roundtrip(PIL, tmp_path):
    pil_img = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    path = str(tmp_path / "test.png")
    pil_img.save(path)
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert_images_equal(rs_loaded, pil_loaded)

@pytest.mark.covers("Image.save", mode="L", target="cpu", variant="png_roundtrip")
def test_save_png_l_roundtrip(PIL, tmp_path):
    pil_img = PIL.Image.new("L", (50, 50), 128)
    rs_img = Image.new("L", (50, 50), 128)
    path = str(tmp_path / "test_l.png")
    pil_img.save(path)
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert_images_equal(rs_loaded, pil_loaded)

@pytest.mark.covers("Image.save", mode="RGBA", target="cpu", variant="png_roundtrip")
def test_save_png_rgba_roundtrip(PIL, tmp_path):
    pil_img = PIL.Image.new("RGBA", (50, 50), (255, 0, 0, 128))
    rs_img = Image.new("RGBA", (50, 50), (255, 0, 0, 128))
    path = str(tmp_path / "test_rgba.png")
    pil_img.save(path)
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert_images_equal(rs_loaded, pil_loaded)

@pytest.mark.covers("Image.save", mode="RGB", target="cpu", variant="jpeg_roundtrip")
def test_save_jpeg_rgb_roundtrip(PIL, tmp_path):
    pil_img = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    path = str(tmp_path / "test.jpg")
    pil_img.save(path)
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert_images_equal(rs_loaded, pil_loaded, tolerance=15)  # JPEG is lossy

@pytest.mark.covers("Image.open", mode="RGB", target="cpu", variant="gif_roundtrip")
def test_open_save_gif_rgb_roundtrip(PIL, tmp_path):
    pil_img = PIL.Image.new("RGB", (50, 50), (255, 0, 0))
    rs_img = Image.new("RGB", (50, 50), (255, 0, 0))
    path = str(tmp_path / "test.gif")
    pil_img.save(path)
    pil_loaded = PIL.Image.open(path)
    rs_loaded = Image.open(path)
    assert_images_equal(rs_loaded, pil_loaded)
```

Add tests for all combinations from matrix 4M:
- PNG: L, LA, RGB, RGBA, 1, P
- JPEG: L, RGB, CMYK
- GIF: L, RGB, 1, P
- BMP: L, RGB, RGBA, 1, P
- TIFF: L, LA, RGB, RGBA, 1, P, CMYK
- WEBP: L, RGB, RGBA

- [ ] **Step 2: Run I/O tests**

```bash
python -m pytest tests/test_image_io.py -v -k "roundtrip"
```
Expected: all format×mode roundtrips pass.

- [ ] **Step 3: Commit**

```bash
git add tests/test_image_io.py
git commit -m "feat: add format×mode roundtrip tests for I/O operations
Covers PNG, JPEG, GIF, BMP, TIFF, WEBP across L, LA, RGB, RGBA, 1, P, CMYK.
Each test saves a PIL image, loads with both PIL and RSPIL, asserts parity."
```

---

### Task 8: Build JS test fixture generator

**Files:**
- Create: `scripts/generate_wasm_fixtures.py`

- [ ] **Step 1: Write fixture generator**

```python
#!/usr/bin/env python3
"""Generate WASM test fixtures from PIL reference outputs.

For each (operation, mode, target) combination that has a WASM target,
run the PIL reference operation and hash the output.
JS tests load these fixtures and compare WASM output hashes.

Usage: python scripts/generate_wasm_fixtures.py [--target wasm|wasm_gpu]
"""
import sys, json, hashlib, yaml
from pathlib import Path
from io import BytesIO

ROOT = Path(__file__).parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
FIXTURES_DIR = ROOT / "pillow-rs-js" / "tests" / "fixtures"

import PIL.Image as PILImage
import PIL.ImageOps as PILImageOps
import PIL.ImageChops as PILImageChops
import PIL.ImageFilter as PILFilter
import PIL.ImageEnhance as PILImageEnhance


def run_pil_operation(op_name, mode, variant_key):
    """Run a PIL operation and return the output image bytes."""
    size = (100, 100)

    # Create input image
    if mode in ("L", "LA", "1"):
        color = 128
    elif mode == "P":
        # Create RGB then convert to P
        img = PILImage.new("RGB", size, (255, 0, 0))
        return _run_op(img.convert("P"), op_name, variant_key)
    else:
        color = (255, 0, 0)
        if mode == "RGBA":
            color = (255, 0, 0, 255)
        elif mode == "CMYK":
            color = None  # will convert

    if color is not None:
        try:
            img = PILImage.new(mode, size, color)
        except Exception:
            return None  # PIL doesn't support this mode for new()
    else:
        img = PILImage.new("RGB", size, (255, 0, 0)).convert(mode)

    return _run_op(img, op_name, variant_key)


def _run_op(img, op_name, variant_key):
    """Execute the PIL operation and return bytes."""
    try:
        module, func = op_name.rsplit(".", 1)
        if module == "Image":
            result = getattr(img, func)() if variant_key == "default" else _dispatch(img, func, variant_key)
        elif module == "ImageOps":
            result = getattr(PILImageOps, func)(img)
        elif module == "ImageChops":
            img2 = PILImage.new(img.mode, img.size, (128, 128, 128) if img.mode == "RGB" else 128)
            result = getattr(PILImageChops, func)(img, img2)
        elif module == "ImageFilter":
            filt = getattr(PILFilter, func)
            result = img.filter(filt)
        elif module == "ImageEnhance":
            enhancer_cls = getattr(PILImageEnhance, func)
            result = enhancer_cls(img).enhance(1.5)
        elif module == "ImageModule":
            if func == "merge":
                bands = img.split()
                result = PILImage.merge(img.mode, bands)
            elif func == "effect_noise":
                result = PILImage.effect_noise(img.size, 10)
            else:
                return None
        else:
            return None

        buf = BytesIO()
        result.save(buf, format="PNG")
        return buf.getvalue()
    except Exception:
        return None


def _dispatch(img, func, variant_key):
    """Handle variant-specific dispatches."""
    if func == "resize":
        if "bilinear" in variant_key:
            return img.resize((50, 50), PILImage.BILINEAR)
        elif "nearest" in variant_key:
            return img.resize((50, 50), PILImage.NEAREST)
        elif "bicubic" in variant_key:
            return img.resize((50, 50), PILImage.BICUBIC)
        elif "lanczos" in variant_key:
            return img.resize((50, 50), PILImage.LANCZOS)
        return img.resize((50, 50))
    if func == "rotate":
        angle = int(variant_key.split("_")[-1]) if "angle" in variant_key else 90
        return img.rotate(angle)
    if func == "transpose":
        method_map = {
            "FLIP_LEFT_RIGHT": PILImage.FLIP_LEFT_RIGHT,
            "FLIP_TOP_BOTTOM": PILImage.FLIP_TOP_BOTTOM,
            "ROTATE_90": PILImage.ROTATE_90,
            "ROTATE_180": PILImage.ROTATE_180,
            "ROTATE_270": PILImage.ROTATE_270,
            "TRANSPOSE": PILImage.TRANSPOSE,
            "TRANSVERSE": PILImage.TRANSVERSE,
        }
        method_name = variant_key.split("=")[-1] if "=" in variant_key else variant_key
        return img.transpose(method_map.get(method_name, PILImage.FLIP_LEFT_RIGHT))
    if func == "filter":
        filt = getattr(PILFilter, variant_key.split("=")[-1] if "=" in variant_key else variant_key.upper(), None)
        if filt:
            return img.filter(filt)
        return img.filter(PILFilter.BLUR)
    if func == "crop":
        return img.crop((25, 25, 75, 75))
    if func == "convert":
        target_mode = variant_key.split("=")[-1] if "=" in variant_key else "L"
        return img.convert(target_mode)
    if func == "paste":
        # Create a small image to paste
        paste_img = PILImage.new(img.mode, (25, 25), (0, 255, 0) if img.mode == "RGB" else 0)
        copy_img = img.copy()
        copy_img.paste(paste_img, (0, 0))
        return copy_img
    return img  # default: return unchanged


def main():
    target_filter = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--target" else None

    with open(MANIFEST_PATH) as f:
        manifest = yaml.safe_load(f)

    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    manifest_data = {"operations": {}}
    count = 0

    for mod_name, mod_def in manifest["modules"].items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict) or item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = item.get("supported_modes", [])
                targets = item.get("supported_targets", ["cpu"])
                if not modes:
                    continue

                for mode in modes:
                    for target in targets:
                        if target not in ("wasm", "wasm_gpu"):
                            continue
                        if target_filter and target != target_filter:
                            continue
                        # Only generate for the default variant for now
                        variant_key = "default"
                        data = run_pil_operation(op_name, mode, variant_key)
                        if data is None:
                            continue
                        h = hashlib.sha256(data).hexdigest()
                        fixture_name = f"{op_name.replace('.', '_')}_{mode}_{variant_key}.json"
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "target": target,
                            "variant": variant_key,
                            "expectedHash": h,
                        }
                        with open(FIXTURES_DIR / fixture_name, "w") as f_out:
                            json.dump(fixture, f_out, indent=2)
                        manifest_data["operations"][fixture_name] = fixture
                        count += 1

    # Write manifest
    with open(FIXTURES_DIR / "manifest.json", "w") as f:
        json.dump(manifest_data, f, indent=2)

    print(f"Generated {count} fixtures in {FIXTURES_DIR}")

if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run fixture generator**

```bash
python scripts/generate_wasm_fixtures.py
```
Expected: generates JSON files in `pillow-rs-js/tests/fixtures/`

- [ ] **Step 3: Commit**

```bash
git add scripts/generate_wasm_fixtures.py pillow-rs-js/tests/fixtures/
git commit -m "feat: add WASM test fixture generator
Generates JSON fixtures with pre-computed PIL reference hashes
for every (op × mode × target=wasm/wasm_gpu) combination.
JS tests load fixtures and compare WASM output hashes."
```

---

### Task 9: Add WASM JS tests with @covers JSDoc

**Files:**
- Modify: `pillow-rs-js/tests/` (existing test files or create new ones)

- [ ] **Step 1: Write JS test structure with @covers annotations**

```javascript
// pillow-rs-js/tests/parity.test.js
const { readFileSync } = require('fs');
const { join } = require('path');
const init, { Image } = require('../pkg/pillow_rs_js.js');

const FIXTURES_DIR = join(__dirname, 'fixtures');

async function loadFixture(name) {
    const data = JSON.parse(readFileSync(join(FIXTURES_DIR, name), 'utf8'));
    return data;
}

beforeAll(async () => {
    await init();
});

/**
 * @covers Image.resize
 * @mode RGB
 * @target wasm
 * @variant default
 */
test('resize RGB default', async () => {
    const fixture = await loadFixture('Image_resize_RGB_default.json');
    const img = Image.new(100, 100, 3); // RGB
    img.fill(255, 0, 0);
    const result = img.resize(50, 50);
    const hash = result.hash();
    expect(hash).toBe(fixture.expectedHash);
});

/**
 * @covers Image.crop
 * @mode RGB
 * @target wasm
 * @variant default
 */
test('crop RGB default', async () => {
    const fixture = await loadFixture('Image_crop_RGB_default.json');
    const img = Image.new(100, 100, 3);
    img.fill(255, 0, 0);
    const result = img.crop(25, 25, 75, 75);
    const hash = result.hash();
    expect(hash).toBe(fixture.expectedHash);
});

/**
 * @covers ImageOps.invert
 * @mode RGB
 * @target wasm
 * @variant default
 */
test('invert RGB default', async () => {
    const fixture = await loadFixture('ImageOps_invert_RGB_default.json');
    const img = Image.new(100, 100, 3);
    img.fill(128, 128, 128);
    const result = img.ops_invert();
    const hash = result.hash();
    expect(hash).toBe(fixture.expectedHash);
});
```

- [ ] **Step 2: Run JS tests**

```bash
cd pillow-rs-js && node --experimental-wasm-modules tests/parity.test.js
```
Expected: fixture-based parity tests pass.

- [ ] **Step 3: Run validate_coverage.py — verify JS markers are detected**

```bash
python scripts/validate_coverage.py manifest.yaml
```
Expected: JS markers show up in actual set, WASM gaps decrease.

- [ ] **Step 4: Commit**

```bash
git add pillow-rs-js/tests/
git commit -m "feat: add WASM JS parity tests with @covers JSDoc annotations
Fixture-based: each test loads a pre-computed PIL reference hash,
runs the WASM operation, and compares output hashes.
@covers JSDoc tags are parsed by validate_coverage.py."
```

---

### Task 10: Integrate into CI

**Files:**
- Create: `scripts/ci_coverage.sh`
- Determine: CI config file (`.github/workflows/` or similar)

- [ ] **Step 1: Write CI coverage script**

```bash
#!/bin/bash
# scripts/ci_coverage.sh — complete CI pipeline with coverage validation
set -e

echo "=== 1. Rust core tests ==="
cargo test -p pillow-rs

echo "=== 2. Python parity tests ==="
python -m pytest tests/ -q --json-report --json-report-file=/tmp/report.json

echo "=== 3. JS/WASM tests ==="
if [ -f pillow-rs-js/tests/run.mjs ]; then
    node pillow-rs-js/tests/run.mjs
fi

echo "=== 4. Coverage validation ==="
python scripts/validate_coverage.py manifest.yaml

echo "=== 5. Generate coverage reports ==="
python scripts/generate_coverage_page.py
python scripts/generate_wasm_coverage.py

echo ""
echo "✅ All checks passed — coverage matrix complete"
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/ci_coverage.sh
```

- [ ] **Step 3: Run locally**

```bash
bash scripts/ci_coverage.sh
```
Expected: all steps pass (or if gaps remain, step 4 exits 1 with gap report).

- [ ] **Step 4: Commit**

```bash
git add scripts/ci_coverage.sh
git commit -m "feat: add CI coverage pipeline script
Runs: cargo test → pytest → JS/WASM tests → validate_coverage.py → generate reports.
Exits 1 on any gap, preventing incomplete coverage from merging."
```

---

### Task 11: Update COVERAGE.md and COVERAGE_WASM.md generators

**Files:**
- Modify: `scripts/generate_coverage_page.py`
- Modify: `scripts/generate_wasm_coverage.py`

- [ ] **Step 1: Update generate_coverage_page.py**

Replace the `FUNC_MAP` dict and `infer_functions` with manifest-driven marker scanning (same logic as `validate_coverage.py`). Add mode matrix table generation.

Add to `generate_coverage_page.py` after the existing generate function:

```python
def generate_mode_matrix(manifest, python_set):
    """Generate per-module mode×operation matrix tables."""
    lines = []
    lines.append("\n## Mode × Operation Coverage Matrix\n")
    lines.append("| Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |")
    lines.append("|-----------|---|---|-----|------|---|---|------|-------|-----|---|---|")

    for mod_name, mod_def in manifest["modules"].items():
        lines.append(f"\n### {mod_name}\n")
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict) or item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = item.get("supported_modes", [])
                if not modes:
                    continue
                # Check which modes have tests
                row = [f"| `{item['name']}` |"]
                all_modes = ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"]
                for mode in all_modes:
                    if mode not in modes:
                        # PIL doesn't support this mode for this op
                        row.append(" N/A |")
                    else:
                        # Check if we have a test
                        has_test = any(
                            cp.op == op_name and cp.mode == mode
                            for cp in python_set
                        )
                        if has_test:
                            row.append(" ✅ |")
                        else:
                            row.append(" ⬜ |")
                lines.append("".join(row))
    return "\n".join(lines)
```

- [ ] **Step 2: Update generate_wasm_coverage.py**

Add WASM-specific mode matrix and multi-target comparison table:

```python
def generate_wasm_mode_matrix(manifest, js_set):
    """Generate WASM mode×operation matrix."""
    # Same structure as mode matrix but filtered to wasm/wasm_gpu targets
    ...
```

- [ ] **Step 3: Regenerate coverage docs**

```bash
python scripts/generate_coverage_page.py
python scripts/generate_wasm_coverage.py
```
Expected: `docs/COVERAGE.md` and `docs/COVERAGE_WASM.md` now contain mode×operation matrices.

- [ ] **Step 4: Commit**

```bash
git add scripts/generate_coverage_page.py scripts/generate_wasm_coverage.py docs/COVERAGE.md docs/COVERAGE_WASM.md
git commit -m "feat: add mode×operation matrices to coverage docs
COVERAGE.md and COVERAGE_WASM.md now include per-module tables
showing which modes are tested (✅), untested (⬜), or PIL-unsupported (N/A)."
```

---

### Task 12: Final validation

**Files:**
- (none new — verification only)

- [ ] **Step 1: Run full CI pipeline**

```bash
bash scripts/ci_coverage.sh
```
Expected: all tests pass, coverage validation passes, coverage docs generated.

- [ ] **Step 2: Run clippy and format check**

```bash
bash scripts/lint.sh
```
Expected: `cargo fmt --check` passes, `cargo clippy` with `-D warnings` passes.

- [ ] **Step 3: Review remaining gaps**

```bash
python scripts/validate_coverage.py manifest.yaml
```
Any remaining gaps are documented in the coverage matrix as ⬜. Either:
- Add the missing tests
- Or update manifest to reflect that the operation doesn't support that mode

- [ ] **Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final validation — all CI checks pass"
```

---

## Post-Implementation Verification

After all tasks are complete:

1. `pytest tests/ --collect-only` — zero MISSING @covers errors
2. `pytest tests/ -q` — all tests pass
3. `python scripts/validate_coverage.py manifest.yaml` — exits 0
4. `bash scripts/ci_coverage.sh` — all steps pass
5. `docs/COVERAGE.md` — contains mode×operation matrices
6. `docs/COVERAGE_WASM.md` — contains WASM mode×operation matrices
7. `git log --oneline` — 12+ clean commits, one per task
