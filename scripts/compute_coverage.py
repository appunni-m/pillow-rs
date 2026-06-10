#!/usr/bin/env python3
"""
Trust-based coverage: a function is TRUSTED only if it has PIL parity tests.
Binary metric — either it's tested or it's not. No weighted formulas.

Usage: python scripts/compute_coverage.py manifest.yaml report.json
"""
import json, sys, yaml
from pathlib import Path
from collections import defaultdict

def load_manifest(path):
    with open(path) as f: return yaml.safe_load(f)

def load_report(path):
    with open(path) as f: return json.load(f)

# ── Test name → manifest function mapping ──────────────────────

FUNC_MAP = {
    # Image — constructors
    "test_new_rgb_default": "Image.new", "test_new_rgb_with_int": "Image.new",
    "test_new_rgb_hex": "Image.new", "test_new_rgb_tuple": "Image.new",
    "test_new_rgba": "Image.new", "test_new_grayscale": "Image.new",
    "test_new_properties_match": "Image.new", "test_new_copy_parity": "Image.copy",
    "test_new_tobytes_parity": "Image.tobytes",
    # Image — IO
    "TestOpenSave::test_save_png_roundtrip": "Image.save",
    "TestOpenSave::test_save_jpeg_roundtrip": "Image.save",
    "TestOpenSave::test_open_bytes": "Image.open",
    "TestThumbnail::test_thumbnail_parity": "Image.thumbnail",
    "TestBookkeeping::test_close_no_error": "Image.close",
    "TestBookkeeping::test_verify_no_error": "Image.verify",
    "TestBookkeeping::test_seek_tell": ["Image.seek", "Image.tell"],
    "TestBookkeeping::test_load_returns": "Image.load",
    # Image — resize
    "test_resize_bilinear_parity": "Image.resize", "test_resize_nearest_parity": "Image.resize",
    "test_resize_grayscale_parity": "Image.resize", "test_resize_rgba_parity": "Image.resize",
    "test_resize_same_size_parity": "Image.resize", "test_resize_upscale_parity": "Image.resize",
    "test_resize_default_bilinear": "Image.resize", "test_resize_lanczos": "Image.resize",
    # Image — crop/rotate/transpose (class-qualified)
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
    # Image — convert
    "TestConvert::test_rgb_to_l_parity": "Image.convert",
    "TestConvert::test_rgba_to_rgb_parity": "Image.convert",
    "TestConvert::test_rgb_to_rgba_parity": "Image.convert",
    "TestConvert::test_rgb_to_la_parity": "Image.convert",
    "TestConvert::test_l_to_rgb_parity": "Image.convert",
    "TestConvert::test_convert_chain_parity": "Image.convert",
    # Image — paste/split/getbands
    "test_paste_image_parity": "Image.paste", "test_paste_color_fill_parity": "Image.paste",
    "test_paste_with_mask_parity": "Image.paste", "test_paste_at_origin_parity": "Image.paste",
    "test_split_rgb_parity": "Image.split", "test_split_rgba_parity": "Image.split",
    "test_split_grayscale_parity": "Image.split",
    "test_getbands_rgb_parity": "Image.getbands", "test_getbands_rgba_parity": "Image.getbands",
    "test_getbands_l_parity": "Image.getbands",
    # Image — filter
    "test_filter_blur_parity": "Image.filter", "test_filter_sharpen_parity": "Image.filter",
    "test_filter_smooth_parity": "Image.filter", "test_filter_contour_works": "Image.filter",
    "test_filter_emboss_works": "Image.filter", "test_filter_find_edges_works": "Image.filter",
    # Image — analysis
    "test_getbbox_parity": "Image.getbbox", "test_getextrema_rgb_parity": "Image.getextrema",
    "test_histogram_rgb_parity": "Image.histogram",
    "test_getpixel_rgb_parity": "Image.getpixel", "test_getpixel_rgba_parity": "Image.getpixel",
    "test_getpixel_grayscale_parity": "Image.getpixel",
    "test_putpixel_rgb_parity": "Image.putpixel",
    # Image — advanced
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
    # ImageChops
    "test_chops_add_parity": "ImageChops.add", "test_chops_subtract_parity": "ImageChops.subtract",
    "test_chops_multiply_parity": "ImageChops.multiply", "test_chops_screen_parity": "ImageChops.screen",
    "test_chops_darker_parity": "ImageChops.darker", "test_chops_lighter_parity": "ImageChops.lighter",
    "test_chops_difference_parity": "ImageChops.difference", "test_chops_invert_parity": "ImageChops.invert",
    "test_add_modulo_works": "ImageChops.add_modulo", "test_subtract_modulo_works": "ImageChops.subtract_modulo",
    "test_constant_works": "ImageChops.constant",
    "TestBlend2::test_blend_parity": "ImageChops.blend",
    "TestComposite2::test_composite_works": "ImageChops.composite",
    "test_duplicate_parity": "ImageChops.duplicate",
    # ImageColor
    "test_getrgb_hex_parity": "ImageColor.getrgb", "test_getrgb_named_parity": "ImageColor.getrgb",
    "test_getcolor_rgb_parity": "ImageColor.getcolor", "test_getcolor_l_parity": "ImageColor.getcolor",
    # ImageDraw
    "test_draw_line_works": "ImageDraw.line", "test_draw_rectangle_outline": "ImageDraw.rectangle",
    "test_draw_rectangle_filled": "ImageDraw.rectangle", "test_draw_ellipse": "ImageDraw.ellipse",
    "test_draw_point": "ImageDraw.point", "test_draw_polygon": "ImageDraw.polygon",
    "test_draw_arc_works": "ImageDraw.arc", "test_draw_chord_works": "ImageDraw.chord",
    "test_draw_pieslice_works": "ImageDraw.pieslice", "test_draw_circle_works": "ImageDraw.circle",
    "test_draw_rounded_rectangle_works": "ImageDraw.rounded_rectangle",
    # ImageEnhance
    "test_enhance_brightness_parity": "ImageEnhance.Brightness",
    "test_enhance_color_parity": "ImageEnhance.Color",
    "test_enhance_contrast_parity": "ImageEnhance.Contrast",
    "test_enhance_sharpness_parity": "ImageEnhance.Sharpness",
    # ImageFilter
    "test_blur_constant": "ImageFilter.BLUR", "test_gaussian_blur_class": "ImageFilter.GaussianBlur",
    "test_max_filter_class": "ImageFilter.MaxFilter",
    # ImageFont
    "test_load_default_raises": "ImageFont.load_default",
    "test_freetype_stub": "ImageFont.FreeTypeFont",
    "test_truetype_stub": "ImageFont.truetype",
    "TestImageFontTruetype::test_truetype_loads_real_font": "ImageFont.truetype",
    # ImageModule
    "test_merge_rgb_parity": "ImageModule.merge",
    "test_imagemodule_fns::test_blend_parity": "ImageModule.blend",
    "test_image_chops_advanced::test_blend_parity": "ImageChops.blend",
    "test_imagemodule_fns::test_composite_works": "ImageModule.composite",
    "test_image_chops_advanced::test_composite_works": "ImageChops.composite",
    # Image — completion
    "test_putdata_rgb_parity": "Image.putdata",
    "test_transform_affine_works": "Image.transform",
    # ImageChops — remaining
    "test_chops_hard_light_works": "ImageChops.hard_light",
    "test_chops_soft_light_works": "ImageChops.soft_light",
    "test_chops_overlay_works": "ImageChops.overlay",
    "test_chops_offset_works": "ImageChops.offset",
    "test_chops_logical_and_works": "ImageChops.logical_and",
    "test_chops_logical_or_works": "ImageChops.logical_or",
    "test_chops_logical_xor_works": "ImageChops.logical_xor",
    # ImageDraw — remaining
    "test_draw_text_parity": "ImageDraw.text",
    "test_draw_multiline_text_works": "ImageDraw.multiline_text",
    "test_draw_textbbox_works": "ImageDraw.textbbox",
    "test_draw_textlength_works": "ImageDraw.textlength",
    "test_draw_multiline_textbbox_works": "ImageDraw.multiline_textbbox",
    "test_draw_regular_polygon_works": "ImageDraw.regular_polygon",
    # ImageOps — remaining
    "test_ops_crop_parity": "ImageOps.crop",
    "test_ops_fit_works": "ImageOps.fit",
    "test_ops_pad_works": "ImageOps.pad",
    # ImagePalette
    "test_palette_copy_parity": "ImagePalette.copy",
    "test_palette_tobytes_parity": "ImagePalette.tobytes",
    "test_palette_getdata_works": "ImagePalette.getdata",
    # ImageOps
    "test_ops_invert_parity": "ImageOps.invert", "test_ops_flip_parity": "ImageOps.flip",
    "test_ops_mirror_parity": "ImageOps.mirror", "test_ops_grayscale_parity": "ImageOps.grayscale",
    "test_ops_posterize_parity": "ImageOps.posterize", "test_ops_solarize_parity": "ImageOps.solarize",
    "test_ops_equalize_parity": "ImageOps.equalize",
    "test_autocontrast_works": "ImageOps.autocontrast",
    "test_contain_parity": "ImageOps.contain", "test_cover_parity": "ImageOps.cover",
    "test_expand_parity": "ImageOps.expand", "test_scale_parity": "ImageOps.scale",
    # ImagePalette
    "test_create_palette": "ImagePalette", "test_copy_palette": "ImagePalette.copy",
    # ImageStat
    "test_frombytes_rgb_parity": "Image.frombytes",
    "test_effect_noise_works": "ImageModule.effect_noise",
    "test_draw_bitmap_works": "ImageDraw.bitmap",
    "test_ops_colorize_works": "ImageOps.colorize",
    "test_palette_getcolor_works": "ImagePalette.getcolor",
    "test_palette_save_works": "ImagePalette.save",
    "test_font_getmetrics_works": "ImageFont.getmetrics",
    "test_font_getname_works": "ImageFont.getname",
    "test_stat_basic": "ImageStat.Stat",
    # ImageSequence
    "test_iterator_exists": "ImageSequence.Iterator",
}

