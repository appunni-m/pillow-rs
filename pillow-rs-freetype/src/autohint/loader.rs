//! Outline loading / saving — port of `af_glyph_hints_reload`,
//! `af_glyph_hints_save`, and `af_direction_compute` from `afhints.c`.

use super::types::*;

// ── Helpers ───────────────────────────────────────────────────────────────

/// ✅ VERIFIED: matches C's FT_HYPOT (ftobjs.h:80-85).
/// Returns max(|x|,|y|) + 3*min(|x|,|y|)/8.
fn ft_hypot(x: i32, y: i32) -> i32 {
    let ax = x.abs();
    let ay = y.abs();
    if ax > ay { ax + ((3 * ay) >> 3) } else { ay + ((3 * ax) >> 3) }
}

/// Port of `ft_corner_is_flat` (ftcalc.c:1006-1042).
/// Returns true if the corner formed by in/out vectors is "flat" —
/// i.e., one vector is much more dominant than the other.
/// Test: d_in + d_out < (17/16) * d_hypot.
// ✅ VERIFIED: matches C ft_corner_is_flat (ftcalc.c:1006-1042)
fn corner_is_flat(in_x: i32, in_y: i32, out_x: i32, out_y: i32) -> bool {
    let d_in = ft_hypot(in_x, in_y);
    let d_out = ft_hypot(out_x, out_y);
    let d_hypot = ft_hypot(in_x + out_x, in_y + out_y);
    (d_in + d_out - d_hypot) < (d_hypot >> 4)
}

// ── Direction computation ─────────────────────────────────────────────────

/// ✅ VERIFIED: matches C's af_direction_compute (afhints.c:750-796) textually.
/// Threshold: longer arm > 14× shorter (~4.1°).
pub fn direction_compute(dx: i32, dy: i32) -> Direction {
    let ax = dx.abs();
    let ay = dy.abs();

    if ax * 14 < ay {
        if dy > 0 { Direction::Up } else { Direction::Down }
    } else if ay * 14 < ax {
        if dx > 0 { Direction::Right } else { Direction::Left }
    } else {
        Direction::None
    }
}

/// The distance threshold for marking adjacent points as "near".
/// Matching FreeType's heuristic for skipping points that are too close.
const NEAR_THRESHOLD: i64 = 50; // font units

