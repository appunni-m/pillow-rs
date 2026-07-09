# Safe Rust Optimization Guide for fontdone

2026-07-08 — Research-driven implementation guide.  Every pattern uses stable,
safe Rust only.  No `unsafe`, no nightly features, no new dependencies.

---

## Core Principle: Defeat Borrow-Checker Clones 

The single biggest performance drain is **Vec clones** forced by the borrow
checker.  Each clone copies segments + edges + contours (50–500 entries, ~1-4KB).
These happen on every glyph, every hinting pass.  Below are the safe patterns
to eliminate every one of them.

### Pattern A: Field Decomposition (Pass Sub-Slices)

**Where it applies:**
- `compute_segments` clones `hints.contours.clone()` (latin.rs:3055)
- `compute_segments` clones `hints.contours.clone()` on every call (called 2×/glyph)
- `align_weak_points` clones `hints.contours.clone()` (latin.rs:4937)

**Why the borrow checker complains:**
```rust
fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    let contours: Vec<usize> = hints.contours.clone(); // clone to release borrow
    let axis = &mut hints.axis[dim as usize];           // can't borrow both
    // ...
}
```

**Solution — pass fields as separate parameters from the caller:**
```rust
// BEFORE (callee):
fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    let contours = hints.contours.clone();  // CLONE
    let axis = &mut hints.axis[dim as usize];
    for &c in &contours { /* use axis and c */ }
}

// AFTER (caller passes split fields):
// In apply_hints:
{
    let (axis_vert, axis_horz) = if do_horz {
        let (left, right) = hints.axis.split_at_mut(1);
        compute_segments_vert(&mut hints.points, &hints.contours, &mut right[0]);
        compute_segments_horz(&hints.contours, &mut left[0])
    } else {
        compute_segments_vert(&hints.contours, &mut hints.axis[1]);
        None
    };
}
```

Better — just pass what's needed:
```rust
// New signatures:
fn compute_segments_on_axis(
    points: &mut [AFPoint],
    contours: &[usize],
    axis: &mut AxisHints,
    metrics: Option<&AfLatinMetrics>,
    dim: Dimension,
    cw_orientation: bool,
) { /* body unchanged, but no clone */ }

// Caller in apply_hints:
compute_segments_on_axis(
    &mut hints.points,
    &hints.contours,     // &[usize] — no clone!
    &mut hints.axis[dim as usize],
    hints.metrics.as_ref(),
    dim,
    hints.cw_orientation,
);
```

**Files touched:** `src/autohint/latin.rs` (1 function signature + 4 call sites)
**Estimated saving:** 30—80ns per call × 2 dims × 2 stages (segments + edges) = 120—320ns
**Risk:** Low — pure refactor, identical algorithm, parity tests will catch any drift

---

### Pattern B: mem::take / mem::replace (Extract-and-Restore)

**Where it applies:**
- `align_strong_points` clones `hints.axis[dim].clone()` (latin.rs:4743)
- The entire `AxisHints` struct is cloned just to read edges while mutating points

**Why the borrow checker complains:**
```rust
fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis_snapshot = hints.axis[dim as usize].clone(); // CLONES edges + segments Vec
    let axis = &axis_snapshot;
    // ... mutates hints.points[i].x / hints.points[i].y while reading axis.edges
}
```

**Solution — take ownership temporarily, then restore:**
```rust
fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    // Take the axis out of hints — zero-cost ownership transfer
    let mut axis = std::mem::take(&mut hints.axis[dim as usize]);
    // Now axis owns the data; hints.axis[dim] is a fresh empty AxisHints

    if axis.edges.is_empty() {
        hints.axis[dim as usize] = axis; // restore
        return;
    }

    // Work with axis.edges and hints.points freely — no conflict!
    let is_vert = dim == Dimension::Vert;
    for i in 0..hints.num_points() {
        let pt = &hints.points[i];
        // ... read axis.edges, write hints.points ...
    }

    // ALWAYS restore, even on early returns
    hints.axis[dim as usize] = axis;
}
```

