#!/usr/bin/env python3
"""Compare pillow-rs-freetype (Rust) getmask output vs PIL ImageFont getmask.
Categorizes every difference. Uses nohint fonts for both.
"""

import hashlib, json, subprocess, sys
from pathlib import Path
from collections import defaultdict, Counter

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"
FONTS_NOHINT_DIR = FIXTURES / "input" / "fonts_nohint"
FONTS_DIR = FIXTURES / "input" / "fonts"

from PIL import ImageFont, Image


def load_matrix():
    with open(MATRIX_PATH) as f:
        return json.load(f)


def rust_dump_all(font_path, size_pt):
    result = subprocess.run(
        ["cargo", "run", "--example", "dump_all_masks", "--", str(font_path), str(size_pt)],
        cwd=str(ROOT), capture_output=True, text=True,
    )
    masks = {}
    for line in result.stdout.strip().split("\n"):
        if not line.strip():
            continue
        try:
            data = json.loads(line)
            masks[data["codepoint"]] = data
        except json.JSONDecodeError:
            pass
    return masks


def pil_render(font_path, size_pt, codepoints):
    font = ImageFont.truetype(str(font_path), size_pt)
    results = {}
    for cp in codepoints:
        ch = chr(cp)
        mask = font.getmask(ch, mode="L")
        w, h = mask.size
        img = Image.frombytes("L", (w, h), bytes(mask))
        raw = img.tobytes()
        sha = hashlib.sha256(raw).hexdigest()
        results[cp] = {
            "codepoint": cp, "char": ch,
            "width": w, "height": h, "raw": raw, "sha256": sha,
            "nonzero": sum(1 for b in raw if b > 0),
        }
    return results


def print_grid(label, w, h, raw):
    print(f"\n  {label} ({w}x{h}):")
    for y in range(h):
        row = raw[y * w : (y + 1) * w]
        chars = []
        for b in row:
            if b == 0: chars.append("  . ")
            elif b == 255: chars.append(" ## ")
            else: chars.append(f"{b:3d}")
        print(f"  y{y:2}: " + " ".join(chars))


def analyze_pixel_diff(rust_raw, pil_raw):
    """Analyze same-size masks."""
    total = len(rust_raw)
    if total == 0:
        return {"same": 0, "total": 0, "pct": 0, "rust_aa": False, "pil_aa": False, "pattern_pct": 100.0}
    same = sum(1 for a, b in zip(rust_raw, pil_raw) if a == b)
    pct = (total - same) / total * 100
    rust_binary = all(b == 0 or b == 255 for b in rust_raw)
    pil_binary = all(b == 0 or b == 255 for b in pil_raw)
    rp = [int(b > 0) for b in rust_raw]
    pp = [int(b > 0) for b in pil_raw]
    pattern = sum(1 for a, b in zip(rp, pp) if a == b) / total * 100
    return {"same": same, "total": total, "pct": pct,
            "rust_aa": not rust_binary, "pil_aa": not pil_binary,
            "pattern_pct": pattern}


