# pillow-rs-freetype Autohinter — Function-Level Reference

**Date:** 2026-06-30 | **Status:** 27,686/27,695 pass (99.97%) | **9 failures remain**

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        font.rs (Font::truetype)                  │
│  Parse tables → build FontData → init AfLatinMetrics            │
│  → metrics_init_widths → metrics_init_blues → metrics_scale_dim │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                    scaler.rs (scale_glyph)                       │
│  GLYF parse → pp1.x shift → scale FU→26.6 → add phantoms      │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                 latin.rs (apply_hints)                           │
│                                 [Entry point: autohinter]        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Phase 1: loader::reload                                  │   │
│  │   copy ox/oy, fx/fy, contour linking, direction chain    │   │
│  │   strong-vs-weak classification ← COMPLEX NUANCES HERE   │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Phase 2: compute_segments(HORZ) → compute_edges(HORZ)    │   │
│  │          hint_edges(HORZ) → align_edge → strong → weak   │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Phase 3: compute_segments(VERT) → compute_edges(VERT)    │   │
│  │          blue_edges → hint_edges(VERT) → align → strong  │   │
│  │          → vertical_separation → weak (IUP)              │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Phase 4: phantom-point adjustment → save_to_outline      │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                      grays.rs (rasterize)                        │
│  convert_glyph → decompose → walk_contour                       │
│  → render_conic (FT_INT64 DDA) → render_line (DDA stepping)     │
│  → sweep → bitmap                                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Function Index

### Phase 0: Metrics Initialization

| Function | File:Line | C Reference | Purpose |
|----------|-----------|-------------|---------|
| `latin_constant` | latin.rs:39 | aflatin.c helper | Per-UPEM constant scaling |
| `flat_threshold` | latin.rs:45 | aflatin.c | upem/14 threshold for edge rounding |
| `sort_pos` | latin.rs:51 | aflatin.c | Sort helper for width arrays |
| `sort_and_quantize_widths` | latin.rs:66 | aflatin.c | Quantize stem widths into canonical values |
| `metrics_init_widths` | latin.rs:117 | af_latin_metrics_init_widths | Extract stem widths from 'o' glyph |
| `extract_widths` | latin.rs:188 | aflatin.c helper | Pull width array from axis hints |
| `metrics_init_blues` | latin.rs:231 | af_latin_metrics_init_blues | Blue zone from OpenType tables |
| `metrics_scale_dim` | latin.rs:460 | af_latin_metrics_scale_dim | Scale metrics for requested ppem |

### Phase 1: Reload + Direction Chain

| Function | File:Line | C Reference | Purpose |
|----------|-----------|-------------|---------|
| `reload` | loader.rs:56 | af_glyph_hints_reload | Load outline into hint structure |
| `build_direction_chain` | loader.rs:253 | afhints.c:1100-1200 | Non-near neighbor detection |
| `corner_is_flat` | loader.rs:21 | ft_corner_is_flat | Flatness test for weak/strong |
| `direction_compute` | loader.rs:32 | af_direction_compute | 8-direction classification |

### Phase 2 & 3: Segments → Edges → Hinting → Alignment