**Critical:** Every early return must `hints.axis[dim as usize] = axis;` first.
A helper or `Drop` guard can enforce this:
```rust
struct AxisGuard<'a> {
    hints: &'a mut GlyphHints,
    dim: Dimension,
    axis: Option<AxisHints>,
}
impl Drop for AxisGuard<'_> {
    fn drop(&mut self) {
        if let Some(axis) = self.axis.take() {
            self.hints.axis[self.dim as usize] = axis;
        }
    }
}
```

**Files touched:** `src/autohint/latin.rs` (1 function)
**Estimated saving:** ~50ns per call × 2 dims = ~100ns
**Risk:** Low — exact same data, just ownership moves

---

### Pattern C: Rc for Large Read-Only Data

**Where it applies:**
- `hints.metrics = metrics.cloned()` (latin.rs:2707)
- Clones entire `AfLatinMetrics` including `non_base_glyphs: Vec<bool>` (300–6000 entries),
  `digit_glyphs: Vec<bool>` (same), and `reverse_adjustment_map: HashMap<u16, u32>` (500 entries)

**Current State:**
```rust
// GlyphHints stores a full clone:
pub struct GlyphHints {
    pub metrics: Option<AfLatinMetrics>, // OWNED CLONE
}

// At construction:
hints.metrics = metrics.cloned(); // ~5-12KB copy for CJK fonts
```

**Solution — store `Rc` instead:**
```rust
pub struct GlyphHints {
    pub metrics: Option<Rc<AfLatinMetrics>>, // shared via Rc
}

// Construction becomes:
hints.metrics = metrics.map(Rc::clone); // refcount bump, ~20ns

// ALL call sites that read metrics continue to work:
hints.metrics.as_ref()  // returns Option<&AfLatinMetrics> — identical!
hints.metrics.clone()   // Rc::clone — refcount bump, not deep copy
hints.metrics.is_some()
hints.metrics.as_ref().is_some_and(|m| m.no_advance_hinting)
// ... every existing call site compiles without changes
```

**Why this is safe:** Rc is read-only sharing.  `AfLatinMetrics` is never mutated
after construction (all fields are set during metrics init, then only read).
No `RefCell` needed.

**Files touched:** `src/autohint/types.rs` (1 field type change), `src/autohint/latin.rs` (2 assignment sites), `src/autohint/cjk.rs` (1 assignment site), `src/autohint/loader.rs` (1 read site)
**Estimated saving:** 50—200ns per glyph (worst for CJK fonts with 6000+ glyph entries)
**Risk:** Extremely low — Rc<AfLatinMetrics> derefs to &AfLatinMetrics, the same
type all callers already use

---

### Pattern D: resize + Index Write Instead of push

**Where it applies:**
- `reload` in loader.rs — `hints.points.reserve(num_points + 2)` then N `push()` calls
- Each `push()` does: capacity check (branch), length increment, write
- For a 200-point glyph that's 200 redundant capacity checks

**Current:**
```rust
hints.points.reserve(num_points + 2);
for (i, sp) in scaled_points.iter().enumerate() {
    let mut pt = AFPoint::default();
    // ... fill pt fields ...
    hints.points.push(pt); // capacity check every iteration
}
```

**Solution — `resize` once, then index:**
```rust
let old_len = hints.points.len();
hints.points.resize(old_len + num_points + 2, AFPoint::default());
for (i, sp) in scaled_points.iter().enumerate() {
    let idx = old_len + i;
    let pt = &mut hints.points[idx];
    // ... fill pt.fx, pt.fy, pt.ox, pt.oy, pt.x, pt.y, pt.flags ...
    // No push — direct write. No capacity check.
}
```

**Why this helps:** `resize` does ONE capacity check in the implementation,
then writes default values.  After that, direct writes skip branches.

**Files touched:** `src/autohint/loader.rs` (1 function)
**Estimated saving:** ~1ns per point × ~50 points/glyph = ~50ns
**Risk:** Very low — `resize` extends the Vec with identical defaults to what push would create

---

### Pattern E: Avoid Intermediate Collections

**Where it applies:**
- `horz_widths_26_6` and `vert_widths_26_6` intermediate Vecs (latin.rs:2766, 2805)
- `block_flags_snapshot` for each blue zone (latin.rs:1883)
- Point tags Vec built from on_curve flags (scaler.rs, font.rs — multiple sites)

**Current:**
```rust
horz_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
// ^^^ allocates a Vec<i32>, then iterates over it in hint_edges
```

