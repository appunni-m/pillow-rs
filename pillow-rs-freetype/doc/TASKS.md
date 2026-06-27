# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1384 | 1910 | 72.5% | PIL 12.2.0 getmask/getbbox |
| FreeType raw | 1377 | 1910 | 72.1% | `/tmp/gen_ft_refs` (FT_LOAD_FORCE_AUTOHINT+FT_LOAD_RENDER) |

**2026-06-27:** Seven commits. **PIL +214, FT +290 from baseline (1170/1087).**

## Fixed

| Commit | Bug | Root cause | Impact |
|--------|-----|-----------|--------|
| `10147a7` | compute_stem_width wrong | Called snap_width (strong-only) instead of C inline logic | +94 PIL, +151 FT |
| `9c7d126` | Linked edge overwritten | Relative-to-anchor re-called compute_stem_width | Part of above |
| `69d93c1` | align_strong_points stale | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix | +15 PIL, +20 FT |
| `dfd49a6` | Missing annotations | Added ✅ VERIFIED markers + CLAUDE.md rule 11 | — |
| `d44fa19` | Post-hinting LSB translation | C's afloader.c:419-530 computes pp1.x translation | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] | Phase 2 relative-to-anchor only set DONE on edge2 | +27 PIL, +27 FT |
| `4558626` | BOUND check overwrites pos | Edge pos was init'd to opos instead of 0 like C | +3 PIL, +5 FT |

## Remaining Failures: 526 PIL / 533 FT

| Type | PIL | FT | Nature |
|------|-----|-----|--------|
| getmask SHA | 491 | 457 | Subpixel coverage differences |
| getbbox | 35 | 66 | Mostly x_max 1px off (pp2.x) + VERT edge Y offsets |
| getlength | 0 | 10 | FT fixture values look wrong (0.56px for "hello") |

### getbbox patterns

**x_max 1px off** (our x_max = expected x_max + 1):
Dominates FT getbbox. Likely pp2.x phantom point not implemented — the
rightmost edge position rounding affects x_max. Also Phase 4 serif 
overlap check (simplified in our port).

**y_min/y_max 1px off** (LiberationSerif):
VERT hinting differences in Phase 4 non-stem edge placement.

### Next recommended steps

| Priority | Item | Est. Impact | Effort |
|----------|------|-------------|--------|
| P1 | pp2.x phantom point (advance width) | ~5-10 bbox | Low |
| P2 | Phase 4 serif overlap check (cross-axis) | ~10 bbox | Medium |
| P3 | Per-point coordinate trace for one VERT glyph | Unknown | High |

## Verified working

- ✅ Edge links: link_segments + compute_edges
- ✅ major_dir: Non-absoluted value matches C
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge grid-fitting: hint_edges Phases 1-4 (V+H) 
- ✅ Point interpolation: align_strong_points, align_edge_points
- ✅ Weak point IUP: align_weak_points, iup_shift, iup_interp
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB translation: pp1.x
- ✅ BOUND check: pos=0 initialization (match C FT_ZERO)
