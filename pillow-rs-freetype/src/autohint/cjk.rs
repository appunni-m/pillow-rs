//! CJK auto-hinter — port of `src/autofit/afcjk.c`.
//!
//! Provides top_to_bottom edge detection for Indic/CJK scripts.
//! The remaining pipeline (hint_edges, align_edge_points, etc.)
//! is shared with latin.rs.
//!
//! Key differences from Latin edge detection:
//!   1. Best-distance matching instead of first-match
//!   2. Linked segment compatibility check
//!   3. Top-to-bottom vs bottom-to-top edge insertion order
//!   4. Simpler edge flags (CJK doesn't use round/straight classification)
//!
//! Verified against: FreeType 2.14.3 afcjk.c:993-1193

use super::types::*;
use crate::casts::i16_from_i32;
use crate::fixed::{ft_mul_fix, ft_mul_div};

/// Compute edges for CJK/Indic scripts using best-distance matching
/// and top-to-bottom edge insertion.
/// Port of af_cjk_hints_compute_edges (afcjk.c:993-1193).
pub fn cjk_compute_edges(hints: &mut GlyphHints, dim: Dimension, top_to_bottom: bool) {
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz { hints.x_scale } else { hints.y_scale };

    axis.edges.clear();

    // Edge distance threshold (matches C: afcjk.c:1032-1037)
    let edge_dist_thresh = {
        let raw = hints.metrics.as_ref()
            .map_or(50, |m| m.axis[dim as usize].edge_distance_threshold);
        let mut edt = ft_mul_fix(raw, scale);
        if edt > 16 { edt = ft_mul_div(16, 0x10000, scale); }
        else { edt = raw; }
        edt
    };

    // Phase 1: Create edges from segments (afcjk.c:1040-1120)
    for seg_idx in 0..axis.segments.len() {
        let seg = &axis.segments[seg_idx];
        let seg_pos = seg.pos as i32;

        // Find best-matching existing edge
        let mut best_edge: Option<usize> = None;
        let mut best_dist = i32::MAX; // was 0xFFFFU in C

        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir != seg.dir { continue; }

            let dist = (edge.fpos as i32 - seg_pos).abs();
            if dist < edge_dist_thresh && dist < best_dist {
                // Check linked segment compatibility (afcjk.c:1065-1085)
                let link = seg.link;
                if link != usize::MAX {
                    let mut ok = true;
                    let mut s1 = edge.first;
                    loop {
                        let link1 = axis.segments[s1].link;
                        if link1 != usize::MAX {
                            let d2 = (
                                axis.segments[link].pos as i32 -
                                axis.segments[link1].pos as i32
                            ).abs();
                            if d2 >= edge_dist_thresh { ok = false; break; }
                        }
                        if s1 == edge.last { break; }
                        s1 = axis.segments[s1].edge_next;
                    }
                    if !ok { continue; }
                }
                best_dist = dist;
                best_edge = Some(e_idx);
            }
        }

        if let Some(e_idx) = best_edge {
            // Add segment to existing edge (afcjk.c:1112-1116)
            let e = &mut axis.edges[e_idx];
            axis.segments[seg_idx].edge_next = e.first;
            let prev_last = e.last;
            axis.segments[prev_last].edge_next = seg_idx;
            e.last = seg_idx;
        } else {
            // Create new edge with sorted insertion (afcjk.c:1088-1109)
            let fpos = i16_from_i32(seg_pos);
            let opos = ft_mul_fix(fpos as i32, scale);
            let new_edge = AFEdge {
                fpos, opos, pos: opos, flags: 0, dir: seg.dir,
                link: usize::MAX, serif: usize::MAX,
                first: seg_idx, last: seg_idx, blue_edge: None,
            };

            // C uses af_axis_hints_new_edge which inserts in sorted position.
            // For top_to_bottom: descending order (larger fpos first).
            // For bottom_to_top: ascending order (smaller fpos first).
            let insert_at = if top_to_bottom {
                // Find first edge with fpos <= new fpos (descending)
                let mut pos = 0;
                while pos < axis.edges.len() && axis.edges[pos].fpos > fpos {
                    pos += 1;
                }
                pos
            } else {
                // Find first edge with fpos >= new fpos (ascending)
                let mut pos = 0;
                while pos < axis.edges.len() && axis.edges[pos].fpos < fpos {
                    pos += 1;
                }
                pos
            };
            axis.edges.insert(insert_at, new_edge);
        }
    }

    // Phase 2: Set segment→edge references (afcjk.c:1156-1168)
    for e_idx in 0..axis.edges.len() {
        let edge = &axis.edges[e_idx];
        let mut seg = edge.first;
        loop {
            if seg == usize::MAX { break; }
            axis.segments[seg].edge = e_idx;
            if seg == edge.last { break; }
            seg = axis.segments[seg].edge_next;
        }
    }

    // Phase 3: Edge flags (afcjk.c:1170-1193) — simplified for non-Hani
    for e in &mut axis.edges {
        e.flags = AF_EDGE_NORMAL;  // always normal for Indic scripts
    }
}
