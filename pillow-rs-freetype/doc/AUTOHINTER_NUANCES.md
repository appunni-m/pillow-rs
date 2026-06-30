# pillow-rs-freetype Autohinter — Logic & Nuances Reference

**Date:** 2026-06-30 | **Status:** 27,686/27,695 pass (99.97%) | **9 failures remain**

This document explains **why** each component exists, **what problem** it solves,
and the **subtle interactions** between components. It is written for someone
debugging a parity difference: you know WHAT diverges, you need to understand
WHY that matters.

---

# Part 1: The Big Picture — What Problem Does Autohinting Solve?

A TrueType outline describes a glyph as a set of contour points in font units (FU).
At small sizes on screen (10-24pt), the outline maps to only a handful of pixels.
Without hinting, diagonal strokes blur, vertical stems have different widths, and
round parts of letters look asymmetric.

Autohinting analyzes the outline geometry and **snaps edges to pixel boundaries**
while preserving the intended proportions. The result is crisp, readable text
at small sizes.

The autohinter operates in two independent dimensions:
- **HORZ (X-axis):** Vertical stems, left/right bearings — controls glyph width
- **VERT (Y-axis):** Horizontal stems, baseline, cap-height, x-height

Each dimension is processed separately with its own edges, segments, and
alignment phases.

---

# Part 2: The Pipeline — Why This Order?

```
  RAW OUTLINE (FU)                     Final output must be crisp
       │                                     at this pixel size
       ▼
  ┌──────────┐
  │  SCALER  │  Scale FU→26.6, apply pp1.x origin shift.
  │          │  Why pp1.x? TrueType defines glyphs relative to
  │          │  (0,0) at the left bearing. Without this shift,
  │          │  the outline is offset, producing different sub-
  │          │  pixel rasterization for italic fonts.
  └────┬─────┘
       │
       ▼
  ┌──────────┐
  │  RELOAD  │  Load scaled coords into hint structure.
  │          │  Compute direction vectors (which way does the
  │          │  contour travel at each point?).
  │          │  
  │          │  Build DIRECTION CHAIN: smooth curves made of
  │          │  many small edges should be treated as ONE smooth
  │          │  segment. Without this, a circle becomes many
  │          │  tiny segments, each getting its own edge, and
  │          │  hinting produces a jagged polygon.
  │          │  
  │          │  Classify WEAK vs STRONG: weak points lie on
  │          │  straight/flat runs and get interpolated later;
  │          │  strong points are corners/inflections and get
  │          │  explicit grid-fitting. WRONG classification
  │          │  here causes the IUP to pick wrong reference
  │          │  points, producing 1-2 unit coordinate drift.
  └────┬─────┘
       │
       ▼
  ┌───────────┐
  │  SEGMENTS │  Find horizontal/vertical runs: groups of
  │           │  consecutive points moving in the same direction.
  │           │  A segment is a candidate for being a "stem edge"
  │           │  or a "round part" of the glyph.
  └────┬──────┘
       │
       ▼
  ┌─────────┐
  │  EDGES  │  Merge overlapping segments into edges.
  │         │  Why merge? The left side of an 'H' might be made
  │         │  of multiple contour segments (serif + stem + serif).
  │         │  They all belong to the SAME vertical edge and
  │         │  should snap to the SAME pixel column.
  │         │  
  │         │  Also assigns edges to BLUE ZONES (baseline,
  │         │  cap-height, x-height, descender). Blue zones
  │         │  are the "guide rails" that edges snap to.
  └────┬────┘
       │
       ▼
  ┌──────────────┐
  │  HINT EDGES  │  4-phase snapping:
  │              │  Phase 1: Snap stem PAIRS to integer pixels,
  │              │           preserving stem width.
  │              │  Phase 2: Snap serifs to their linked stems.
  │              │  Phase 3: Snap to blue zones (baseline etc).
  │              │  Phase 4: Propagate alignment to unaligned
  │              │           edges via anchor chains.
  └────┬─────────┘
       │
       ▼
  ┌────────────────────────────────────────────┐
  │  ALIGN EDGE POINTS → STRONG → WEAK (IUP)  │
  │                                            │
  │  align_edge: Snap points that belong to an │
  │  edge to that edge's hinted position.      │
  │                                            │
  │  align_strong: Grid-fit "strong" points    │
  │  (corners) by interpolating between the    │
  │  two nearest hinted edges.                 │
  │                                            │
  │  align_weak (IUP): Interpolate "weak"      │
  │  points (straight runs) between the        │
  │  nearest strong points.                    │
  │                                            │
  │  Nuance: the IUP result depends on which   │
  │  points were classified as STRONG vs WEAK. │
  │  If a point that C classifies as STRONG is │
  │  WEAK in our code, the IUP uses a different │
  │  reference pair → 1-2 unit output drift.   │
  └────────────────────────────────────────────┘
       │
       ▼
  ┌──────────────────┐
  │  PHANTOM ADJUST  │  Shift glyph to pixel grid using pp1.x.
  │                  │  Why here? The autohinter has now changed
  │                  │  the leftmost edge position. The phantom
  │                  │  adjustment re-aligns to the new pixel grid.
  └──────────────────┘
       │
       ▼
  ┌────────────┐
  │  RASTERIZE │  Convert hinted outline → 8-bit alpha bitmap.
  │            │  DDA stepping on each line segment, accumulating
  │            │  cell cover/area. Sweep per scanline → pixel.
  └────────────┘
```

