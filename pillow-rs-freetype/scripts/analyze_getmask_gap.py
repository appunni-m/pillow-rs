#!/usr/bin/env python3
"""Systematic analysis of getmask gap between pillow-rs-freetype (Rust) and PIL reference.

Compares our Rust output (via dump_mask_compare example) with freetype-py
FT_LOAD_DEFAULT|RENDER output — the autohinted path PIL's getmask takes on
the bytecode-stripped `fonts_nohint` inputs.
Categorizes each failure type.
"""

import freetype
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent
MANIFEST_DIR = ROOT
FIXTURES = ROOT / "tests" / "fixtures"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"
FONTS_DIR = FIXTURES / "input" / "fonts"         # Original fonts (with bytecode)
FONTS_NOHINT_DIR = FIXTURES / "input" / "fonts_nohint"  # Bytecode-stripped (autohinted at render)

LOAD_FLAGS = 0x4  # FT_LOAD_RENDER; FT_LOAD_DEFAULT triggers the autohinter

# ── Representative glyphs to analyze ──
# Picked to cover different glyph types: simple, curved, diagonal, wide, narrow
GLYPH_TYPES = {
    # (codepoint, char, category)
    33: ("!", "narrow-vertical"),
    46: (".", "dot-tiny"),
    65: ("A", "diagonal-caps"),
    67: ("C", "curved-simple"),
    79: ("O", "oval"),
    103: ("g", "curved-descender"),
    124: ("|", "vertical-bar"),
    35: ("#", "hashtag-complex"),
    87: ("W", "wide-diagonal"),
}

FONT_MAP = {
    "DejaVuSans": "DejaVuSans.ttf",
    "LiberationSerif": "LiberationSerif-Regular.ttf",
}


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def render_freetype_py(font_path: Path, size_pt: float, ch: str):
    """Render with freetype-py using FT_LOAD_DEFAULT|RENDER (matches PIL getmask)."""
    face = freetype.Face(str(font_path))
    face.set_char_size(int(size_pt * 64))  # 26.6 format
    face.load_char(ch, LOAD_FLAGS)
    glyph = face.glyph
    bmp = glyph.bitmap

    width = bmp.width
    rows = bmp.rows
    if width <= 0 or rows <= 0:
        return b"", 0, 0, [0, 0, 0, 0]

    raw = bytes(bmp.buffer)
    left = glyph.bitmap_left
    top = glyph.bitmap_top
    bbox = [left, top - rows, left + width, top]
    return raw, width, rows, bbox


def run_rust_dump(font_path: Path, size_pt: float, codepoint: int) -> dict:
    """Call the Rust dump_mask_compare example."""
    result = subprocess.run(
        [
            "cargo", "run", "--example", "dump_mask_compare", "--",
            str(font_path), str(size_pt), str(codepoint),
        ],
        cwd=str(MANIFEST_DIR),
        capture_output=True,
        text=True,
    )
    # Parse the JSON output (last line before cargo status)
    stderr = result.stderr
    stdout = result.stdout
    # Find JSON in stdout
    try:
        data = json.loads(stdout)
        return data
    except json.JSONDecodeError:
        print(f"Failed to parse Rust output for U+{codepoint:04X}")
        print(f"STDOUT: {stdout[:500]}")
        print(f"STDERR: {stderr[:500]}")
        return None


def load_expected_from_matrix(font_name: str, size_pt: int, codepoint: int) -> dict:
    """Load expected values from coverage matrix."""
    with open(MATRIX_PATH) as f:
        matrix = json.load(f)
    test_id = f"{font_name}_{size_pt}_{codepoint}_getmask"
    for row in matrix["rows"]:
        if row["id"] == test_id:
            return {
                "id": row["id"],
                "ref_sha256": row.get("ref_sha256"),
                "ref_size": row.get("ref_size"),
            }
    return None


