# Autohinter — Why Everything Works The Way It Does

This document explains **why** each component exists, **what problem** it solves,
and the **subtle interactions** between components. Read this when debugging a
parity difference: you know WHAT diverges, you need to understand WHY that matters.

---

## Pipeline Overview

```
RAW OUTLINE (FU)
      │
      ▼
  ┌──────────┐
  │  SCALER  │  Scale FU→26.6, apply pp1.x origin shift.
  │          │  Why pp1.x? TrueType defines glyphs relative to (0,0) at
  │          │  the left bearing. Without this shift, the outline is offset,
  │          │  producing different sub-pixel rasterization for italic fonts.
  └────┬─────┘
       │
       ▼
  ┌──────────┐
  │  RELOAD  │  Load coords, compute direction vectors.
  │          │  Build DIRECTION CHAIN: smooth curves made of many small
  │          │  edges should be treated as ONE smooth segment. Without this,
  │          │  a circle becomes many tiny segments, each getting its own
  │          │  edge, and hinting produces a jagged polygon.
  │          │  Classify WEAK vs STRONG: weak points lie on straight runs
  │          │  and get interpolated later; strong points are corners and
  │          │  get explicit grid-fitting. Wrong classification here causes
  │          │  IUP to pick wrong reference points (+1-2 unit drift).
  └────┬─────┘
       │
       ▼
  ┌───────────┐
  │  SEGMENTS │  Find horizontal/vertical runs of consecutive points.
  └────┬──────┘
       │
       ▼
  ┌─────────┐
  │  EDGES  │  Merge overlapping segments into edges.
  │         │  Why merge? Left side of 'H' = serif + stem + serif = 3
  │         │  segments but 1 edge → must snap to 1 pixel column.
  └────┬────┘
       │
       ▼
  ┌──────────────┐
  │  HINT EDGES  │  4-phase snapping:
  │              │  P1: Snap stem PAIRS to integer pixels (preserving width)
  │              │  P2: Snap serifs to linked stems
  │              │  P3: Snap to blue zones (baseline, cap-height, x-height)
  │              │  P4: Propagate alignment via anchor chains
  └────┬─────────┘
       │
       ▼
  ┌──────────────────────────────────────────┐
  │  ALIGN EDGE → STRONG → WEAK (IUP)        │
  │  Edge: snap points belonging to an edge  │
  │  Strong: interpolate corners between edges│
  │  IUP: interpolate weak points between     │
  │        strong anchors                     │
  │  Nuance: IUP result depends on which      │
  │  points were classified STRONG vs WEAK    │
  └──────────────────────────────────────────┘
       │
       ▼
  ┌──────────────────┐
  │  PHANTOM ADJUST  │  Shift glyph to pixel grid via pp1.x
  └──────────────────┘
       │
       ▼
  ┌────────────┐
  │  RASTERIZE │  DDA → cell cover/area → sweep → bitmap
  └────────────┘
```

---

## `build_direction_chain` — Preventing Curve Fragmentation

**Problem:** A smooth curve like 'O' has many contour points with slightly
different per-point directions. If `compute_segments` sees these raw directions,
it splits the curve into dozens of tiny segments → each gets its own edge →
each edge independently hinted → jagged circle, not a smooth one.

**Solution:** Walk the contour accumulating taxicab distance. When accumulated
distance exceeds `near_limit` (20 * UPEM / 2048 FU), the points between are
"non-near" — they form a single smooth run. Override all their in_dir/out_dir
to the accumulated direction. Now `compute_segments` sees one long segment
instead of many short ones.

**Critical nuance — UPEM dependency:** `near_limit = 20 * upem / 2048`.
- UPEM=2048: near_limit = 20 FU → sparse chain network
- UPEM=1000: near_limit = 9 FU → dense chain network (more points merge)

At UPEM=1000, MORE points get merged into chains because small coordinate
deltas quickly exceed 9 FU. This creates a fundamentally different direction-chain
topology, which feeds into different WEAK/STRONG classifications downstream.

---

## Strong-vs-Weak Classification — The Heart of IUP Accuracy

**Problem:** Which points need explicit grid-fitting (STRONG) and which can
be interpolated (WEAK)? Get this wrong and IUP picks different reference
anchors, shifting entire contour sections.

**Decision tree for each point:**

### Case 1: CONTROL flag set → always WEAK
Bézier control point. Position determined by on-curve endpoints.

### Case 2: in_dir == out_dir (both non-None) → always WEAK
Point lies on a straight segment, not a corner.

### Case 3: in_dir == out_dir == None ("both-None") — the tricky case

Two sequential sub-tests:

