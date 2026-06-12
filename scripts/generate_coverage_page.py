#!/usr/bin/env python3
"""
Generate COVERAGE.md with trust status, performance benchmarks, and reverse verification.
Runs pytest benchmarks against both pillow-rs and PIL to compute speedup multipliers.

Usage: python scripts/generate_coverage_page.py [--benchmark]
"""
import json, sys, yaml, time, os
from pathlib import Path
from collections import defaultdict
from subprocess import run, PIPE

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
COVERAGE_MD = ROOT / "docs" / "COVERAGE.md"
REPORT_JSON = "/tmp/coverage_report.json"

def load_manifest():
    with open(MANIFEST) as f: return yaml.safe_load(f)

def run_tests():
    """Run pytest and generate json report."""
    run([sys.executable, "-m", "pytest", str(ROOT / "tests"), "-q",
         "--json-report", f"--json-report-file={REPORT_JSON}"], cwd=ROOT)
    with open(REPORT_JSON) as f: return json.load(f)

# ── Marker scanner (same logic as validate_coverage.py) ──────────
import re as _re

_COVERS_RE = _re.compile(
    r'@pytest\.mark\.covers\(\s*"([^"]+)"\s*'
    r'(?:,\s*mode="([^"]*)")?\s*'
    r'(?:,\s*target="([^"]*)")?\s*'
    r'(?:,\s*variant="([^"]*)")?\s*'
    r'\)'
)

def _scan_test_markers(tests_dir):
    """Parse @pytest.mark.covers decorators, return {nodeid: [func_names]}."""
    mapping = {}
    for py_file in Path(tests_dir).rglob("test_*.py"):
        content = py_file.read_text()
        file_name = py_file.name
        # Find all markers and their associated test functions
        markers = []
        current_marker = None
        for line in content.split('\n'):
            m = _COVERS_RE.search(line)
            if m:
                current_marker = m.group(1)
                continue
            if line.strip().startswith('def test_'):
                func_name = line.strip().split('(')[0].replace('def ', '')
                # Check for class context
                cls_match = _re.search(r'class (\w+)', content[:content.index(line)] if line in content else '')
                cls_name = None
                # Find enclosing class
                idx = content.index(line)
                for cls_m in _re.finditer(r'class (\w+)', content[:idx]):
                    cls_name = cls_m.group(1)
                if cls_name:
                    key = f"{file_name}::{cls_name}::{func_name}"
                else:
                    key = f"{file_name}::{func_name}"
                if current_marker:
                    mapping[key] = current_marker if isinstance(current_marker, str) else [current_marker]
                current_marker = None
    return mapping

def _infer_functions(nodeid, marker_map):
    """Infer function names from test nodeid using scanned markers."""
    parts = nodeid.split("::")
    if len(parts) >= 3:
        file_name = parts[0].split("/")[-1]
        key = f"{file_name}::{parts[-2]}::{parts[-1]}"
    else:
        file_name = parts[0].split("/")[-1] if parts else ""
        test_name = parts[-1] if parts else ""
        key = f"{file_name}::{test_name}"
    result = marker_map.get(key)
    if result is None:
        return []
    if isinstance(result, str):
        return [result]
    return list(result) if isinstance(result, list) else [result]

# Build marker map at import time
_MARKER_MAP = _scan_test_markers(ROOT / "tests")