| Function | File:Line | C Reference | Purpose |
|----------|-----------|-------------|---------|
| `apply_hints` | latin.rs:707 | af_latin_hints_apply | Main autohint entry point |
| `compute_segments` | latin.rs:854 | compute_segments | Find horizontal/vertical runs |
| `compute_edges` | latin.rs:1099 | compute_edges | Group segments into edges |
| `compute_blue_edges` | latin.rs:544 | compute_blue_edges | Assign edges to blue zones |
| `link_segments_inner` | latin.rs:1322 | aflatin.c | Stem pairing for segments |
| `snap_width` | latin.rs:1427 | aflatin.c | Snap stem width to standard |
| `align_linked_edge` | latin.rs:1459 | aflatin.c | Align paired stem edge |
| `align_serif_edge` | latin.rs:1489 | aflatin.c | Align serif to stem edge |
| `compute_stem_width` | latin.rs:1503 | compute_stem_width | Current stem width from edges |
| `hint_edges` | latin.rs:1665 | af_latin_hint_edges | 4-phase edge snapping |
| `align_edge_points` | latin.rs:2095 | align_edge_points | Snap contour points to edges |
| `align_strong_points` | latin.rs:2133 | align_strong_points | Interpolate strong points |
| `iup_shift` | latin.rs:2216 | af_iup_shift | Uniform shift for IUP |
| `iup_interp` | latin.rs:2228 | af_iup_interp | Linear interpolation for IUP |
| `align_weak_points` | latin.rs:2267 | align_weak_points | IUP for weak points |
| `vertical_separation_adjustments` | latin.rs:620 | vertical_separation | i/j dot-body separation |

### Phase 4: Output + Rendering

| Function | File:Line | C Reference | Purpose |
|----------|-----------|-------------|---------|
| `scale_glyph` | scaler.rs:93 | TT_Load_Glyph | Scale + hint + bbox + translate |
| `autohint_glyph` | scaler.rs:272 | bridge | Thin wrapper calling apply_hints |
| `rasterize` | grays.rs:126 | gray_raster_render | Outline → bitmap rasterization |
| `render_line` | grays.rs:288 | gray_render_line | DDA line stepping |
| `render_conic` | grays.rs:424 | gray_render_conic | DDA quadratic Bézier |
| `sweep` | grays.rs:688 | gray_sweep | Per-scanline → bitmap |

---

## Function Nuances

### `reload` (loader.rs:56) — C: af_glyph_hints_reload (afhints.c:874-1310)

**Purpose:** Load scaled outline points + raw font-unit glyph data into the AFPoint array. Links contour points via prev/next circular doubly-linked lists. Computes per-point direction vectors.

**Execution:**
1. Clear old points/contours
2. For each point: set `fx/fy` from raw font units, `ox/oy=x=y` from scaled 26.6
3. Link contour points into circular prev/next chains
4. Compute orientation (cw/ccw)
5. Compute y_minima/y_maxima per contour
6. Compute in_dir/out_dir per point from prev→pt→next vectors
7. Mark near points (AF_FLAG_NEAR)
8. **Call `build_direction_chain`** — overrides in_dir/out_dir for smooth curves
9. **Strong-vs-weak classification** (see below)

**Strong-vs-Weak Classification Nuances (lines 200-233):**

The classification loop iterates all points after `build_direction_chain` has set
direction-chain `u`/`v` pointers. For each point, four cases:

```
Case 1: CONTROL (on-curve flag, not applicable to TrueType)
  → ALWAYS weak.  Point is a Bézier control point.

Case 2: in_dir == out_dir AND both != None
  → ALWAYS weak.  Point lies on a straight segment (not a corner).

Case 3: in_dir == out_dir AND both == None
  → The "both-None" case.  Two sub-tests from C:
  
  Test A — XOR quadrant check (afhints.c:1221-1245):
    If (in_x XOR out_x) >= 0 AND (in_y XOR out_y) >= 0:
      → WEAK.  The in/out vectors point in the same general direction.
    This checks: do in and out share the same sign on both axes?
  
  Test B — ft_corner_is_flat (afhints.c:1276-1290):
    If corner_is_flat(in, out) returns true:
      → WEAK.  AND updates pv->u and nu->v index deltas.
      Critical: the delta update changes the direction chain for
      subsequent classification of neighboring points.
    
  If BOTH tests fail:
      → STRONG.  Falls through to the "spike" check.

Case 4: in_dir == -out_dir (spike)
  → ALWAYS weak (afhints.c:1293).  Point where direction reverses.
```

