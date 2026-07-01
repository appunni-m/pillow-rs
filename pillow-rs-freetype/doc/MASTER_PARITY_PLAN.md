# 100% SHA-256 Parity — Master Implementation Plan

## Pipeline Architecture: C → Rust Mapping

The FreeType autohinter pipeline for a single glyph is:

```
C: af_latin_hints_apply (aflatin.c:4957-5215)
  ├─ af_glyph_hints_reload (afhints.c:1087)  ← load outline → points with WEAK/STRONG
  ├─ af_latin_hints_detect_features (aflatin.c:2514)
  │    ├─ af_latin_hints_compute_segments    ← group contour runs into segments
  │    ├─ af_latin_hints_link_segments       ← find stem/serif pairs by scoring
  │    └─ af_latin_hints_compute_edges       ← merge overlapping segments into edges
  ├─ af_latin_hints_compute_blue_edges       ← assign edges to blue zones
  ├─ af_latin_hints_hint_edges               ← 4-phase grid-fitting
  │    ├─ Phase 1: blue-zone alignment
  │    ├─ Phase 2: stem alignment
  │    ├─ Phase 3: serif alignment
  │    └─ Phase 4: anchor propagation
  ├─ af_glyph_hints_align_edge_points        ← snap contour points to edge positions
  ├─ af_glyph_hints_align_strong_points      ← interpolate strong points
  ├─ af_glyph_hints_align_weak_points        ← IUP for weak points
  └─ af_glyph_hints_apply_vertical_separation ← tilde/cedilla adjustments

Rust: apply_hints (latin.rs:822)
  ├─ loader::reload                          ← ✅ VERIFIED: matches C reload
  ├─ compute_segments (HORZ)                 ← ✅ VERIFIED: same logic as C
  ├─ extract_widths + link_segments (HORZ)   ← ✅ VERIFIED: same scoring
  ├─ compute_edges (HORZ)                    ← ✅ VERIFIED: same merging logic
  ├─ hint_edges (HORZ)                       ← ✅ VERIFIED: 4-phase grid-fitting
  ├─ align_edge_points (HORZ)                ← ✅ VERIFIED: point snapping
  ├─ align_strong_points (HORZ)              ← ✅ VERIFIED: edge interpolation
  ├─ align_weak_points (HORZ)                ← ✅ VERIFIED: IUP
  ├─ compute_segments (VERT)                 ← ✅ same logic
  ├─ extract_widths + link_segments (VERT)   ← ✅ same logic
  ├─ compute_edges (VERT)                    ← ✅ same logic
  ├─ compute_blue_edges                      ← ✅ VERIFIED: blue zone assignment
  ├─ hint_edges (VERT)                       ← ✅ VERIFIED: 4-phase grid-fitting
  │    └─ BOUND check: now dynamic for top_to_bottom scripts
  ├─ align_edge_points (VERT)                ← ✅ VERIFIED
  ├─ align_strong_points (VERT)              ← ✅ VERIFIED
  ├─ align_weak_points (VERT)                ← ✅ VERIFIED
  └─ vertical_separation_adjustments         ← ✅ PORTED: tilde/cedilla moves
```

## Gap Analysis: Why 853 failures remain

All pipeline functions are VERIFIED for Latin (7,600/7,600 pass).
The 853 non-Latin failures fall into three categories:

### Category A: Missing edge sort in compute_edges for top_to_bottom (376 failures)

**Source:** latin.rs:1430-1433

Current code:
```rust
if axis.edges.len() > 1 {
    let top_to_bottom = hints.metrics.as_ref()
        .map_or(false, |m| m.top_to_bottom_hinting) && dim == Dimension::Vert;
    // then sort ascending or descending
}
```

**C equivalent:** af_axis_hints_new_edge (afhints.c:197-276)

The edge SORT is correct (confirmed by C trace — edges ascend in INITIAL, same as our code). The issue is NOT in the sort direction. It's in `af_latin_hints_link_segments` which uses `axis->major_dir` for segment linking priority. In C, for top_to_bottom scripts, `axis->major_dir` is set to `AF_DIR_UP` (aflatin.c:1581-1583) which changes which segments get linked → different segment pairs → different edges → different positions → different hinting output.

**Fix:** Apply major_dir=Up for VERT dimension of top_to_bottom scripts, AND also adjust the segment linking scoring to handle the reversed coordinate system.

**Files:** latin.rs:1056-1060, latin.rs:1556-1650

### Category B: Subscript/superscript codepoint resolution (153 failures)

**Source:** globals.rs line where blue zones are computed

**C equivalent:** FreeType uses HarfBuzz GSUB to resolve subscript/superscript features. The codepoint U+1D62 (subscript i) gets assigned to LATB style with subscript-specific blue strings. Our coverage scan assigns LATN because the codepoint falls in LATN ranges.

**Fix:** Override blue zone entries in `get_metrics()` to use LATN for latb/latp when a glyph is shared between sub/sup and regular Latin. The override must be per-glyph-index, not per-script. Check: if the glyph index for a subscript codepoint matches the glyph index for a regular Latin codepoint → this glyph is shared → use LATN blue entries.

**Files:** globals.rs:130-135

### Category C: 1-FU algorithmic drift (324 failures across 16 scripts)

**Source:** hint_edges, align_strong_points, or align_weak_points

**C trace comparison needed:** Run debug_glyph with RUST_LOG=trace and compare per-phase edge positions with C's TRACE output for one representative failing glyph per script.

**Files:** latin.rs:1924-2180 (hint_edges), 2452-2598 (align_strong_points)

## Implementation Order

### Phase 1: Sub/superscript fix (153 failures → 0) — ~30 minutes

1. In `globals.rs::get_metrics()`, add per-glyph-index check:
   - If a glyph index appears in LATB/LATP blue strings AND in LATN blue strings → use LATN entries
   - This handles the shared-glyph case without needing HarfBuzz

### Phase 2: top_to_bottom major_dir (376 failures → ~80) — ~1 hour

1. Set `axis.major_dir = Direction::Up` for VERT dimension of top_to_bottom scripts
2. Adjust `link_segments_inner` scoring for top_to_bottom coordinate system
3. Verify per-glyph with C trace comparison

### Phase 3: 1-FU drift debugging (324 failures → 0) — ~2 hours

1. For each script with 1-3% fail rate, trace one glyph with debug_glyph
2. Compare edge positions at each phase with C TRACE output
3. Fix the specific divergence point

### Unaddressed: Cherokee, Hebrew, Kannada

These scripts have 15-78% fail rates. They need full per-glyph tracing to identify the root divergence. May require C code instrumentation (fprintf in C's compute_edges/hint_edges) to compare per-function output.

## Total Estimated Effort

| Phase | Failures | Est. Time | Files |
|-------|----------|-----------|-------|
| 1: Sub/superscript | 153 | 30 min | globals.rs |
| 2: top_to_bottom major_dir | 376 | 60 min | latin.rs |
| 3: 1-FU drift | 324 | 120 min | latin.rs |
| **Total** | **853** | **~3.5 hours** | |

## Verification Strategy

After each phase:
1. cargo test -p pillow-rs-freetype --test direct_ft_compare
2. Verify no regression on Latin matrix (7,600/7,600)
3. Verify per-script pass rates improve as expected
4. Commit with detailed per-pixel before/after analysis
