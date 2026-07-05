#!/usr/bin/env python3
"""Build unified FFI input fixtures from existing FreeType matrix inputs.

The generated files intentionally contain inputs only.  Expected outputs are
produced at test runtime by scripts/gen_unified_oracle.c and compared against
the Rust FFI facade.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = FIXTURE_DIR / "inputs"
FONT_DIR = FIXTURE_DIR / "input" / "fonts_autohint"

GENERATED_FILES = (
    "ft_get_char_index_matrix.json",
    "ft_load_char_matrix.json",
    "ft_load_glyph_matrix.json",
    "ft_render_glyph_matrix.json",
    "ft_size_metrics_matrix.json",
)

LOAD_FLAGS = {
    "FT_LOAD_DEFAULT": 0,
    "FT_LOAD_RENDER": 4,
    "FT_LOAD_NO_BITMAP": 8,
    "FT_LOAD_NO_HINTING": 2,
    "FT_LOAD_FORCE_AUTOHINT": 32,
    "FT_LOAD_TARGET_MONO": 0x20000,
    "FT_LOAD_TARGET_LCD": 0x30000,
    "FT_LOAD_TARGET_LCD_V": 0x40000,
}

RENDER_MODES = {
    "FT_RENDER_MODE_NORMAL": 0,
    "FT_RENDER_MODE_LIGHT": 1,
    "FT_RENDER_MODE_MONO": 2,
    "FT_RENDER_MODE_LCD": 3,
    "FT_RENDER_MODE_LCD_V": 4,
}

MATRIX_TARGETS = {
    "native_tt_default_matrix.json": {
        "getmask": ("get_char_index", "load_char", "load_glyph"),
        "getmetrics": ("size_metrics",),
    },
    "force_autohint_matrix.json": {
        "getmask": ("get_char_index", "load_char"),
    },
    "no_hinting_matrix.json": {
        "getmask": ("get_char_index", "load_char", "load_glyph"),
    },
    "metrics_only_matrix.json": {
        "metrics_only": ("get_char_index", "load_char", "load_glyph"),
    },
    "render_mono_matrix.json": {
        "getmask": ("get_char_index", "load_char", "load_glyph", "render_glyph"),
    },
    "render_lcd_matrix.json": {
        "getmask": ("get_char_index", "load_char", "load_glyph", "render_glyph"),
    },
}

SUBJECTS = {
    "get_char_index": "freetype.FT_Get_Char_Index",
    "load_char": "freetype.FT_Load_Char",
    "load_glyph": "freetype.FT_Load_Glyph",
    "render_glyph": "freetype.FT_Render_Glyph",
    "size_metrics": "freetype.FT_Size_Metrics",
}

CASES = {
    "get_char_index": "matrix_char_code",
    "load_char": "matrix_load",
    "load_glyph": "matrix_load",
    "render_glyph": "matrix_render",
    "size_metrics": "matrix_active_size_metrics",
}

SCHEMAS = {
    "get_char_index": "scalar",
    "load_char": "glyph_slot",
    "load_glyph": "glyph_slot",
    "render_glyph": "glyph_slot",
    "size_metrics": "size_metrics",
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if generated files are stale")
    args = parser.parse_args()

    generated = build_generated_inputs()
    if args.check:
        stale = []
        for name, document in generated.items():
            path = INPUT_DIR / name
            expected = dump_json(document)
            if not path.exists() or path.read_text() != expected:
                stale.append(name)
        if stale:
            raise SystemExit(f"stale unified fixture input files: {', '.join(stale)}")
        return

    INPUT_DIR.mkdir(parents=True, exist_ok=True)
    for name in GENERATED_FILES:
        path = INPUT_DIR / name
        if name in generated:
            path.write_text(dump_json(generated[name]))
        elif path.exists():
            path.unlink()

    total = sum(len(document["matrix_cases"]) for document in generated.values())
    print(f"generated {total} unified matrix input specs")
    for name in GENERATED_FILES:
        print(f"  {name}: {len(generated[name]['matrix_cases'])}")


def build_generated_inputs() -> dict[str, dict]:
    outputs = {
        name: {
            "version": 1,
            "generated_by": "scripts/build_unified_fixture_inputs.py",
            "matrix_cases": [],
            "cases": [],
        }
        for name in GENERATED_FILES
    }

    for matrix_name, operation_targets in MATRIX_TARGETS.items():
        for operation, targets in operation_targets.items():
            for target in targets:
                outputs[file_name_for_target(target)]["matrix_cases"].append(
                    {
                        "id": f"{target}.matrix.{matrix_name.removesuffix('_matrix.json')}.{operation}",
                        "subject": SUBJECTS[target],
                        "case": CASES[target],
                        "operation": operation_for_target(target),
                        "schema": SCHEMAS[target],
                        "source": {
                            "matrix": matrix_name,
                            "row_operation": operation,
                            "requires_glyph_index": target == "load_glyph",
                        },
                        "classifiers": [
                            "area:ffi",
                            "kind:function" if target != "size_metrics" else "kind:record",
                            f"matrix:{matrix_name.removesuffix('_matrix.json')}",
                        ],
                    }
                )

    for document in outputs.values():
        document["matrix_cases"].sort(key=lambda case: case["id"])
    return outputs


def file_name_for_target(target: str) -> str:
    return {
        "get_char_index": "ft_get_char_index_matrix.json",
        "load_char": "ft_load_char_matrix.json",
        "load_glyph": "ft_load_glyph_matrix.json",
        "render_glyph": "ft_render_glyph_matrix.json",
        "size_metrics": "ft_size_metrics_matrix.json",
    }[target]


def operation_for_target(target: str) -> str:
    return {
        "get_char_index": "get_char_index",
        "load_char": "load_char",
        "load_glyph": "load_glyph",
        "render_glyph": "render_glyph",
        "size_metrics": "size_metrics",
    }[target]


def dump_json(document: dict) -> str:
    return json.dumps(document, indent=2, sort_keys=True) + "\n"


if __name__ == "__main__":
    main()