def main():
    print("=" * 80)
    print("RUST vs PIL GETMASK — DEFINING THE GAP")
    print("=" * 80)

    matrix = load_matrix()
    expected = {}
    for row in matrix["rows"]:
        if row["operation"] == "getmask":
            expected[row["id"]] = row

    all_cp = list(range(33, 127))

    # Master comparison: Rust (nohint font) vs PIL (nohint font)
    # Both use the same bytecode-stripped font
    print("\nUsing: NOHINT font for both Rust and PIL")
    print("-" * 80)

    gap_categories = defaultdict(list)  # gap_name -> [test_ids]
    border_diff_examples = []

    for size in [10, 12, 16, 20, 24]:
        print(f"\n--- Size {size}pt ---")

        rust = rust_dump_all(FONTS_NOHINT_DIR / "DejaVuSans.ttf", size)
        pil = pil_render(FONTS_NOHINT_DIR / "DejaVuSans.ttf", size, all_cp)

        size_bins = Counter()

        for cp in all_cp:
            test_id = f"DejaVuSans_{size}_{cp}_getmask"
            r = rust.get(cp)
            p = pil.get(cp)
            e = expected.get(test_id)

            if not r or not p:
                continue

            rw, rh = r["width"], r["height"]
            pw, ph = p["width"], p["height"]
            r_px = bytes.fromhex("".join(r["pixels_hex"]))
            p_px = p["raw"]

            # Check if Rust matches expected bbox size
            ew, eh = (e["ref_size"][0], e["ref_size"][1]) if e else (0, 0)
            rust_matches_exp_size = (rw == ew and rh == ew) if e else None  # typo: eh not ew
            rust_matches_exp_size = (rw == ew and rh == eh) if e else None  # fixed

            if rw == pw and rh == ph:
                # Same size: analyze pixel differences
                diff = analyze_pixel_diff(r_px, p_px)
                if diff["same"] == diff["total"]:
                    gap = "PERFECT-MATCH(rust==pil)"
                elif diff["pattern_pct"] >= 95:
                    # Same shape, just different AA values
                    gap = "SAME-SHAPE-AA-diff"
                elif diff["pattern_pct"] >= 70:
                    gap = "SIMILAR-SHAPE"
                else:
                    gap = "DIFFERENT-SHAPE"
                gap_categories[gap].append(test_id)
                size_bins[gap] += 1
            else:
                # Different size
                if abs(rw - pw) <= 2 and abs(rh - ph) <= 2:
                    gap = f"BORDER-DIFF(rust={rw}x{rh} pil={pw}x{ph})"
                    if size == 10:
                        border_diff_examples.append((test_id, r, p))
                else:
                    gap = f"SIZE-MISMATCH(rust={rw}x{rh} pil={pw}x{ph})"
                gap_categories[gap].append(test_id)
                size_bins[gap] += 1

        for g, c in size_bins.most_common(8):
            print(f"  {c:>3d}: {g}")

        # Size match stats
        same_size = sum(1 for cp in all_cp
                       if cp in rust and cp in pil
                       and rust[cp]["width"] == pil[cp]["width"]
                       and rust[cp]["height"] == pil[cp]["height"])
        print(f"  Same-size masks: {same_size}/{len(all_cp)}")
        if e:
            rust_e_size = sum(1 for cp in all_cp
                            if cp in rust and (tid := f"DejaVuSans_{size}_{cp}_getmask") in expected
                            and rust[cp]["width"] == expected[tid]["ref_size"][0]
                            and rust[cp]["height"] == expected[tid]["ref_size"][1])
            pil_e_size = sum(1 for cp in all_cp
                           if cp in pil and (tid := f"DejaVuSans_{size}_{cp}_getmask") in expected
                           and pil[cp]["width"] == expected[tid]["ref_size"][0]
                           and pil[cp]["height"] == expected[tid]["ref_size"][1])
            print(f"  Rust matches Expected size: {rust_e_size}/{len(all_cp)}")
            print(f"  PIL matches Expected size:  {pil_e_size}/{len(all_cp)}")

    # ── DETAILED PIXEL COMPARISON for representative glyphs at 10pt ──
    print("\n\n" + "=" * 80)
    print("DETAILED PIXEL GRID: Rust vs PIL at 10pt (nohint font)")
    print("=" * 80)

    rust_10 = rust_dump_all(FONTS_NOHINT_DIR / "DejaVuSans.ttf", 10)
    pil_10 = pil_render(FONTS_NOHINT_DIR / "DejaVuSans.ttf", 10, all_cp)

    detail_glyphs = [
        (124, "|", "vertical-bar"),
        (103, "g", "curved-descender"),
        (65, "A", "diagonal-caps"),
        (46, ".", "dot-tiny"),
        (33, "!", "narrow-vertical"),
        (67, "C", "curved-simple"),
        (79, "O", "oval"),
        (35, "#", "hashtag-complex"),
        (87, "W", "wide-diagonal"),
        (106, "j", "curved-descender-dot"),
        (48, "0", "digit-zero"),
    ]

    for cp, ch, cat in detail_glyphs:
        r = rust_10.get(cp)
        p = pil_10.get(cp)
        if not r or not p:
            continue
        r_px = bytes.fromhex("".join(r["pixels_hex"]))
        p_px = p["raw"]
        test_id = f"DejaVuSans_10_{cp}_getmask"
        e = expected.get(test_id)

        print(f"\n{'='*70}")
        print(f"  '{ch}' U+{cp:04X} ({cat})")
        print(f"{'='*70}")

        if r["width"] == p["width"] and r["height"] == p["height"]:
            diff = analyze_pixel_diff(r_px, p_px)
            print(f"  Same size: {r['width']}x{r['height']}")
            print(f"  Match: {diff['same']}/{diff['total']} pixels ({100-diff['pct']:.1f}%)")
            print(f"  Pattern match: {diff['pattern_pct']:.1f}%")
            print(f"  Rust has AA: {diff['rust_aa']}, PIL has AA: {diff['pil_aa']}")
        else:
            print(f"  Rust: {r['width']}x{r['height']}  PIL: {p['width']}x{p['height']}")

        print_grid("Rust", r["width"], r["height"], r_px)
        print_grid("PIL", p["width"], p["height"], p_px)

        if e:
            with open(FIXTURES / "outputs" / "raws" / f"DejaVuSans_10_{cp}_getmask.bin", "rb") as f:
                exp_raw = f.read()
            ew, eh = e["ref_size"]
            print_grid("Expected (from matrix)", ew, eh, exp_raw)

    # ── FINAL SUMMARY ──
    print("\n\n" + "=" * 80)
    print("FINAL CATEGORIZATION (Rust vs PIL, nohint font, all sizes)")
    print("=" * 80)

    # Count total
    total = sum(len(v) for v in gap_categories.values())
    print(f"\nTotal comparisons: {total}")

    # Group into high-level categories
    cat_a = []  # Perfect match
    cat_b = []  # Same size, same shape, different AA
    cat_c = []  # Same size, different shape
    cat_d = []  # Border/size diff
    cat_e = []  # Size mismatch

    for gap, ids in gap_categories.items():
        if "PERFECT" in gap:
            cat_a.extend(ids)
        elif "SAME-SHAPE" in gap:
            cat_b.extend(ids)
        elif "SIMILAR" in gap:
            cat_c.extend(ids)
        elif "BORDER" in gap:
            cat_d.extend(ids)
        elif "SIZE" in gap:
            cat_e.extend(ids)
        else:
            cat_c.extend(ids)

    print(f"\n  (a) PERFECT MATCH (Rust == PIL pixel-for-pixel):            {len(cat_a)} tests")
    print(f"  (b) SAME MASK SIZE + SAME SHAPE, different AA values:     {len(cat_b)} tests")
    print(f"  (c) SAME MASK SIZE, visibly different shape:              {len(cat_c)} tests")
    print(f"  (d) BORDER DIFF (1-2 pixel off in width/height):          {len(cat_d)} tests")
    print(f"  (e) SIZE MISMATCH (completely different dimensions):      {len(cat_e)} tests")

    # Root cause analysis
    print(f"\n{'='*70}")
    print("ROOT CAUSE ANALYSIS")
    print(f"{'='*70}")
    print()
    print("Key findings from pixel-level comparison:")
    print()

    # Count how many same-size masks have pattern match
    same_size_same_shape = len(cat_a) + len(cat_b)
    same_size_diff_shape = len(cat_c)
    diff_size = len(cat_d) + len(cat_e)

    print(f"  1. RASTERIZER GAP (same mask size, different AA values):")
    print(f"     {same_size_same_shape} tests — same outline coords, same bbox,")
    print(f"     but our AA values differ from PIL's FreeType AA values.")
    print(f"     Root cause: grays.rs smooth rasterizer produces different")
    print(f"     fractional coverage values than FreeType's ftgrays.c")
    print()

    print(f"  2. OUTLINE/SHAPE GAP (same mask size, different shape):")
    print(f"     {same_size_diff_shape} tests — outlines diverge, affecting")
    print(f"     pixel distribution.")
    print(f"     Root cause: scaler differences (FT_MulFix divergence,")
    print(f"     translation offsets, ppem computation)")
    print()

    print(f"  3. BBOX/SIZE GAP (different mask dimensions):")
    print(f"     {diff_size} tests — bbox computation gives different result.")
    print(f"     Root cause: compute_cbox() or translation offset in scaler, ")
    print(f"     or PIL uses hinted outlines (even for nohint fonts via autohinter)")
    print()

    if len(cat_a) > 0:
        print(f"  4. PERFECT MATCHES: {len(cat_a)} tests match exactly")
        for tid in sorted(cat_a)[:5]:
            print(f"     - {tid}")

    print()
    print("CRITICAL INSIGHT: PIL's getmask ALWAYS uses some form of hinting.")
    print("Even with bytecode stripped (nohint font), PIL's FreeType falls back")
    print("to the autohinter, which grid-fits outlines before rasterization.")
    print("Our pure-Rust renderer uses NO hinting at all (unscaled outlines).")
    print("Therefore, the reference fixtures generated with PIL contain AUTOHINTED")
    print("output, while our Rust code produces UNHINTED output.")
    print()
    print("To achieve 100% getmask parity, we MUST either:")
    print("  (a) Implement the autohinter, OR")
    print("  (b) Regenerate fixtures with truly unhinted rendering (raw freetype-py")
    print("      with FT_LOAD_NO_HINTING, no FT_LOAD_RENDER) and compare at outline/sha level")


if __name__ == "__main__":
    main()