**Critical nuance (2026-06-30 fix):** The XOR test and `corner_is_flat` must NOT
be OR'd together as a boolean expression. C executes them sequentially:
1. XOR passes → WEAK (no delta update)
2. XOR fails → corner_is_flat → WEAK AND update deltas
3. Both fail → STRONG (fall through to spike check)

Our previous code OR'd them: `xor_check || corner_is_flat(...)`. This is logically
equivalent for the WEAK/STRONG result but MISSES the delta update from the
`corner_is_flat` branch. The delta update (`pv->u`, `nu->v`) changes which
neighbor points are consulted for subsequent classifications, cascading to
different WEAK flags for downstream points.

**Also fixed:** Spike detection was guarded by `flags & AF_FLAG_NEAR != 0`.
C has no such guard — any spike is unconditionally weak.

### `build_direction_chain` (loader.rs:253) — C: afhints.c:1100-1200

**Purpose:** For smooth curve segments, overrides per-point in_dir/out_dir with
a unified segment direction, preventing `compute_segments` from splitting a
single smooth curve into multiple short segments.

**Algorithm:**
1. For each contour, walk backwards from start to find "first" — the first point
   whose prev is non-near (taxicab distance >= near_limit2)
2. Walk forward from `first`, accumulating dx/dy taxicab distances
3. When accumulated distance >= near_limit:
   - Set `curr->u = next - curr` (forward pointer to next non-near)
   - Set `next->v = -curr->u` (backward pointer to prev non-near)
   - Override all intermediate points' in_dir/out_dir to accumulated direction
4. Continue until back to `first`

**Key constants:** `near_limit = 20 * upem / 2048`, `near_limit2 = 2 * near_limit - 1`

**Nuance:** The backward walk uses `near_limit2` (not `near_limit`) for the threshold.
This is because the accumulated distance might have opposite direction, so a
larger threshold ensures we find a genuinely non-near point.

### `align_strong_points` (latin.rs:2133) — C: af_glyph_hints_align_strong_points (afhints.c:1413-1585)

**Purpose:** Interpolate non-weak, non-touched points between edges.

**Algorithm:**
1. For each point, skip if `already_touched` or `WEAK_INTERPOLATION`
2. `u` = point's fx (HORZ) or fy (VERT), `ou` = point's ox or oy
3. Find enclosing edges via binary/linear search on `fpos`
4. Three cases:
   - Before first edge: `u = edge[0].pos - (edge[0].opos - ou)`
   - After last edge: `u = edge[last].pos + (ou - edge[last].opos)`
   - Between edges: linear interpolation: `u = before.pos + ft_mul_fix(fu - before.fpos, scale)`
5. Store result and set TOUCH flag.

**Nuance:** The `WEAK_INTERPOLATION` skip is critical — any point marked WEAK
in `reload` bypasses this function entirely and goes to IUP. **This is the root
cause of the 18 residual failures.** A point incorrectly marked WEAK changes
which points serve as IUP reference anchors.

### `hint_edges` (latin.rs:1665) — C: af_latin_hint_edges (aflatin.c:4214-4927)

**4-phase algorithm:**
1. **Phase 1: Stem alignment** — align stem pairs to integer pixels, preserving stem width
2. **Phase 2: Serif alignment** — align serif edges to their linked stem edges
3. **Phase 3: Blue zone alignment** — snap edges with blue-zone assignments
4. **Phase 4: Anchor alignment** — propagate alignment from blue edges to non-blue edges

Each edge has: `fpos` (font-unit position), `opos` (original scaled position),
`pos` (current hinted position, modified in-place through phases).

### `compute_edges` (latin.rs:1099) — C: af_latin_hints_compute_edges (aflatin.c:2144-2530)

**Purpose:** Group segments into edges. Multiple contour segments can merge into
a single edge if they share the same direction and similar position.

**Algorithm:**
1. For each segment, look for existing edge at approximately the same position
   (within `edge_distance_threshold`)
2. If found: update edge's position range and last segment
3. If not found: create new edge
4. After all segments: compute serif links, edge flags (ROUND/NORMAL)

