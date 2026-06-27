# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1354 | 1910 | 70.9% | PIL 12.2.0 getmask/getbbox |
| FreeType raw | 1345 | 1910 | 70.4% | `/tmp/gen_ft_refs` (FT_LOAD_FORCE_AUTOHINT+FT_LOAD_RENDER) |

**2026-06-27:** Five commits. **PIL +184, FT +258 from baseline (1170/1087).**

## Fixed

| Commit | Bug | Root cause | Impact |
|--------|-----|-----------|--------|
| `10147a7` | compute_stem_width wrong | Called snap_width (strong-only) instead of C inline logic | +94 PIL, +151 FT |
| `9c7d126` | Linked edge overwritten | Relative-to-anchor re-called compute_stem_width | Part of above |
| `69d93c1` | align_strong_points stale | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix | +15 PIL, +20 FT |
| `dfd49a6` | Missing annotations | Added ✅ VERIFIED markers + CLAUDE.md rule 11 | — |
| `d44fa19` | Post-hinting translation missing | C's afloader.c:419-530 computes pp1.x and translates by -pp1.x | +32 PIL, +42 FT |

## Remaining Failures: 556 PIL / 565 FT

| Type | Count | Change | Likely cause |
|------|-------|--------|-------------|
| getmask SHA mismatch | ~1004 | -9 | Subpixel anti-aliasing coverage differences |
| getbbox mismatch | ~107 | -65 | Remaining HORZ edge + pp2.x advance issues |
| getlength mismatch | ~10 | — | Advance width (pp2.x phantom point) |

### getbbox remaining issues

- **pp2.x (right phantom point)** not implemented — affects advance width and some bbox right-edge positions
- **Serif overlap check** in Phase 4 (aflatin.c:4655-4690) is simplified — currently treats all serifs as valid
- LiberationSerif shows different bbox pattern from DejaVuSans

### getmask SHA mismatches

Most are subpixel coverage differences. Likely root causes:
- Point interpolation producing slightly different 26.6 coordinates
- Rasterizer detail (fractional pixel coverage)

### getlength

Advance width from pp2.x phantom point not yet implemented.

## Verified working

- ✅ Edge links: link_segments + compute_edges
- ✅ major_dir: Non-absoluted value matches C
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge grid-fitting: hint_edges Phases 1-4 (V+H)
- ✅ Point interpolation: align_strong_points, align_edge_points
- ✅ Weak point IUP: align_weak_points, iup_shift, iup_interp
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB translation: pp1.x = FT_PIX_ROUND(edge.pos - edge.opos)

## Debugging tools

| Tool | Purpose |
|------|---------|
| `cargo run --example cmp_glyph -- DejaVuSans 10 A` | Quick single-glyph test |
| `cargo test -p pillow-rs-freetype test_font_coverage_matrix -- --nocapture` | Full matrix |
| `LD_LIBRARY_PATH=~/.local/lib /tmp/trace_ft_debug <font> <size> <char>` | C native hinter |
| `/tmp/verify_ft_ref2` | Dump autohinted x/y coords from C FreeType |
| C test programs in `/tmp/` | Various single-use C traces |

## Key learnings

1. **FT_LOAD_RENDER ≠ autohinter**: C's trace_ft_debug used FT_LOAD_RENDER
   (native TrueType hinter), not FT_LOAD_FORCE_AUTOHINT. Outputs differ.

2. **Post-hinting translation is critical**: C's afloader.c translates the
   outline by -FT_PIX_ROUND(edge.pos - edge.opos) after hinting, aligning
   the LSB to the pixel grid. Without this, bbox is off by 1-2px.

3. **PIL uses native hinter**: PIL's ImageFont uses FreeType's default
   (native) hinter for TrueType fonts. Our PIL parity tests compare
   autohinter output against native hinter output — they WILL differ for
   glyphs where native and auto hinting disagree.
