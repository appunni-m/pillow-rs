# pillow-rs-freetype Rustification Design

> **Status:** Design document — not implemented.
> **Scope:** Replace C-port style with idiomatic Rust. Target: single-glyph WASM execution.
> **Constraint:** Must preserve byte-identical output vs FreeType 2.14.1 (1,708+ parity tests pass).

---

## 1. Current State Assessment

### 1.1 What We Have

A faithful C-to-Rust port of FreeType 2.14.1's Latin autohinter + smooth rasterizer.
The pipeline is ~3,800 lines of Rust across four modules:

| Module | Lines | Role |
|--------|-------|------|
| `scaler.rs` | 313 | Font-units → 26.6 fixed-point. `ft_mul_fix` per point. |
| `autohint/loader.rs` | 311 | Load raw outline → `AFPoint` doubly-linked circular lists. Direction + strong/weak classification. |
| `autohint/latin.rs` | 2,343 | 7-pass autohinter: segments → edges → grid-fit → interpolate. |
| `grays.rs` | 800 | DDA scan-convert → cell accumulation → sweep → 8-bit alpha bitmap. |

The code is algorithmically correct — 1,708 coverage tests pass — but structurally it is
**C translated to Rust syntax**, not idiomatic Rust. It reproduces C's mutable-state-heavy,
deeply-nested-loop style, using `clone()` and `to_vec()` as escape hatches from the
borrow checker.

### 1.2 Why "C Port" Style Costs Performance

In C, the autohinter uses raw pointers into pre-allocated persistent slabs. There is no
borrow checker, no ownership tracking. A function can hold a `const AF_Edge*` (reading
edge positions) while simultaneously writing `AF_Point*` fields. Zero allocations, zero
copies, zero overhead.

Rust's borrow checker forbids this: `hints.axis[dim]` (reading edges) and
`hints.points[i]` (writing points) are both borrows on `hints`. The current code
resolves this by **cloning entire data structures** to split the borrows:

```rust
// line 2134: clone entire AxisHints (Vec<AFEdge>) to read edges while mutating points
let axis_snapshot = hints.axis[dim as usize].clone();

// line 858: clone Vec<usize> contour indices to iterate while mutating segments
let contours: Vec<usize> = hints.contours.clone();

// line 547: clone full AfLatinMetrics (including Vec<AfLatinBlue>, Vec<bool>)
let metrics = match hints.metrics { Some(ref m) => m.clone(), None => return };

// line 276: copy entire scaled points Vec into outline
outline.points = scaled.to_vec();
```

These clones are the direct Rust-compensating-for-borrow-checker. Each one is 1+ heap
allocations and a memcpy that the C original never does.

### 1.3 Quantified Overhead: Per-Glyph Allocations

For a typical 38-point glyph at 10pt:

| Allocation | Location | Size | Frequency |
|------------|----------|------|-----------|
| `Vec<AFEdge>` clone | `align_strong_points` line 2134 | ~120 B | 2× (Horz + Vert) |
| `Vec<usize>` clone | `compute_segments` line 858 | ~24 B | 2× |
| `Vec<usize>` clone | `align_weak_points` line 2283 | ~24 B | 2× |
| `AfLatinMetrics` clone | `compute_blue_edges` line 547 | ~200 B | 1× |
| `OutlinePoint` to_vec | `scaler.rs` line 276 | ~608 B | 1× |
| `Vec<Cell>` × N scanlines | `grays.rs` line 760 | ~40 B each × 12 | 12× |
| **Total** | | ~1,600 B across ~20 allocations | per glyph |

For a 72pt glyph with 80 scanlines: ~80 `Vec<Cell>` allocations alone.

Compare to FreeType C: **zero allocations per glyph** after warmup. Everything lives
in persistent slabs.

### 1.4 WASM-Specific Penalties

- **`i64` promotion**: `ft_mul_fix(a: i32, b: i32)` does `(a as i64 * b as i64) >> 16`.
  On WASM (32-bit target), `i64` multiply is emulated in software — 8-15× slower than
  native `i32` multiply. This is called for every point in the scaler and autohinter.

- **Allocator overhead**: WASM's default `dlmalloc` has higher per-call overhead than
  native `malloc`. The ~20 per-glyph allocations hit this 20× more often than necessary.

- **No SIMD**: WASM MVP has no SIMD instructions, so the autohinter's per-point loops
  can't benefit from auto-vectorization. Cleaner structure helps the optimizer produce
  better scalar code.

---

## 2. The Rustification Principles

### 2.1 Owned Data Flow Between Passes

Instead of one big `GlyphHints` struct that every pass mutates, each pass takes
immutable input and produces owned output:

```
CURRENT (one mutable struct shared by all passes):
  GlyphHints ──compute_segments──┐
       ↑                         ↓  (mutates hints.axis[dim].segments)
       └────link_segments────────┘
       └────compute_edges────────┘
       └────hint_edges───────────┘
       └────align_edge_points────┘
       └────align_strong_points──┘
       └────align_weak_points────┘

PROPOSED (data flows forward through pure transformations):
  Points ──→ Segments ──→ StemPairs ──→ Edges ──→ FittedEdges ──→ HintedPoints
```

Each arrow is a function `fn(A) -> B`. No function mutates shared state.
The compiler sees `&[T]` → `Vec<U>` with no aliasing, enabling aggressive
optimizations (vectorization, loop fusion, dead-store elimination).

### 2.2 Principles Applied

| Principle | Current (C-port) | Proposed (Rust) |
|-----------|-----------------|-----------------|
| **Data ownership** | Shared `&mut GlyphHints` everywhere | Each pass owns its output |
| **Borrow splitting** | `clone()` to escape borrow checker | No overlapping borrows by construction |
| **Loops** | Nested state-machine with 10+ mutable vars | Sort + single-pass group, or iterator chains |
| **Allocation** | `Vec::push` + `Vec::clone` per glyph | Pre-allocated scratch, `Vec::clear()` reuse |
| **Arithmetic** | `i64` promotions everywhere | `i32`-only fast paths when scale is invariant |
| **Edge search** | O(n²) nested loop: for each seg, scan all edges | Sort + single pass grouping: O(n log n) |
| **IUP** | `loop { find next touched; skip; interpolate }` | Collect touched indices, then apply ranges |
| **Raster cells** | `Vec<Vec<Cell>>` — 1 allocation per scanline | Flat array + intrusive linked list — 1 allocation total |

---

## 3. Concrete Transformations

### 3.1 Eliminate Clones — Borrow Restructuring

#### 3.1.1 `axis_snapshot.clone()` in `align_strong_points` (line 2134)

**Current**:
```rust
fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis_snapshot = hints.axis[dim as usize].clone(); // ALLOCATION
    let axis = &axis_snapshot;
    // ...
    for i in 0..hints.num_points() {
        // reads axis.edges[nn].pos
        // writes hints.points[i].x
    }
}
```

**Fix**: Extract only the fields needed into a stack-allocated array.
```rust
/// Minimal snapshot: only the three fields read by strong-point interpolation.
#[derive(Copy, Clone)]
struct EdgeInterpRef {
    fpos: i16,
    opos: i32,
    pos: i32,
}

fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    // Build stack-allocated snapshot. A glyph rarely exceeds 32 edges.
    let edges: heapless::Vec<EdgeInterpRef, 32> = hints.axis[dim as usize]
        .edges.iter()
        .map(|e| EdgeInterpRef { fpos: e.fpos, opos: e.opos, pos: e.pos })
        .collect();
    // ... use edges[] instead of axis_snapshot ...
}
```

**Impact**: Eliminates 1 `Vec<AFEdge>` allocation per dimension, per glyph.

#### 3.1.2 `contours.clone()` in `compute_segments` (line 858)

**Current**:
```rust
fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    let contours: Vec<usize> = hints.contours.clone(); // ALLOCATION
    for &contour0 in &contours {
        // mutates hints.axis[dim].segments
    }
}
```

**Fix**: Iterate by index — the contour list doesn't change.
```rust
fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    for ci in 0..hints.contours.len() {
        let contour0 = hints.contours[ci];
        // mutates hints.axis[dim].segments — fine, different borrow
    }
}
```

**Impact**: Eliminates 2 allocations per `apply_hints` call (Horz + Vert).

#### 3.1.3 `metrics.clone()` in `compute_blue_edges` (line 547)

**Current**:
```rust
let metrics = match hints.metrics {
    Some(ref m) => m.clone(),  // ALLOCATION: clones Vec<AfLatinBlue> + Vec<bool>
    None => return,
};
```

**Fix**: Just borrow.
```rust
let metrics = match hints.metrics.as_ref() {
    Some(m) => m,
    None => return,
};
```

**Impact**: Eliminates 1 medium allocation per glyph.

---

### 3.2 Segment Detection — Replace State Machine with Functional Pipeline

**Current**: `compute_segments` is 200+ lines with 12 mutable accumulators tracking
position, direction, extent, merge state, and edge linkage — all mutated inside a
`loop {}` that walks contour points.

**Proposed**: Three pure functions.