---

# Part 3: Function Nuances — Why Things Are Done This Way

## `build_direction_chain` — Preventing Curve Fragmentation

**Problem:** A smooth curve like 'O' has many contour points with slightly
different per-point directions. If we let `compute_segments` see these raw
directions, it splits the curve into dozens of tiny segments. Each segment
gets its own edge. Each edge gets independently hinted. The result: a jagged
circle, not a smooth one.

**Solution:** Walk the contour accumulating taxicab distance. When the
accumulated distance exceeds `near_limit` (20 * UPEM / 2048 FU), the points
between are "non-near" — they form a single smooth run. Override all their
in_dir/out_dir to the accumulated direction. Now `compute_segments` sees
one long segment instead of many short ones.

**Nuance — UPEM dependency:** `near_limit = 20 * upem / 2048`. At UPEM=1000,
this is 9 FU. At UPEM=2048, it's 20 FU. At UPEM=1000, MORE points get merged
into chains (the threshold is more easily exceeded), creating a denser
direction-chain network. This is why UPEM=1000 fonts like NotoSerifDisplay
behave differently — their direction-chain topology is fundamentally different.

## Strong-vs-Weak Classification — The Heart of IUP Accuracy

**Problem:** Which points need explicit grid-fitting (STRONG) and which can
be interpolated (WEAK)? Get this wrong and the IUP picks different reference
anchors, shifting entire contour sections by 1-2 units.

**Decision tree for each point:**

### Case 1: CONTROL flag set
The point is a Bézier control point. Always WEAK — its position is determined
by the curve's on-curve endpoints.

### Case 2: in_dir == out_dir, both non-None
The point lies on a straight segment (not a corner). Always WEAK — straight
lines between strong corner points get interpolated.

### Case 3: in_dir == out_dir == None
**Both directions are "None"** — neither horizontal nor vertical. This is
the tricky case. Two sub-checks:

**3a. XOR quadrant test:** If the in-vector and out-vector point in the same
general direction (same sign on X, same sign on Y), the point is effectively
on a straight run → WEAK.

**3b. corner_is_flat test:** Measures whether one vector is much more dominant
than the other. If the corner is "flat enough" (the longer vector dominates),
the point is WEAK.

**Critical nuance that caused our bug:** When `corner_is_flat` returns true,
the code updates the direction-chain pointers (`pv->u` and `nu->v`). These
deltas change which neighbor points get consulted for DOWNSTREAM classifications.
Our old code OR'd the two checks: `xor || corner_is_flat(...)`. When XOR was
true, the short-circuit skipped `corner_is_flat` and its delta update. A
downstream point that should have seen the updated neighbor saw an old value
instead, flipping its WEAK/STRONG classification.

### Case 4: in_dir == -out_dir (spike)
The direction reverses — this is a sharp corner. Always WEAK.

