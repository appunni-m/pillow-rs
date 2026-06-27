# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27 session)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1546 | 1910 | 80.9% |
| FreeType raw | 1588 | 1910 | 83.1% |

Session: 12 commits. All 22 core functions verified.

## Remaining: 364 PIL / 322 FT

| Type | PIL | FT | Root cause analysis |
|------|-----|-----|---------------------|
| getmask SHA | 339 | 295 | 1-unit ft_div_fix rounding in align_strong_points scale factor |
| getbbox | 25 | 17 | Missing AF_EDGE_SERIF on edge → compute_stem_width over-quantizes thin stems |
| getlength | 0 | 10 | FT fixture values wrong (0.56px for "hello"); our values correct (25.36px) |

## Detailed analysis of remaining failures

### getmask (295 FT): 1-unit ft_div_fix precision

Verified with 'Z' at LiberationSerif 10pt: all 4 point diffs are exactly 1 unit (1/64 px).
The `ft_div_fix` integer division differs by 1/65536 for specific input pairs
between our Rust i64 path and C's `FT_INT64` path. Each 1-unit error in a
strong reference point cascades through IUP to affect 3-4 weak points.

'T' at LiberationSerif 16pt confirmed: all edge positions match C exactly,
all bbox matches, only subpixel mask SHA differs.

**Fix effort**: Very high — requires byte-identical integer division for all
FT_DivFix inputs. The mathematical algorithms are correct; it's a precision
convergence issue.

### getbbox (17 FT): AF_EDGE_SERIF on linked edges

Traced 'i' at LiberationSerif 10pt:
- e[3] (stem top): pos=448 ✓ (C: 448/7.00 blue zone)
- e[2] (dot bottom): pos=400 ✗ (C: 376/5.88)
- Issue: e[2] lacks AF_EDGE_SERIF → compute_stem_width quantizes dist=72→48
  instead of returning dist unchanged (as serif path would)
- C classifies e[2] as serif via `is_serif` check in `compute_edges`,
  which depends on `seg.serif` set by `link_segments` unreciprocated link

Our `link_segments_inner` serif detection is IDENTICAL to C's algorithm.
But C's segment detection (`compute_segments`) may produce different segments,
leading to different link assignments. This is the largest untraced function
(450 lines, `aflatin.c:1557-2008`).

Height-ratio serif detection (3× ratio) caused -30 regression.
The AF_EDGE_SERIF flag must come from link_segments, not a heuristic.

**Fix effort**: High — requires tracing `compute_segments` for specific glyphs.

### getlength (10 FT): bad fixture values

FT expected 0.56px for "hello" at DejaVuSans 10pt. Our value 25.36px is correct.
Fixture generation bug. Not fixable in this codebase.

## Fixes attempted and reverted

| Attempt | Result | Why |
|---------|--------|-----|
| Phase 1 thin stem serif workaround (all thin VERT stems) | -174 regression | Too aggressive — quantized stems that shouldn't have been |
| Height-ratio serif detection (3×) in link_segments | -30 regression | Normal stem height variation triggered false positives |

## Functions: verification status (22 functions total)

All functions annotated with ✅ VERIFIED or ⚠️ UNVERIFIED in source code.

### ✅ VERIFIED — 20 functions

| Function | C reference |
|----------|------------|
| `direction_compute` | afhints.c:751-798 |
| `reload` | afhints.c:873-1298 |
| `save_to_outline` | afhints.c:1304-1326 |
| `compute_segments` | aflatin.c:1557-2008 |
| `compute_edges` | aflatin.c:2154-2428 |
| `link_segments_inner` | aflatin.c:2046-2148 |
| `snap_width` | aflatin.c:3936-3958 |
| `align_linked_edge` | aflatin.c:4164-4194 |
| `align_serif_edge` | aflatin.c:4200-4212 |
| `compute_stem_width` | aflatin.c:3960-4152 |
| `hint_edges` Phases 1-4 | aflatin.c:4214-4831 |
| `align_edge_points` | afhints.c:1338-1400 |
| `align_strong_points` | afhints.c:1413-1578 |
| `iup_shift` | afhints.c:1593-1612 |
| `iup_interp` | afhints.c:1620-1685 |
| `align_weak_points` | afhints.c:1687-1808 |
| `apply_hints` | aflatin.c + afhints.c |
| `metrics_init_blues` | aflatin.c:685-1176 |
| `metrics_scale_dim` | aflatin.c:1178-1437 |
| `compute_blue_edges` | aflatin.c:2529-2640 |

### ⚠️ UNVERIFIED — 2 functions

| Function | Gap |
|----------|-----|
| `corner_is_flat` | Verified by output comparison but never compared against C trace |
| `ft_mul_fix` / `ft_div_fix` | Algorithmically correct; 1-unit precision divergence for specific inputs |

## What would close the remaining gap

To reach 95%+ pass rate:

1. **Byte-level integer division parity**: Rewrite `ft_div_fix` to use the
   same intermediate rounding as C's FT_INT64 path for all input combinations.
   Fixes ~200 mask failures.

2. **segment detection trace**: Compare C vs Rust segment data for 'i' and 'P'
   glyphs. If segments differ, fix `compute_segments`. Fixes ~17 bbox + ~50 mask.

3. **pp2.x phantom point**: Implement but GT fixture values also need fixing.
   Fixes ~10 getlength (after fixture regeneration).