def load_expected_raw(font_name: str, size_pt, codepoint: int) -> bytes | None:
    """Load expected raw binary from outputs/raws/."""
    dump_name = f"{font_name}_{size_pt}_{codepoint}_getmask.bin"
    dump_path = FIXTURES / "outputs" / "raws" / dump_name
    if dump_path.exists():
        return dump_path.read_bytes()
    return None


def categorize_failure(
    rust_width: int, rust_height: int,
    freetype_width: int, freetype_height: int,
    expected_size: list | None,
    rust_pixels: bytes,
    freetype_pixels: bytes,
    expected_pixels: bytes | None,
) -> str:
    """Categorize the type of failure."""
    issues = []

    # Size comparison
    if expected_size:
        exp_w, exp_h = expected_size
        if rust_width != exp_w or rust_height != exp_h:
            issues.append(f"size-vs-expected(rust={rust_width}x{rust_height} expected={exp_w}x{exp_h})")

    if rust_width != freetype_width or rust_height != freetype_height:
        issues.append(f"size-vs-freetype-py(rust={rust_width}x{rust_height} ft={freetype_width}x{freetype_height})")

    if rust_width == freetype_width and rust_height == freetype_height:
        # Same dimensions - compare pixels
        if rust_pixels == freetype_pixels:
            issues.append("IDENTICAL-to-freetype-py(but-diff-from-expected)")
        else:
            # Pixel-by-pixel comparison
            same = sum(1 for a, b in zip(rust_pixels, freetype_pixels) if a == b)
            total = len(rust_pixels)
            diff_pct = (total - same) / total * 100 if total > 0 else 0
            if diff_pct < 5:
                issues.append(f"minor-pixel-diff({diff_pct:.1f}%)")
            elif diff_pct < 30:
                issues.append(f"moderate-pixel-diff({diff_pct:.1f}%)")
            else:
                issues.append(f"major-pixel-diff({diff_pct:.1f}%)")
    else:
        # Different dimensions - could be shift, scaling, or bbox
        if abs(rust_width - freetype_width) <= 1 and abs(rust_height - freetype_height) <= 1:
            issues.append("border-diff(1px)")
        else:
            issues.append("size-mismatch")

    # Compare Rust with expected
    if expected_pixels is not None:
        exp_w, exp_h = expected_size if expected_size else (0, 0)
        if rust_width == exp_w and rust_height == exp_h:
            same = sum(1 for a, b in zip(rust_pixels, expected_pixels) if a == b)
            total = len(rust_pixels)
            diff_pct = (total - same) / total * 100 if total > 0 else 0
            issues.append(f"vs-expected-same-size-diff({diff_pct:.1f}%)")

    return "; ".join(issues) if issues else "unknown"


def print_pixel_grid(name: str, width: int, height: int, pixels: bytes):
    """Print a pixel grid visualization."""
    print(f"\n  {name} ({width}x{height}):")
    for y in range(height):
        row = pixels[y * width : (y + 1) * width]
        chars = []
        for b in row:
            if b == 0:
                chars.append("  . ")
            elif b == 255:
                chars.append(" ## ")
            else:
                chars.append(f"{b:3d}")
        print(f"  y{y:2d}: " + " ".join(chars))


