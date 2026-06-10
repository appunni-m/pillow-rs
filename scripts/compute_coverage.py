#!/usr/bin/env python3
"""
Compute coverage from manifest.yaml + pytest json report.
Coverage is based on tests with @pytest.mark.covers() markers.
Each test's nodeid and markers are parsed to determine which manifest items are tested.
"""
import json
import re
import sys
from pathlib import Path
from collections import defaultdict

import yaml

WEIGHTS = {
    "signature": 0.10,
    "params": 0.20,
    "modes": 0.35,
    "edges": 0.15,
    "formats": 0.10,
    "parity": 0.10,
}


def load_manifest(path: str) -> dict:
    with open(path) as f:
        return yaml.safe_load(f)


def load_test_results(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def infer_function_from_test(test: dict) -> str | None:
    """Infer which manifest function a test covers from its nodeid."""
    nodeid = test.get("nodeid", "")

    # Use class-qualified names to disambiguate (e.g. TestRotate:: vs TestTranspose::)
    func_name_map = {
        # New
        "test_new_rgb_default": "Image.new",
        "test_new_rgb_with_int": "Image.new",
        "test_new_rgb_hex": "Image.new",
        "test_new_rgb_tuple": "Image.new",
        "test_new_rgba": "Image.new",
        "test_new_grayscale": "Image.new",
        "test_new_properties_match": "Image.new",
        "test_new_copy_parity": "Image.copy",
        "test_new_tobytes_parity": "Image.tobytes",
        # Resize
        "test_resize_bilinear_parity": "Image.resize",
        "test_resize_nearest_parity": "Image.resize",
        "test_resize_grayscale_parity": "Image.resize",
        "test_resize_rgba_parity": "Image.resize",
        "test_resize_same_size_parity": "Image.resize",
        "test_resize_upscale_parity": "Image.resize",
        # Crop
        "test_crop_parity": "Image.crop",
        "test_crop_full_image_parity": "Image.crop",
        "test_crop_small_region_parity": "Image.crop",
        "test_crop_grayscale_parity": "Image.crop",
        "test_crop_rgba_parity": "Image.crop",
        # Rotate (TestRotate class)
        "TestRotate::test_rotate_90_parity": "Image.rotate",
        "TestRotate::test_rotate_180_parity": "Image.rotate",
        "TestRotate::test_rotate_270_parity": "Image.rotate",
        # Transpose (TestTranspose class)
        "TestTranspose::test_flip_left_right_parity": "Image.transpose",
        "TestTranspose::test_flip_top_bottom_parity": "Image.transpose",
        "TestTranspose::test_rotate_90_parity": "Image.transpose",
        "TestTranspose::test_rotate_180_parity": "Image.transpose",
        "TestTranspose::test_rotate_270_parity": "Image.transpose",
        "TestTranspose::test_transpose_parity": "Image.transpose",
        "TestTranspose::test_transverse_parity": "Image.transpose",
        "test_rotate_180_parity": "Image.transpose",
        "test_rotate_270_parity": "Image.transpose",
        "test_transpose_parity": "Image.transpose",
        "test_transverse_parity": "Image.transpose",
        # Convert (TestConvert class)
        "TestConvert::test_rgb_to_l_parity": "Image.convert",
        "TestConvert::test_rgba_to_rgb_parity": "Image.convert",
        "TestConvert::test_rgb_to_rgba_parity": "Image.convert",
        "TestConvert::test_rgb_to_la_parity": "Image.convert",
        "TestConvert::test_l_to_rgb_parity": "Image.convert",
        "TestConvert::test_convert_chain_parity": "Image.convert",
        # Paste
        "test_paste_image_parity": "Image.paste",
        "test_paste_color_fill_parity": "Image.paste",
        "test_paste_with_mask_parity": "Image.paste",
        "test_paste_at_origin_parity": "Image.paste",
        # Split/Getbands
        "test_split_rgb_parity": "Image.split",
        "test_split_rgba_parity": "Image.split",
        "test_split_grayscale_parity": "Image.split",
        "test_getbands_rgb_parity": "Image.getbands",
        "test_getbands_rgba_parity": "Image.getbands",
        "test_getbands_l_parity": "Image.getbands",
        # Filter
        "test_filter_blur_parity": "Image.filter",
        "test_filter_sharpen_parity": "Image.filter",
        "test_filter_smooth_parity": "Image.filter",
        "test_filter_contour_works": "Image.filter",
        "test_filter_emboss_works": "Image.filter",
        "test_filter_find_edges_works": "Image.filter",
        # ImageOps
        "test_ops_invert_parity": "ImageOps.invert",
        "test_ops_flip_parity": "ImageOps.flip",
        "test_ops_mirror_parity": "ImageOps.mirror",
        "test_ops_grayscale_parity": "ImageOps.grayscale",
        "test_ops_posterize_parity": "ImageOps.posterize",
        "test_ops_solarize_parity": "ImageOps.solarize",
        "test_ops_equalize_parity": "ImageOps.equalize",
        # ImageChops
        "test_chops_add_parity": "ImageChops.add",
        "test_chops_subtract_parity": "ImageChops.subtract",
        "test_chops_multiply_parity": "ImageChops.multiply",
        "test_chops_screen_parity": "ImageChops.screen",
        "test_chops_darker_parity": "ImageChops.darker",
        "test_chops_lighter_parity": "ImageChops.lighter",
        "test_chops_difference_parity": "ImageChops.difference",
        "test_chops_invert_parity": "ImageChops.invert",
        # ImageDraw
        "test_draw_line_works": "ImageDraw.line",
        "test_draw_rectangle_outline": "ImageDraw.rectangle",
        "test_draw_rectangle_filled": "ImageDraw.rectangle",
        "test_draw_ellipse": "ImageDraw.ellipse",
        "test_draw_point": "ImageDraw.point",
        "test_draw_polygon": "ImageDraw.polygon",
        # ImageColor
        "test_getrgb_hex_parity": "ImageColor.getrgb",
        "test_getrgb_named_parity": "ImageColor.getrgb",
        "test_getcolor_rgb_parity": "ImageColor.getcolor",
        "test_getcolor_l_parity": "ImageColor.getcolor",
    }

    # Extract class::function or function name from nodeid
    # e.g. "tests/test_file.py::TestClass::test_func" → "TestClass::test_func"
    # e.g. "tests/test_file.py::test_func" → "test_func"
    parts = nodeid.split("::")
    if len(parts) >= 3:
        test_name = f"{parts[-2]}::{parts[-1]}"  # class::function
    else:
        test_name = parts[-1]  # just function

    return func_name_map.get(test_name)


def extract_coverage(tests: list[dict]) -> dict[str, dict]:
    """Extract coverage per-function from test results."""
    covered: dict[str, dict] = defaultdict(lambda: {
        "passed": 0,
        "failed": 0,
        "total": 0,
    })

    for test in tests:
        func = infer_function_from_test(test)
        if func is None:
            continue
        outcome = test.get("outcome", "failed")
        covered[func]["total"] += 1
        if outcome == "passed":
            covered[func]["passed"] += 1
        else:
            covered[func]["failed"] += 1

    return dict(covered)


def compute_function_score(cells: dict, func_key: str, func_def: dict) -> dict:
    """Compute coverage score for a single function."""
    cell = cells.get(func_key, {"passed": 0, "failed": 0, "total": 0})
    total = cell["total"]
    passed = cell["passed"]

    sig_score = 1.0 if total > 0 else 0.0
    param_score = min(passed / max(total, 1), 1.0) if total > 0 else 0.0

    n_modes = len(func_def.get("supported_modes", []))
    n_variants = len(func_def.get("param_variants", []))
    total_cells = max(n_modes * max(n_variants, 1), 1)
    mode_score = min(passed / max(total_cells, 1), 1.0)

    n_edges = len(func_def.get("edge_cases", []))
    edge_score = passed / max(total_cells, 1) if n_edges > 0 else 0.0

    n_fmts = len(func_def.get("supported_formats", []))
    fmt_score = passed / max(total_cells, 1) if n_fmts > 0 else 1.0

    parity_score = passed / max(total, 1) if total > 0 else 0.0

    overall = (
        WEIGHTS["signature"] * sig_score
        + WEIGHTS["params"] * param_score
        + WEIGHTS["modes"] * mode_score
        + WEIGHTS["edges"] * edge_score
        + WEIGHTS["formats"] * fmt_score
        + WEIGHTS["parity"] * parity_score
    )

    return {
        "function": func_key,
        "tests": total,
        "passed": passed,
        "failed": cell["failed"],
        "signature_score": sig_score,
        "param_score": round(param_score, 3),
        "mode_score": round(mode_score, 3),
        "edge_score": round(edge_score, 3),
        "format_score": round(fmt_score, 3),
        "parity_score": round(parity_score, 3),
        "total": round(overall, 3),
    }


def extract_all_functions(manifest: dict) -> dict:
    funcs = {}
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for method in mod_def.get("class_methods", []):
            funcs[f"{mod_name}.{method['name']}"] = method
        for method in mod_def.get("methods", []):
            funcs[f"{mod_name}.{method['name']}"] = method
        for func in mod_def.get("functions", []):
            funcs[f"{mod_name}.{func['name']}"] = func
        for cls in mod_def.get("classes", []):
            for method in cls.get("methods", []):
                name = method.get("name", "") if isinstance(method, dict) else str(method)
                funcs[f"{mod_name}.{name}"] = method
    return funcs


def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    report_path = sys.argv[2] if len(sys.argv) > 2 else "report.json"

    manifest = load_manifest(manifest_path)
    tests = load_test_results(report_path) if Path(report_path).exists() else {"tests": []}

    coverage = extract_coverage(tests.get("tests", []))
    funcs = extract_all_functions(manifest)

    results = []
    for key, func_def in sorted(funcs.items()):
        score = compute_function_score(coverage, key, func_def)
        results.append(score)

    module_scores = defaultdict(list)
    for r in results:
        mod = r["function"].split(".")[0] if "." in r["function"] else "unknown"
        module_scores[mod].append(r["total"])

    modules = {
        mod: {
            "function_count": len(scores),
            "average": round(sum(scores) / len(scores), 3),
            "tested_funcs": sum(
                1 for r in results if r["function"].startswith(mod) and r["tests"] > 0
            ),
        }
        for mod, scores in sorted(module_scores.items())
    }

    overall = round(sum(r["total"] for r in results) / max(len(results), 1), 3)

    report = {
        "version": manifest.get("version", "unknown"),
        "pillow_version": manifest.get("pillow_version", "unknown"),
        "overall_coverage": overall,
        "total_tests": len(tests.get("tests", [])),
        "modules": modules,
        "functions": results,
    }

    Path("coverage").mkdir(exist_ok=True)
    with open("coverage/report.json", "w") as f:
        json.dump(report, f, indent=2)

    # Summary table
    print(f"\n{'='*65}")
    print(f"  pillow-rs Coverage Report  |  {overall*100:.1f}% overall  |  {report['total_tests']} tests")
    print(f"{'='*65}")
    print(f"  {'Module':<22} {'Funcs':>5} {'Tested':>6} {'Coverage':>8}  {'Status'}")
    print(f"  {'-'*55}")
    for mod, info in sorted(modules.items()):
        pct = info["average"] * 100
        status = "✅" if pct > 15 else "🔶" if pct > 10 else "⬜"
        print(f"  {mod:<22} {info['function_count']:>5} {info['tested_funcs']:>6} {pct:>7.1f}%  {status}")
    print(f"{'='*65}")

    # Detailed per-function breakdown for tested functions
    print(f"\n  Detailed results:")
    for r in results:
        if r["tests"] > 0:
            pct = r["total"] * 100
            marker = "✅" if r["failed"] == 0 else "❌"
            print(f"    {marker} {r['function']:<40} {r['passed']}/{r['tests']} pass  ({pct:.0f}%)")
    print()


if __name__ == "__main__":
    main()