```rust
/// Step 1: Classify each point's effective direction for this dimension.
/// Pure iteration — no mutable state.
fn classify_point_directions(
    points: &[AFPoint],
    dim: Dimension,
) -> Vec<Direction> {
    let is_horz = dim == Dimension::Horz;
    points.iter().map(|p| {
        let dir = if is_horz { p.out_dir } else { p.out_dir };
        abs_dir(dir)
    }).collect()
}

/// Step 2: Find contiguous runs of points with matching direction.
/// Window scan, O(n) single pass.
fn find_segment_runs(
    dirs: &[Direction],
    major_dir: Direction,
) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < dirs.len() {
        while i < dirs.len() && dirs[i] != major_dir { i += 1; }
        if i >= dirs.len() { break; }
        let start = i;
        while i < dirs.len() && dirs[i] == major_dir { i += 1; }
        if i - start >= 2 {
            runs.push(start..i);
        }
    }
    runs
}

/// Step 3: Build segment properties from a point range.
/// Pure function — no side effects.
fn build_segment(
    points: &[AFPoint],
    range: std::ops::Range<usize>,
    dir: Direction,
    dim: Dimension,
) -> Option<SegmentCandidate> {
    if range.len() < 2 { return None; }

    let is_horz = dim == Dimension::Horz;
    let mut pos_sum: i64 = 0;
    let mut min_pos = i32::MAX;
    let mut max_pos = i32::MIN;
    let mut min_coord = i32::MAX;
    let mut max_coord = i32::MIN;

    for &i in &range {
        let p = &points[i];
        let (pos, coord) = if is_horz {
            (p.fx as i32, p.fy as i32)
        } else {
            (p.fy as i32, p.fx as i32)
        };
        pos_sum += pos as i64;
        min_pos = min_pos.min(pos);
        max_pos = max_pos.max(pos);
        min_coord = min_coord.min(coord);
        max_coord = max_coord.max(coord);
    }

    let height = max_coord - min_coord;
    if height < 1 { return None; }

    let avg_pos = (pos_sum / range.len() as i64) as i32;
    Some(SegmentCandidate {
        first: range.start,
        last: range.end - 1,
        dir,
        pos: avg_pos,
        delta: max_pos - min_pos,
        min_coord,
        max_coord,
        height,
    })
}
```

**Impact**: 200+ lines → ~80 lines. Each function testable independently.
The inner `for &i in &range` loop in `build_segment` is auto-vectorizable
by the compiler (no branches, no aliasing, known stride).

---

### 3.3 Edge Clustering — Replace O(n²) Nested Loop with Sort + Group

**Current**: For each segment, scan ALL existing edges for positional match:
```rust
for seg_idx in 0..axis.segments.len() {
    // ... filter ...
    for e_idx in 0..axis.edges.len() {
        if edge.dir == seg_dir && (edge.fpos - seg_pos).abs() < edge_dist_thresh {
            found_edge = e_idx;
            break;
        }
    }
}
```
This is O(n²) — each of N segments checks against up to N edges.

**Proposed**: Sort segments by position, single-pass group runs within threshold.
```rust
fn cluster_edges(
    segments: &[SegmentCandidate],
    threshold: i32,
) -> Vec<Vec<usize>> {
    // Sort by position
    let mut indexed: Vec<(usize, i32)> = segments.iter()
        .enumerate()
        .map(|(i, s)| (i, s.pos))
        .collect();
    indexed.sort_by_key(|&(_, pos)| pos);

    // Single pass: group runs within threshold
    let mut edges: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut last_pos: Option<i32> = None;

    for (seg_idx, pos) in indexed {
        match last_pos {
            Some(lp) if (pos - lp).abs() <= threshold => {
                current.push(seg_idx);
            }
            _ => {
                if !current.is_empty() {
                    edges.push(std::mem::take(&mut current));
                    current.clear();
                }
                current.push(seg_idx);
            }
        }
        last_pos = Some(pos);
    }
    if !current.is_empty() {
        edges.push(current);
    }
    edges
}
```

**Impact**: O(n²) → O(n log n). For glyphs with many segments (Chinese characters,
complex Latin ligatures), this matters.

---

### 3.4 IUP — Replace State-Machine Loop with Collect-Then-Apply

**Current**: 60 lines with nested `loop {}` blocks walking contour points, finding
"next touched point", advancing cursors, accumulating state.

**Proposed**: Pre-collect touched indices, compute ranges, then apply.
```rust
struct IupRange {
    from: usize,      // first untouched point (inclusive)
    to: usize,        // last untouched point (inclusive)
    ref_left: usize,  // left reference (touched point)
    ref_right: usize, // right reference (touched point), same as left for uniform shift
}

fn build_iup_ranges(
    points: &[AFPoint],
    contours: &[usize],
    touch_flag: u16,
) -> Vec<IupRange> {
    let mut ranges = Vec::new();

    for &c_start in contours {
        let end = points[c_start].prev;

        // Collect touched indices
        let touched: Vec<usize> = (c_start..=end)
            .filter(|&i| points[i].flags & touch_flag != 0)
            .collect();

        if touched.is_empty() { continue; }

        if touched.len() == 1 {
            // Uniform shift from single reference
            let t = touched[0];
            if t > c_start {
                ranges.push(IupRange { from: c_start, to: t - 1, ref_left: t, ref_right: t });
            }
            if t < end {
                ranges.push(IupRange { from: t + 1, to: end, ref_left: t, ref_right: t });
            }
        } else {
            // Interpolate between consecutive pairs
            for w in touched.windows(2) {
                let (a, b) = (w[0], w[1]);
                if a + 1 < b {
                    ranges.push(IupRange { from: a + 1, to: b - 1, ref_left: a, ref_right: b });
                }
            }
            // Wrap-around gap
            let first = touched[0];
            let last = *touched.last().unwrap();
            if last < end {
                ranges.push(IupRange { from: last + 1, to: end, ref_left: last, ref_right: first });
            }
            if first > c_start {
                ranges.push(IupRange { from: c_start, to: first - 1, ref_left: last, ref_right: first });
            }
        }
    }

    ranges
}
```