**Solution — pass iterator or slice directly:**
```rust
// Change hint_edges signature:
fn hint_edges(hints: &mut GlyphHints, dim: Dimension, std_widths: &[i32], ppem: i32) {
    // Use std_widths directly — it's already in memory from AfLatinMetrics.widths
}
// Caller:
hint_edges(&mut hints, dim, &metrics.axis[dim as usize].widths[..wc], ppem);
// But wait — we need .cur not .org.  Still, can avoid the intermediate allocation:
```

Actually the issue is that `AfWidth` has `.org` and `.cur` fields, and `hint_edges`
wants `i32` values.  The simplest fix: pass `&[AfWidth]` to `hint_edges` and
extract `.cur` inside.  Or compute `widths.iter().map(|w| w.cur)` as a slice
if the layout allows — but `AfWidth` is `{ org: i32, cur: i32 }` so the `.cur`
fields aren't contiguous.  An intermediate Vec is needed for the common case.

**Better fix for the case where width_count == 0:**
```rust
let widths_26_6: &[i32] = if wc > 0 {
    // Only allocate when there are actually widths
    &*horz_widths_26_6_storage
} else {
    &[]
};
```
This avoids the alloc when `width_count == 0` (common for CJK fallback fonts).

**Risk:** Low — just adds an early-out

---

### Pattern F: Precompute Loop Invariants

**Where it applies:**
- `compute_segments` checks `hints.cw_orientation` for major_dir per call,
  but it's invariant per glyph
- `flat_threshold` computed from `hints.metrics.as_ref().map_or(146, |m| m.units_per_em / 14)`
  per call, but `units_per_em` never changes for a font
- `len_threshold`, `len_score`, `dist_score` in `link_segments_inner` computed per call

**Already partially done by our previous work.**  The remaining low-hanging fruit:

```rust
// compute_segments: flat_threshold is per-call, but we access hints.metrics every time.
// Cache at entry point:
fn apply_hints(...) {
    let upem = metrics.map_or(2048, |m| m.units_per_em);
    let flat_threshold = upem / 14;
    // ... pass to compute_segments instead of recomputing
}
```

**Risk:** None — pure constant hoisting

---

### Pattern G: Monotonically-Increasing Fast Path

**Where it applies:**
- `set_cell` in grays.rs — binary search fallback when cells are emitted out of order
- TrueType outlines are emitted left-to-right, so cells are almost always
  monotonically increasing in x

**Current:**
```rust
fn set_cell(&mut self, ex: i32, ey: i32) {
    // ... dumpster check, fast paths for same-x and append ...
    // Fallback: binary search + insert
    match scanline.binary_search_by_key(&ex, |c| c.x) {
        Ok(idx) => { self.current_idx = idx; }
        Err(idx) => {
            scanline.insert(idx, Cell { x: ex, cover: 0, area: 0 });
            self.current_idx = idx;
        }
    }
}
```

**Solution — monotonicity flag:**
Already present as the `last.x < ex` check before the binary search fallback.
The fast path (append) is hit >95% of the time.  No further optimization needed
here — the existing code is already optimal for the common case.

---

### Pattern H: size_of + Layout Guarantees for Vec Reinterpretation

**Where it applies:**
- `contours` field: `Vec<u16>` in GlyphOutline → `Vec<i16>` in Outline (scaler.rs:1151)
- Same memory layout — `u16` and `i16` are both 2 bytes
- Currently does `.iter().map(|&e| e as i16).collect()` — allocates new Vec

**Using `bytemuck` (or manual reinterp with debug_assertions):**
```rust
// SAFETY: u16 and i16 have identical size, alignment, and bit patterns
// for values that are non-negative (contour endpoints are always ≤ num_points < 2^15)
use bytemuck::cast_vec;

let contours_i16: Vec<i16> = cast_vec(contours_u16);
```
But we can't use `bytemuck` since the project rules say no new dependencies.

**Safe alternative — store as `i16` from the start:**
Change `GlyphOutline.end_pts_of_contours` from `Vec<u16>` to `Vec<i16>`
and do the cast once during glyf parsing.  This saves the collect per glyph.

**Files touched:** `src/tt/glyf.rs` (struct field + parser), `src/scaler.rs` (one line), `src/font.rs` (one line)
**Risk:** Low— TrueType spec says end_pts ≤ num_points ≤ 32767 for simple glyphs