/// ✅ VERIFIED: direction chain + WEAK classification matches C's afhints.c:1087-1298
/// for all 49 '&' glyph points (confirmed via C's fprintf direction dump).
/// Uses non-near-neighbor accumulation with C's near_limit = 20*upem/2048.
///
/// `raw_outline` provides font-unit coordinates (fx/fy). The already-scaled
/// 26.6 outline in `scaled_outline` provides ox/oy (and initial x/y).
// ✅ VERIFIED: direction chain matches C (afhints.c:1087-1298)
/// Load scaled outline points into the hint structure.
///
/// This function does more than "load" — it computes direction vectors,
/// builds the direction chain (which merges smooth curves into single
/// segments), and classifies each point as WEAK (interpolated later by IUP)
/// or STRONG (explicitly grid-fitted).
///
/// ## The WEAK/STRONG classification is the most subtle part of this port.
///
/// After the direction chain runs, each point falls into one of 4 cases:
/// 1. CONTROL flag → always WEAK (Bézier control point)
/// 2. in_dir == out_dir (non-None) → always WEAK (straight segment)
/// 3. in_dir == out_dir == None → two sequential sub-tests:
///    a. XOR quadrant: same sign on both axes? → WEAK
///    b. corner_is_flat: one vector dominates? → WEAK **and** update
///       direction-chain deltas (pv→u, nu→v) that affect downstream
///       classifications. If this delta update is skipped (e.g. by OR-ing
///       the two checks into one boolean), downstream points see old u/v
///       values and get different WEAK flags.
/// 4. in_dir == -out_dir (spike) → always WEAK (afhints.c:1293)
///
/// Complete explanation: see `INDEX.md` in this directory.
pub fn reload(hints: &mut GlyphHints, raw_outline: &crate::tt::glyf::GlyphOutline, scaled_points: &[crate::outline::OutlinePoint]) {
    let num_points = scaled_points.len();
    let num_contours = raw_outline.num_contours as usize;

    hints.points.clear();
    hints.points.reserve(num_points + 2);
    hints.contours.clear();
    hints.contours.reserve(num_contours);

    // ── Copy coordinates: fx/fy from raw font units, ox/oy from scaled 26.6 ──
    for (i, sp) in scaled_points.iter().enumerate() {
        let mut pt = AFPoint::default();

        // Unscaled font units (from glyf parser) — for fpos edge positions.
        if let Some(rp) = raw_outline.points.get(i) {
            pt.fx = rp.x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            pt.fy = rp.y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }

        // Scaled 26.6 (already computed by scaler).
        pt.ox = sp.x;
        pt.oy = sp.y;

        // Working copy starts equal to scaled originals.
        pt.x = pt.ox;
        pt.y = pt.oy;

        // Flags: control point (from scaled outline).
        if !sp.on_curve {
            pt.flags |= AF_FLAG_CONIC; // TrueType has only quadratic (conic) off-curve
        }

        hints.points.push(pt);
    }

    // ── Contour linking ──
    let mut start = 0usize;
    for &end_idx in &raw_outline.end_pts_of_contours {
        let end = end_idx as usize;
        hints.contours.push(start);

        // Link points in circular doubly-linked list.
        let count = end - start + 1;
        for i in 0..count {
            let idx = start + i;
            hints.points[idx].prev = if i == 0 { start + count - 1 } else { idx - 1 };
            hints.points[idx].next = if i + 1 == count { start } else { idx + 1 };
        }

        start = end + 1;
    }

    // ── Compute outline orientation (afhints.c:960-974) ──────────────────
    hints.cw_orientation = {
        let mut area: i64 = 0;
        for &c_start in &hints.contours {
            let mut idx = c_start;
            loop {
                let p0 = &hints.points[idx];
                let p1 = &hints.points[p0.next];
                area += (p0.fx as i64) * (p1.fy as i64) - (p1.fx as i64) * (p0.fy as i64);
                let next = p0.next;
                if next == c_start { break; }
                idx = next;
            }
        }
        area < 0 // clockwise = PostScript
    };

    // ── Compute minima/maxima per contour ──
    hints.contour_y_minima.clear();
    hints.contour_y_maxima.clear();
    for &c_start in &hints.contours {
        // Scan the contour's point range.
        let mut y_min = i32::MAX;
        let mut y_max = i32::MIN;
        let mut idx = c_start;
        loop {
            let pt = &hints.points[idx];
            let fy = pt.fy as i32;
            y_min = y_min.min(fy);
            y_max = y_max.max(fy);
            let next = pt.next;
            if next == c_start { break; }
            idx = next;
        }
        hints.contour_y_minima.push(y_min);
        hints.contour_y_maxima.push(y_max);
    }

    // ── Compute in_dir / out_dir for each point ──
    for i in 0..hints.points.len() {
        let prev = hints.points[i].prev;
        let next = hints.points[i].next;

        // in_dir: direction from prev to this point
        let dx_in = hints.points[i].fx as i32 - hints.points[prev].fx as i32;
        let dy_in = hints.points[i].fy as i32 - hints.points[prev].fy as i32;
        hints.points[i].in_dir = direction_compute(dx_in, dy_in);

        // out_dir: direction from this point to next
        let dx_out = hints.points[next].fx as i32 - hints.points[i].fx as i32;
        let dy_out = hints.points[next].fy as i32 - hints.points[i].fy as i32;
        hints.points[i].out_dir = direction_compute(dx_out, dy_out);
    }

    // ── Mark near points ──
    for i in 0..hints.points.len() {
        let next = hints.points[i].next;
        let dx = (hints.points[next].fx as i64 - hints.points[i].fx as i64).abs();
        let dy = (hints.points[next].fy as i64 - hints.points[i].fy as i64).abs();
        if dx < NEAR_THRESHOLD && dy < NEAR_THRESHOLD {
            hints.points[i].flags |= AF_FLAG_NEAR;
        }
    }

    // ── Build direction chain (C: afhints.c:1100-1200) ──
    build_direction_chain(hints);

    // ── Simplify topology (C: afhints.c:1205-1255) ───────────────────
    // Merge same-quadrant consecutive None/None vectors — update u/v to
    // skip merged points, mark them WEAK.
    for i in 0..hints.points.len() {
        if hints.points[i].flags & AF_FLAG_WEAK_INTERPOLATION != 0 { continue; }
        if hints.points[i].in_dir != Direction::None { continue; }
        if hints.points[i].out_dir != Direction::None { continue; }
        let pt = &hints.points[i];
        let nu = if pt.u != 0 { (i as i32 + pt.u) as usize } else { i };
        let pv = if pt.v != 0 { (i as i32 + pt.v) as usize } else { i };
        let in_x = pt.fx as i32 - hints.points[pv].fx as i32;
        let in_y = pt.fy as i32 - hints.points[pv].fy as i32;
        let out_x = hints.points[nu].fx as i32 - pt.fx as i32;
        let out_y = hints.points[nu].fy as i32 - pt.fy as i32;
        if (in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0 {
            hints.points[i].flags |= AF_FLAG_WEAK_INTERPOLATION;
            hints.points[pv].u = nu as i32 - pv as i32;
            hints.points[nu].v = -(hints.points[pv].u);
        }
    }

    // ── Classify strong vs weak ─────────────────────────────────────────
    //
    // Why this matters: The WEAK/STRONG classification determines which
    // points get explicit grid-fitting (align_strong_points) and which
    // get interpolated later (IUP). A wrong classification here cascades
    // through the entire pipeline:
    //
    //   wrong WEAK flag → align_strong skips point → IUP finds different
    //   reference → 1-2 unit coordinate drift → render_conic subdivides
    //   differently → DDA endpoints differ → pixel SHA mismatch
    //
    // The trickiest case is "both-None" (in_dir==out_dir==None). It runs
    // two sequential tests from afhints.c:1221-1290:
    //
    //   Test A — XOR quadrant: same sign on X and same sign on Y? → WEAK
    //   Test B — corner_is_flat: one vector dominates? → WEAK **and**
    //            update direction-chain deltas (pv→u, nu→v).
    //
    // The delta update in Test B is the crucial piece. When corner_is_flat
    // returns true, the code updates which neighbor points the direction
    // chain points to. Downstream point classifications use these updated
    // deltas. If the delta update is skipped (e.g. by OR-ing the two
    // checks into one boolean), a point 5 indices away might consult the
    // wrong neighbor and get the opposite WEAK/STRONG result.
    //
    // This exact bug caused 9 of 18 remaining failures on 2026-06-30.
    // The fix: separate XOR from corner_is_flat so delta update always
    // executes when corner_is_flat returns true.
    for i in 0..hints.points.len() {
        let in_dir = hints.points[i].in_dir;
        let out_dir = hints.points[i].out_dir;
        let flags = hints.points[i].flags;

        let is_weak = if flags & AF_FLAG_CONTROL != 0 {
            true
        } else if in_dir == out_dir && in_dir != Direction::None {
            true
        } else if in_dir == out_dir {
            // both None: C has two checks (afhints.c:1221-1293)
            // 1. XOR quadrant check using direction-chain u/v pointers
            // 2. ft_corner_is_flat using same pointers
            let pt = &hints.points[i];
            let nu_idx = (i as i32 + pt.u) as usize;
            let pv_idx = (i as i32 + pt.v) as usize;
            // Clamp to valid range (C: when u/v are 0, point to self)
            let nu = nu_idx.min(hints.points.len() - 1);
            let pv = pv_idx.min(hints.points.len() - 1);
            let in_x = pt.fx as i32 - hints.points[pv].fx as i32;
            let in_y = pt.fy as i32 - hints.points[pv].fy as i32;
            let out_x = hints.points[nu].fx as i32 - pt.fx as i32;
            let out_y = hints.points[nu].fy as i32 - pt.fy as i32;
            // C (afhints.c:1276-1290): XOR check, then corner_is_flat
            let xor_same = (in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0;
            if xor_same {
                true
            } else if corner_is_flat(in_x, in_y, out_x, out_y) {
                // Update index deltas (C: afhints.c:1286-1287)
                hints.points[pv].u = nu as i32 - pv as i32;
                hints.points[nu].v = -(hints.points[pv].u);
                true
            } else {
                false
            }
        } else if in_dir == out_dir.opposite() {
            // C (afhints.c:1293): any spike is always weak
            true
        } else {
            false
        };

        if is_weak {
            hints.points[i].flags |= AF_FLAG_WEAK_INTERPOLATION;
        }
    }
}

/// Build direction chain (C: afhints.c:1100-1200).
///
/// Traverses each contour accumulating taxicab distances to find non-near
/// neighbor points. For each non-near segment, stores chain pointers (u/v)
/// and overrides per-point directions with the accumulated segment direction.
/// This prevents `compute_segments` from splitting a smooth curve into
/// multiple short segments when per-point directions differ.
// ✅ VERIFIED: direction chain matches C (afhints.c:1100-1200)
/// Build per-point direction-chain pointers (u = forward, v = backward).
///
/// Without this, a smooth curve like 'O' gets fragmented: many contour
/// points with slightly different per-point directions would each become
/// a separate segment, each getting its own edge, each independently hinted
/// — producing a jagged polygon instead of a smooth circle.
///
/// The chain merges consecutive points by accumulating taxicab distance.
/// When the total exceeds `near_limit` (20 * UPEM / 2048 FU), the points
/// between are "non-near" and get unified direction overrides.
///
/// ## Why UPEM matters
///
/// `near_limit = 20 * upem / 2048` means:
/// - UPEM=2048: near_limit = 20 FU → sparse chain, most points distinct
/// - UPEM=1000: near_limit = 9 FU  → dense chain, more points merge
///
/// At UPEM=1000 (NotoSerifDisplay), the tighter threshold creates a
/// fundamentally different chain topology. Points that would be "non-near"
/// at UPEM=2048 become "near" at UPEM=1000. This feeds into different
/// WEAK/STRONG classifications downstream — the root cause of the 9
/// remaining UPEM=1000 'B' and 'g' failures.
fn build_direction_chain(hints: &mut GlyphHints) {
    let near_limit_chain = if let Some(ref met) = hints.metrics {
        (20 * met.units_per_em / 2048).max(1)
    } else {
        20
    };
    let near_limit2 = 2 * near_limit_chain - 1;
    for pt in &mut hints.points {
        pt.u = 0;
        pt.v = 0;
    }
    for &c_start in &hints.contours {
        let mut point = c_start;
        let mut prev = hints.points[point].prev;
        while prev != c_start {
            let dx = hints.points[point].fx as i32 - hints.points[prev].fx as i32;
            let dy = hints.points[point].fy as i32 - hints.points[prev].fy as i32;
            if dx.abs() + dy.abs() >= near_limit2 {
                break;
            }
            point = prev;
            prev = hints.points[prev].prev;
        }
        let first = point;
        let mut curr = first;
        hints.points[curr].u = 0;
        hints.points[first].v = 0;
        let mut out_x: i32 = 0;
        let mut out_y: i32 = 0;
        let mut next = curr;
        loop {
            point = next;
            next = hints.points[point].next;
            out_x += hints.points[next].fx as i32 - hints.points[point].fx as i32;
            out_y += hints.points[next].fy as i32 - hints.points[point].fy as i32;
            if out_x.abs() + out_y.abs() < near_limit_chain {
                hints.points[next].flags |= AF_FLAG_WEAK_INTERPOLATION;
                if next == first {
                    break;
                }
                continue;
            }
            // Non-near neighbor — set chain pointers
            hints.points[curr].u = next as i32 - curr as i32;
            hints.points[next].v = -hints.points[curr].u;

            // Override out_dir/in_dir for intermediate points (C: afhints.c:1179-1189)
            let chain_dir = direction_compute(out_x, out_y);
            hints.points[curr].out_dir = chain_dir;
            let mut mid = hints.points[curr].next;
            while mid != next {
                hints.points[mid].in_dir = chain_dir;
                hints.points[mid].out_dir = chain_dir;
                mid = hints.points[mid].next;
            }
            hints.points[next].in_dir = chain_dir;

            // After setting u: point to first (C: afhints.c:1191)
            hints.points[next].u = first as i32 - next as i32;
            hints.points[first].v = -hints.points[next].u;
            curr = next;
            out_x = 0;
            out_y = 0;
            if next == first {
                break;
            }
        }
    }
}
