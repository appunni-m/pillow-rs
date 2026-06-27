# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Δ from baseline |
|---------|------|-------|------|-----------------|
| PIL | 1385 | 1910 | 72.5% | +215 |
| FreeType raw | 1380 | 1910 | 72.3% | +293 |

Baseline: PIL 1170, FT 1087. **Session total: 8 commits.**

## Fixed (2026-06-27)

| Commit | Bug | Root cause | Impact |
|--------|-----|-----------|--------|
| `10147a7` | compute_stem_width | Called snap_width instead of C inline logic | +94 PIL, +151 FT |
| `9c7d126` | Linked edge overwritten | Relative-to-anchor re-called compute_stem_width | merged above |
| `69d93c1` | align_strong_points | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix | +15 PIL, +20 FT |
| `d44fa19` | Post-hinting LSB missing | C's afloader.c:419 computes pp1.x translate | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] | Phase 2 only set DONE on edge2, not current edge | +27 PIL, +27 FT |
| `4558626` | BOUND overwrites pos | Edge init'd with pos=opos instead of 0 like C | +3 PIL, +5 FT |
| `913d1e0` | edge2 DONE set too early | Anchor sets edge2 DONE in common code, blocking STEM | +1 PIL, +3 FT |

## Remaining: 525 PIL / 530 FT

| Type | PIL | FT | Nature |
|------|-----|-----|--------|
| getmask SHA | 490 | 456 | Subpixel coverage differences |
| getbbox | 35 | 64 | Mostly DejaVuSans x_max+1, LiberationSerif y offsets |
| getlength | 0 | 10 | FT fixture values wrong (0.56px for "hello") |

## Root Cause Analysis

### DejaVuSans x_max+1 bbox (~25 FT failures)

Traced '2' at 16pt: C produces xMax=9px, we produce xMax=8px. 
C's VERT edges: pos=64, 448, 545. Ours: pos=64, 415, 512.

The anchor stem sets edge[2].pos=512 (LINK), but C's STEM path 
re-positions it to 545. Our fix now allows this re-positioning,
but edge positions still differ by a few units. Requires detailed
per-point coordinate trace of C's hint_edges.

### LiberationSerif (most remaining SHA + bbox failures)

PIL uses native TrueType hinter; our autohinter produces different 
shapes. LiberationSerif has subtle glyph details where autohinter
and native hinter diverge significantly.

### Subpixel SHA mismatches (~456 FT)

'A' at DejaVuSans 10pt: p0.y=444 vs C's 437 (0.11px diff).
FT_DivFix/FT_MulFix rounding chains differ between our port and C.

## Next Steps

| Priority | Item | Est. Impact | Effort |
|----------|------|-------------|--------|
| P1 | pp2.x phantom point (advance × getlength) | ~10 getlength | Low |
| P2 | Per-point coordinate trace for VERT '2' | ~5-10 bbox | High |
| P3 | Phase 4 serif overlap check | ~5 bbox | Medium |

## Verified working

- ✅ Edge links: link_segments + compute_edges
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge grid-fitting: hint_edges Phases 1-4
- ✅ Point interpolation: align_strong_points, align_edge_points
- ✅ Weak point IUP: align_weak_points, iup_shift, iup_interp
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB translation: pp1.x
- ✅ BOUND check: pos=0 initialization
- ✅ Phase 2 DONE flag: edge[i] DONE always set, edge2 DONE only in rel-to-anchor