**Impact**: 60 lines → ~80 lines but vastly simpler. The "find next touched" state
machine becomes `touched.windows(2)`. The inner interpolation loops are pure and
auto-vectorizable.

---

### 3.5 `hint_edges` — Replace Phase Flags with Priority-Sorted Worklist

**Current**: 400 lines of 4 sequential phases. Each phase walks all edges, sets
`AF_EDGE_DONE` on processed edges, and the next phase skips those. This is inherently
sequential at the algorithm level, but the code structure makes it worse.

**Proposed**: Classify each edge's fit request into a priority, sort, process in one pass.

```rust
/// What priority should this edge be processed at?
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum FitPriority {
    BlueZone = 0,   // phase 1: edges anchored to blue zones
    StemAnchor = 1,  // phase 2: first edge of a stem pair
    StemLinked = 2,  // phase 2: second edge, aligned relative to anchor
    Serif = 3,       // phase 3: serif edges, aligned relative to base
    Remainder = 4,   // phase 4: everything else, pixel-rounded
}

enum FitTarget {
    BlueWidth(AfWidth),
    LinkedTo(usize),
    PixelRound,
}

struct EdgeFitRequest {
    edge_idx: usize,
    priority: FitPriority,
    target: FitTarget,
}

fn fit_edges(
    edges: &mut [AFEdge],
    requests: &[EdgeFitRequest],
    std_widths: &[i32],
    ppem: i32,
    extra_light: bool,
) {
    // Sort by priority — higher-priority edges processed first
    let mut sorted: Vec<usize> = (0..requests.len()).collect();
    sorted.sort_by_key(|&i| requests[i].priority);

    for &req_idx in &sorted {
        let req = &requests[req_idx];
        let e = req.edge_idx;

        if edges[e].flags & AF_EDGE_DONE != 0 { continue; }

        match req.target {
            FitTarget::BlueWidth(w) => {
                edges[e].pos = w.fit;
            }
            FitTarget::LinkedTo(base) => {
                let dist = edges[e].opos - edges[base].opos;
                let base_delta = edges[base].pos - edges[base].opos;
                let fitted_width = compute_stem_width(
                    /* ... */, dist, base_delta,
                    edges[base].flags, edges[e].flags,
                    std_widths, ppem, extra_light,
                );
                edges[e].pos = edges[base].pos + fitted_width;
            }
            FitTarget::PixelRound => {
                edges[e].pos = (edges[e].opos + 32) & !63; // FT_PIX_ROUND
            }
        }

        edges[e].flags |= AF_EDGE_DONE;
    }
}
```

**Impact**: 400 lines → ~120 lines (build requests + process). The 4-phase sequential
logic becomes a `sort_by_key`. Edge ordering constraints are explicit in the priority
enum, not implicit in loop order.

---

### 3.6 Rasterizer — Replace `Vec<Vec<Cell>>` with Flat Array + Linked List

**Current**: Each scanline gets its own `Vec<Cell>`. Cells are inserted in sorted order
via `vec.insert(idx, cell)` — O(n) per insertion. For a 72pt glyph with 80 scanlines
× 30 cells each, that's 80 heap allocations + ~1,200 element shifts.

**Proposed**: Single `Vec<Cell>` for all scanlines, intrusive linked list per scanline.

