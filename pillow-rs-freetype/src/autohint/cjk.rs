//! CJK auto-hinter — port of `src/autofit/afcjk.c`.
//!
//! Provides top_to_bottom edge detection for Indic/CJK scripts.
//! The remaining pipeline (hint_edges, align_edge_points, etc.)
//! is shared with latin.rs.
//!
//! Key differences from Latin edge detection:
//!   1. Best-distance matching instead of first-match
//!   2. Linked segment compatibility check
//!   3. Top-to-bottom edge insertion order
//!
//! Note: Bengali/Devanagari/Gurmukhi use AF_WRITING_SYSTEM_LATIN (confirmed
//! via C trace). The only difference from Latin is top_to_bottom_hinting=true
//! for the VERT dimension. The edge sort direction is handled directly in
//! latin.rs::compute_edges. This module is reserved for future afcjk.c port.
//!
//! Verified against: FreeType 2.14.3 afcjk.c:993-1193

use super::types::*;
use crate::casts::i16_from_i32;
use crate::fixed::{ft_mul_fix, ft_mul_div};

/// Compute edges for CJK scripts using best-distance matching.
/// Port of af_cjk_hints_compute_edges (afcjk.c:993-1193).
/// Uses linked-segment compatibility check (not present in Latin path).
#[allow(dead_code)]
pub fn cjk_compute_edges(hints: &mut GlyphHints, dim: Dimension, top_to_bottom: bool) {
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz { hints.x_scale } else { hints.y_scale };
    axis.edges.clear();

    let edge_dist_thresh = {
        let raw = hints.metrics.as_ref()
            .map_or(50, |m| m.axis[dim as usize].edge_distance_threshold);
        let mut edt = ft_mul_fix(raw, scale);
        if edt > 16 { edt = ft_mul_div(16, 0x10000, scale); }
        else { edt = raw; }
        edt
    };

    for seg_idx in 0..axis.segments.len() {
        let seg = &axis.segments[seg_idx];
        let seg_pos = seg.pos as i32;
        let mut best_edge: Option<usize> = None;
        let mut best_dist = i32::MAX;

        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir != seg.dir { continue; }
            let dist = (edge.fpos as i32 - seg_pos).abs();
            if dist < edge_dist_thresh && dist < best_dist {
                let link = seg.link;
                if link != usize::MAX {
                    let mut ok = true;
                    let mut s1 = edge.first;
                    loop {
                        let link1 = axis.segments[s1].link;
                        if link1 != usize::MAX {
                            let d2 = (axis.segments[link].pos as i32 -
                                      axis.segments[link1].pos as i32).abs();
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
            let e = &mut axis.edges[e_idx];
            axis.segments[seg_idx].edge_next = e.first;
            let pl = e.last;
            axis.segments[pl].edge_next = seg_idx;
            e.last = seg_idx;
        } else {
            let fpos = i16_from_i32(seg_pos);
            let opos = ft_mul_fix(fpos as i32, scale);
            let new_edge = AFEdge {
                fpos, opos, pos: opos, flags: 0, dir: seg.dir,
                link: usize::MAX, serif: usize::MAX,
                first: seg_idx, last: seg_idx, blue_edge: None,
            };
            let insert_at = if top_to_bottom {
                let mut p = 0;
                while p < axis.edges.len() && axis.edges[p].fpos > fpos { p += 1; }
                p
            } else {
                let mut p = 0;
                while p < axis.edges.len() && axis.edges[p].fpos < fpos { p += 1; }
                p
            };
            axis.edges.insert(insert_at, new_edge);
        }
    }

    for e_idx in 0..axis.edges.len() {
        let mut s = axis.edges[e_idx].first;
        loop {
            axis.segments[s].edge = e_idx;
            if s == axis.edges[e_idx].last { break; }
            s = axis.segments[s].edge_next;
        }
    }
    for e in &mut axis.edges { e.flags = AF_EDGE_NORMAL; }
}
