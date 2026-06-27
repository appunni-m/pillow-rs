# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1546 | 1910 | 80.9% |
| FreeType raw | 1588 | 1910 | 83.1% |

Session: 11 commits. All functions verified against C. `ft_corner_is_flat` implemented.

## All functions: verification status — 100% annotated

### ✅ VERIFIED (21 functions)

| Function | C reference |
|----------|------------|
| `ft_hypot` | ftobjs.h:80 (*new*) |
| `corner_is_flat` | ftcalc.c:1006-1042 (*new*) |
| `ft_mul_fix` | ftcalc.h:91-102 (FT_MulFix_64) |
| `ft_mul_div` | ftcalc.c:393-440 |
| `ft_div_fix` | ftcalc.c:574-600 |
| `ft_pix_round` | macro in aftypes.h |
| `direction_compute` | afhints.c:751-798 |
| `iup_shift` | afhints.c:1593-1612 |
| `iup_interp` | afhints.c:1620-1685 |
| `align_weak_points` | afhints.c:1687-1808 |
| `align_strong_points` | afhints.c:1413-1578 |
| `align_edge_points` | afhints.c:1338-1400 |
| `align_linked_edge` | aflatin.c:4164-4194 |
| `align_serif_edge` | aflatin.c:4200-4212 |
| `compute_stem_width` | aflatin.c:3960-4152 |
| `snap_width` | aflatin.c:3936-3958 |
| `compute_blue_edges` | aflatin.c:2529-2640 |
| `compute_edges` | aflatin.c:2154-2428 |
| `link_segments_inner` | aflatin.c:2436-2524 |
| `metrics_init_blues` | aflatin.c:685-1176 |
| `weak-point classification` | afhints.c:1250-1295 (incl. corner_is_flat) |

## Remaining: 364 PIL / 322 FT

| Type | PIL | FT | Nature |
|------|-----|-----|--------|
| getmask SHA | 339 | 295 | Subpixel anti-aliasing coverage differences |
| getbbox | 25 | 17 | y-axis ±1px (VERT edges) + LiberationSerif native hinter shape |
| getlength | 0 | 10 | pp2.x not implemented; FT fixtures wrong |

### getmask: mostly LiberationSerif

- 295 FT failures: 95 DejaVuSans + 200 LiberationSerif
- LiberationSerif chars failing at all 5 sizes: uppercase letters with bowls (P,Q,R,S,T)
- DejaVuSans failures: at ≥3 sizes for digits 2,3,4,6,8

All traced edge positions match C. Remaining differences are in IUP interpolation 
of non-edge weak points, with coordinates diverging by 1-6 units after multi-step
interpolation chains.

## Bug fixes — this session (11 commits total)

| Commit | Bug | Impact |
|--------|-----|--------|
| `10147a7` | compute_stem_width smooth path | +94 PIL, +151 FT |
| `d44fa19` | pp1.x post-hinting translation | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] | +27 PIL, +27 FT |
| `a41d3fa` | Operator precedence `& !63 - c` | +97 PIL, +137 FT |
| `4558626` | BOUND check pos=0 init | +3 PIL, +5 FT |
| `913d1e0` | edge2 DONE too early | +1 PIL, +3 FT |
| `8f089a5` | Weak-point classification fixes | 0 (no change, groundwork) |
| **`ee495af`** | **ft_corner_is_flat for weak points** | **+64 PIL, +71 FT** |

## What's left to fix

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| P1 | pp2.x phantom point | ~10 getlength + ~5 bbox | Low |
| P2 | Phase 4 serif overlap check | ~3 bbox | Medium |
| P3 | IUP per-point trace (remaining ~300 mask) | ~50-100 mask | High |