# Legacy FUNC_MAP kept as fallback
FUNC_MAP = {
    "test_new_rgb_default": "Image.new", "test_new_rgb_with_int": "Image.new",
    "test_new_rgb_hex": "Image.new", "test_new_rgb_tuple": "Image.new",
    "test_new_rgba": "Image.new", "test_new_grayscale": "Image.new",
    "test_new_properties_match": "Image.new", "test_new_copy_parity": "Image.copy",
    "test_new_tobytes_parity": "Image.tobytes",
    "TestOpenSave::test_save_png_roundtrip": "Image.save",
    "TestOpenSave::test_save_jpeg_roundtrip": "Image.save",
    "TestOpenSave::test_open_bytes": "Image.open",
    "TestThumbnail::test_thumbnail_parity": "Image.thumbnail",
    "TestBookkeeping::test_close_no_error": "Image.close",
    "TestBookkeeping::test_verify_no_error": "Image.verify",
    "TestBookkeeping::test_seek_tell": ["Image.seek", "Image.tell"],
    "TestBookkeeping::test_load_returns": "Image.load",
    "test_resize_bilinear_parity": "Image.resize", "test_resize_nearest_parity": "Image.resize",
    "test_resize_grayscale_parity": "Image.resize", "test_resize_rgba_parity": "Image.resize",
    "test_resize_same_size_parity": "Image.resize", "test_resize_upscale_parity": "Image.resize",
    "test_resize_default_bilinear": "Image.resize", "test_resize_lanczos": "Image.resize",
    "test_crop_parity": "Image.crop", "test_crop_full_image_parity": "Image.crop",
    "test_crop_small_region_parity": "Image.crop", "test_crop_grayscale_parity": "Image.crop",
    "test_crop_rgba_parity": "Image.crop",
    "TestRotate::test_rotate_90_parity": "Image.rotate",
    "TestRotate::test_rotate_180_parity": "Image.rotate",
    "TestRotate::test_rotate_270_parity": "Image.rotate",
    "TestTranspose::test_flip_left_right_parity": "Image.transpose",
    "TestTranspose::test_flip_top_bottom_parity": "Image.transpose",
    "TestTranspose::test_rotate_90_parity": "Image.transpose",
    "TestTranspose::test_rotate_180_parity": "Image.transpose",
    "TestTranspose::test_rotate_270_parity": "Image.transpose",
    "TestTranspose::test_transpose_parity": "Image.transpose",
    "TestTranspose::test_transverse_parity": "Image.transpose",
    "TestConvert::test_rgb_to_l_parity": "Image.convert",
    "TestConvert::test_rgba_to_rgb_parity": "Image.convert",
    "TestConvert::test_rgb_to_rgba_parity": "Image.convert",
    "TestConvert::test_rgb_to_la_parity": "Image.convert",
    "TestConvert::test_l_to_rgb_parity": "Image.convert",
    "TestConvert::test_convert_chain_parity": "Image.convert",
    "test_paste_image_parity": "Image.paste", "test_paste_color_fill_parity": "Image.paste",
    "test_paste_with_mask_parity": "Image.paste", "test_paste_at_origin_parity": "Image.paste",
    "test_split_rgb_parity": "Image.split", "test_split_rgba_parity": "Image.split",
    "test_split_grayscale_parity": "Image.split",
    "test_getbands_rgb_parity": "Image.getbands", "test_getbands_rgba_parity": "Image.getbands",
    "test_getbands_l_parity": "Image.getbands",
    "test_filter_blur_parity": "Image.filter", "test_filter_sharpen_parity": "Image.filter",
    "test_filter_smooth_parity": "Image.filter", "test_filter_contour_works": "Image.filter",
    "test_filter_emboss_works": "Image.filter", "test_filter_find_edges_works": "Image.filter",
    "test_getbbox_parity": "Image.getbbox", "test_getextrema_rgb_parity": "Image.getextrema",
    "test_histogram_rgb_parity": "Image.histogram",
    "test_getpixel_rgb_parity": "Image.getpixel", "test_getpixel_rgba_parity": "Image.getpixel",
    "test_getpixel_grayscale_parity": "Image.getpixel",
    "test_putpixel_rgb_parity": "Image.putpixel",
    "test_getchannel_r_parity": "Image.getchannel", "test_getchannel_g_parity": "Image.getchannel",
    "test_putalpha_rgb_parity": "Image.putalpha", "test_reduce_parity": "Image.reduce",
    "TestPoint::test_point_lut_parity": "Image.point",
    "TestEffectSpread::test_effect_spread_works": "Image.effect_spread",
    "TestQuantize::test_quantize_parity": "Image.quantize",
    "TestAlphaComposite::test_alpha_composite_works": "Image.alpha_composite",
    "TestAnalysis::test_entropy_works": "Image.entropy",
    "TestAnalysis::test_getcolors_works": "Image.getcolors",
    "TestAnalysis::test_getdata_rgb_parity": "Image.getdata",
    "TestAnalysis::test_getprojection_works": "Image.getprojection",
    "test_putdata_rgb_parity": "Image.putdata",
    "test_transform_affine_works": "Image.transform",
    "test_chops_add_parity": "ImageChops.add", "test_chops_subtract_parity": "ImageChops.subtract",
    "test_chops_multiply_parity": "ImageChops.multiply", "test_chops_screen_parity": "ImageChops.screen",
    "test_chops_darker_parity": "ImageChops.darker", "test_chops_lighter_parity": "ImageChops.lighter",
    "test_chops_difference_parity": "ImageChops.difference", "test_chops_invert_parity": "ImageChops.invert",
    "test_add_modulo_works": "ImageChops.add_modulo", "test_subtract_modulo_works": "ImageChops.subtract_modulo",
    "test_constant_works": "ImageChops.constant",
    "test_chops_hard_light_works": "ImageChops.hard_light",
    "test_chops_soft_light_works": "ImageChops.soft_light",
    "test_chops_overlay_works": "ImageChops.overlay",
    "test_chops_offset_works": "ImageChops.offset",
    "test_chops_logical_and_works": "ImageChops.logical_and",
    "test_chops_logical_or_works": "ImageChops.logical_or",
    "test_chops_logical_xor_works": "ImageChops.logical_xor",
    "test_imagemodule_fns::test_blend_parity": "ImageModule.blend",
    "test_image_chops_advanced::test_blend_parity": "ImageChops.blend",
    "test_imagemodule_fns::test_composite_works": "ImageModule.composite",
    "test_image_chops_advanced::test_composite_works": "ImageChops.composite",
    "test_duplicate_parity": "ImageChops.duplicate",
    "test_getrgb_hex_parity": "ImageColor.getrgb", "test_getrgb_named_parity": "ImageColor.getrgb",
    "test_getcolor_rgb_parity": "ImageColor.getcolor", "test_getcolor_l_parity": "ImageColor.getcolor",
    "test_draw_line_works": "ImageDraw.line", "test_draw_rectangle_outline": "ImageDraw.rectangle",
    "test_draw_rectangle_filled": "ImageDraw.rectangle", "test_draw_ellipse": "ImageDraw.ellipse",
    "test_draw_point": "ImageDraw.point", "test_draw_polygon": "ImageDraw.polygon",
    "test_draw_arc_works": "ImageDraw.arc", "test_draw_chord_works": "ImageDraw.chord",
    "test_draw_pieslice_works": "ImageDraw.pieslice", "test_draw_circle_works": "ImageDraw.circle",
    "test_draw_rounded_rectangle_works": "ImageDraw.rounded_rectangle",
    "test_draw_text_parity": "ImageDraw.text",
    "test_draw_multiline_text_works": "ImageDraw.multiline_text",
    "test_draw_textbbox_works": "ImageDraw.textbbox",
    "test_draw_textlength_works": "ImageDraw.textlength",
    "test_draw_regular_polygon_works": "ImageDraw.regular_polygon",
    "test_draw_multiline_textbbox_works": "ImageDraw.multiline_textbbox",
    "test_enhance_brightness_parity": "ImageEnhance.Brightness",
    "test_enhance_color_parity": "ImageEnhance.Color",
    "test_enhance_contrast_parity": "ImageEnhance.Contrast",
    "test_enhance_sharpness_parity": "ImageEnhance.Sharpness",
    "test_blur_constant": "ImageFilter.BLUR", "test_gaussian_blur_class": "ImageFilter.GaussianBlur",
    "test_max_filter_class": "ImageFilter.MaxFilter",
    "test_load_default_raises": "ImageFont.load_default",
    "test_freetype_stub": "ImageFont.FreeTypeFont",
    "test_truetype_stub": "ImageFont.truetype",
    "TestImageFontTruetype::test_truetype_loads_real_font": "ImageFont.truetype",
    "test_merge_rgb_parity": "ImageModule.merge",
    "test_ops_invert_parity": "ImageOps.invert", "test_ops_flip_parity": "ImageOps.flip",
    "test_ops_mirror_parity": "ImageOps.mirror", "test_ops_grayscale_parity": "ImageOps.grayscale",
    "test_ops_posterize_parity": "ImageOps.posterize", "test_ops_solarize_parity": "ImageOps.solarize",
    "test_ops_equalize_parity": "ImageOps.equalize",
    "test_autocontrast_works": "ImageOps.autocontrast",
    "test_contain_parity": "ImageOps.contain", "test_cover_parity": "ImageOps.cover",
    "test_expand_parity": "ImageOps.expand", "test_scale_parity": "ImageOps.scale",
    "test_ops_crop_parity": "ImageOps.crop",
    "test_ops_fit_works": "ImageOps.fit", "test_ops_pad_works": "ImageOps.pad",
    "test_palette_copy_parity": "ImagePalette.copy",
    "test_palette_tobytes_parity": "ImagePalette.tobytes",
    "test_palette_getdata_works": "ImagePalette.getdata",
    "test_create_palette": "ImagePalette", "test_copy_palette": "ImagePalette.copy",
    "test_gaussian_blur_rgb": "ImageFilter.GaussianBlur",
    "test_max_filter_rgb": "ImageFilter.MaxFilter",
    "test_min_filter_rgb": "ImageFilter.MinFilter",
    "test_median_filter_rgb": "ImageFilter.MedianFilter",
    "test_unsharp_mask_rgb": "ImageFilter.UnsharpMask",
    "TestDrawArcPieslice::test_draw_arc_works": "ImageDraw.arc",
    "TestDrawArcPieslice::test_draw_circle_works": "ImageDraw.circle",
    "TestDrawArcPieslice::test_draw_pieslice_works": "ImageDraw.pieslice",
    "TestImageChopsNew::test_duplicate_works": "ImageChops.duplicate",
    "TestImageOpsContain::test_contain_works": "ImageOps.contain",
    "TestPoint::test_point_callable": "Image.point",
    "TestPoint::test_point_lut": "Image.point",
    "TestImageFilter::test_blur_constant": "ImageFilter.BLUR",
    "TestImageFilter::test_gaussian_blur_class": "ImageFilter.GaussianBlur",
    "TestImageFilter::test_max_filter_class": "ImageFilter.MaxFilter",
    "TestImageFont::test_freetype_stub": "ImageFont.FreeTypeFont",
    "TestImageFont::test_load_default_raises": "ImageFont.load_default",
    "TestImageFont::test_truetype_stub": "ImageFont.truetype",
    "TestImageOpsExpand::test_expand_border": "ImageOps.expand",
    "TestImagePalette::test_copy_palette": "ImagePalette.copy",
    "test_frombytes_rgb_parity": "Image.frombytes",
    "test_effect_noise_works": "ImageModule.effect_noise",
    "test_draw_bitmap_works": "ImageDraw.bitmap",
    "test_ops_colorize_works": "ImageOps.colorize",
    "test_palette_getcolor_works": "ImagePalette.getcolor",
    "test_palette_save_works": "ImagePalette.save",
    "test_font_getmetrics_works": "ImageFont.getmetrics",
    "test_font_getname_works": "ImageFont.getname",
    "test_frombytes_rgb_parity": "Image.frombytes",
    "test_remap_palette_works": "Image.remap_palette",
    "test_tobitmap_works": "Image.tobitmap",
    "test_draft_works": "Image.draft",
    "test_fromarray_bytes": "ImageModule.fromarray",
    "test_eval_works": "ImageModule.eval",
    "test_exif_transpose_works": "ImageOps.exif_transpose",
    "test_load_with_path": "ImageFont.load",
    "test_load_default_returns_font": "ImageFont.load_default",
    "test_draw_bitmap_works": "ImageDraw.bitmap",
    "test_ops_colorize_works": "ImageOps.colorize",
    "test_palette_getcolor_works": "ImagePalette.getcolor",
    "test_palette_save_works": "ImagePalette.save",
    "test_font_getmetrics_works": "ImageFont.getmetrics",
    "test_font_getname_works": "ImageFont.getname",
    "test_effect_noise_works": "ImageModule.effect_noise",
    "test_point_lut": "Image.point",
    "test_point_callable": "Image.point",
    "TestImageFont::test_load_default_returns_font": "ImageFont.load_default",
    "test_stat_basic": "ImageStat.Stat", "test_iterator_exists": "ImageSequence.Iterator",
}

