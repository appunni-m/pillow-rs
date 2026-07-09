# fontdone Optimization Audit — 2026-07-08

Systematic scan of all hot-path source files.  Each entry includes file,
line range, problem, and recommended approach.

---

## 1. `src/scaler.rs` — Glyph Scaling Pipeline

### 1.1 Double pass over scaled points (bbox + origin shift)
- **Lines**: 1083–1123
- **Problem**: Two `for p in &scaled` loops — one for CBox computation,
  one for origin translation.  Points live in L1 cache on second pass
  but the first pass also mutates `x_min/y_min/x_max/y_max` every
  iteration.
- **Optimization**: Fuse into single pass.  Compute `x_min/y_min/x_max/y_max`
  in the same loop that applies `p.x -= off_x; p.y -= off_y`.  Need to
  compute `off_x/off_y` from the first point before entering the fused loop.

```rust
// BEFORE: two passes
for p in &scaled { /* min/max */ }     // pass 1
let off_x = ft_pix_floor(x_min);
let off_y = ft_pix_floor(y_min);
for p in &mut scaled { /* translate */ } // pass 2

// AFTER: single pass
let mut x_min = scaled[0].x;
let mut y_min = scaled[0].y;
let mut x_max = x_min;
let mut y_max = y_min;
for p in &mut scaled {
    // read before mutation
    x_min = x_min.min(p.x); x_max = x_max.max(p.x);
    y_min = y_min.min(p.y); y_max = y_max.max(p.y);
    // then translate (needs off_x pre-computed from iteration 0)
    p.x -= off_x;
    p.y -= off_y;
}
```
- **Estimated impact**: ~5-8% scalar improvement on simple glyphs

### 1.2 `outline_exact_bbox` — per-point allocation for bounding box
- **Lines**: 1433–1530+
- **Problem**: Allocates `Vec::with_capacity(8)` per contour for a temp stack,
  walks every contour with quadratic bezier clipping.  Called unconditionally.
