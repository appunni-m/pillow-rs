# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1322 | 1910 | 69.2% | PIL 12.2.0 getmask/getbbox (FreeType 2.14.3) |
| FreeType raw | 1303 | 1910 | 68.2% | `/tmp/gen_ft_refs` (FT_LOAD_RENDER from vendored 2.14.3) |

**2026-06-27:** Four commits applied:
1. `compute_stem_width` smooth path: C inline logic instead of snap_width (+94 PIL, +151 FT)
2. `hint_edges` Phase 2: linked-edge overwrite fix (+43 PIL, +45 FT)
3. `align_strong_points`: rewrite to match C's linear-scan + scale-based algorithm (+15 PIL, +20 FT)
4. CLAUDE.md rule 11: C-verification annotations

**Total from baseline (1170/1087): PIL +152, FT +216.**

## Fixed Bugs

### ✅ BUG 1: `compute_stem_width` smooth path used wrong function
- Called `snap_width` (strong-hinting) instead of C's inline smooth logic (aflatin.c:4016-4075):
  - Standard-width snap: `|dist - stdw| < 40` → stdw, clamp ≥48
  - Fractional-pixel quant: `delta = dist & 63`, redistribute
  - bdelta+round (simplified, bdelta=0)

### ✅ BUG 2: Serif path missing `return` in `compute_stem_width`
- `return dist` immediately, matching C's `goto Done_Width`

### ✅ BUG 3: `hint_edges` Phase 2 overwrote linked edge position
- C sets `edge2->pos` inline (cur_pos1 + cur_len/2), no `af_latin_align_linked_edge` call
- We were re-calling `compute_stem_width` and overwriting

### ✅ REFACTOR: `align_strong_points` matches C algorithm exactly
- Linear scan for first edge with `fpos >= u` (afhints.c:1492)
- Exact-match snap to edge (afhints.c:1496)
- Scale-based interpolation: FT_DivFix + FT_MulFix (afhints.c:1523)
- Correct before-first / after-last fallback paths (afhints.c:1456-1470)

### ✅ NOT-A-BUG: Edge links, major_dir, segment detection
- All confirmed working through eprintln tracing

## Remaining Work (588 PIL / 607 FT failures)

### [ ] Point-level differences (e.g., p0.y=444 vs C's 437)
- C produces p0.y=437, Rust produces p0.y=444 for DejaVuSans 10pt 'A'
- Edge positions, segment detection, and interpolation math are all verified
- Both FT_MulDiv and FT_DivFix+FT_MulFix give 444
- Root cause unknown — may need C source-level trace

### [ ] `hint_edges` Phase 4 serif cross-axis check
- Simplified (skips segment `v` coord overlap check from aflatin.c:4655-4690)
- May cause false serif pairing for some glyphs

### [ ] Point-on-edge exact match path (afhints.c:1496)
- New `align_strong_points` has this path but not yet exercised in test matrix

### [ ] Rasterization differences
- Even with identical hinted points, subpixel rasterization can differ
- Remaining failures may be mostly raster-level, not hinting-level

### [ ] Re-validate after each fix
- `cargo test -p pillow-rs-freetype test_font_coverage_matrix -- --nocapture`

## Code Annotations

Key functions in `pillow-rs-freetype/src/autohint/latin.rs` reference C source:

| Function | C reference | Status |
|----------|------------|--------|
| `compute_stem_width` | aflatin.c:3993-4075,4076-4152 | ✅ Smooth + strong verified |
| `snap_width` | aflatin.c:2725-2767 | ✅ Verified (strong path only) |
| `hint_edges` Phase 2 | aflatin.c:4340-4564 | ✅ Stem alignment verified |
| `align_strong_points` | afhints.c:1413-1578 | ✅ Algorithm matches C |
| `link_segments_inner` | aflatin.c:2015-2148 | ✅ Verified |
| `compute_edges` | aflatin.c:2182-2495 | ✅ Verified |
| `align_edge_points` | afhints.c:1338-1400 | ✅ Verified |
| `align_weak_points` | afhints.c:1687-1808 | ✅ Verified |
| `metrics_scale_dim` | aflatin.c:1178-1437 | ✅ Verified |

## Debugging Tools

| Tool | Location | Purpose |
|---|---|---|
| FreeType 2.14.3 debug .so | `~/.local/lib/libfreetyped.so` | C trace with `FT2_DEBUG` |
| C tracer binary | `/tmp/trace_ft_debug` | Dumps hinted 26.6 points + bitmap |
| C outline dump | `/tmp/check_p0` etc. | Quick single-glyph point dump |
| FT reference generator | `/tmp/gen_ft_refs` + `scripts/gen_ft_matrix.py` | Regenerates coverage_matrix_ft.json |
| PIL reference generator | `scripts/generate_font_refs.py` | Regenerates coverage_matrix.json |
| Rust cmp_glyph | `examples/cmp_glyph.rs` | Quick single-glyph comparison |
| Coverage test | `cargo test -p pillow-rs-freetype test_font_coverage_matrix` | Full matrix |

## NOT the Problem

- ❌ Point interpolation (IUP): algorithmically correct
- ❌ Edge links: link_segments_inner + compute_edges work correctly
- ❌ major_dir: Non-absoluted value matches C
- ❌ Version mismatch: All sources use FreeType 2.14.3
- ❌ Standard widths: VERT=194, HORZ=156 confirmed
- ❌ rasterizer (grays.rs): Byte-accurate to ftgrays.c