**Why WEAK for a sharp corner?** Because the strong-point interpolation would
try to grid-fit it, but a spike has no meaningful geometric position to snap
to. It's better to let IUP interpolate it from surrounding strong anchors.

## `compute_edges` — Why Segments Need to Merge

**Problem:** The left side of a serif 'H' might be composed of:
- Top serif: points 0-5, direction Up
- Main stem: points 5-15, direction Up
- Bottom serif: points 15-20, direction Up

These are three separate segments from `compute_segments`, but they all
belong to the SAME vertical edge. If they don't merge, each gets its own
edge position, and the stem appears wavy instead of one crisp column.

**Solution:** `compute_edges` uses an `edge_distance_threshold` (standard_width / 5)
to decide when segments are "at the same position" and should merge.

**Nuance:** The threshold comes from `metrics_scale_dim`, which scales the
standard width by the current ppem. At small sizes, the threshold shrinks,
allowing tighter segment grouping.

## `hint_edges` — The 4-Phase Dance

**Why 4 phases?** Edge positions are interdependent. You cannot snap a stem
pair to pixels without knowing the stem width. You cannot snap a serif without
knowing where its linked stem landed. You cannot snap to a blue zone without
first establishing which edges are stems vs serifs.

**Phase 1 — Stem Alignment:** Find linked edge pairs (stem pairs). Compute the
standard stem width for this font/size. Adjust both edges to integer pixel
positions while keeping the width correct. Example: stem width = 2.3px →
snap to 2px (edges[0]=0, edges[1]=2).

**Phase 2 — Serif Alignment:** Serifs are linked to a stem edge. Snap the
serif to the same position as its stem, minus the original offset.
Example: serif was 0.1px left of stem → after snap, still 0 or 1px left.

**Phase 3 — Blue Zone Alignment:** Edges assigned to blue zones (baseline,
cap-height) get snapped to the blue zone's grid position. Blue zones have
pre-computed "shoot" positions at integer pixel heights.

**Phase 4 — Anchor Alignment:** After phases 1-3, some edges may still be
unaligned. They get anchored to the nearest aligned edge via the edge link
chain, preserving their relative offset.

## `align_strong_points` — Bridging Edges and IUP

**Problem:** Strong points (corners) lie BETWEEN edges. They need to move
with the hinted edge positions, not snap to an edge directly.

**Algorithm:** For each strong point, find the two edges that bracket its
font-unit position. Interpolate linearly: `new_pos = before_edge.hinted_pos
+ scale * (point_fu - before_edge.fu)`. The scale is computed from the
two edges' pre/post hinting positions.

**Nuance — why WEAK matters:** Points classified as WEAK skip this function
entirely. They go to IUP instead. If a point C classifies as STRONG but we
classify as WEAK, it doesn't get grid-fitted → different starting value for
IUP → IUP picks different reference → cascade of coordinate differences.

## `align_weak_points` (IUP) — The Interpolation Engine

**Problem:** How to move all remaining (weak) points so they follow the
hinted strong-point skeleton?

**Algorithm:** Walk each contour. Find consecutive TOUCHED (strong) points.
All weak points between them get linearly interpolated:
`weak.u = strong1.u + scale * (weak.v - strong1.v)`

**Nuance — reference selection order matters:** The contour is walked from
start. The first TOUCHED point found becomes ref1, the next becomes ref2.
If a point that SHOULD be touched isn't (wrong WEAK classification), the
walk skips it and finds the NEXT touched point instead. Different ref →
different scale → different interpolation → 1-2 unit drift.

---

# Part 4: Font Category Behavior

## Upright Serif (DejaVuSerif, LiberationSerif — not italic)

```
Characteristics: UPEM=2048, both HORZ and VERT hinting active
near_limit: 20 FU
pp1.x: ~0 (xMin ≈ lsb)

Pipeline flow:
  1. reload → direction chain builds sparse u/v network (near_limit=20)
  2. compute_segments(HORZ) → 6-8 segments (balanced straights + curves)
  3. compute_edges(HORZ) → 5-8 edges (serif stems + main stems)
  4. hint_edges(HORZ) → 4-phase snap to pixel grid
  5. Same for VERT dimension
  6. Strong classification is straightforward at UPEM=2048 (fewer near points)
  7. IUP produces correct interpolation
  8. Result: MATCHES C

Why this category passes: The large near_limit (20 FU) creates sparse direction
chains. Most points get distinct u/v neighbors. WEAK/STRONG classification
is unambiguous. The corner_is_flat test rarely triggers, so the delta update
issue doesn't surface.
```

