#!/usr/bin/env python3
"""One-time migration: convert all legacy fixtures to new JSON-driven format."""
import json
from pathlib import Path

FIXTURES_DIR = Path(__file__).parent.parent.parent / "tests" / "fixtures"
TYPE_MAP = {"image": "method", "filter": "filter", "dual": "dual",
            "draw": "draw", "enhance": "enhance", "module": "classmethod", "value": "value"}


def migrate(fixture):
    """Convert a legacy fixture to the self-describing format."""
    op_name = fixture["op"]
    module, target = op_name.rsplit(".", 1) if "." in op_name else (op_name, op_name)
    params = fixture.get("params", {})

    # Determine type from legacy op name conventions
    typ = _infer_type(op_name, params)
    # For dual ops that use prep, move prep into params
    if op_name.startswith("ImageChops.logical"):
        params = dict(params, prep="convert('1', dither='NONE')")

    new_fixture = {
        "format_version": 1,
        "operation": {
            "type": TYPE_MAP.get(typ, typ),
            "module": module,
            "target": target,
            "params": params
        },
        "input": {
            "mode": fixture.get("inputMode", fixture.get("mode", "RGB")),
            "size": fixture.get("inputSize", [100, 100]),
            "bytes": fixture.get("inputBytes", "")
        },
        "config": {
            "reference_bytes_rgb": fixture.get("inputBytesRgb", fixture.get("inputBytes", "")),
            "targets": fixture.get("targets", ["cpu"])
        }
    }

    # Dual-image ops add a second input (same image for now)
    if typ == "dual":
        new_fixture["input2"] = dict(new_fixture["input"])

    # Expected
    if "expectedHash" in fixture:
        new_fixture["expected"] = {"result_type": "hash", "value": fixture["expectedHash"]}
    elif "expectedValue" in fixture:
        new_fixture["expected"] = {"result_type": "value", "value": fixture["expectedValue"]}
    elif "expectedError" in fixture:
        new_fixture["expected"] = {"result_type": "error", "value": fixture["expectedError"]}
    else:
        new_fixture["expected"] = {"result_type": "value", "value": None}

    return new_fixture


def _infer_type(op_name, params):
    """Infer the operation type from the op name and params."""
    if op_name.startswith("ImageFilter."): return "filter"
    if op_name.startswith("ImageDraw."): return "draw"
    if op_name.startswith("ImageEnhance."): return "enhance"
    if op_name.startswith("ImageColor."): return "value"
    if op_name.startswith("ImagePalette."): return "value"
    if op_name.startswith("ImageFont."): return "value"
    if op_name.startswith("ImageStat."): return "value"
    if op_name.startswith("ImageSequence."): return "value"
    if op_name.startswith("ImageModule."):
        if op_name.rsplit(".", 1)[1] in ("blend", "composite"): return "dual"
        if op_name.rsplit(".", 1)[1] in ("new", "effect_noise", "fromarray",
                                          "frombytes", "open", "eval", "merge"):
            return "classmethod"
        return "method"
    if op_name.startswith("ImageChops."):
        func = op_name.rsplit(".", 1)[1]
        if func in ("invert", "duplicate", "constant", "offset"): return "method"
        return "dual"
    # Image instance methods
    func = op_name.rsplit(".", 1)[1] if "." in op_name else ""
    if func in ("tobytes", "split", "getbands", "getbbox", "getextrema", "histogram",
                "getpixel", "getcolors", "getdata", "getprojection", "entropy",
                "load", "close", "verify", "seek", "tell", "tobitmap",
                "mode", "size", "width", "height", "format", "info",
                "getexif", "getim", "getpalette", "getxmp", "get_flattened_data",
                "get_child_images", "apply_transparency", "palette",
                "is_animated", "n_frames", "has_transparency_data", "show"):
        return "value"
    return "method"


def main():
    count = 0
    for fpath in sorted(FIXTURES_DIR.glob("*.json")):
        with open(fpath) as f:
            legacy = json.load(f)
        new_fixture = migrate(legacy)
        with open(fpath, "w") as f:
            json.dump(new_fixture, f, indent=2)
        count += 1
    print(f"Migrated {count} fixtures to {FIXTURES_DIR}")


if __name__ == "__main__":
    main()
