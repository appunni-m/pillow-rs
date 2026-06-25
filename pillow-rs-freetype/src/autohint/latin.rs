//! Latin-script auto-hinting — port of `src/autofit/aflatin.c`.
//!
//! Implements the core pipeline for grid-fitting Latin glyph outlines:
//!   segment detection → edge grouping → grid-fitting → point interpolation.
//!
//! This is a simplified port focusing on the vertical dimension (Y-axis /
//! horizontal edges) for the grid-fitting that makes the biggest visual
//! difference. The full FreeType autohinter includes blue zones, stem-width
//! histograms, serif handling, and per-glyph adjustments that we defer.

use crate::fixed::ft_mul_fix;
use super::types::*;
use super::loader;

/// Top-level entry: apply Latin auto-hinting to an outline.
///
/// Port of `af_latin_hints_apply` — the coordination function that calls
/// reload → detect_features → hint_edges → align_points → save.
pub fn apply_hints(
    outline: &mut crate::outline::Outline,
    raw_outline: &crate::tt::glyf::GlyphOutline,
    x_scale: i32,
    y_scale: i32,
    x_delta: i32,
    y_delta: i32,
) {
    let mut hints = GlyphHints::new(x_scale, y_scale, x_delta, y_delta);

    // Step 1: Load outline into hints (raw font units → fx/fy; scaled 26.6 → ox/oy)
    loader::reload(&mut hints, raw_outline, &outline.points);
    if hints.num_points() == 0 {
        return;
    }

    // Step 2: Process vertical dimension (Y-axis / horizontal edges)
    // This is the dimension that affects the '|' bar top/bottom alignment.
    compute_segments(&mut hints, Dimension::Vert);
    compute_edges(&mut hints, Dimension::Vert);
    hint_edges(&mut hints, Dimension::Vert);
    align_edge_points(&mut hints, Dimension::Vert);
    align_strong_points(&mut hints, Dimension::Vert);
    align_weak_points(&mut hints, Dimension::Vert);

    // Step 3: Process horizontal dimension (X-axis / vertical edges)
    compute_segments(&mut hints, Dimension::Horz);
    compute_edges(&mut hints, Dimension::Horz);
    hint_edges(&mut hints, Dimension::Horz);
    align_edge_points(&mut hints, Dimension::Horz);
    align_strong_points(&mut hints, Dimension::Horz);
    align_weak_points(&mut hints, Dimension::Horz);

    // Step 4: Write back
    hints.save_to_outline(outline);
}

// ── Segment detection ─────────────────────────────────────────────────────
//
// Port of `af_latin_hints_compute_segments` (aflatin.c:1557–2008).

/// Threshold for considering a run of points as "flat" — used to decide
/// whether an edge should be rounded.  `units_per_em / 14` is the FreeType
/// default; we hard-code 146 (~2048/14).
const FLAT_THRESHOLD: i32 = 146;