## Italic Serif (DejaVuSerif-Italic, DejaVuSerifCondensed-Italic)

```
Characteristics: UPEM=2048, italic → NO HORIZONTAL hinting
pp1.x: -1 (xMin differs from lsb by 1 FU)

Pipeline flow:
  1. reload → direction chain same as upright
  2. HORZ pipeline SKIPPED entirely (AF_SCALER_FLAG_NO_HORIZONTAL)
  3. VERT pipeline runs normally
  4. pp1.x = -1 FU applied → shifts all contour X coords by +1 FU
  5. Result: MATCHES C (after pp1.x fix in commit 04975f8)

Why pp1.x matters: The +1 FU shift changes 26.6 coordinates for approximately
5% of points (those whose scaled value crosses a rounding threshold). This
changes the rasterizer DDA `prod` initialization, producing different cell
cover/area for those pixels. The entire 309→18 reduction came from this one fix.

Why NO HORIZONTAL for italic: The italic slant makes vertical stems unreliable
for edge detection. The autohinter skips HORZ and only hints Y-axis features
(horizontal stems: crossbars, serifs, baseline alignment).
```

## UPEM=1000 Bold (NotoSerifDisplay-Bold, NotoSerifDisplay-BoldItalic)

```
Characteristics: UPEM=1000, bold
near_limit: 9 FU (tight — many points become "near")
x_scale: 50332 (not identity → 26.6 rounding introduces fractional units)

Pipeline flow:
  1. reload → direction chain builds DENSE u/v network (near_limit=9)
     Many points merge into direction chains because the accumulation of
     small coordinate deltas quickly exceeds 9 FU.
  2. The dense network means u/v deltas affect NEIGHBOR classification.
     A delta update from corner_is_flat changes which neighbor a downstream
     point consults.
  3. compute_segments sees fewer but larger segments (more points merged).
  4. compute_edges produces FEWER edges than UPEM=2048 equivalents.
  5. Strong classification is subtle: the XOR test and corner_is_flat
     interact through the direction-chain deltas.
  6. Result: FIXED for '5' glyphs (9/9 now pass).
     STILL FAILING for 'B' (gid=37, 3 contours) and 'g' (gid=74, descender).

Why UPEM=1000 is different: At UPEM=1000, 1 FU = 1 26.6 unit at 12pt (x_scale
nearly identity). The near_limit is 9 FU instead of 20. This means a point
shift of 9 FU triggers a chain transition, vs 20 FU at UPEM=2048. Points that
would be "non-near" at UPEM=2048 become "near" at UPEM=1000, changing the
entire direction-chain topology.

Why 'B' and 'g' still fail: These glyphs have different contour topologies.
'B' has 3 contours (outer, upper bowl, lower bowl). Each has its own
direction-chain walk. The tighter near_limit affects each contour differently.
'g' has a descender that extends below the baseline, creating an additional
direction-chain segment with different u/v relationships.

Why the fix only partially worked: The XOR/corner_is_flat fix corrected the
strong-vs-weak logic itself, but the DIRECTION CHAIN deltas (u/v pointers)
that feed into that logic still differ from C. At UPEM=1000 with near_limit=9,
small differences in the backward-walk starting point or the accumulated
taxicab distance can produce completely different chain topologies, even
when the classification logic itself is correct.
```

## Liberation Bold/Mono/NarrowItalic

```
Characteristics: UPEM=2048, bold/mono/narrow variants
fpgm+prep tables present but FORCE_AUTOHINT bypasses native hinting
Standard stem widths differ from "regular" fonts

Pipeline flow:
  Fonts with bold weight have wider standard_width → larger
  edge_distance_threshold (standard_width/5) in compute_edges. This
  changes which segments merge into which edges.

  Monospaced fonts (LiberationMono) have uniform advance widths. The
  'l' glyph is a simple vertical stroke — only one real edge, making
  IUP reference selection more sensitive to which points are touched.

  Narrow italic fonts (LiberationSansNarrow-BoldItalic) have NO_HORIZONTAL
  plus compressed metrics. The narrow width means smaller inter-point
  distances, affecting the near/non-near classification in the
  direction chain.

Result: 3 still failing.

Why these are hard: They're edge cases in the metrics. Bold changes stem-width
thresholds. Mono changes the expected edge count for simple glyphs. Narrow
changes the point density. Each interacts with the direction-chain topology
and WEAK/STRONG classification in a different way.
```

