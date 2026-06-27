# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1384 | 1910 | 72.5% | PIL 12.2.0 getmask/getbbox |
| FreeType raw | 1377 | 1910 | 72.1% | `/tmp/gen_ft_refs` (FT_LOAD_FORCE_AUTOHINT+FT_LOAD_RENDER) |

**2026-06-27:** Seven commits. **PIL +214, FT +290 from baseline (1170/1087).**

## Fixed (this session)

| Commit | Bug | Root cause | Impact |
|--------|-----|-----------|--------|
| `10147a7` | compute_stem_width wrong | Called snap_width (strong-only) instead of C inline logic | +94 PIL, +151 FT |
| `9c7d126` | Linked edge overwritten | Relative-to-anchor re-called compute_stem_width | Part of above |
| `69d93c1` | align_strong_points stale | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix | +15 PIL, +20 FT |
| `dfd49a6` | Missing annotations | Added ✅ VERIFIED markers + CLAUDE.md rule 11 | — |
| `d44fa19` | Post-hinting LSB missing | C's afloader.c:419 computes pp1.x translate | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] | Phase 2 relative-to-anchor only set DONE on edge2 | +27 PIL, +27 FT |
| `4558626` | BOUND check overwrites pos | Edge pos init'd to opos instead of 0 like C's FT_ZERO | +3 PIL, +5 FT |

## Remaining: 526 PIL / 533 FT

| Type | PIL | FT | Nature |
|------|-----|-----|--------|
| getmask SHA | 491 | 457 | Subpixel coverage - mostly LiberationSerif (autohinter ≠ native) |
| getbbox | 35 | 66 | x_max+1 (DejaVuSans ~27), y offsets (LiberationSerif ~35) |
| getlength | 0 | 10 | FT fixtures wrong (0.56px for "hello"), our values correct |

## Root Cause Analysis (2026-06-27)

### DejaVuSans x_max+1 bbox (27 of 37 FT DejaVuSans failures)

Traced '2' at 16pt: C produces 5 HORZ edges, we produce 3.

Our edges: fpos=150, fpos=887, fpos=1098
C's edges: fpos≈150, 160, 887, 1090, 1098 (5 edges)

The missing edges at fpos≈160 (left stem inner) and fpos≈1090 (right stem inner) cause the segment linker to pair edge[0] (fpos=150, leftmost) with edge[2] (fpos=1098, rightmost), forming a "stem" spanning the full glyph width. The anchor's align_linked_edge then fits this 474-unit span to 448 (7px rounded), instead of the correct 97-unit stem width.

C's Phase 2 correctly pairs the inner stem edges (only possible with 5 edges), which then override the anchor's position during relative-to-anchor processing.

**Required fix**: Investigate segment detection → edge creation. The `edge_dist_thresh` is 32 font units (same as C). Segments at pos=150 and pos=160 (diff=10) should merge, but C keeps them separate. Either C's segment positions differ, or the edge creation logic handles near-threshold positions differently.

### LiberationSerif y-axis offsets (~35 FT getbbox + most PIL getmask)

PIL uses native TrueType hinter; our autohinter produces different glyph shapes. Not fixable without full pixel parity between autohinter and native hinter.

### Subpixel SHA mismatches

'A' at DejaVuSans 10pt: edges match exactly, but p0.y=444 vs C's 437 (7-unit difference, ~0.11px). This causes 10+ pixel value differences in anti-aliased output. Root cause: FT_DivFix/FT_MulFix rounding in VERT interpolation gives different intermediate values.

## Next Steps (priority order)

1. **Segment detection**: Trace C's segment positions and edge creation for '2' at 16pt to understand why C produces 5 edges vs our 3
2. **pp2.x phantom point**: Pass advance_width to apply_hints, compute hinted advance = FT_PIX_ROUND(pp2.x - pp1.x)
3. **Phase 4 serif overlap**: Implement cross-axis overlap check (aflatin.c:4655-4690)

## Verified working

- ✅ Edge links: link_segments + compute_edges (⚠️ but produces wrong links when edges missing)
- ✅ major_dir: Non-absoluted value matches C
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge grid-fitting: hint_edges Phases 1-4 (V+H)
- ✅ Point interpolation: align_strong_points, align_edge_points
- ✅ Weak point IUP: align_weak_points, iup_shift, iup_interp
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB translation: pp1.x
- ✅ BOUND check: pos=0 initialization

## Debugging tools

| Tool | Purpose |
|------|---------|
| `cargo run --example cmp_glyph -- DejaVuSans 10 A` | Single glyph render |
| `cargo test -p pillow-rs-freetype test_font_coverage_matrix_pil -- --nocapture` | PIL parity |
| `cargo test -p pillow-rs-freetype test_font_coverage_matrix_freetype -- --nocapture` | FT parity |
| `/tmp/trace_2_16_debug` | C autohinter point dump for '2' at 16pt |
| `/tmp/dump_A_full` | C autohinter point dump for 'A' at 10pt |