def infer_functions(test):
    """Return list of manifest function names this test covers."""
    nodeid = test.get("nodeid", "")
    parts = nodeid.split("::")
    if len(parts) >= 3:
        test_name = f"{parts[-2]}::{parts[-1]}"
    else:
        test_name = parts[-1] if parts else ""
    file_name = parts[0].split("/")[-1].replace(".py", "") if "::" in nodeid else ""
    file_key = f"{file_name}::{test_name}" if file_name else ""
    result = FUNC_MAP.get(file_key) or FUNC_MAP.get(test_name)
    if result is None:
        return []
    if isinstance(result, str):
        return [result]
    return result

def extract_all(manifest):
    funcs = {}
    for mod, mod_def in manifest.get("modules", {}).items():
        for key in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(key, []):
                if isinstance(item, dict):
                    funcs[f"{mod}.{item['name']}"] = item.get("status", "stub")
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict):
                for m in cls.get("methods", []):
                    name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                    funcs[f"{mod}.{name}"] = cls.get("status", "stub")
    return funcs

def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    report_path = sys.argv[2] if len(sys.argv) > 2 else "report.json"

    manifest = load_manifest(manifest_path)
    tests = load_report(report_path) if Path(report_path).exists() else {"tests": []}

    # Which functions have passing tests?
    tested = defaultdict(lambda: {"passed": 0, "failed": 0})
    for test in tests.get("tests", []):
        funcs = infer_functions(test)
        for func in funcs:
            if test.get("outcome") == "passed":
                tested[func]["passed"] += 1
            else:
                tested[func]["failed"] += 1

    # Build report
    all_funcs = extract_all(manifest)

    implemented = {k for k, v in all_funcs.items() if v == "implemented"}
    stubs = {k for k, v in all_funcs.items() if v == "stub"}
    trusted = {k for k in implemented if tested[k]["passed"] > 0 and tested[k]["failed"] == 0}
    untrusted = {k for k in implemented if k not in trusted}
    broken = {k for k in implemented if tested[k]["failed"] > 0}

    total = len(implemented)
    trust_pct = (len(trusted) / max(total, 1)) * 100

    mod_stats = defaultdict(lambda: {"impl": 0, "trusted": 0})
    for k in implemented: mod_stats[k.split(".")[0]]["impl"] += 1
    for k in trusted: mod_stats[k.split(".")[0]]["trusted"] += 1

    print(f"\n{'='*65}")
    print(f"  pillow-rs TRUST Report — {trust_pct:.0f}% of implemented API has PIL parity tests")
    print(f"{'='*65}")
    print(f"  {'Module':<22} {'Impl':>5} {'Trusted':>7} {'Untested':>8}  {'Status'}")
    print(f"  {'-'*55}")
    for mod, stats in sorted(mod_stats.items()):
        impl, tr = stats["impl"], stats["trusted"]
        unt = impl - tr
        status = "✅" if unt == 0 else "⚠️" if tr > 0 else "⬜"
        print(f"  {mod:<22} {impl:>5} {tr:>7} {unt:>8}  {status}")
    print(f"{'='*65}")

    # Untrusted functions
    if untrusted:
        print(f"\n  ⚠️  UNTESTED ({len(untrusted)} functions):")
        for k in sorted(untrusted):
            print(f"    - {k}")
    if broken:
        print(f"\n  ❌ BROKEN ({len(broken)} functions):")
        for k in sorted(broken):
            print(f"    - {k} ({tested[k]['failed']} failing tests)")

    print(f"\n  ✅ TRUSTED: {len(trusted)} functions backed by PIL parity tests")
    print(f"  ⚠️  UNTESTED: {len(untrusted)} implemented but no tests")
    print(f"  ⬜ STUBS: {len(stubs)} not yet implemented")
    print(f"  ❌ BROKEN: {len(broken)} have failing tests")
    print()

if __name__ == "__main__":
    main()