def main():
    print("=" * 80)
    print("GETMASK GAP ANALYSIS")
    print("=" * 80)
    print()

    # Load matrix for reference
    with open(MATRIX_PATH) as f:
        matrix = json.load(f)

    font_name = "DejaVuSans"
    font_nohint_path = FONTS_NOHINT_DIR / FONT_MAP[font_name]
    font_original_path = FONTS_DIR / FONT_MAP[font_name]
    sizes = [10, 12, 16, 20, 24]

    # Build freetype-py cache first
    print("Building freetype-py reference renderings...")
    ft_cache = {}  # (font_type, size, cp) -> (pixels, w, h, bbox)

    for font_type, font_path in [("nohint", font_nohint_path), ("original", font_original_path)]:
        print(f"\n  Font: {font_type}")
        for size in sizes:
            for cp, (ch, cat) in GLYPH_TYPES.items():
                raw, w, h, bbox = render_freetype_py(font_path, size, ch)
                sha = sha256_hex(raw)
                ft_cache[(font_type, size, cp)] = {
                    "pixels": raw, "width": w, "height": h, "bbox": bbox, "sha256": sha
                }

    # Get expected from matrix
    expected_data = {}
    for size in sizes:
        for cp in GLYPH_TYPES:
            exp = load_expected_from_matrix(font_name, size, cp)
            exp_raw = load_expected_raw(font_name, size, cp)
            expected_data[(size, cp)] = {
                "matrix": exp,
                "raw": exp_raw,
            }

    # Print summary table header
    print("\n" + "=" * 80)
    print("SUMMARY: Glyph-level comparison")
    print("=" * 80)
    print(f"{'Glyph':>6s} {'Sz':>3s} {'Cat':<20s} {'Rust':>10s} {'FT-orig':>10s} {'FT-nohint':>10s} {'Expected':>10s} {'Rust==FT?':>10s} {'Gap Type'}")
    print("-" * 120)

    categories = defaultdict(list)

    for size in sizes:
        for cp, (ch, cat) in GLYPH_TYPES.items():
            # Run Rust
            rust = run_rust_dump(font_nohint_path, size, cp)
            if rust is None:
                continue

            ft_orig = ft_cache[("original", size, cp)]
            ft_nohint = ft_cache[("nohint", size, cp)]
            exp_data = expected_data[(size, cp)]
            exp_size = exp_data["matrix"]["ref_size"] if exp_data["matrix"] else None

            rust_w, rust_h = rust["width"], rust["height"]
            rust_sha = rust["sha256"]
            rust_pixels = bytes.fromhex("".join(rust["pixels_hex"]))
            ft_orig_pixels = ft_orig["pixels"]
            ft_nohint_pixels = ft_nohint["pixels"]
            ft_orig_sha = ft_orig["sha256"]
            ft_nohint_sha = ft_nohint["sha256"]
            exp_sha = exp_data["matrix"]["ref_sha256"][:16] if exp_data["matrix"] else "N/A"
            exp_w, exp_h = exp_size if exp_size else (0, 0)

            # Check: does Rust match freetype-py (FT_LOAD_DEFAULT, autohinted)?
            # `ft_nohint` here = the bytecode-stripped `fonts_nohint` font (autohinted).
            rust_size_str = f"{rust_w}x{rust_h}"
            ft_orig_size = f"{ft_orig['width']}x{ft_orig['height']}"
            ft_nohint_size = f"{ft_nohint['width']}x{ft_nohint['height']}"
            exp_size_str = f"{exp_w}x{exp_h}" if exp_size else "N/A"

            # Direct pixel comparison Rust vs freetype-py (both fonts)
            rust_vs_orig = "MATCH" if rust_pixels == ft_orig_pixels else "DIFF"
            rust_vs_nohint = "MATCH" if rust_pixels == ft_nohint_pixels else "DIFF"

            # Categorize
            gap = categorize_failure(
                rust_w, rust_h,
                ft_orig["width"], ft_orig["height"],
                exp_size,
                rust_pixels, ft_orig_pixels,
                exp_data["raw"],
            )

            # Also check against ft_nohint
            if rust_pixels == ft_nohint_pixels and rust_pixels != ft_orig_pixels:
                gap += " [RUST==FT-NOHINT but !=FT-ORIG]"
            elif rust_pixels == ft_orig_pixels and rust_pixels != ft_nohint_pixels:
                gap += " [RUST==FT-ORIG but !=FT-NOHINT]"
            elif rust_pixels != ft_orig_pixels and rust_pixels != ft_nohint_pixels:
                gap += " [RUST!=BOTH-FT]"

            categories[gap].append(f"{font_name}_{size}_{cp}_getmask")

            print(f"{ch:>6s} {size:>3d} {cat:<20s} {rust_size_str:>10s} {ft_orig_size:>10s} {ft_nohint_size:>10s} {exp_size_str:>10s} {rust_vs_orig:>10s} {gap[:80]}")

    # ── Category summary ──
    print("\n" + "=" * 80)
    print("CATEGORY BREAKDOWN")
    print("=" * 80)
    for cat, ids in sorted(categories.items(), key=lambda x: len(x[1]), reverse=True):
        print(f"  {len(ids):>4d} tests: {cat}")
        for tid in ids[:3]:
            print(f"         - {tid}")
        if len(ids) > 3:
            print(f"         ... and {len(ids)-3} more")

    # ── Detailed pixel dumps for representative glyphs ──
    print("\n" + "=" * 80)
    print("DETAILED PIXEL GRID COMPARISON")
    print("=" * 80)

    detail_glyphs = [
        (10, 124, "|", "vertical-bar"),
        (10, 103, "g", "curved-descender"),
        (10, 65, "A", "diagonal-caps"),
        (10, 46, ".", "dot-tiny"),
        (10, 33, "!", "narrow-vertical"),
    ]

    for size, cp, ch, cat in detail_glyphs:
        rust = run_rust_dump(font_nohint_path, size, cp)
        if rust is None:
            continue
        ft_orig = ft_cache[("original", size, cp)]
        ft_nohint = ft_cache[("nohint", size, cp)]
        exp_data = expected_data[(size, cp)]

        print(f"\n{'─'*70}")
        print(f"  Glyph '{ch}' (U+{cp:04X}) at {size}pt — Category: {cat}")
        print(f"{'─'*70}")

        rust_pixels = bytes.fromhex("".join(rust["pixels_hex"]))
        bbox_str = f"bbox={rust['bbox']}"
        print(f"\n  Rust: {rust['width']}x{rust['height']} {bbox_str} sha256={rust['sha256'][:16]}... nonzero={rust['nonzero_pixels']}")
        print_pixel_grid("Rust output", rust["width"], rust["height"], rust_pixels)

        print(f"\n  FT-orig: {ft_orig['width']}x{ft_orig['height']} bbox={ft_orig['bbox']} sha256={ft_orig['sha256'][:16]}...")
        print_pixel_grid("FT-original", ft_orig["width"], ft_orig["height"], ft_orig["pixels"])

        print(f"\n  FT-nohint: {ft_nohint['width']}x{ft_nohint['height']} bbox={ft_nohint['bbox']} sha256={ft_nohint['sha256'][:16]}...")
        print_pixel_grid("FT-nohint", ft_nohint["width"], ft_nohint["height"], ft_nohint["pixels"])

        if exp_data["raw"] is not None:
            exp_mat = exp_data["matrix"]
            exp_w, exp_h = exp_mat["ref_size"]
            exp_sha = exp_mat["ref_sha256"]
            print(f"\n  Expected: {exp_w}x{exp_h} sha256={exp_sha[:16]}...")
            print_pixel_grid("Expected", exp_w, exp_h, exp_data["raw"])

        # Direct comparison summary
        rt = rust_pixels
        fo = ft_orig["pixels"]
        fn = ft_nohint["pixels"]
        exp = exp_data.get("raw")

        print(f"\n  Comparisons:")
        print(f"    Rust == FT-orig:     {rt == fo}")
        print(f"    Rust == FT-nohint:   {rt == fn}")
        print(f"    FT-orig == FT-nohint: {fo == fn}")
        if exp is not None:
            print(f"    Rust == Expected:    {rt == exp}")
            print(f"    FT-orig == Expected:  {fo == exp}")
            print(f"    FT-nohint == Expected: {fn == exp}")


if __name__ == "__main__":
    main()