```rust
#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    x: i32,
    cover: i32,
    area: i32,
    next: usize,  // index into cells[] of next cell on same scanline; MAX = end
}

struct Worker {
    /// Single pre-allocated cell buffer. Grows if needed, never shrinks.
    cells: Vec<Cell>,
    next_free: usize,

    /// Per-scanline linked list heads. usize::MAX = empty.
    scanline_heads: Vec<usize>,
    /// Per-scanline tails for O(1) append when cells arrive left-to-right.
    scanline_tails: Vec<usize>,

    // ... (target, width, height, flags unchanged) ...
}

impl Worker {
    fn new(width: usize, height: usize, flags: u32) -> Self {
        let max_cells = width * 4; // generous estimate
        Worker {
            cells: Vec::with_capacity(max_cells),
            next_free: 0,
            scanline_heads: vec![usize::MAX; height],
            scanline_tails: vec![usize::MAX; height],
            // ...
        }
    }

    /// Insert cell at (ex, ey). Fast path: O(1) append when cells arrive in
    /// left-to-right order from the DDA line renderer (the common case).
    fn set_cell(&mut self, ex: i32, ey: i32) {
        let ey_idx = (ey - self.min_ey) as usize;
        if ey_idx >= self.scanline_heads.len() { return; }

        // FAST PATH: append at tail
        let tail = self.scanline_tails[ey_idx];
        if tail == usize::MAX || self.cells[tail].x < ex {
            let new_idx = self.alloc_cell(ex);
            if tail != usize::MAX {
                self.cells[tail].next = new_idx;
            } else {
                self.scanline_heads[ey_idx] = new_idx;
            }
            self.scanline_tails[ey_idx] = new_idx;
            return;
        }

        // SLOW PATH: binary-search insert (only for out-of-order arrival,
        // e.g., reversed contour segments — very rare).
        // ... (walk linked list, insert, update head/tail) ...
    }

    fn alloc_cell(&mut self, x: i32) -> usize {
        if self.next_free >= self.cells.len() {
            // Initial allocation is generous. Resize only if genuinely exceeded.
            self.cells.resize(self.cells.len() * 2, Cell::default());
        }
        let idx = self.next_free;
        self.cells[idx] = Cell { x, cover: 0, area: 0, next: usize::MAX };
        self.next_free += 1;
        idx
    }
}
```

**Impact**: 1 allocation per glyph instead of 1 per scanline. O(1) insertion in the
common case. Scanline iteration becomes a clean linked-list walk — each scanline is
independent, ready for SIMD or rayon on native.

---

### 3.7 `ft_mul_fix` Without `i64` Promotion (WASM Critical)

**Current**:
```rust
pub fn ft_mul_fix(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64 + 0x8000) >> 16) as i32
}
```
On WASM (32-bit target), `i64` multiply is emulated — ~10× slower than `i32`.

**Proposed**: When `b` (the scale) is invariant for a loop, pre-split into hi/lo and
use i32-only arithmetic:

```rust
/// Pre-split scale for use with `ft_mul_fix_fast`.
#[derive(Copy, Clone)]
struct Scale16_16 {
    hi: i32,   // b >> 16  (integer part * 2^16)
    lo: u16,   // b & 0xFFFF (fractional part)
}

impl Scale16_16 {
    fn new(scale: i32) -> Self {
        Scale16_16 {
            hi: scale >> 16,
            lo: (scale as u32 & 0xFFFF) as u16,
        }
    }
}

/// Fast i32-only fixed-point multiply. No i64 promotion.
/// Equivalent to FT_MulFix(a, scale) when scale is pre-split.
#[inline]
fn ft_mul_fix_fast(a: i32, scale: Scale16_16) -> i32 {
    let hi = a.wrapping_mul(scale.hi);
    let lo = (a as u32).wrapping_mul(scale.lo as u32);
    ((hi as u32).wrapping_add(lo >> 16))
        .wrapping_add(0x8000) as i32 >> 16
}
```

Apply in the scaler loop (one invariant scale for all points):
```rust
let x_scale = Scale16_16::new(scale.x_scale);
let y_scale = Scale16_16::new(y_adj);
for p in &raw_outline.points {
    scaled.push(OutlinePoint {
        x: ft_mul_fix_fast(p.x, x_scale),
        y: ft_mul_fix_fast(p.y, y_scale),
        on_curve: p.on_curve,
    });
}
```

Also apply in `align_strong_points` (one scale per edge-pair, invariant within the loop):
```rust
let scale = Scale16_16::new(ft_div_fix(pos_delta, fpos_delta));
let val = before.pos + ft_mul_fix_fast(offset, scale);
```

**Impact**: On WASM: 2-3× speedup in scaler and strong-point interpolation loops.
On native: measurable improvement from avoiding i64 → smaller code, better register
allocation.

---

### 3.8 Pre-allocated Scratch Buffer

Replace per-glyph allocations with a reusable scratch buffer held by the `Font`:

```rust
/// Reusable scratch buffer for glyph processing.
/// Created once per Font, cleared (not deallocated) between getmask() calls.
pub(crate) struct GlyphScratch {
    /// AFPoint array: pre-allocated to font's max points per glyph.
    points: Vec<AFPoint>,
    /// Temporary scaled coordinates (26.6).
    scaled_coords: Vec<OutlinePoint>,
    /// Per-dimension segments. Rarely exceeds points/3.
    segments_horz: Vec<AFSegment>,
    segments_vert: Vec<AFSegment>,
    /// Per-dimension edges. Rarely exceeds 32.
    edges_horz: Vec<AFEdge>,
    edges_vert: Vec<AFEdge>,
    /// Rasterizer cell buffer: flat array, pre-allocated generous capacity.
    raster_cells: Vec<Cell>,
    raster_scanline_heads: Vec<usize>,
    raster_scanline_tails: Vec<usize>,
    raster_target: Vec<u8>,
    /// Contour start indices.
    contours: Vec<usize>,
    /// Contour y extrema.
    contour_y_minima: Vec<i32>,
    contour_y_maxima: Vec<i32>,
}

impl GlyphScratch {
    pub fn new(max_points: usize, max_raster_width: usize, max_raster_height: usize) -> Self {
        let max_cells = max_raster_width * 4;
        GlyphScratch {
            points: Vec::with_capacity(max_points + 2),
            scaled_coords: Vec::with_capacity(max_points),
            segments_horz: Vec::with_capacity(max_points / 3),
            segments_vert: Vec::with_capacity(max_points / 3),
            edges_horz: Vec::with_capacity(32),
            edges_vert: Vec::with_capacity(32),
            raster_cells: Vec::with_capacity(max_cells),
            raster_scanline_heads: vec![usize::MAX; max_raster_height],
            raster_scanline_tails: vec![usize::MAX; max_raster_height],
            raster_target: vec![0u8; max_raster_width * max_raster_height],
            contours: Vec::with_capacity(16),
            contour_y_minima: Vec::with_capacity(16),
            contour_y_maxima: Vec::with_capacity(16),
        }
    }

    /// Reset all Vecs to empty without deallocating capacity.
    pub fn clear(&mut self) {
        self.points.clear();
        self.scaled_coords.clear();
        self.segments_horz.clear();
        self.segments_vert.clear();
        self.edges_horz.clear();
        self.edges_vert.clear();
        self.raster_cells.clear();
        // Re-fill scanline head/tail sentinels
        self.raster_scanline_heads.fill(usize::MAX);
        self.raster_scanline_tails.fill(usize::MAX);
        // raster_target zeroed by rasterizer
        self.contours.clear();
        self.contour_y_minima.clear();
        self.contour_y_maxima.clear();
    }
}
```

**Impact**: After the first `getmask()` call, **zero heap allocations** per glyph.
Only stack operations and `Vec::clear()` (which is a pointer bump — O(1)).

---

## 4. Proposed Pipeline Architecture

### 4.1 `apply_hints` — Restructured

```rust
pub fn apply_hints(
    outline: &mut Outline,
    raw_outline: &GlyphOutline,
    x_scale: i32,
    y_scale: i32,
    glyph_index: u16,
    metrics: Option<&AfLatinMetrics>,
    is_italic: bool,
    scratch: &mut GlyphScratch,
) {
    // ── Stage 0: Load and classify points ──
    load_points_into(&mut scratch.points, raw_outline, &outline.points);
    classify_point_directions(&mut scratch.points, metrics);
    classify_strong_weak(&mut scratch.points);

    // ── Stage 1: Process each dimension ──
    for dim in [Dimension::Horz, Dimension::Vert] {
        if dim == Dimension::Horz && is_italic { continue; }

        // 1a. Find segment runs
        let dirs: Vec<Direction> = scratch.points.iter()
            .map(|p| if dim == Dimension::Horz { abs_dir(p.out_dir) } else { abs_dir(p.out_dir) })
            .collect();
        let major_dir = major_direction(&scratch.points, dim);
        let runs = find_segment_runs(&dirs, major_dir);

        // 1b. Build segment candidates
        let candidates: Vec<SegmentCandidate> = runs.iter()
            .filter_map(|r| build_segment(&scratch.points, r.clone(), major_dir, dim))
            .collect();

        // 1c. Find stem pairs
        let stems = pair_stems(&candidates, major_dir, metrics);

        // 1d. Cluster into edges
        let threshold = edge_distance_threshold(metrics, dim);
        let edge_groups = cluster_edges(&candidates, threshold);

        // 1e. Build edge structures
        let edges: Vec<AFEdge> = edge_groups.iter()
            .map(|group| build_edge(group, &candidates, metrics, dim))
            .collect();

        // 1f. Compute fit requests with priorities
        let requests = build_fit_requests(&edges, &stems, metrics, dim);

        // 1g. Apply fitting
        let mut fitted = edges;
        fit_edges(&mut fitted, &requests, metrics, dim);

        // 1h. Interpolate points
        apply_edge_snapping(&mut scratch.points, &fitted, dim);
        apply_strong_interpolation(&mut scratch.points, &fitted, dim);
    }

    // ── Stage 2: IUP (weak-point interpolation) ──
    for dim in [Dimension::Horz, Dimension::Vert] {
        let touch_flag = if dim == Dimension::Horz { AF_FLAG_TOUCH_X } else { AF_FLAG_TOUCH_Y };
        setup_iup_uv(&mut scratch.points, dim);
        let ranges = build_iup_ranges(&scratch.points, &scratch.contours, touch_flag);
        apply_iup_ranges(&mut scratch.points, &ranges);
    }

    // ── Stage 3: Write back ──
    write_points_to_outline(&scratch.points, outline);
}
```