- **Optimization**: Compute exact bbox lazily — only when `FT_Outline_Get_BBox`
  is requested (many callers don't need it).  Or use a pre-allocated scratch
  buffer on the scaler context.

### 1.3 `autohint_pp1x_fu` computed even when `!use_autohint`
- **Lines**: 786-792
- **Problem**: `autohint_pp1x_fu` is computed via ternary even when
  `use_autohint == false`.  It's only consumed inside `if use_autohint`.
- **Fix**: Move into the autohint block (already done for shifted_raw).

### 1.4 `contours: outline_raw.end_pts_of_contours...collect()` — always cloned
- **Line**: 1151–1154 (Outline construction)
- **Problem**: `end_pts_of_contours` is always `.iter().map(|&e| e as i16).collect()`
  creating a new `Vec<i16>` even though it's already available as `Vec<u16>`.
- **Optimization**: Store as `Vec<i16>` in `GlyphOutline` from the start,
  or use `Vec::from_raw_parts` after transmuting the u16 Vec (same layout).

---

## 2. `src/grays.rs` — Gray Rasterizer

### 2.1 `render_line` DDA — if-else chain in hot loop
- **Lines**: 606–650
- **Problem**: The inner DDA loop has 4 if-else branches checking `prod`
  thresholds every iteration.  Each arm computes `ft_udiv` which is
  expensive.  The compiler cannot vectorize because the branch pattern
  is a state machine (one branch per iteration).
- **Suggested approach**: Pre-compute the quadrant from `dx`/`dy` signs
  outside the loop, then use a table-driven state machine.  C does this
  too (it's a classic Bresenham variant), so parity matters.
- **Risk**: High — parity-critical code.  Any change must produce
  identical cell output.

### 2.2 `set_cell` — binary search fallback allocates
- **Lines**: 439–471
- **Problem**: When cells are emitted out-of-order (rare in TrueType but
  happens with complex cubic flattening), `binary_search` + `insert`
  shifts the Vec.  The common path is append-only (last.x < ex).
- **Optimization**: Track whether we've ever hit the out-of-order path
  for a scanline; if not, skip binary search entirely (branch prediction
  already handles this well).

### 2.3 `sweep` — `fill_rule` called twice per pixel when area ≠ 0
- **Lines**: 997–1032
- **Problem**: For the common case (`cover` transitions, `area` non-zero),
  `fill_rule` is called twice: once for the span fill, once for the
  individual pixel.  `fill_rule` contains shift + conditional + mask.
- **Optimization**: Compute `area_coverage = fill_rule(area, fill)` only
  once, then `span_coverage = fill_rule(cover, fill)` only when span
  actually needs filling.

### 2.4 `scanlines.resize_with` per pass
- **Line**: 1071
- **Problem**: `resize_with` allocates fresh `Vec` for each scanline
  that wasn't present before.  On first pass this allocates all scanlines;
  on subsequent passes (LCD rendering does 3 passes over same outline),
  scanlines already exist so only `clear()` runs.
- **Status**: Already mitigated by reusable `RasterScratch` on Font.
  But `resize_with` still checks capacity every call.
- **Better**: Use a fixed upper-bound pre-allocation.  `max_ey - min_ey`
  for a 24pt glyph is at most ~40px; pre-allocate all scanlines once
  and never resize.

### 2.5 `render_conic` — shift loop + DDA
- **Lines**: 686–726
- **Problem**: The conic flattening uses a shift-based dynamic-step
  counter, then loops `count` times calling `render_line`.  Each
  `render_line` call goes through the full DDA dispatch.
- **Idiom**: This is a direct port of C's `gray_render_conic`.
  Algorithmic changes risk parity.

---

## 3. `src/autohint/latin.rs` — Auto-hinter Core (5031 lines)

### 3.1 `compute_segments` — `hints.contours.clone()` every call
- **Line**: 3055
- **Problem**: Clones the entire `Vec<usize>` of contours before
  iterating.  Borrow-checker workaround because `compute_segments`
  mutates `hints.axis` while reading `hints.contours`.
- **Optimization**: Split into two functions: one that reads contours
  (immutable borrow) and one that writes axis (mutable borrow).  Or
  pass contours as `&[usize]` from the caller who already holds the
  immutable reference.

### 3.2 `compute_segments` — `let _ = &mut _prev_max_flags` dead stores
- **Lines**: 3205–3208
- **Problem**: Three `let _ = &mut ...` statements that are C parity
  artifacts (C uses these for next-iteration merge comparisons).
  Rust ignores them but they confuse readers.
- **Fix**: Document as intentional dead-stores for C parity, or add
  `#[allow(unused_assignments)]` at function level and remove the
  `let _ = &mut` stubs.

### 3.3 `compute_edges` — inner edge-matching loop
- **Lines**: 3463–3473
- **Problem**: Linear scan `for e_idx in 0..axis.edges.len()` to find
  matching edge by `(edge.dir == seg_dir) && (fpos - seg_pos).abs() < threshold`.
  Edges are already sorted by position; use binary search.
- **Estimated improvement**: For contours with 50+ segments, this is
  the dominant cost in edge computation.

### 3.4 `hint_edges` — `align_linked_edge` called per stem pair
- **Line**: 4145
- **Problem**: Phase 1 (stem snap) scans all edges O(n²) looking for
  linked pairs.  Phase 2 (serif snap) scans again.  Phase 3 (blue snap)
  scans again.  Phase 4 (anchor snap) scans once more.
- **Optimization**: Pre-compute linked-pair sets in one pass, then
  feed them to each phase.  Currently each phase re-discovers links.

### 3.5 `align_strong_points` clones `hints.axis[dim]`
- **Lines**: 4743
- **Problem**: `let axis_snapshot = hints.axis[dim as usize].clone()`
  clones the entire `AxisHints` struct (segments Vec + edges Vec)
  to avoid borrow-checker conflict with point mutation.
- **Fix**: Use `std::mem::take` to extract the axis temporarily,
  or use split borrows with raw pointers (unsafe but well-scoped).

### 3.6 `align_weak_points` clones `hints.contours`
- **Lines**: 4937
- **Problem**: Same pattern — clone to avoid borrow conflict.
- **Fix**: Same as above.

### 3.7 `apply_hints` — `metrics.cloned()` on AfLatinMetrics
- **Line**: 2708
- **Problem**: Clones the entire `AfLatinMetrics` struct including
  `non_base_glyphs: Vec<bool>` (up to 6000 entries for CJK fonts),
  `digit_glyphs: Vec<bool>` (same size), and `reverse_adjustment_map`
  (HashMap).  This clone is on the hot path.
- **Optimization**: Store metrics behind `Rc` and only `Rc::clone`
  the pointer.  `GlyphHints.metrics` already uses `Option<AfLatinMetrics>`
  instead of `Option<Rc<...>>`.
- **Estimated impact**: ~200ns per glyph for CJK, ~50ns for Latin.

### 3.8 `horz_widths_26_6` / `vert_widths_26_6` — intermediate collect
- **Lines**: 2766, 2805
- **Problem**: `widths.iter().take(wc).map(|w| w.cur).collect()` creates
  an intermediate Vec of width values, then iterates over it again in
  `hint_edges`.  Avoids the intermediate allocation.
- **Fix**: Pass `&[AfWidth]` and `wc` directly to `hint_edges` and let
  it extract `.cur` on-the-fly.

---

## 4. `src/autohint/loader.rs` — Outline Reload (405 lines)

### 4.1 `build_direction_chain` — repeated `fx as i32` casts
- **Lines**: 350–400
- **Problem**: Every iteration of the direction-chain loop reads
  `hints.points[next].fx as i32` twice (once for out_x accumulator,
  once for direction_compute).  `fx` is `i16` so the cast is free,
  but the double read introduces a load from memory.
- **Mitigation**: Already cached via `out_x` accumulator.

### 4.2 `reload` — `hints.points.reserve(num_points + 2)` then push
- **Line**: 88
- **Problem**: `reserve` + `push` loop is the idiomatic way but
  `reserve` only prevents reallocation; each `push` still does a
  capacity check (branch) and write.
- **Optimization**: Use `hints.points.resize(num_points, AFPoint::default())`
  then fill in-place.  `resize` does one allocation + memset, then
  direct writes avoid push overhead.

---

## 5. `src/ffi/handles.rs` — FFI Wrappers (1698 lines)

### 5.1 `outline_to_ffi_snapshot` always builds FT_Vector Vec
- **Lines**: 1625–1650
- **Problem**: Every `FT_Load_Glyph` call converts `Vec<OutlinePoint>`
  to `Vec<FT_Vector>` (`x: i64::from(point.x)`) and builds tags Vec.
  This is ~40 bytes per point just for the FFI view.
- **Optimization**: Deferred compute (we attempted this but parity test
  uses `slot.outline` directly).  Could make `FT_GlyphSlot.outline` a
  `OnceLock<FT_OutlineSnapshot>` computed on first access.

### 5.2 `face_to_ffi` parses SFNT tables eagerly
- **Lines**: 561–650
- **Problem**: `face_to_ffi` calls `font.load_sfnt_table` for head,
  maxp, hhea, vhea, post, pclt — six table reads + parses on every
  `FT_New_Memory_Face`.  Benchmark creates face once, so this is
  amortized, but production code doing face-per-glyph pays this.
- **Status**: Acceptable for our benchmark pattern (face created once).

### 5.3 `FT_Render_Glyph` clones source_face
- **Lines**: 1342
- **Problem**: `slot.source_face.clone()` clones entire `Face` struct
  (including `Font` + `RenderFontCache` + `BTreeMap`).
- **Status**: `FT_Render_Glyph` is rarely called in benchmark path.
  `FT_Load_Glyph` with `FT_LOAD_RENDER` renders inline (our optimization).

---

## 6. `src/render.rs` — Rendering Dispatch (2851 lines)

### 6.1 `render_lcd` and `render_lcd_v` create local RasterScratch
- **Lines**: 2670, 2708
- **Problem**: Despite the reusable scratch on `Font`, LCD paths create
  their own `RasterScratch` locally.  LCD rendering does 3 passes —
  each pass re-allocates scanlines on first use.
- **Fix**: Thread `font.raster_scratch` through LCD render paths too.

### 6.2 `render_sdf` — SDF path not covered by benchmark
- **Lines**: 385+
- **Status**: Not on hot path.

---

## 7. `src/tt/glyf.rs` — Glyf Parser (701 lines)

### 7.1 `parse_simple_glyph` — repeated per-point tag allocation
- **Lines**: 448–600
- **Problem**: Decodes flags with RLE expansion into `Vec<u8>`, then
  decodes X and Y delta streams with separate per-point loops.
  Each loop does `pos` bounds-check + flag decode.
- **Optimization**: Decode flags, X, and Y in a single pass with 3
  position cursors.  C's `TT_Load_Simple_Glyph` does this because it
  writes directly into a pre-allocated `FT_Vector` array.

### 7.2 `load_glyph_outline` clone on cache hit
- **Lines**: 70 (tables.rs)
- **Problem**: Returns `Ok(outline.clone())` on cache hit — clones
  entire `GlyphOutline` (points Vec + end_pts Vec + instructions Vec).
- **Fix**: Return `Rc::clone` (already done in commit `e801baab` but
  we need to audit that the Rc refactor didn't regress).

---

## 8. Cross-Cutting Concerns

### 8.1 Per-glyph `FontData` clone via `Arc::new(data.clone())`
- **File**: `src/scaler.rs:687`
- **Problem**: When autohint fallback runs, `std::sync::Arc::new(data.clone())`
  clones 750KB+ of font data.
- **Fix**: `FontData.self_arc` (commit `dbb903c6`) provides a cached Arc
  pointer; verify this path is actually using it.

### 8.2 `loaded_outline.outline.clone()` in `GlyphSlot::render`
- **File**: `src/api.rs:571`
- **Problem**: `FT_Render_Glyph` path (not `FT_LOAD_RENDER`) clones outline.
- **Status**: Not on benchmark hot path; `FT_LOAD_RENDER` takes ownership.

### 8.3 Non-zero-winding `fill_rule` is branchy
- **File**: `src/grays.rs:86–98`
- **Problem**: `fill_rule` has 3 branches (coverage sign check, fill-mode
  toggle, clamp to 255).  Called per-pixel in `sweep`.
- **Optimization**: Could precompute a lookup table `[u8; 512]` mapping
  `coverage & 0x1FF` → output byte.  Would replace 3 branches with a
  single array lookup.

---

## 9. Priority Ranking (Impact × Ease)

| # | Change | Est. Impact | Risk | File |
|---|--------|-------------|------|------|
| 1 | Fuse bbox + origin-shift loop | 5-8% scalar | Low | scaler.rs |
| 2 | Rc metrics instead of clone | 50-200ns | Low | latin.rs |
| 3 | Binary search edges in compute_edges | 3-5% autohint | Low | latin.rs |
| 4 | Remove intermediate width collect | 20-50ns | Low | latin.rs |
| 5 | fill_rule LUT | 2-4% render | Low | grays.rs |
| 6 | Pre-allocate grays scanlines | 1-3% render | Low | grays.rs |
| 7 | Thread scratch through LCD renders | 1-2% LCD | Low | render.rs |
| 8 | Single-pass glyf parse (flags+X+Y) | 2-5% parse | Medium | glyf.rs |
| 9 | Eliminate axis clone in align_strong_points | 50-100ns | Medium | latin.rs |
| 10 | Eliminate contours clone in compute_segments | 30-60ns | Medium | latin.rs |
| 11 | Deferred outline snapshot (OnceLock) | 200-500ns | Medium | handles.rs |
| 12 | Table-driven DDA in render_line | 5-10% render | High | grays.rs |