**Key threshold:** `edge_distance_threshold = ft_mul_fix(standard_width/5, scale)`, capped at 16 FU.

---

## Block Diagrams: Font Type Behavior

### Type 1: Upright Serif (DejaVuSerif, LiberationSerif — NOT italic)

```
Font: UPEM=2048, upright
pp1.x: ~0 (xMin ≈ lsb)
HORZ hinting: ENABLED → both X and Y axes hinted
VERT hinting: ENABLED
Blue zones: CAP-TOP, X-HEIGHT, BASELINE, DESCENDER
Segment count: 6-8 per dimension (balanced curved+straight runs)
Result: MATCHES C (all passing)
```

```
reload → direction_chain → weak/strong classification
  ┌──────────────────────────────────────┐
  │ compute_segments(HORZ) → 6 segments  │
  │ compute_edges(HORZ) → 5 edges        │
  │ hint_edges(HORZ, 4 phases)           │
  │ align_edge → strong → IUP            │
  ├──────────────────────────────────────┤
  │ compute_segments(VERT) → 8 segments  │
  │ compute_edges(VERT) → 5 edges        │
  │ compute_blue_edges → blue assignment │
  │ hint_edges(VERT, 4 phases)           │
  │ vertical_separation → IUP            │
  └──────────────────────────────────────┘
  phantom adjust → save → rasterize ✓
```

### Type 2: Italic Serif (DejaVuSerif-Italic, DejaVuSerifCondensed-Italic)

```
Font: UPEM=2048, italic
pp1.x: -1 (xMin differs from lsb by 1)
HORZ hinting: DISABLED (AF_SCALER_FLAG_NO_HORIZONTAL)
VERT hinting: ENABLED
Blue zones: Same as upright
Key difference: NO_HORIZONTAL flag → skip entire HORZ pipeline
Result: MATCHES C after pp1.x fix
```

```
reload → direction_chain → weak/strong classification
  ┌──────────────────────────────────────┐
  │ HORZ SKIPPED (italic flag)           │
  ├──────────────────────────────────────┤
  │ compute_segments(VERT) → 5 segments  │ (Y-axis only)
  │ compute_edges(VERT) → ...edges       │
  │ compute_blue_edges → blue assignment │
  │ hint_edges(VERT, 4 phases)           │
  │ vertical_separation → IUP            │
  └──────────────────────────────────────┘
  phantom adjust → save → rasterize ✓
```

**Nuance:** `pp1.x = -1` shifts all contour X coords by +1 FU. Without this shift
(our pre-fix code), 26.6 coordinates differ by 1 unit for some points, changing
the rasterizer DDA `prod` initialization. Fix: applied in scaler.rs, commit `04975f8`.

### Type 3: UPEM=1000 Bold (NotoSerifDisplay-Bold)

```
Font: UPEM=1000, bold, upright
pp1.x: 0 (xMin = lsb)
x_scale: 50332 (not identity → fractional 26.6 rounding)
near_limit: 20*1000/2048 = 9 (much smaller threshold)
near_limit2: 17

Key difference: Smaller UPEM means near_limit is ~9 FU instead of ~20 FU.
This changes which points the direction chain considers "near" — more points
get merged into direction-chain segments, altering u/v pointers and WEAK
classification.
Result: FIXED (9/9 '5' glyphs now pass)
         REMAINING: 'B' (gid=37) and 'g' (gid=74) still fail
```

```
reload → direction_chain (near_limit=9)
  → weak/strong: MORE points become NEAR at UPEM=1000
  → u/v deltas: different chain topology
  → WEAK flags: pt[20] incorrectly WEAK in Rust (was STRONG in C)
  → FIXED: corner_is_flat delta update + spike unconditional weak
  → Result: 9/18 '5' glyphs now MATCH

REMAINING: 'B' and 'g' — different glyph indices, different contour topologies.
           Same root cause mechanism (WEAK classification) but affecting
           different points in different contours.
```