/// Faithful port of `af_latin_hints_compute_segments` (aflatin.c:1557).
#[allow(unused_assignments, unused_variables)]
fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    let contours: Vec<usize> = hints.contours.clone();
    let axis = &mut hints.axis[dim as usize];

    // Per-point u/v axis swap (aflatin.c:1582). Stored on the point's u/v fields.
    let is_horz = dim == Dimension::Horz;
    for pt in &mut hints.points {
        if is_horz { pt.u = pt.fx as i32; pt.v = pt.fy as i32; }
        else       { pt.u = pt.fy as i32; pt.v = pt.fx as i32; }
    }

    // major_dir magnitude (aflatin.c:1577).
    let major_dir = if axis.major_dir == Direction::None {
        // Initialize: TrueType outlines are CCW → Vert major_dir = Right, Horz = Up.
        let d = if is_horz { Direction::Up } else { Direction::Right };
        axis.major_dir = d;
        d
    } else {
        axis.major_dir
    };

    axis.segments.clear();
    let points = &hints.points;

    for &contour0 in &contours {
        let mut point = contour0;
        let mut last = points[point].prev;
        let mut on_edge = false;
        // segment_dir tracks the direction of the current open segment.
        let mut segment_dir = major_dir;

        let mut min_pos: i32 = 32000; let mut max_pos: i32 = -32000;
        let mut min_coord: i32 = 32000; let mut max_coord: i32 = -32000;
        let mut min_flags: u16 = 0; let mut max_flags: u16 = 0;
        let mut min_on_coord: i32 = 32000; let mut max_on_coord: i32 = -32000;

        let mut seg_first: usize = 0; // index of first point of current segment
        let mut prev_seg: Option<usize> = None; // index of previous segment in axis.segments

        // prev_* buffers for merge logic (aflatin.c:1631-1638).
        let mut prev_min_pos = min_pos; let mut prev_max_pos = max_pos;
        let mut prev_min_coord = min_coord; let mut prev_max_coord = max_coord;
        let mut prev_min_flags = min_flags; let mut _prev_max_flags = max_flags;
        let mut _prev_min_on_coord = min_on_coord; let mut _prev_max_on_coord = max_on_coord;

        // If we're already on an edge at the start, walk backwards to its start (aflatin.c:1644).
        if points[point].flags & AF_FLAG_IGNORE == 0
            && abs_dir(points[last].out_dir) == major_dir
            && abs_dir(points[point].out_dir) == major_dir
        {
            last = point;
            loop {
                point = points[point].prev;
                if abs_dir(points[point].out_dir) != major_dir {
                    point = points[point].next;
                    break;
                }
                if point == last { break; }
            }
        }

        last = point;
        let mut passed = false;

        loop {
            let p = &points[point];
            if on_edge {
                let u = p.u; min_pos = min_pos.min(u); max_pos = max_pos.max(u);
                let v = p.v;
                if v < min_coord { min_coord = v; min_flags = p.flags; }
                if v > max_coord { max_coord = v; max_flags = p.flags; }
                if p.flags & AF_FLAG_CONTROL == 0 {
                    if v < min_on_coord { min_on_coord = v; }
                    if v > max_on_coord { max_on_coord = v; }
                }

                if p.flags & AF_FLAG_IGNORE != 0
                    || p.out_dir != segment_dir
                    || point == last
                {
                    // End of segment.
                    let same_start_as_prev = prev_seg.is_some()
                        && seg_first == axis.segments[prev_seg.unwrap()].last;
                    let new_seg = p.flags & AF_FLAG_IGNORE != 0 || prev_seg.is_none()
                        || !same_start_as_prev;

                    if new_seg {
                        // Record a new segment.
                        let pos = ((min_pos + max_pos) >> 1) as i16;
                        let delta = ((max_pos - min_pos) >> 1) as i16;
                        let mut flags = 0u8;
                        if (min_flags | max_flags) & AF_FLAG_CONTROL != 0
                            && (max_on_coord - min_on_coord) < FLAT_THRESHOLD
                        {
                            flags |= AF_EDGE_ROUND;
                        }
                        axis.segments.push(AFSegment {
                            flags, dir: segment_dir, pos, delta,
                            min_coord: min_coord as i16, max_coord: max_coord as i16,
                            first: seg_first, last: point,
                            edge: usize::MAX, edge_next: usize::MAX,
                            link: usize::MAX, serif: usize::MAX, score: 32000,
                        });
                        let cur = axis.segments.len() - 1;
                        prev_seg = Some(cur);
                        prev_min_pos = min_pos; prev_max_pos = max_pos;
                        prev_min_coord = min_coord; prev_max_coord = max_coord;
                        prev_min_flags = min_flags; let _ = &mut _prev_max_flags;
                        let _ = &mut _prev_min_on_coord; let _ = &mut _prev_max_on_coord;
                    } else {
                        // Merge with previous segment (same start point). Simplified:
                        // unify bounds if directions match, else keep longer segment.
                        let prev_dir = axis.segments[prev_seg.unwrap()].dir;
                        if prev_dir == segment_dir {
                            min_pos = min_pos.min(prev_min_pos); max_pos = max_pos.max(prev_max_pos);
                            min_coord = min_coord.min(prev_min_coord); max_coord = max_coord.max(prev_max_coord);
                            let pos = ((min_pos + max_pos) >> 1) as i16;
                            let delta = ((max_pos - min_pos) >> 1) as i16;
                            let s = &mut axis.segments[prev_seg.unwrap()];
                            s.last = point; s.pos = pos; s.delta = delta;
                            s.min_coord = min_coord as i16; s.max_coord = max_coord as i16;
                        } else if (prev_max_coord - prev_min_coord).abs() > (max_coord - min_coord).abs() {
                            // discard current: extend prev's last only.
                            let pos = ((prev_min_pos.min(min_pos) + prev_max_pos.max(max_pos)) >> 1) as i16;
                            let s = &mut axis.segments[prev_seg.unwrap()];
                            s.last = point; s.pos = pos;
                        } else {
                            // discard prev: current replaces it.
                            let pos = ((min_pos.min(prev_min_pos) + max_pos.max(prev_max_pos)) >> 1) as i16;
                            let s = &mut axis.segments[prev_seg.unwrap()];
                            s.last = point; s.pos = pos;
                            s.min_coord = min_coord as i16; s.max_coord = max_coord as i16;
                            s.dir = segment_dir;
                        }
                    }

                    on_edge = false;
                }
            }

            if point == last {
                if passed { break; }
                passed = true;
            }

            // Start a new segment if not on edge and out_dir matches major dir.
            let p = &points[point];
            if p.flags & AF_FLAG_IGNORE == 0
                && !on_edge
                && (abs_dir(p.out_dir) == major_dir || point == p.prev)
            {
                if axis.segments.len() > 1000 { axis.segments.clear(); return; }
                segment_dir = p.out_dir;
                seg_first = point;
                min_pos = p.u; max_pos = p.u;
                min_coord = p.v; max_coord = p.v;
                min_flags = p.flags; max_flags = p.flags;
                if p.flags & AF_FLAG_CONTROL != 0 {
                    min_on_coord = 32000; max_on_coord = -32000;
                } else {
                    min_on_coord = p.v; max_on_coord = p.v;
                }
                on_edge = true;
            }

            point = points[point].next;
        }
    }
}

