# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1322 | 1910 | 69.2% | PIL 12.2.0 getmask/getbbox (FreeType 2.14.3) |
| FreeType raw | 1303 | 1910 | 68.2% | `/tmp/gen_ft_refs` (FT_LOAD_RENDER from vendored 2.14.3) |

**2026-06-27:** Four commits applied. **PIL +152, FT +216 from baseline (1170/1087).**

## Fixed

| Bug | Root cause | Impact |
|-----|-----------|--------|
| `compute_stem_width` smooth path | Used `snap_width` (strong-hinting) instead of C's inline logic (aflatin.c:4016-4075) | +94 PIL, +151 FT |
| Serif path missing `return` | C `goto Done_Width` returns immediately; Rust fell through to snap_width | Part of above |
| Linked-edge overwrite | Relative-to-anchor in `hint_edges` re-called compute_stem_width, overwriting edge2->pos | +43 PIL, +45 FT |
| `align_strong_points` algorithm | Rewrote to C's linear-scan + FT_DivFix/FT_MulFix (afhints.c:1492-1540) | +15 PIL, +20 FT |

## Remaining Failures: 588 PIL / 607 FT

### Failure categories

| Type | Count (both matrices) | Likely cause |
|------|----------------------|-------------|
| getmask SHA mismatch | ~1013 | Subpixel raster/hinting differences |
| getbbox mismatch | ~172 | HORZ edge position offsets (1-2px shifts) |
| getlength mismatch | ~10 | Advance width computation |

### Bbox offsets — HORZ edge position discrepancy

**Example:** '!' at DejaVuSans 10pt (`DejaVuSans_10_33`):
- C produces HORZ edge positions: 66 and 127 (26.6), PIL bbox x_min=0
- Rust produces HORZ edge positions: 130 and 191 (26.6), Rust bbox x_min=2
- Manual calculation of anchor path in Phase 2 gives 130 (matches Rust)
- C gets 66 somehow — **root cause NOT yet identified**
- Same x_scale (0x5000), same standard width (61), same compute_stem_width logic

**Investigation needed:** Build C-level trace that dumps every intermediate value
in `af_latin_hint_edges` Phase 2 for '!' at 10pt. Compare edge->opos, cur_len,
org_center, cur_pos1, error1/error2, and final pos values between C and Rust.

### SHA mismatches

Most SHA failures are subpixel-level differences. Given that edge positions
are verified identical for 'A' at 10pt, the remaining SHA mismatches likely come from:
- Point interpolation producing slightly different 26.6 coordinates
- Rasterizer producing slightly different coverage values

### getlength failures

Advance width computation likely differs from PIL's. Not autohinter-related.

## Verified working

- ✅ Edge links: `link_segments_inner` + `compute_edges` ✅
- ✅ `major_dir`: Non-absoluted value matches C ✅
- ✅ Blue zones: `metrics_init_blues` + `metrics_scale_dim` ✅
- ✅ Edge grid-fitting: `hint_edges` Phases 1-4 (V+H) ✅
- ✅ Point interpolation: `align_strong_points`, `align_edge_points` ✅
- ✅ Weak point IUP: `align_weak_points`, `iup_shift`, `iup_interp` ✅
- ✅ Fixed-point math: `ft_mul_div`, `ft_mul_fix`, `ft_div_fix` ✅

## Debugging tools

| Tool | Purpose |
|------|---------|
| `cargo run --example cmp_glyph -- DejaVuSans 10 A` | Quick single-glyph test |
| `cargo test -p pillow-rs-freetype test_font_coverage_matrix -- --nocapture` | Full matrix |
| `LD_LIBRARY_PATH=~/.local/lib /tmp/trace_ft_debug <font> <size> <char>` | C hinted points |
| C test programs in `/tmp/` | Various single-use C traces |