### Type 4: Liberation Bold/Mono/NarrowItalic

```
Font: UPEM=2048, bold/mono/narrow, fpgm+prep tables present
pp1.x: 0
The fpgm/prep tables exist but FORCE_AUTOHINT bypasses native hinting.
These fonts have different stem-width profiles and may have different
segment thresholds due to bold/narrow characteristics.
Result: 3 still failing (LiberationMono 'l', LiberationSerif '$',
        LiberationSansNarrow ';')
```

---

## Critical Code Paths

### The Weak/Strong cascade (affects ALL 18 original failures)

```
build_direction_chain → sets u/v pointers
  → strong-vs-weak loop in reload:
    → Case 3 (both-None):
      → XOR check → corner_is_flat → delta update
        → different u/v for neighbor points
          → different WEAK flags for downstream points
            → align_strong_points skips WEAK points
              → different IUP reference pair
                → 1-2 unit coordinate difference
                  → render_conic subdivides differently
                    → render_line DDA endpoints differ
                      → cell cover/area accumulate differently
                        → 1-4 alpha unit pixel difference
```

### The pp1.x cascade (fixed — affected 309 original failures)

```
TT_Load_Glyph: pp1.x = xMin - lsb
  → FT_Outline_Translate(-pp1.x) shifts all X coords in FU
    → 26.6 coordinates differ by 1 unit for ~5% of points
      → renders: different cbox → different off_x/off_y
        → DDA endpoints differ → pixel mismatch
```

---

## Verification Matrix

| Font Category | Count | pp1.x fix | WEAK fix | Status |
|---------------|-------|-----------|----------|--------|
| Upright (UPEM=2048) | ~15,000 | ✅ | N/A | ALL PASS |
| Italic (UPEM=2048) | ~5,000 | ✅ | N/A | ALL PASS |
| ExtraLight (UPEM=2048) | ~1,500 | ✅ | N/A | ALL PASS |
| Math (UPEM=1000) | ~2,000 | ✅ | N/A | ALL PASS |
| Display Bold (UPEM=1000) '5' | 9 | ✅ | ✅ | **FIXED** |
| Display Bold (UPEM=1000) 'B' | 5 | ✅ | ❌ | REMAINING |
| Display Bold (UPEM=1000) 'g' | 1 | ✅ | ❌ | REMAINING |
| Liberation Bold | 1 | ✅ | ❌ | REMAINING |
| Liberation Mono 'l' | 1 | ✅ | ❌ | REMAINING |
| Liberation NarrowItalic | 1 | ✅ | ❌ | REMAINING |
| **TOTAL** | **27,695** | **-300** | **-9** | **27,686 pass** |

---

## Remaining 9 Failures — Category

| Glyph | Sizes | Category | Likely Cause |
|-------|-------|----------|-------------|
| NSDB 'B' (gid=37) | 10-24pt (5) | UPEM=1000, 3 contours | Same WEAK classification for a different contour topology |
| NSDB 'g' (gid=74) | 24pt (1) | UPEM=1000, descender | Same mechanism, more complex contour with descender |
| LiberationSerif '$' | 10pt (1) | UPEM=2048, bold | Different stem-width thresholds |
| LiberationMono 'l' | 16pt (1) | UPEM=2048, monospace | Monospace stem widths affect edge distance threshold |
| LiberationSansNarrow ';' | 20pt (1) | UPEM=2048, narrow, italic | NO_HORIZONTAL + narrow metrics |

The UPEM=1000 'B' and 'g' failures are the same root mechanism as '5' — the
small `near_limit` (9 FU) creates different direction-chain u/v pointers for
different contour topologies, flipping WEAK/STRONG classification. The
Liberation failures are UPEM=2048 with bold/narrow/mono characteristics that
affect stem-width computation and edge-distance thresholds.