---

### Pattern I: `debug_assertions` Guard on Trace Logging

**Where it applies:**
- Every `log::log_enabled!` call in hot loops (grays sweep, render_line, render_conic)
- Even when logging is disabled at runtime, the guard check executes
- Already `#[cfg(debug_assertions)]`-guarded in many places

**Status:** Already done.  No further optimization needed.

---

### Pattern J: `#[inline]` on Trivial Accessors in Hot Paths 

**Where it applies:**
- `trunc()`, `fract()`, `ft_div_mod()`, `ft_udivprep()`, `ft_udiv()` in grays.rs
- These are called in the innermost rasterizer loops

**Status:** Already `#[inline]`.  With LTO, the compiler inlines these aggressively.

---

## Implementation Order 

| Priority | Pattern | Files | Est. Saving | Lines Changed |
|----------|---------|-------|-------------|---------------|
| **1** | C — Rc metrics | types.rs + 3 call sites | 50-200ns | ~10 lines |
| **2** | B — mem::take axis | latin.rs:4743 | ~100ns | ~15 lines |
| **3** | A — field decomposition for contours | latin.rs:3055,4937 | ~200ns | ~40 lines |
| **4** | D — resize + index write | loader.rs:88-170 | ~50ns | ~15 lines |
| **5** | H — i16 contours from parse | glyf.rs, scaler.rs, font.rs | ~30ns | ~10 lines |
| **6** | E — avoid intermediate alloc | latin.rs:2766 | ~20ns | ~5 lines |
| **—** | Total combined | | ~450-600ns | ~95 lines |

Total estimated improvement: 3,450ns → ~2,900ns (16% reduction, ~2.4x vs C)

---

## What NOT to Do (And Why)

1. **Don't convert `if-else` chains in the DDA renderer to `match`.**  The DDA
   state machine (grays.rs:610-648) has overlapping predicate conditions that
   are NOT a simple enum discriminant.  A `match` on a computed integer would
   need to evaluate the exact same predicates to compute the discriminant, then
   match on it — strictly more work.

2. **Don't use `unsafe { get_unchecked }`.**  The project has `#![deny(unsafe_code)]` on core.
   All bounds-check elimination must come from compiler auto-vectorization with
   known-length slices (the compiler already eliminates bounds checks when iterating
   with `for x in &slice[..known_len]`).

3. **Don't add smallvec/arrayvec dependency.**  The project's `deny.toml` restricts
   dependencies.  Inlining small-array logic is simpler and avoids bloat.

4. **Don't use `unsafe` transmute for u16→i16 Vec.**  Even though the layout is
   identical, transmuting Vec<T> to Vec<U> is technically UB under stacked borrows
   if the Vec is ever reallocated.  Use proper conversion at parse time instead.

5. **Don't precompute fill_rule LUT.**  The `area` value shifted right by 9 ranges
   over ±4,194,304 — a LUT covering even 10% of that would be 800KB.  Not worth it.

6. **Don't merge the DDA branches into a jump table.**  The DDA predicates are
   computed per-iteration from `prod`, `dx`, `dy`, `ONE_PIXEL` — they change
   every step.  A precomputed state machine would need to recalculate all
   predicates to pick the right state, then execute it — identical to if-else.

---

## Safe Rust Patterns Used Throughout

| Pattern | What it solves | Where used |
|---------|---------------|------------|
| `Rc<T>` | Shared read-only data without clone | Pattern C |
| `mem::take` + restore | Temporary ownership without clone | Pattern B |
| Field decomposition | Borrow split without clone | Pattern A |
| `resize` + index write | Skip push capacity checks | Pattern D |
| `Vec::with_capacity` | Pre-size to avoid realloc | Already done (round 9) |
| `match` on tuples | Cleaner branch discrimination | Already done (round 13-14) |
| `.min()/.max()` | Vectorizable min/max | Already done (round 13) |
| `iter().partition_point()` | Binary search on sorted slices | grays set_cell |
| `Option::as_deref` | Zero-cost Option<&T> → Option<&T> | everywhere |
| `#[inline]` on trivial functions | Cross-crate inlining with LTO | fixed.rs, scaler.rs |