#[inline]
fn abs_dir(d: Direction) -> Direction {
    match d {
        Direction::Up => Direction::Up,
        Direction::Down => Direction::Up,
        Direction::Right => Direction::Right,
        Direction::Left => Direction::Right,
        Direction::None => Direction::None,
    }
}

// ── Edge detection ─────────────────────────────────────────────────────────
//
// Port of `af_latin_hints_compute_edges` (aflatin.c:2154–2500).
// Groups segments at nearby positions into edges.

/// Maximum distance (in font units) for two segments to be grouped into the
/// same edge.
const EDGE_DISTANCE_THRESHOLD: i32 = 50; // ~0.2px at upem=2048 ppem=10

fn compute_edges(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    axis.edges.clear();

    // For each segment, find or create its edge.
    for seg_idx in 0..axis.segments.len() {
        let seg = &axis.segments[seg_idx];

        // Skip segments with no direction.
        if seg.dir == Direction::None {
            continue;
        }

        let seg_pos = seg.pos as i32;
        let mut found_edge = usize::MAX;

        // Look for an existing edge at approximately this position.
        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir == seg.dir && (edge.fpos as i32 - seg_pos).abs() < EDGE_DISTANCE_THRESHOLD {
                found_edge = e_idx;
                break;
            }
        }

        if found_edge == usize::MAX {
            // Create a new edge.
            let fpos = seg_pos.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let scale = if dim == Dimension::Vert {
                hints.y_scale
            } else {
                hints.x_scale
            };
            let opos = ft_mul_fix(fpos as i32, scale);
            let edge = AFEdge {
                fpos,
                opos,
                pos: opos,
                flags: 0,
                dir: seg.dir,
                link: usize::MAX,
                serif: usize::MAX,
                first: seg_idx,
                last: seg_idx,
            };
            axis.edges.push(edge);
            // Update the segment's edge reference.
            axis.segments[seg_idx].edge = axis.edges.len() - 1;
        } else {
            // Append segment to existing edge.
            let e = &mut axis.edges[found_edge];
            let prev_last = e.last;
            axis.segments[prev_last].edge_next = seg_idx;
            e.last = seg_idx;
            axis.segments[seg_idx].edge = found_edge;
        }
    }
}