**Test A — XOR quadrant (afhints.c:1221-1245):**
If in-vector and out-vector share the same sign on both axes → WEAK.
The point is effectively on a straight run.

**Test B — corner_is_flat (afhints.c:1276-1290):**
Measures whether one vector dominates the other. If "flat enough" → WEAK.
**AND critically: updates direction-chain deltas** (`pv→u`, `nu→v`).

**The delta update is the key nuance.** When corner_is_flat returns true,
the code sets `pv->u = next_u - prev_v` and `next_u->v = -pv->u`. These
deltas change which neighbor points get consulted for DOWNSTREAM point
classifications. A point 5 indices away might see a different neighbor
because of a delta update on pt[20].

**Our bug (fixed in commit `1ecd364`):** The old code OR'd the two tests:
`xor || corner_is_flat(...)`. When XOR was true, the short-circuit OR
skipped `corner_is_flat` and its delta update. A downstream point saw an
old u/v value instead of the updated one → different WEAK classification
→ different IUP reference → +1 unit output → pixel mismatch.

### Case 4: in_dir == -out_dir (spike) → always WEAK
Direction reverses — sharp corner. Better interpolated than grid-fitted.

---

## The 12-Step Cascade — From Code to Pixel

This is the complete chain that caused the 18→9 reduction:

```
Step 1:  build_direction_chain sets u/v pointers on each point.
         At UPEM=1000, near_limit=9 FU → dense chain network.

Step 2:  u/v pointers determine neighbor points consulted in "both-None" case.

Step 3:  "both-None" runs XOR quadrant check → fails → corner_is_flat.

Step 4:  corner_is_flat returns true → C updates pv→u, nu→v deltas.
         Old Rust code OR'd the checks, skipping this update.

Step 5:  Downstream point classification sees stale u/v → different WEAK result.

Step 6:  pt[20] classified WEAK in Rust (was STRONG in C).

Step 7:  align_strong_points skips WEAK points → pt[20] stays at ox=62.

Step 8:  IUP walks contour looking for TOUCHED points.
         C finds pt[20] (touched). Rust skips to pt[21] (touched).

Step 9:  IUP reference pair differs:
         C: (pt[20]=62→33, pt[14]=271→256) → scale=69926
         R: (pt[21]=88→59,  pt[14]=271→256) → scale=70550

Step 10: Different scale → pt[15] = 201 (C) vs 200 (R). +1 unit in 26.6.

Step 11: +1 unit → render_conic subdivides differently.

Step 12: render_line DDA endpoints differ by 3 subpixel units →
         cell cover/area differs by 1 alpha unit → SHA mismatch.
```

A missing delta update on point 20 changes IUP output for point 15, which
changes pixel alpha at coordinate (2,5) by 1 unit out of 255. That 0.4%
alpha difference fails the SHA-256 test.

---

## Font Category Behavior

### Upright Serif (UPEM=2048)
```
near_limit: 20 FU → sparse direction chain
Most points get distinct u/v neighbors.
WEAK/STRONG classification is straightforward.
corner_is_flat rarely triggers → delta update issue doesn't surface.
Result: ALL PASS.
```

### Italic (UPEM=2048)  
```
NO_HORIZONTAL flag set → X-axis hinting skipped entirely.
pp1.x = -1 → shifts all contour X by +1 FU → different 26.6 coords.
Result: PASS (after pp1.x fix).
```

### UPEM=1000 Bold (NotoSerifDisplay)
```
near_limit: 9 FU → dense direction chain.
Many small coordinate deltas exceed 9 FU quickly → points merge into chains.
Dense network means delta updates propagate through more downstream points.
Result: '5' FIXED (9/9), 'B' and 'g' still fail (different contour topologies).
```

### Liberation Bold/Mono/NarrowItalic (UPEM=2048)
```
Bold: wider standard_width → larger edge_distance_threshold.
Mono: uniform advance widths → 'l' has only one real edge → IUP fragile.
Narrow italic: NO_HORIZONTAL + compressed metrics → dense point spacing.
Result: 3 still failing.
```

---

## Debugging Checklist

When a specific point diverges between C and Rust:

```
□ Run standalone binary for ONE glyph (not test suite) → unambiguous traces
□ Compare edge fpos/opos/pos after hint_edges → edges OK?
□ Compare touch flags after align_edge_points → segment assignment OK?
□ Compare WEAK flags from reload → classification matches C?
□ If WEAK differs: trace direction-chain u/v for that point
□ If u/v differs: compare corner_is_flat inputs (same neighbors?)
□ If neighbors differ: trace build_direction_chain backward walk
□ Compare IUP reference indices in iup_interp → same ref pair?
□ Compare final pixel hex dump → byte-identical to C?
```