---

# Part 5: The WEAK_INTERPOLATION Cascade — 12 Steps from Code to Pixel

This is the complete chain that caused the 18→9 reduction. Understanding this
cascade is essential for fixing the remaining 9.

```
Step 1: build_direction_chain sets u/v pointers on each point.
        At UPEM=1000, near_limit=9 FU → dense chain network.

Step 2: The u/v pointers determine which neighbor points are consulted
        in the "both-None" classification case (Case 3).

Step 3: The "both-None" case runs two sub-checks:
        a. XOR quadrant (same sign on both axes?)
        b. corner_is_flat (one vector dominates?)
        
Step 4: When corner_is_flat returns true, C updates:
          pv->u = next_u - prev_v     (backward neighbor's forward pointer)
          next_u->v = -pv->u          (forward neighbor's backward pointer)
        These deltas CHANGE which points are consulted in Step 2 for
        DOWNSTREAM classifications.

Step 5: Our old code OR'd the two checks. Short-circuit skipped
        the delta update. Downstream points saw old u/v values.

Step 6: Different u/v values → different inputs to corner_is_flat
        on downstream points → different WEAK/STRONG result.

Step 7: pt[20] classified as WEAK in Rust (was STRONG in C).

Step 8: align_strong_points skips WEAK points → pt[20] stays at ox=62
        instead of being interpolated to x=33.

Step 9: align_weak_points walks the contour looking for TOUCHED points.
        C finds pt[20] (touched), Rust skips to pt[21] (touched).

Step 10: IUP reference pair changes:
         C: (pt[20]=62→33, pt[14]=271→256) → scale=69926
         R: (pt[21]=88→59, pt[14]=271→256) → scale=70550

Step 11: Different scale → pt[15] interpolates to 201 (C) vs 200 (R).
         +1 unit in 26.6 coordinates.

Step 12: +1 unit → render_conic midpoint subdivides differently →
         render_line DDA endpoints differ by 3 subpixel units →
         cell cover/area accumulates 1 alpha unit differently →
         SHA-256 mismatch.

Moral: A missing delta update on point 20 changes IUP output for point 15,
       which changes pixel alpha at coordinate (2,5) by 1 unit out of 255.
       That 0.4% alpha difference is enough to fail the SHA-256 test.
```

---

# Part 6: Debugging Protocols

## Finding WEAK/STRONG Classification Bugs

1. Run standalone binary for ONE glyph (not test suite)
2. Trace `reload`'s WEAK flags for the specific point that diverges
3. If WEAK flags differ, trace the direction-chain u/v for that point
4. Compare C's `corner_is_flat` inputs — are they the same neighbors?
5. If neighbors differ, trace `build_direction_chain` backward walk
   — different `first` point? Different accumulated distances?
6. Fix the delta update or the chain construction

## Finding IUP Reference Pair Bugs

1. Trace `iup_interp` call with the gap indices
2. If gap indices differ, trace which points are TOUCHED after `align_strong`
3. If touch flags differ, trace WEAK flags from `reload` (see above)
4. If everything matches but IUP output still differs, check `ft_mul_fix`
   rounding for the specific numeric range (exhaustive tests cover this)

## Verification Checklist

```
□ One glyph standalone binary → unambiguous traces
□ Outline after autohint → point coordinates match C?          → scaler OK
□ Edge fpos/opos/pos after hint_edges → match C?              → edges OK
□ Touch flags after align_edge_points → match C?              → segments OK
□ Strong-point positions after align_strong → match C?        → interpolation OK
□ IUP reference indices in iup_interp → match C?              → classification OK
□ IUP output values → match C?                                → computation OK
□ Final pixel hex dump → byte-identical to C?                 → COMPLETE
```