// ── Edge grid-fitting ──────────────────────────────────────────────────────
//
// Port of `af_latin_hint_edges` (aflatin.c:4214–4831), simplified to basic
// pixel-grid snapping without blue zones or stem-width quantization.

fn hint_edges(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    for edge in &mut axis.edges {
        // FT_PIX_ROUND in 26.6.
        edge.pos = (edge.opos + 32) & !63;
    }
}

// ── Edge-point alignment ───────────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_edge_points` (afhints.c:1338–1400).
// Moves all points belonging to an edge to that edge's grid-fitted position.

fn align_edge_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &hints.axis[dim as usize];
    let is_vert = dim == Dimension::Vert;

    for edge in &axis.edges {
        let pos = edge.pos;
        let mut seg_idx = edge.first;
        loop {
            if seg_idx == usize::MAX { break; }
            let seg = &axis.segments[seg_idx];
            let mut pt_idx = seg.first;
            loop {
                if is_vert {
                    hints.points[pt_idx].y = pos;
                    hints.points[pt_idx].flags |= AF_FLAG_TOUCH_Y;
                } else {
                    hints.points[pt_idx].x = pos;
                    hints.points[pt_idx].flags |= AF_FLAG_TOUCH_X;
                }
                if pt_idx == seg.last { break; }
                pt_idx = hints.points[pt_idx].next;
            }
            if seg_idx == edge.last { break; }
            seg_idx = seg.edge_next;
        }
    }
}

// ── Strong-point alignment (IP) ────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_strong_points` (afhints.c:1413–1578).
// Strong points are corners/angles that haven't been touched by edges.
// They get interpolated between the nearest two edges on either side.

fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &hints.axis[dim as usize];
    let is_vert = dim == Dimension::Vert;

    if axis.edges.is_empty() {
        return;
    }

    for i in 0..hints.num_points() {
        let pt = &hints.points[i];
        let already_touched = if is_vert {
            pt.flags & AF_FLAG_TOUCH_Y != 0
        } else {
            pt.flags & AF_FLAG_TOUCH_X != 0
        };
        let is_weak = pt.flags & AF_FLAG_WEAK_INTERPOLATION != 0;

        if already_touched || is_weak {
            continue;
        }

        // Strong point — find enclosing edges and interpolate.
        // fpos is the along-axis position (match segment dimension convention):
        //   Vert dim → along-axis = Y → use fy
        //   Horz dim → along-axis = X → use fx
        let pt_fpos = if is_vert {
            pt.fy as i32
        } else {
            pt.fx as i32
        };

        // Find edges before and after this point.
        let mut before: Option<&AFEdge> = None;
        let mut after: Option<&AFEdge> = None;
        for edge in &axis.edges {
            let efpos = edge.fpos as i32;
            if efpos <= pt_fpos {
                before = Some(edge);
            }
            if efpos >= pt_fpos && after.is_none() {
                after = Some(edge);
            }
        }

        match (before, after) {
            (Some(b), Some(a)) if b.fpos != a.fpos => {
                // Interpolate: how far is pt between before and after?
                let range = (a.fpos - b.fpos) as i32;
                let pos = (a.pos - b.pos) as i32;
                let offset = (pt_fpos - b.fpos as i32) as i64;
                // Linear interpolation in 26.6.
                let interpolated = b.pos as i64
                    + (offset * pos as i64) / range as i64;
                let val = interpolated as i32;
                if is_vert {
                    hints.points[i].y = val;
                    hints.points[i].flags |= AF_FLAG_TOUCH_Y;
                } else {
                    hints.points[i].x = val;
                    hints.points[i].flags |= AF_FLAG_TOUCH_X;
                }
            }
            (Some(b), None) => {
                // Point after the last edge: shift by the edge's delta from original.
                let delta = b.pos - b.opos;
                if is_vert {
                    hints.points[i].y = hints.points[i].oy + delta;
                    hints.points[i].flags |= AF_FLAG_TOUCH_Y;
                } else {
                    hints.points[i].x = hints.points[i].ox + delta;
                    hints.points[i].flags |= AF_FLAG_TOUCH_X;
                }
            }
            (None, Some(a)) => {
                // Point before the first edge: shift by edge delta.
                let delta = a.pos - a.opos;
                if is_vert {
                    hints.points[i].y = hints.points[i].oy + delta;
                    hints.points[i].flags |= AF_FLAG_TOUCH_Y;
                } else {
                    hints.points[i].x = hints.points[i].ox + delta;
                    hints.points[i].flags |= AF_FLAG_TOUCH_X;
                }
            }
            _ => {}
        }
    }
}

