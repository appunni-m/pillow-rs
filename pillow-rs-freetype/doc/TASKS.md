# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1381 | 1910 | 72.3% | PIL 12.2.0 getmask/getbbox |
| FreeType raw | 1372 | 1910 | 71.8% | `/tmp/gen_ft_refs` (FT_LOAD_FORCE_AUTOHINT+FT_LOAD_RENDER) |

**2026-06-27:** Six commits. **PIL +211, FT +285 from baseline (1170/1087).**

## Fixed

| Commit | Bug | Root cause | Impact |
|--------|-----|-----------|--------|
| `10147a7` | compute_stem_width wrong | Called snap_width (strong-only) instead of C inline logic | +94 PIL, +151 FT |
| `9c7d126` | Linked edge overwritten | Relative-to-anchor re-called compute_stem_width | Part of above |
| `69d93c1` | align_strong_points stale | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix | +15 PIL, +20 FT |
| `dfd49a6` | Missing annotations | Added ✅ VERIFIED markers + CLAUDE.md rule 11 | — |
| `d44fa19` | Post-hinting LSB translation | C's afloader.c:419-530 computes pp1.x translation | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] | Phase 2 relative-to-anchor only set DONE on edge2 | +27 PIL, +27 FT |

## Remaining Failures: 529 PIL / 538 FT

| Type | Count | Nature |
|------|-------|--------|
| getmask SHA mismatch | ~963 | Subpixel anti-aliasing coverage differences between our autohinter and C's |
| getbbox mismatch | ~107 | HORZ edge positions + PIL y-coordinate convention (native vs autohinter) |
| getlength mismatch | ~10 | Advance width; FT fixture values suspect (0.56px for "hello") |

### getmask — subpixel parity

- 'A' at DejaVuSans 10pt: edges verified identical. p0.y=444 (ours) vs 437 (C). 
  Difference 7/64=0.11px causes ~10 pixel value differences in rows 1-4.
- Root cause: VERT edge interpolation gives slightly different values.
  Same edges produce different scales due to FT_DivFix/FT_MulFix rounding.

### getbbox — autohinter vs native hinter

- **Y-axis (PIL coords)**: PIL uses native TrueType hinter. Our autohinter produces
  different glyph shapes. Char '_' at 10pt: ours 7×1px, PIL 1×8px. Not fixable.
- **X-axis (FT coords)**: 40 DejaVuSans getbbox failures show 1px x_max differences.
  Likely pp2.x rounding not implemented.

### getlength

- PIL fixture: 25.36px → matches ours. FT fixture: 0.56px → clearly wrong.
  pp2.x would only change by ~0.5px. Fixture generation bug.

## What's left to fix (actionable)

| Priority | Item | Est. Impact | Effort |
|----------|------|-------------|--------|
| P1 | pp2.x phantom point (advance width) | ~10 getlength + ~5 bbox | Low |
| P2 | Per-point coordinate trace for one VERT glyph | Unknown cascade | High |
| P3 | HORZ stem edge tracing for one bbox failure | ~5-10 bbox | Medium |
| P4 | LiberationSerif font-specific edge/segment issues | ~38 PIL mask | High |

## Next recommended step

**pp2.x** is the lowest-hanging fruit: modifies `apply_hints` to return (pp1x, pp2x),
compute `hinted_advance = FT_PIX_ROUND(pp2x - pp1x)`, propagate through
`ScaledGlyph.advance_width`, and use in `getbbox`/`getlength`.

## Verified working

- ✅ Edge links: link_segments + compute_edges
- ✅ major_dir: Non-absoluted value matches C
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge grid-fitting: hint_edges Phases 1-4 (V+H) — ⚠️ edge[i] DONE bug fixed
- ✅ Point interpolation: align_strong_points, align_edge_points
- ✅ Weak point IUP: align_weak_points, iup_shift, iup_interp
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB translation: pp1.x = FT_PIX_ROUND(edge.pos - edge.opos)
