# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1546 | 1910 | 80.9% |
| FreeType raw | 1588 | 1910 | 83.1% |

Session: 12 commits. `ft_corner_is_flat` (ee495af) was the last functional fix.

## Remaining: 364 PIL / 322 FT

| Type | PIL | FT | Cause |
|------|-----|-----|-------|
| getmask SHA | 339 | 295 | 1-unit ft_div_fix rounding diffs cascading through IUP |
| getbbox | 25 | 17 | y-axis ±1px (VERT edges/blue zones) |
| getlength | 0 | 10 | pp2.x not implemented |

## Analysis of remaining mask failures

### 'Z' at LiberationSerif 10pt — traced 2026-06-27

All 4 point divergences are exactly 1 unit (1/64 pixel):
- p3: 118 vs C's 117
- p4: 92 vs C's 91
- p11: 188 vs C's 189
- p14: 302 vs C's 303

These come from `ft_div_fix` producing scale factors that differ by 1/65536
in the `align_strong_points` interpolation step. The error cascades through
IUP: p14 is a strong reference point (off by 1), which causes p3's IUP
interpolation to also be off by 1.

Root cause: `ft_div_fix(21, 289)` — numerator 21 fits in 5 bits, denominator
289. The integer division `((21<<16) + 289/2) / 289` may round differently
between our i64-based implementation and C's FT_INT64 path for some
specific input pairs due to the signed sign-extraction step.

### '2' at DejaVuSans 16pt — VERIFIED PASS 2026-06-27

All 29 points match C exactly after IUP, confirming the algorithm is correct.
The `ft_corner_is_flat` fix (ee495af) resolved this glyph completely.

## All functions: verification status (22 functions annotated)

| Status | Count | Functions |
|--------|-------|-----------|
| ✅ VERIFIED | 19 | All core algorithm functions |
| ⚠️ SIMPLIFIED | 2 | Phase 4 serif overlap, some blue zone dedup |
| ⚠️ UNVERIFIED | 1 | `compute_segments` (segment detection — largest untraced function) |

## What's left

### Low effort / high ROI

| Item | Impact | Effort |
|------|--------|--------|
| pp2.x phantom point | ~10 getlength | Low — same pattern as pp1.x (implemented) |
| Phase 4 serif overlap check | ~3-5 bbox | Medium |

### High effort / lower ROI

| Item | Impact | Effort |
|------|--------|--------|
| ft_div_fix 1-unit rounding convergence | ~100-200 mask | Very high — requires byte-identical integer division for all input pairs |
| compute_segments verification | ~50 mask | High — 500+ line function |
| LiberationSerif font-specific segment topology | ~150 mask | High — font-family-specific bugs |