// ── Weak-point alignment (IUP) ─────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_weak_points` (afhints.c:1687–1808).
// Weak points (control points, straight-run points) are interpolated between
// the nearest touched points in the same contour.

fn align_weak_points(hints: &mut GlyphHints, dim: Dimension) {
    let is_vert = dim == Dimension::Vert;

    for &c_start in &hints.contours.clone() {
        // Phase 1: Set u = hinted, v = original. Find touched points.
        let mut idx = c_start;
        loop {
            let pt = &mut hints.points[idx];
            if is_vert {
                pt.u = pt.y; // hinted so far
                pt.v = pt.oy; // original scaled
            } else {
                pt.u = pt.x;
                pt.v = pt.ox;
            }
            let next = pt.next;
            if next == c_start { break; }
            idx = next;
        }

        // Phase 2: Find runs of untouched points and interpolate between
        // the enclosing touched points.
        let mut touched_prev: Option<(usize, i32, i32)> = None; // (idx, u, v)

        idx = c_start;
        loop {
            let pt = &hints.points[idx];
            let is_touched = if is_vert {
                pt.flags & AF_FLAG_TOUCH_Y != 0
            } else {
                pt.flags & AF_FLAG_TOUCH_X != 0
            };

            if is_touched {
                touched_prev = Some((idx, pt.u, pt.v));
            } else if let Some((prev_idx, prev_u, prev_v)) = touched_prev {
                // Find the next touched point (or use this as the end).
                let mut next_touched: Option<(usize, i32, i32)> = None;
                let mut scan = pt.next;
                while scan != c_start {
                    let st = &hints.points[scan];
                    let st_touched = if is_vert {
                        st.flags & AF_FLAG_TOUCH_Y != 0
                    } else {
                        st.flags & AF_FLAG_TOUCH_X != 0
                    };
                    if st_touched {
                        next_touched = Some((scan, st.u, st.v));
                        break;
                    }
                    scan = hints.points[scan].next;
                }

                if let Some((next_idx, next_u, next_v)) = next_touched {
                    // Interpolate all points between prev and next.
                    let mut interp = hints.points[prev_idx].next;
                    while interp != next_idx {
                        let ipt = &hints.points[interp];
                        // Linear interpolation in original space (v).
                        let total_v_range = next_v - prev_v;
                        let pt_offset = ipt.v - prev_v;
                        if total_v_range != 0 {
                            let fraction = pt_offset as i64;
                            let total = total_v_range as i64;
                            let new_u = prev_u as i64
                                + (fraction * (next_u - prev_u) as i64) / total;
                            if is_vert {
                                hints.points[interp].y = new_u as i32;
                            } else {
                                hints.points[interp].x = new_u as i32;
                            }
                        } else {
                            // Degenerate: just copy prev_u.
                            if is_vert {
                                hints.points[interp].y = prev_u;
                            } else {
                                hints.points[interp].x = prev_u;
                            }
                        }
                        interp = hints.points[interp].next;
                    }
                } else {
                    // No next touched point in contour (shouldn't happen,
                    // since the first/last point of each edge segment is
                    // touched): apply uniform shift from prev.
                    let delta = prev_u - prev_v;
                    if is_vert {
                        hints.points[idx].y = hints.points[idx].oy + delta;
                    } else {
                        hints.points[idx].x = hints.points[idx].ox + delta;
                    }
                }
            }

            let next = hints.points[idx].next;
            if next == c_start { break; }
            idx = next;
        }
    }
}