### 4.2 Rasterizer — Restructured

```rust
pub fn rasterize(outline: Outline, scratch: &mut GlyphScratch) -> Result<RasterResult, FontError> {
    if outline.points.is_empty() || outline.n_contours == 0 {
        return Ok(RasterResult { width: 0, height: 0, pixels: Vec::new() });
    }

    let width = (outline.cbox_x_max - outline.cbox_x_min) as usize;
    let height = (outline.cbox_y_max - outline.cbox_y_min) as usize;

    // Ensure scratch target is large enough
    if scratch.raster_target.len() < width * height {
        scratch.raster_target.resize(width * height, 0u8);
    }

    // Ensure scanline head/tail arrays are large enough
    let total_scanlines = height.max(1);
    if scratch.raster_scanline_heads.len() < total_scanlines {
        scratch.raster_scanline_heads.resize(total_scanlines, usize::MAX);
        scratch.raster_scanline_tails.resize(total_scanlines, usize::MAX);
    }

    let mut worker = Worker::from_scratch(scratch, width, height, outline.flags);
    worker.convert_glyph(
        &outline.points,
        &outline.contours,
        outline.n_contours,
        outline.cbox_x_min,
        outline.cbox_x_max,
        outline.cbox_y_min,
        outline.cbox_y_max,
    )?;

    // Copy out the rendered pixels (cheap, width*height bytes)
    let pixel_count = width * height;
    let mut pixels = vec![0u8; pixel_count];
    pixels.copy_from_slice(&worker.target[..pixel_count]);

    Ok(RasterResult { width, height, pixels })
}
```

---

## 5. Implementation Plan

### 5.1 Phase-by-Phase

| Phase | Description | Effort | Risk | Cumulative Gain |
|-------|------------|--------|------|-----------------|
| **P1** | Fix clones (3.1.1–3.1.3) | 0.5 day | Low | Eliminates 6 allocs per glyph |
| **P2** | `scaled.to_vec()` → move | 10 min | Zero | Eliminates 1 alloc + copy per glyph |
| **P3** | `ft_mul_fix_fast` for invariant-scale loops | 0.5 day | Low (needs correctness verification) | 2-3× faster scaler on WASM |
| **P4** | Flat array rasterizer (3.6) | 2 days | **High** — 1,708 parity tests must pass | Biggest single win: eliminates scanline-per-allocation |
| **P5** | Segment rewrite (3.2) | 1 day | Medium — must match edge positions exactly | ~120 lines saved, auto-vectorizable |
| **P6** | Edge clustering rewrite (3.3) | 0.5 day | Medium | O(n²) → O(n log n) |
| **P7** | IUP rewrite (3.4) | 0.5 day | Medium | ~60 lines → collect-then-apply |
| **P8** | `hint_edges` priority queue (3.5) | 1 day | **High** — most complex algorithm | 400 lines → ~120 lines |
| **P9** | Pre-allocated scratch buffer (3.8) | 1 day | Medium (integration) | Zero allocs after warmup |

**Total**: ~7 days of focused work.

### 5.2 Verification Strategy

After EACH phase, run:
```bash
cargo test -p pillow-rs-freetype -- --test-threads=1
```

The 1,708 parity tests must pass identically. Any deviation indicates a bug in
the rewrite. This is why each phase is isolated — you verify incremental correctness.

For the high-risk phases (P4 rasterizer, P8 hint_edges), build the verification
sequentially:
1. Dump intermediate state from C FreeType (via `FT2_DEBUG` environment variable)
2. Dump intermediate state from the rewritten Rust code (via `eprintln!` or `log::trace!`)
3. Diff the dumps to find the first point of divergence
4. Fix, re-verify

### 5.3 Testing Order

Start with the simplest glyphs and work up:
1. `'|'` (single straight segment, 2 edges)
2. `'-'` (single straight segment)
3. `'A'` (3 contours, diagonal + horizontal edges)
4. `'&'` (complex, 3 contours, 6+ segments per dimension)
5. `'g'` (descender, blue zone interaction)
6. Chinese character (complex, many segments)

---

## 6. Expected Performance

### 6.1 Single-Glyph, 10pt DejaVu Sans 'A'

| Metric | Current | After P1-P3 | After P4-P8 | After P9 | C FreeType |
|--------|---------|-------------|-------------|-----------|------------|
| Allocations | ~20 | ~12 | ~4 | 0 (after warmup) | 0 |
| `i64` ops | ~500 | ~100 | ~100 | ~100 | 0 (C uses 32-bit) |
| Scalpel time | ~1.5µs | ~0.8µs | ~0.6µs | ~0.5µs | ~0.4µs |
| Autohinter time | ~3.0µs | ~2.5µs | ~1.8µs | ~1.5µs | ~1.2µs |
| Raster time | ~2.0µs | ~2.0µs | ~0.5µs | ~0.4µs | ~0.3µs |
| **Total** | **~6.5µs** | **~5.3µs** | **~2.9µs** | **~2.4µs** | **~1.9µs** |

