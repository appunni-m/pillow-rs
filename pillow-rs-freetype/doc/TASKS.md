# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1482 | 1910 | 77.6% |
| FreeType raw | 1517 | 1910 | 79.4% |

## Remaining: 428 PIL / 393 FT

| Type | PIL | FT | Primary cause |
|------|-----|-----|-------------|
| getmask SHA | 403 | 366 | IUP weak-point interpolation subpixel errors |
| getbbox | 25 | 17 | y-axis ±1 from VERT edge + LiberationSerif native hinter |
| getlength | 0 | 10 | pp2.x + fixture values wrong |

## Verified working (annotations added)

Functions now marked with ✅ VERIFIED in source:
- `iup_shift` (afhints.c:1593-1612) — identical
- `iup_interp` (afhints.c:1620-1685) — identical, same scale + mul_fix  
- `align_weak_points` (afhints.c:1687-1808) — identical algorithm
- `align_edge_points` (afhints.c:1338-1400) — identical
- `align_strong_points` (afhints.c:1413-1578) — verified, ft_div_fix + ft_mul_fix match C
- `compute_blue_edges` (aflatin.c:2529-2640) — identical
- `compute_edges` (aflatin.c:2154-2428) — identical, pos=0 init
- `link_segments_inner` (aflatin.c:2436-2524) — identical
- `snap_width` (aflatin.c:3936-3958) — identical
- `align_linked_edge` (aflatin.c:4164-4194) — identical
- `align_serif_edge` (aflatin.c:4200-4212) — identical
- `compute_stem_width` (aflatin.c:3960-4152) — verified (smooth+serif+strong paths)
- `ft_mul_fix` — verified against FT_MulFix_64 (ftcalc.h:91-102), ab>>63 matches C

## Verified — key functions

- ✅ `ft_mul_fix`: FT_MulFix_64 uses `ab + 0x8000 + (ab>>63)`, which our code matches exactly
- ✅ `ft_mul_div` / `ft_div_fix`: Absolute-value sign-handling identical to C
- ✅ Edge positions: All 8 edges for '8' at 16pt match C exactly (64,95,161,192,448,479,545,576)

## Known issues needing fix

### 1. IUP subpixel divergence (366 FT mask failures)

Traced for '8' at 16pt: 3/48 weak points differ by 4-10 units. Root cause is in **pre-IUP strong-point values**, not IUP itself. p13 is classified as WEAK, so align_strong_points skips it. IUP interpolates between two touched reference points (p12→226, p14→95), producing p13=164. C produces p13=168. The 4-unit difference comes from either:
- Different touched reference point values (strong points have slightly different interpolation)
- Different `ox` values for the reference points

### 2. Weak-point classification (no impact, needs ft_corner_is_flat)

The `|| None` → `&& None` fix is correct but requires also implementing C's `ft_corner_is_flat` check (afhints.c:1272-1282). Without it, the fix caused massive regression (-355 tests). The corner_is_flat check prevents points with one dominant direction vector from being classified as strong.

### 3. pp2.x phantom point (10 FT getlength + ~5 bbox x_max)

Low-effort, not yet implemented. Requires passing advance_width through apply_hints, computing pp2.x = FT_PIX_ROUND(edge2.pos + old_rsb).

## Next steps

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| P1 | pp2.x phantom point | ~10 getlength + ~5 bbox | Low |
| P2 | Per-point IUP trace (compare strong-point values) | ~100 mask | High |
| P3 | Implement ft_corner_is_flat | ~50 mask | Medium |
| P4 | LiberationSerif font-specific | ~200 mask | High |
