# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (2026-06-27)

| Backend | Pass | Total | Rate | Δ from original |
|---------|------|-------|------|----------------|
| PIL | 1482 | 1910 | 77.6% | +312 |
| FreeType raw | 1517 | 1910 | 79.4% | +430 |

Original: PIL 1170, FT 1087. **Session: 9 commits.**

## Fixed

| Commit | Bug | Impact |
|--------|-----|--------|
| `10147a7` | compute_stem_width used snap_width (strong-only) | +94 PIL, +151 FT |
| `69d93c1` | align_strong_points wrong algorithm | +15 PIL, +20 FT |
| `d44fa19` | Post-hinting pp1.x translation missing | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] in Phase 2 | +27 PIL, +27 FT |
| `4558626` | BOUND overwrites pos (edge init pos=opos→0) | +3 PIL, +5 FT |
| `913d1e0` | edge2 DONE set too early in anchor path | +1 PIL, +3 FT |
| **`a41d3fa`** | **Operator precedence: `(a+b+32) & !63 - c` → `((a+b+32) & !63) - c`** | **+97 PIL, +137 FT** |

## Remaining: 428 PIL / 393 FT

| Type | PIL | FT | Root cause |
|------|-----|-----|-----------|
| getmask SHA | 403 | 366 | IUP interpolation subpixel differences; LiberationSerif ×250 |
| getbbox | 25 | 17 | y-axis ±1 (VERT edge alignment + IUP) |
| getlength | 0 | 10 | pp2.x not implemented; FT fixture values also wrong (0.56px) |

## Detailed getbbox failures

### FT (17): all y-axis ±1
- 5× LiberationSerif ymax-1 (PIL native hinter gives different shapes)
- 4× LiberationSerif ymin-1 
- 3× LiberationSerif ymin-1 + ymax-1
- 5× DejaVuSans ymin-1/ymax+1

### PIL (25): mostly LiberationSerif ymin+x → PIL coords differ from native hinter
- 11× LiberationSerif ymin+1, 3× ymax+1, 2× ymin+2

## IUP interpolation — 366 FT mask failures

Traced '8' at DejaVuSans 16pt: all 8 edge positions match C exactly (64, 95, 161, 192, 448, 479, 545, 576). Only 3/48 points differ: p12.x=226→233(diff=7), p13.x=164→168(diff=4), p34.x=150→160(diff=10). These are weak points interpolated via iup_interp. The ±4-10 unit differences (= ±0.06-0.16px) cause pixel value differences in anti-aliased output, triggering SHA mismatches.

**Cause**: The IUP scale factor `ft_mul_div(u2-u1, 0x10000, v2-v1)` differs microscopically between C (FT_DivFix) and Rust. The u/v values for the touched reference points differ due to prior rounding in align_edge_points and align_strong_points rounding chains. Requires byte-level parity debugging.

## LiberationSerif: 265/366 FT mask failures

Most LiberationSerif chars that fail at ALL 5 sizes are uppercase letters with bowls (P, Q, R, S, T at codes 80-84). These have 7 or more HORZ edges. Our edge positions match C's for the traced cases ('B' at 16pt), but the interpolated weak points diverge. LiberationSerif also shows y-axis bbox ±1 failures (autohinter vs native hinter shapes differ).

## Next Steps

| Priority | Item | Est. Impact | Effort |
|----------|------|-------------|--------|
| P1 | pp2.x phantom point (advance → getlength) | ~10 getlength | Medium |
| P2 | Phase 4 serif overlap check (cross-axis u/v) | ~3-5 bbox | Medium |
| P3 | IUP byte-level parity (per-point trace) | ~50-100 mask | High |
| P4 | LiberationSerif-specific segment topology | ~100 mask | High |

## Verified working

- ✅ compute_stem_width: smooth + serif paths match C
- ✅ align_strong_points: linear-scan + FT_DivFix/FT_MulFix
- ✅ hint_edges Phases 1-4: edge positions match C exactly
- ✅ Post-hinting pp1.x: LSB pixel translation
- ✅ BOUND check: edge pos=0 initialization
- ✅ Phase 2 DONE flags: correct for anchor + rel-to-anchor
- ✅ cur_pos2 operator precedence: parenthesized correctly
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
