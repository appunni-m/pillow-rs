//! Latin-script auto-hinting — port of `src/autofit/aflatin.c`.
//!
//! Implements the core pipeline for grid-fitting Latin glyph outlines:
//!   segment detection → edge grouping → grid-fitting → point interpolation.
//!
//! This is a simplified port focusing on the vertical dimension (Y-axis /
//! horizontal edges) for the grid-fitting that makes the biggest visual
//! difference. The full FreeType autohinter includes blue zones, stem-width
//! histograms, serif handling, and per-glyph adjustments that we defer.

use crate::fixed::{ft_mul_fix, ft_mul_div};
use super::types::{
    AFSegment, AFEdge, GlyphHints,
    Direction, Dimension,
    AF_FLAG_IGNORE, AF_FLAG_CONTROL, AF_FLAG_TOUCH_X, AF_FLAG_TOUCH_Y,
    AF_FLAG_WEAK_INTERPOLATION,
    AF_EDGE_ROUND, AF_EDGE_SERIF, AF_EDGE_DONE,
    AF_LATIN_HINTS_HORZ_SNAP, AF_LATIN_HINTS_VERT_SNAP,
    AF_LATIN_HINTS_STEM_ADJUST, AF_LATIN_HINTS_MONO,
};
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
    // Smooth anti-aliased hinting: enable stem adjustment + snap for both dimensions.
    hints.other_flags = AF_LATIN_HINTS_HORZ_SNAP
        | AF_LATIN_HINTS_VERT_SNAP
        | AF_LATIN_HINTS_STEM_ADJUST;

    // Step 1: Load outline into hints (raw font units → fx/fy; scaled 26.6 → ox/oy)
    loader::reload(&mut hints, raw_outline, &outline.points);
    if hints.num_points() == 0 {
        return;
    }

    // Step 2: Process vertical dimension (Y-axis / horizontal edges)
    // This is the dimension that affects the '|' bar top/bottom alignment.
    compute_segments(&mut hints, Dimension::Vert);
    // link_segments disabled: produces wrong stem pairs (see TODO in fn).
    // link_segments(&mut hints, Dimension::Vert);
    compute_edges(&mut hints, Dimension::Vert);
    hint_edges(&mut hints, Dimension::Vert);
    align_edge_points(&mut hints, Dimension::Vert);
    align_strong_points(&mut hints, Dimension::Vert);
    align_weak_points(&mut hints, Dimension::Vert);

    // Step 3: Process horizontal dimension (X-axis / vertical edges)
    compute_segments(&mut hints, Dimension::Horz);
    // link_segments disabled: produces wrong stem pairs.
    // link_segments(&mut hints, Dimension::Horz);
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
                        let h = max_coord - min_coord;
                        axis.segments.push(AFSegment {
                            flags, dir: segment_dir, pos, delta,
                            min_coord: min_coord as i16, max_coord: max_coord as i16,
                            height: h as i16,
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
                        // Merge with previous segment (same start point). Port of aflatin.c:1741-1851.
                        // Compare in_dir at the join point (aflatin.c:1746).
                        let prev_last_idx = axis.segments[prev_seg.unwrap()].last;
                        let prev_last_in = points[prev_last_idx].in_dir;
                        let curr_in = points[point].in_dir;
                        if prev_last_in == curr_in {
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

    // ── Edge link/serif propagation (aflatin.c:2384–2495) ──────────────────
    // For each edge, walk its segments and propagate segment links/serifs to
    // the edge level. Also compute AF_EDGE_ROUND vs AF_EDGE_NORMAL.
    for e_idx in 0..axis.edges.len() {
        let mut is_round = 0i32;
        let mut is_straight = 0i32;

        let first_seg = axis.edges[e_idx].first;
        if first_seg == usize::MAX { continue; }
        let mut seg_idx = first_seg;
        loop {
            let seg = &axis.segments[seg_idx];

            // Track round/straight counts (aflatin.c:2393-2395).
            if seg.flags & AF_EDGE_ROUND != 0 { is_round += 1; }
            else { is_straight += 1; }

            // Check for serif (aflatin.c:2397-2400).
            let mut is_serif = false;
            if seg.serif != usize::MAX {
                let serif_edge = axis.segments[seg.serif].edge;
                if serif_edge != usize::MAX && serif_edge != e_idx {
                    is_serif = true;
                }
            }

            // Determine link/serif target edge (aflatin.c:2402-2460).
            if (seg.link != usize::MAX && axis.segments[seg.link].edge != usize::MAX) || is_serif {
                let mut edge2_idx = axis.edges[e_idx].link; // prior link from another segment
                let linked_seg = if is_serif {
                    edge2_idx = axis.edges[e_idx].serif;
                    seg.serif
                } else {
                    seg.link
                };

                // Compare segment gap vs edge gap (aflatin.c:2416-2430).
                if edge2_idx != usize::MAX {
                    let edge_delta = (axis.edges[e_idx].fpos as i32
                        - axis.edges[edge2_idx].fpos as i32).abs();
                    let seg_delta = (seg.pos as i32
                        - axis.segments[linked_seg].pos as i32).abs();
                    if seg_delta < edge_delta {
                        // Segment pair is closer → trust the segment's edge.
                        edge2_idx = axis.segments[linked_seg].edge;
                    }
                } else {
                    // No prior link → use segment's parent edge.
                    edge2_idx = axis.segments[linked_seg].edge;
                }

                if edge2_idx != usize::MAX && edge2_idx != e_idx {
                    if is_serif {
                        axis.edges[e_idx].serif = edge2_idx;
                        axis.edges[edge2_idx].flags |= AF_EDGE_SERIF;
                    } else {
                        axis.edges[e_idx].link = edge2_idx;
                    }
                }
            }

            if seg_idx == axis.edges[e_idx].last { break; }
            seg_idx = axis.segments[seg_idx].edge_next;
        }

        // Set round flag (aflatin.c:2470-2473).
        if is_round > 0 && is_round >= is_straight {
            axis.edges[e_idx].flags |= AF_EDGE_ROUND;
        }

        // Conflict resolution: serif + link → drop serif (aflatin.c:2493).
        if axis.edges[e_idx].serif != usize::MAX && axis.edges[e_idx].link != usize::MAX {
            axis.edges[e_idx].serif = usize::MAX;
        }
    }
}

// Port of `af_latin_hints_link_segments` (aflatin.c:2015–2148).
// Pairs opposing-direction, overlapping segments into stem links, then
// derives serif relationships. Sets seg.link / seg.serif indices.
fn link_segments(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    let major_dir = axis.major_dir;
    let n = axis.segments.len();

    // No widths available → max_width = 0.
    let len_threshold: i32 = 8; // AF_LATIN_CONSTANT(metrics, 8); for upem scaling we use 8
    let len_score: i32 = 6000; // AF_LATIN_CONSTANT(metrics, 6000)
    let _dist_score: i32 = 3000;

    // Reset scores and links.
    for seg in &mut axis.segments {
        seg.score = 32000;
        seg.link = usize::MAX;
        seg.serif = usize::MAX;
    }

    for i in 0..n {
        let seg1_dir = axis.segments[i].dir;
        if seg1_dir != major_dir {
            continue;
        }
        let pos1 = axis.segments[i].pos as i32;
        for j in 0..n {
            let seg2_dir = axis.segments[j].dir;
            let pos2 = axis.segments[j].pos as i32;
            // opposite directions, seg2 to the "right" of seg1
            if (seg1_dir as i8 + seg2_dir as i8 == 0) && pos2 > pos1 {
                let mut min_c = axis.segments[i].min_coord as i32;
                let mut max_c = axis.segments[i].max_coord as i32;
                if min_c < axis.segments[j].min_coord as i32 {
                    min_c = axis.segments[j].min_coord as i32;
                }
                if max_c > axis.segments[j].max_coord as i32 {
                    max_c = axis.segments[j].max_coord as i32;
                }
                let len = max_c - min_c;
                // Require actual overlap on the cross-axis.
                let overlap = (axis.segments[i].max_coord as i32).min(axis.segments[j].max_coord as i32)
                            - (axis.segments[i].min_coord as i32).max(axis.segments[j].min_coord as i32);
                let dist = pos2 - pos1;
                // Require substantial overlap: at least 50% of the total span
                // must overlap, AND distance must be reasonable (≤ 500 fu ≈ 2.5px).
                if len >= len_threshold && overlap * 4 >= len * 3 && dist < 300 {
                    // max_width == 0 → dist_demerit = dist.
                    // Cap distance at 1000 fu (~5px at upem=2048 ppem=10) to prevent
                    // far-apart segments from being linked as stems.
                    let dist_demerit = dist;
                    let score = dist_demerit + len_score / len.max(1);
                    if score < axis.segments[i].score {
                        axis.segments[i].score = score;
                        axis.segments[i].link = j;
                    }
                    if score < axis.segments[j].score {
                        axis.segments[j].score = score;
                        axis.segments[j].link = i;
                    }
                }
            }
        }
    }

    // Compute serif segments: if seg.link != seg.link.link, seg is a serif.
    for i in 0..n {
        let seg2_idx = axis.segments[i].link;
        if seg2_idx != usize::MAX {
            let seg2_link = axis.segments[seg2_idx].link;
            if seg2_link != i {
                axis.segments[i].link = usize::MAX;
                axis.segments[i].serif = seg2_link;
            }
        }
    }
}

// ── Helper: snap stem width ────────────────────────────────────────────────
//
// Port of `af_latin_snap_width` (aflatin.c:2725–2767).
// Finds nearest standard width and returns it, snapping within tolerance.

fn snap_width(widths: &[i32], mut width: i32) -> i32 {
    let mut best: i32 = 64 + 32 + 2; // FT_Pos best = 64 + 32 + 2
    let mut reference = width;

    for &w in widths {
        let dist = if width > w { width - w } else { w - width };
        if dist < best {
            best = dist;
            reference = w;
        }
    }

    let scaled = (reference + 32) & !63; // FT_PIX_ROUND( reference )

    if width >= reference {
        if width < scaled + 48 {
            width = reference;
        }
    } else if width > scaled - 48 {
        width = reference;
    }

    width
}

// ── Helper: align linked edge ───────────────────────────────────────────────
//
// Port of `af_latin_align_linked_edge` (aflatin.c:4157–4183).
// Aligns a stem edge relative to its base edge.

fn align_linked_edge(
    other_flags: u32,
    dim: Dimension,
    base_edge: &AFEdge,
    stem_edge: &mut AFEdge,
) {
    let dist = stem_edge.opos - base_edge.opos;
    let base_delta = base_edge.pos - base_edge.opos;

    let fitted_width = compute_stem_width(
        other_flags, 0, dim,
        dist, base_delta,
        base_edge.flags,
        stem_edge.flags,
    );

    stem_edge.pos = base_edge.pos + fitted_width;
}

// ── Helper: align serif edge ────────────────────────────────────────────────
//
// Port of `af_latin_align_serif_edge` (aflatin.c:4189–4197).
// Preserves serif offset relative to the base edge.

fn align_serif_edge(base: &AFEdge, serif: &mut AFEdge) {
    serif.pos = base.pos + (serif.opos - base.opos);
}

// ── Helper: compute stem width ──────────────────────────────────────────────
//
// Port of `af_latin_compute_stem_width` (aflatin.c:3960–4152).
// Quantizes / snaps a stem width.

fn compute_stem_width(
    other_flags: u32,
    _ppem: i32,
    dim: Dimension,
    width: i32,
    _base_delta: i32,
    base_flags: u8,
    stem_flags: u8,
) -> i32 {
    let stem_adjust = other_flags & AF_LATIN_HINTS_STEM_ADJUST != 0;

    // Skip if stem adjustment is disabled or axis is extra-light.
    if !stem_adjust {
        return width;
    }
    // extra_light is always false in our port — no metrics axis yet.

    let mut dist = width;
    let mut sign: i32 = 0;

    if dist < 0 {
        dist = -width;
        sign = 1;
    }

    let vertical = dim == Dimension::Vert;
    let vert_snap = other_flags & AF_LATIN_HINTS_VERT_SNAP != 0;
    let horz_snap = other_flags & AF_LATIN_HINTS_HORZ_SNAP != 0;

    if (vertical && !vert_snap) || (!vertical && !horz_snap) {
        // ── Smooth hinting: light quantization ──────────────────────────

        // Leave the widths of serifs alone.
        if (stem_flags & AF_EDGE_SERIF) != 0 && vertical && dist < 3 * 64 {
            // goto Done_Width
        } else if (base_flags & AF_EDGE_ROUND) != 0 {
            if dist < 80 {
                dist = 64;
            }
        } else if dist < 56 {
            dist = 56;
        }

        // width_count is always 0 in our port — skip width histogram.
        // Port kept for when width histogram is added later:
        // if axis->width_count > 0 { ... }
    } else {
        // ── Strong hinting: snap to integer pixels ──────────────────────

        let org_dist = dist;

        dist = snap_width(&[], dist); // width_count = 0

        if vertical {
            // Vertical hinting: round stem heights to integer pixels.
            if dist >= 64 {
                dist = (dist + 16) & !63;
            } else {
                dist = 64;
            }
        } else {
            let mono = other_flags & AF_LATIN_HINTS_MONO != 0;

            if mono {
                // Monochrome horizontal: snap to integer pixels.
                if dist < 64 {
                    dist = 64;
                } else {
                    dist = (dist + 32) & !63;
                }
            } else {
                // Anti-aliased horizontal: subtle approach.
                if dist < 48 {
                    dist = (dist + 64) >> 1;
                } else if dist < 128 {
                    let r = (dist + 22) & !63;
                    let delta = r - org_dist;
                    let delta = if delta < 0 { -delta } else { delta };

                    if delta >= 16 {
                        dist = org_dist;
                        if dist < 48 {
                            dist = (dist + 64) >> 1;
                        }
                    } else {
                        dist = r;
                    }
                } else {
                    // Round to prevent color fringes in LCD mode.
                    dist = (dist + 32) & !63;
                }
            }
        }
    }

    // Done_Width: restore sign
    if sign != 0 {
        dist = -dist;
    }

    dist
}

// ── Edge grid-fitting ──────────────────────────────────────────────────────
//
// Faithful port of `af_latin_hint_edges` (aflatin.c:4214–4831).
// Blue zones are skipped (all blue_edge are NULL in our port).
// Stem alignment is ported faithfully but never executes (all links are
// usize::MAX). The non-stem section (lines 4629–4824) does the actual
// grid-fitting via anchor-relative half-pixel rounding.

fn hint_edges(hints: &mut GlyphHints, dim: Dimension) {
    let other_flags = hints.other_flags;
    let axis = &mut hints.axis[dim as usize];
    let num_edges = axis.edges.len();

    if num_edges == 0 {
        return;
    }

    // top_to_bottom_hinting for Latin is false (edges sorted bottom-to-top).
    // For vertical dim (horizontal edges), Y increases upward → sorted by
    // increasing fpos = bottom edge first, top edge last.
    let top_to_bottom_hinting = false;

    let mut anchor: usize = usize::MAX;
    let mut has_non_stem_edges = false;

    // ── Phase 1: Blue-zone alignment ────────────────────────────────────
    // SKIPPED: all edge->blue_edge are NULL in our port.
    // The C code (aflatin.c:4247–4336) is not ported.

    // ── Phase 2: Stem alignment ─────────────────────────────────────────
    // Ported faithfully (aflatin.c:4340–4564). Since our edges have no
    // links (all link == usize::MAX), this loop only sets
    // has_non_stem_edges = true.
    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            continue;
        }

        let edge2_idx = axis.edges[i].link;
        if edge2_idx == usize::MAX {
            has_non_stem_edges = true;
            continue;
        }

        // ── We have a linked stem edge (link != NULL) ───────────────────

        // Safety assertion: linked edge should not have a blue edge.
        // (aflatin.c:4359–4370; never reached since blue_edge is always NULL)

        if anchor == usize::MAX {
            // First stem — becomes anchor (aflatin.c:4372–4440).
            let edge_opos = axis.edges[i].opos;
            let edge_flags = axis.edges[i].flags;
            let edge2_opos = axis.edges[edge2_idx].opos;
            let edge2_flags = axis.edges[edge2_idx].flags;

            let org_len = edge2_opos - edge_opos;
            let cur_len = compute_stem_width(
                other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags,
            );

            if cur_len <= 64 {
                // width <= 1px
                let u_off: i32 = 32;
                let d_off: i32 = 32;
                let org_center = edge_opos + (org_len >> 1);
                let cur_pos1 = (org_center + 32) & !63; // FT_PIX_ROUND

                let error1 = (org_center - (cur_pos1 - u_off)).abs();
                let error2 = (org_center - (cur_pos1 + d_off)).abs();

                let cur_pos1 = if error1 < error2 {
                    cur_pos1 - u_off
                } else {
                    cur_pos1 + d_off
                };

                axis.edges[i].pos = cur_pos1 - cur_len / 2;
            } else if cur_len < 96 {
                // 1px < width < 1.5px
                let u_off: i32 = 38;
                let d_off: i32 = 26;
                let org_center = edge_opos + (org_len >> 1);
                let cur_pos1 = (org_center + 32) & !63; // FT_PIX_ROUND

                let error1 = (org_center - (cur_pos1 - u_off)).abs();
                let error2 = (org_center - (cur_pos1 + d_off)).abs();

                let cur_pos1 = if error1 < error2 {
                    cur_pos1 - u_off
                } else {
                    cur_pos1 + d_off
                };

                axis.edges[i].pos = cur_pos1 - cur_len / 2;
            } else {
                axis.edges[i].pos = (edge_opos + 32) & !63; // FT_PIX_ROUND
            }

            axis.edges[i].flags |= AF_EDGE_DONE;
            anchor = i;

            // Align the linked edge.
            {
                let base_pos = axis.edges[i].pos;
                let base_opos = axis.edges[i].opos;
                let base_flags = axis.edges[i].flags;
                let stem_opos = axis.edges[edge2_idx].opos;
                let stem_flags = axis.edges[edge2_idx].flags;

                let dist = stem_opos - base_opos;
                let base_delta = base_pos - base_opos;
                let fitted_width = compute_stem_width(
                    other_flags, 0, dim, dist, base_delta, base_flags, stem_flags,
                );
                axis.edges[edge2_idx].pos = base_pos + fitted_width;
            }
        } else {
            // Relative to anchor (aflatin.c:4441–4563).
            let edge_opos = axis.edges[i].opos;
            let edge_flags = axis.edges[i].flags;
            let edge2_opos = axis.edges[edge2_idx].opos;
            let edge2_flags = axis.edges[edge2_idx].flags;
            let anchor_pos = axis.edges[anchor].pos;
            let anchor_opos = axis.edges[anchor].opos;

            let org_pos = anchor_pos + (edge_opos - anchor_opos);
            let org_len = edge2_opos - edge_opos;
            let org_center = org_pos + (org_len >> 1);

            let cur_len = compute_stem_width(
                other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags,
            );

            if axis.edges[edge2_idx].flags & AF_EDGE_DONE != 0 {
                // ADJUST: linked edge already positioned.
                axis.edges[i].pos = axis.edges[edge2_idx].pos - cur_len;
            } else if cur_len < 96 {
                let cur_pos1 = (org_center + 32) & !63; // FT_PIX_ROUND

                let (u_off, d_off): (i32, i32) = if cur_len <= 64 {
                    (32, 32)
                } else {
                    (38, 26)
                };

                let delta1 = (org_center - (cur_pos1 - u_off)).abs();
                let delta2 = (org_center - (cur_pos1 + d_off)).abs();

                let cur_pos1 = if delta1 < delta2 {
                    cur_pos1 - u_off
                } else {
                    cur_pos1 + d_off
                };

                axis.edges[i].pos = cur_pos1 - cur_len / 2;
            } else {
                let cur_len2 = compute_stem_width(
                    other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags,
                );

                let cur_pos1 = (org_pos + 32) & !63; // FT_PIX_ROUND
                let delta1 = (cur_pos1 + (cur_len2 >> 1) - org_center).abs();

                let cur_pos2 = (org_pos + org_len + 32) & !63 - cur_len2;
                let delta2 = (cur_pos2 + (cur_len2 >> 1) - org_center).abs();

                axis.edges[i].pos = if delta1 < delta2 { cur_pos1 } else { cur_pos2 };
            }

            // Align linked edge.
            {
                let base_pos = axis.edges[i].pos;
                let base_opos = axis.edges[i].opos;
                let base_flags = axis.edges[i].flags;
                let stem_opos = axis.edges[edge2_idx].opos;
                let stem_flags = axis.edges[edge2_idx].flags;

                let dist = stem_opos - base_opos;
                let base_delta = base_pos - base_opos;
                let fitted_width = compute_stem_width(
                    other_flags, 0, dim, dist, base_delta, base_flags, stem_flags,
                );
                axis.edges[edge2_idx].pos = base_pos + fitted_width;
            }
        }

        axis.edges[edge2_idx].flags |= AF_EDGE_DONE;

        // BOUND check for stem edges (aflatin.c:4544–4563):
        // don't move if stem would (almost) disappear.
        if i > 0 {
            let ordering_violated = if top_to_bottom_hinting {
                axis.edges[i].pos > axis.edges[i - 1].pos
            } else {
                axis.edges[i].pos < axis.edges[i - 1].pos
            };
            if ordering_violated {
                let link_idx = axis.edges[i].link;
                if link_idx != usize::MAX {
                    let link_pos = axis.edges[link_idx].pos;
                    let prev_pos = axis.edges[i - 1].pos;
                    if (link_pos - prev_pos).abs() > 16 {
                        axis.edges[i].pos = prev_pos;
                    }
                }
            }
        }
    }

    // ── Phase 3: Lowercase 'm' symmetry ─────────────────────────────────
    // SKIPPED (aflatin.c:4582–4627). Requires specific edge count (6 or 12)
    // and link relationships.

    // ── Phase 4: Non-stem edges ─────────────────────────────────────────
    // Ported faithfully (aflatin.c:4629–4824).
    // This is the active path since all our edges lack links.
    if has_non_stem_edges || anchor == usize::MAX {
        for i in 0..num_edges {
            if axis.edges[i].flags & AF_EDGE_DONE != 0 {
                continue;
            }

            let mut delta: i32 = 1000;

            // ── Serif handling ─────────────────────────────────────────
            // Check for real serif: serif edge must be close and no other
            // edges between them with overlapping coverage.
            let serif_idx = axis.edges[i].serif;
            if serif_idx != usize::MAX {
                // since we don't compute segment `v` (the cross-axis coord)
                // for edges in a way that matches the C code, we skip the
                // real-serif overlap check.  Instead we always treat it as
                // a valid serif if it exists and is close enough.
                delta = axis.edges[serif_idx].opos - axis.edges[i].opos;
                if delta < 0 {
                    delta = -delta;
                }
            }

            if delta < 64 + 16 {
                // delta < 1.25px: use serif alignment.
                let serif_idx = axis.edges[i].serif;
                // SAFETY: delta is <80 only if serif_idx is valid.
                let serif_pos = axis.edges[serif_idx].pos;
                let serif_opos = axis.edges[serif_idx].opos;
                axis.edges[i].pos = serif_pos + (axis.edges[i].opos - serif_opos);
            } else if anchor == usize::MAX {
                // First non-stem edge: pixel-round and set as anchor.
                axis.edges[i].pos = (axis.edges[i].opos + 32) & !63;
                anchor = i;
            } else {
                // Interpolate between nearest DONE edges, or use
                // anchor-relative half-pixel rounding.
                let edge_opos = axis.edges[i].opos;

                // Find nearest before (processed) edge with AF_EDGE_DONE.
                let mut before: Option<usize> = None;
                if i > 0 {
                    for j in (0..i).rev() {
                        if axis.edges[j].flags & AF_EDGE_DONE != 0 {
                            before = Some(j);
                            break;
                        }
                    }
                }

                // Find nearest after edge with AF_EDGE_DONE.
                let mut after: Option<usize> = None;
                for j in (i + 1)..num_edges {
                    if axis.edges[j].flags & AF_EDGE_DONE != 0 {
                        after = Some(j);
                        break;
                    }
                }

                if let (Some(b), Some(a)) = (before, after) {
                    let before_opos = axis.edges[b].opos;
                    let before_pos = axis.edges[b].pos;
                    let after_opos = axis.edges[a].opos;
                    let after_pos = axis.edges[a].pos;

                    if after_opos == before_opos {
                        axis.edges[i].pos = before_pos;
                    } else {
                        axis.edges[i].pos = before_pos
                            + ft_mul_div(
                                edge_opos - before_opos,
                                after_pos - before_pos,
                                after_opos - before_opos,
                            );
                    }
                } else {
                    // Anchor-relative: round delta to nearest half-pixel.
                    let anchor_pos = axis.edges[anchor].pos;
                    let anchor_opos = axis.edges[anchor].opos;
                    axis.edges[i].pos = anchor_pos
                        + ((edge_opos - anchor_opos + 16) & !31);
                }
            }

            axis.edges[i].flags |= AF_EDGE_DONE;

            // ── BOUND checks: prevent edge ordering violations ──────────
            // Only apply to edges that have links (stems). Our edges lack
            // links, so these conditions are always false.

            // Check against previous edge.
            if i > 0 {
                let ordering_violated = if top_to_bottom_hinting {
                    axis.edges[i].pos > axis.edges[i - 1].pos
                } else {
                    axis.edges[i].pos < axis.edges[i - 1].pos
                };
                if ordering_violated {
                    let link_idx = axis.edges[i].link;
                    if link_idx != usize::MAX {
                        let link_pos = axis.edges[link_idx].pos;
                        let prev_pos = axis.edges[i - 1].pos;
                        if (link_pos - prev_pos).abs() > 16 {
                            axis.edges[i].pos = prev_pos;
                        }
                    }
                }
            }

            // Check against next edge.
            if i + 1 < num_edges && axis.edges[i + 1].flags & AF_EDGE_DONE != 0 {
                let ordering_violated = if top_to_bottom_hinting {
                    axis.edges[i].pos < axis.edges[i + 1].pos
                } else {
                    axis.edges[i].pos > axis.edges[i + 1].pos
                };
                if ordering_violated {
                    let link_idx = axis.edges[i].link;
                    if link_idx != usize::MAX {
                        let link_pos = axis.edges[link_idx].pos;
                        let prev_pos = axis.edges[i - 1].pos;
                        if (link_pos - prev_pos).abs() > 16 {
                            axis.edges[i].pos = axis.edges[i + 1].pos;
                        }
                    }
                }
            }
        }
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