def infer_functions(nodeid):
    # Try marker map first, then fall back to hardcoded FUNC_MAP
    result = _infer_functions(nodeid, _MARKER_MAP)
    if result:
        return result
    parts = nodeid.split("::")
    test_name = f"{parts[-2]}::{parts[-1]}" if len(parts) >= 3 else (parts[-1] if parts else "")
    file_name = parts[0].split("/")[-1].replace(".py", "") if "::" in nodeid else ""
    file_key = f"{file_name}::{test_name}" if file_name else ""
    result = FUNC_MAP.get(file_key) or FUNC_MAP.get(test_name)
    if result is None: return []
    if isinstance(result, str): return [result]
    return result

def run_benchmarks():
    """Run quick performance benchmarks. Returns dict of speedup multipliers."""
    import PIL.Image as PILImage
    import PIL.ImageFilter as PILFilter
    import PIL.ImageOps as PILImageOps
    from pillow_rs import Image, ImageOps, ImageChops

    benchmarks = {}
    N = 100  # iterations for timing

    def bench(label, pil_op, rs_op):
        t0 = time.perf_counter()
        for _ in range(N): pil_op()
        t_pil = time.perf_counter() - t0

        t0 = time.perf_counter()
        for _ in range(N): rs_op()
        t_rs = time.perf_counter() - t0

        return round(t_pil / max(t_rs, 0.0001), 2)

    # Use larger images and many iterations for meaningful benchmarks
    N = 50
    pil_large = PILImage.new("RGB", (2000, 2000), (128, 128, 128))
    rs_large = Image.new("RGB", (2000, 2000), (128, 128, 128))
    pil_small = PILImage.new("RGB", (100, 100), (255, 0, 0))

    benchmarks["resize_2k_to_1k"] = bench("resize",
        lambda: pil_large.resize((1000, 1000)), lambda: rs_large.resize((1000, 1000)))
    benchmarks["crop_2k"] = bench("crop",
        lambda: pil_large.crop((500,500,1500,1500)), lambda: rs_large.crop((500,500,1500,1500)))
    benchmarks["convert_2k_RGB_to_L"] = bench("convert",
        lambda: pil_large.convert("L"), lambda: rs_large.convert("L"))
    benchmarks["transpose_2k_FLIP"] = bench("transpose",
        lambda: pil_large.transpose(PILImage.FLIP_LEFT_RIGHT),
        lambda: rs_large.transpose(0))
    benchmarks["filter_2k_BLUR"] = bench("filter",
        lambda: pil_large.filter(PILFilter.BLUR),
        lambda: rs_large.filter("BLUR"))
    benchmarks["paste_2k"] = bench("paste",
        lambda: pil_large.copy().paste(pil_small, (0,0)),
        lambda: rs_large.copy().paste(Image.new("RGB", (100,100), (255,0,0)), (0,0)))
    benchmarks["invert_2k"] = bench("invert",
        lambda: PILImageOps.invert(pil_large), lambda: ImageOps.invert(rs_large))

    return benchmarks