### 6.2 WASM-Specific, 10pt 'A'

| Metric | Current WASM | After Rustification |
|--------|-------------|-------------------|
| Total per glyph | ~20µs | ~4-6µs |
| `i64` emulation overhead | ~8µs | ~0.5µs |
| Allocator overhead | ~5µs | ~0µs (after warmup) |
| **Speedup vs. current** | — | **3-5×** |

### 6.3 Throughput: String of 100 Glyphs

| Scenario | Current | After | Speedup |
|----------|---------|-------|---------|
| Native, first call | ~650µs | ~240µs | 2.7× |
| Native, warm (zero-alloc) | ~650µs | ~180µs | 3.6× |
| WASM, first call | ~2,000µs | ~600µs | 3.3× |
| WASM, warm | ~2,000µs | ~350µs | 5.7× |

---

## 7. What NOT To Do

### 7.1 Don't Add GPU Compute

The autohinter processes 20-200 points. GPU launch overhead (~10µs) exceeds the
entire CPU autohinter time (~2µs for a 50-point glyph). GPU would be 5-10× slower.

The rasterizer processes small bitmaps (12×15 to 200×200 pixels). Even at 200×200,
the CPU rasterizes in ~5µs. The GPU would need 10µs just to launch the compute
shader, before a single pixel is touched.

GPU wins only for **batched compositing of hundreds of glyphs onto large images**.
That's a separate design document.

### 7.2 Don't Add SIMD Intrinsics (Yet)

The functional pipeline restructure makes inner loops pure and auto-vectorizable.
The compiler (LLVM) can do this for you without intrinsics. Wait until profiling
proves a specific loop isn't auto-vectorizing before adding `std::simd` or `wide`.

### 7.3 Don't Add rayon

WASM has no threads. rayon is a compile error on `wasm32-unknown-unknown`. The
pipeline must remain single-threaded for WASM. For native, rayon can be added
later as a separate feature flag (`feature = "parallel"`) that enables
`par_iter()` on the glyph-level loop in `getmask_batch()`.

### 7.4 Don't Add Dependencies

The current crate has exactly 2 production dependencies: `log` and `thiserror`.
Keep it this way. The suggested `heapless::Vec` for stack-allocated snapshots is
optional — a fixed-size array `[EdgeInterpRef; 32]` with a manual `len: usize`
works fine and adds zero dependencies.

---

## 8. Code Quality Metrics

### 8.1 Target

| Metric | Current | Target |
|--------|---------|--------|
| `latin.rs` lines | 2,343 | ~1,000 |
| `grays.rs` lines | 800 | ~500 |
| Allocations per `getmask()` | ~20 | 0 (after warmup) |
| `clone()` calls in hot path | 7 | 0 |
| `i64` operations in hot path | ~500 | ~100 |
| Functions >100 lines | 8 | 0 |
| Max nesting depth | 5 | 2 |
| Cyclomatic complexity (max fn) | ~40 | ~15 |

### 8.2 Testability

| Metric | Current | Target |
|--------|---------|--------|
| Pure functions (no side effects) | 2 | 15+ |
| Functions testable without GlyphHints | 2 | 10+ |
| Functions testable without Outline | 2 | 8+ |

---

## 9. Migration Strategy

### 9.1 Non-Breaking, Incremental

The restructure is designed to be done incrementally:

1. Each phase (P1-P9) is a self-contained change that can be committed independently
2. After each phase, the full test suite must pass
3. The intermediate data types (`SegmentCandidate`, `EdgeInterpRef`, `IupRange`) are
   introduced alongside the existing code, not replacing it
4. Once all phases are complete, the old code paths are removed
5. The existing `GlyphHints` → `Outline` API is preserved — only internals change

### 9.2 Rollback Safety

Each phase produces a single commit. If a phase introduces a subtle bug that the
1,708 parity tests don't catch, the commit can be reverted independently without
affecting other phases.

### 9.3 Parallel Development

Phases P1-P3 (clones, to_vec, ft_mul_fix_fast) are independent of the restructuring
phases and can be done immediately with near-zero risk.

---

## 10. References

- FreeType 2.14.1 source: `freetype/src/autofit/aflatin.c`, `freetype/src/smooth/ftgrays.c`
  (vendored read-only reference in `pillow-rs-freetype/freetype/`)
- Current port: `pillow-rs-freetype/src/autohint/latin.rs`, `pillow-rs-freetype/src/grays.rs`
- Parity tests: `pillow-rs-freetype/tests/coverage_matrix_tests.rs` (1,708 tests)
- Debugging protocol: `CLAUDE.md` lines 102-287 (C-vs-Rust comparison methodology)
- Verification annotations: `CLAUDE.md` lines 289-335 (`✅ VERIFIED`, `⚠️ BUG` markers)