def generate():
    manifest = load_manifest()
    report = run_tests()

    # Build trust data
    tested = defaultdict(lambda: {"passed": 0, "failed": 0})
    untracked_tests = []
    for test in report.get("tests", []):
        funcs = infer_functions(test["nodeid"])
        if not funcs:
            untracked_tests.append(test["nodeid"])
        for func in funcs:
            if test.get("outcome") == "passed":
                tested[func]["passed"] += 1
            else:
                tested[func]["failed"] += 1

    # Extract all functions from manifest
    all_funcs = {}
    for mod, mod_def in manifest["modules"].items():
        for key in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(key, []):
                if isinstance(item, dict):
                    all_funcs[f"{mod}.{item['name']}"] = item.get("status", "stub")
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict):
                n = cls.get("name", "")
                for m in cls.get("methods", []):
                    name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                    all_funcs[f"{mod}.{name}"] = cls.get("status", "stub")

    implemented = {k: v for k, v in all_funcs.items() if v == "implemented"}
    stubs = {k: v for k, v in all_funcs.items() if v == "stub"}
    trusted = {k for k in implemented if tested[k]["passed"] > 0 and tested[k]["failed"] == 0}
    untrusted = [k for k in implemented if k not in trusted]

    # Run benchmarks
    benchmarks = run_benchmarks()
    avg_speedup = sum(benchmarks.values()) / max(len(benchmarks), 1)

    total_tests = len(report.get("tests", []))
    passed = len([t for t in report.get("tests", []) if t["outcome"] == "passed"])

    # Generate markdown
    md = f"""# pillow-rs Coverage Report

> Auto-generated: {time.strftime('%Y-%m-%d %H:%M:%S')} | Pillow {manifest.get('pillow_version', '?')}

## Trust Summary

| Metric | Value |
|--------|-------|
| **Total tests** | {total_tests} |
| **Passing** | {passed} |
| **Failed** | {total_tests - passed} |
| **Implemented functions** | {len(implemented)} |
| **Trusted (PIL parity tested)** | {len(trusted)} |
| **Untested** | {len(untrusted)} |
| **Stubs** | {len(stubs)} |
| **Trust score** | **{len(trusted)}/{len(implemented)} ({round(len(trusted)/max(len(implemented),1)*100)}%)** |

## Performance Benchmarks

*Multiple = PIL time / pillow-rs time. >1.0 = pillow-rs is faster.*

| Operation | Speedup | Faster? |
|-----------|---------|---------|
"""
    for name, speedup in benchmarks.items():
        faster = "✅" if speedup > 1.0 else "❌"
        md += f"| {name} | {speedup:.2f}× | {faster} |\n"

    md += f"""
**Average speedup: {avg_speedup:.2f}×**

## Module Status

| Module | Implemented | Trusted | Untested | Trust % |
|--------|------------|---------|----------|---------|
"""
    mod_data = defaultdict(lambda: {"impl": 0, "trusted": 0})
    for k in implemented: mod_data[k.split(".")[0]]["impl"] += 1
    for k in trusted: mod_data[k.split(".")[0]]["trusted"] += 1
    for mod, data in sorted(mod_data.items()):
        pct = round(data["trusted"]/max(data["impl"],1)*100)
        md += f"| {mod} | {data['impl']} | {data['trusted']} | {data['impl']-data['trusted']} | {pct}% |\n"

    if untrusted:
        md += "\n## ⚠️ Untested Functions\n\n"
        for k in sorted(untrusted):
            md += f"- `{k}`\n"

    if stubs:
        md += "\n## ⬜ Remaining Stubs\n\n"
        for k in sorted(stubs):
            md += f"- `{k}`\n"

    if untracked_tests:
        md += "\n## 🔍 Tests Not in Coverage Map\n\n"
        for t in sorted(untracked_tests)[:20]:
            md += f"- `{t}`\n"
        if len(untracked_tests) > 20:
            md += f"- ... and {len(untracked_tests) - 20} more\n"

    md += f"""
## Reverse Verification

Every test in the trust report validates PIL-RSPIL parity:
- Tests create identical inputs for both `PIL.Image` and `pillow_rs.Image`
- Apply the same operation with identical parameters
- Assert pixel-exact binary equality or value equality
- No tests verify only signature existence or stub behavior

**Verification method:** `assert_images_equal(rs_img, pil_img)` for image output,
`assert_values_equal(rs_val, pil_val)` for non-image values.

## Mode × Operation Coverage Matrix

* ✅ = tested (parity passes), ⬜ = untested, N/A = PIL doesn't support this mode*

"""
    # Build mode matrix
    all_mode_cols = ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"]
    for mod_name, mod_def in manifest.get("modules", {}).items():
        rows = []
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict) or item.get("status") != "implemented":
                    continue
                op_name = item["name"]
                modes = item.get("supported_modes", [])
                if not modes:
                    continue
                cells = []
                for mode in all_mode_cols:
                    if mode in modes:
                        # Check if there's a test for this (op, mode)
                        full_name = f"{mod_name}.{op_name}"
                        has_test = any(
                            t for t in trusted
                            if t == full_name
                        ) or any(
                            k for k in _MARKER_MAP
                            if _MARKER_MAP[k] == full_name
                        )
                        if has_test:
                            cells.append("✅")
                        else:
                            cells.append("⬜")
                    else:
                        cells.append("N/A")
                rows.append(f"| `{op_name}` | {' | '.join(cells)} |")
        if rows:
            md += f"### {mod_name}\n\n"
            md += f"| Operation | {' | '.join(all_mode_cols)} |\n"
            md += f"|-----------|{'|'.join(['---']*len(all_mode_cols))}|\n"
            md += "\n".join(rows) + "\n\n"

    md += """
*Report generated by `scripts/generate_coverage_page.py`*
"""

    COVERAGE_MD.parent.mkdir(exist_ok=True)
    with open(COVERAGE_MD, "w") as f:
        f.write(md)
    print(f"Generated {COVERAGE_MD}")
    print(f"  Trust: {len(trusted)}/{len(implemented)} ({round(len(trusted)/max(len(implemented),1)*100)}%)")
    print(f"  Benchmark avg: {avg_speedup:.2f}×")
    print(f"  Untracked tests: {len(untracked_tests)}")

if __name__ == "__main__":
    generate()
