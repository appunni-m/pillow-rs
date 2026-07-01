//! CJK auto-hinter — port of `src/autofit/afcjk.c`.
//!
//! CJK/Indic scripts (beng, deva, guru, knda, mong, goth) use
//! AF_WRITING_SYSTEM_LATIN with top_to_bottom_hinting=true.
//! The key difference is in metrics computation:
//!
//!   af_cjk_metrics_init_widths (afcjk.c:63-270)
//!     — Segment-based stem width detection from the standard character.
//!       The Latin 'o'-based approach produces different stem widths
//!       for non-Latin scripts, which changes edge_distance_threshold
//!       → different edge groupings → different hinting output.
//!
//!   af_cjk_metrics_init_blues (afcjk.c:273-620)
//!     — Blue zone computation with flat/fill pairs instead of
//!       flat/round pairs. Indic scripts use the SAME blue string
//!       entries as Latin but with different extremum selection.
//!
//! C reference: FreeType 2.14.3, afcjk.c (2370 lines)
//!
//! Node: Bengali/Devanagari/Gurmukhi use AF_WRITING_SYSTEM_LATIN
//! (confirmed via C trace: style=0x2005 for Bengali). The only
//! difference from Latin is top_to_bottom_hinting=true for VERT
//! dimension. The edge sort direction and hint_edges BOUND checks
//! are handled in latin.rs.

use super::types::*;
use crate::casts::i16_from_i32;
use crate::fixed::{ft_mul_fix, ft_mul_div};

/// Compute CJK-style stem widths from a standard character glyph.
/// Port of af_cjk_metrics_init_widths (afcjk.c:63-270).
///
/// FreeType calls af_latin_hints_{compute_segments,link_segments}
/// on the standard character glyph, then extracts stem widths from
/// bidirectional segment pairs. This is DIFFERENT from the Latin
/// 'o'-based approach which scans individual stems from the 'o' outline.
///
/// For Indic scripts, the standard character is script-specific
/// (e.g., Bengali uses U+09E6 "০" from afscript.h).
///
/// UNVERIFIED: full port with annotated C line references.
pub fn cjk_metrics_init_widths(
    metrics: &mut AfLatinMetrics,
    outline: &crate::tt::glyf::GlyphOutline,
    scaled_points: &[crate::outline::OutlinePoint],
) {
    // afcjk.c:78 — init hints
    let upem = metrics.units_per_em;
    let mut hints = GlyphHints::new(0x10000, 0x10000, 0, 0);

    // afcjk.c:140-141 — identity scale dummy metrics
    let dummy = AfLatinMetrics::new(upem, 1);
    hints.metrics = Some(dummy);

    // afcjk.c:155 — reload outline into hints
    super::loader::reload(&mut hints, outline, scaled_points);

    // afcjk.c:166-202 — for each dimension: compute segments, link, extract widths
    for dim in 0..2 {
        let d = if dim == 0 { Dimension::Horz } else { Dimension::Vert };

        // afcjk.c:184 — compute segments (shared Latin function)
        super::latin::compute_segments(&mut hints, d);

        // afcjk.c:195-199 — link segments (shared Latin function, width_count=0
        // means no per-width scoring adjustment — same as our default)
        super::latin::link_segments(&mut hints, d);

        let axis = &hints.axis[dim];
        // afcjk.c:218 — 16 is AF_CJK_MAX_WIDTHS
        let mut widths = [AfWidth::default(); 16];
        let mut num_widths: usize = 0;

        // afcjk.c:222-238 — only consider bidirectional stem pairs
        for seg_idx in 0..axis.segments.len() {
            let seg = &axis.segments[seg_idx];
            let link_idx = seg.link;
            if link_idx == usize::MAX { continue; }
            let link = &axis.segments[link_idx];
            // afcjk.c:225 — bidirectional check
            if link.link != seg_idx || link_idx <= seg_idx { continue; }

            // afcjk.c:230-235 — distance as stem width
            let dist = (seg.pos as i32 - link.pos as i32).abs();
            if num_widths < 16 {
                widths[num_widths].org = dist;
                num_widths += 1;
            }
        }

        // afcjk.c:246-248 — sort and quantize (same function as Latin)
        if num_widths > 1 {
            // Insertion sort by .org (afcjk.c calls af_sort_and_quantize_widths)
            for i in 1..num_widths {
                let val = widths[i];
                let mut j = i;
                while j > 0 && val.org < widths[j - 1].org {
                    widths[j] = widths[j - 1];
                    j -= 1;
                }
                widths[j] = val;
            }
            // afcjk.c:247 — cluster within threshold = upem/100 (heuristic)
            let threshold = upem / 100;
            let mut out: usize = 0;
            let mut cur_org = widths[0].org as i32;
            let mut cur_sum = widths[0].org as i32;
            let mut cur_count: i32 = 1;
            for i in 1..num_widths {
                if (widths[i].org as i32 - cur_org).abs() <= threshold {
                    cur_sum += widths[i].org as i32;
                    cur_count += 1;
                } else {
                    widths[out].org = cur_sum / cur_count;
                    out += 1;
                    cur_org = widths[i].org as i32;
                    cur_sum = widths[i].org as i32;
                    cur_count = 1;
                }
            }
            if cur_count > 0 {
                widths[out].org = cur_sum / cur_count;
                out += 1;
            }
            num_widths = out;
        }

        // afcjk.c:252-269 — set standard_width and edge_distance_threshold
        let m_axis = &mut metrics.axis[dim];
        let stdw = if num_widths > 0 {
            widths[0].org as i32
        } else {
            // afcjk.c:256 — AF_LATIN_CONSTANT(metrics, 50) = 50 * upem / 2048
            (50 * upem / 2048) as i32
        };

        m_axis.standard_width = stdw;
        // afcjk.c:259 — "let's try 20% of the smallest width"
        m_axis.edge_distance_threshold = stdw / 5;
        m_axis.extra_light = false;  // afcjk.c:260
        m_axis.width_count = num_widths;

        // Copy widths (max AF_LATIN_MAX_WIDTHS)
        for i in 0..num_widths {
            if i < 16 {
                m_axis.widths[i] = widths[i];
            }
        }
    }
}

/// Compute edges for CJK/Indic scripts using best-distance matching.
/// Port of af_cjk_hints_compute_edges (afcjk.c:993-1193).
///
/// Key differences from Latin edge detection:
///   1. Best-distance matching instead of first-match
///   2. Linked segment compatibility check
///   3. Top-to-bottom edge insertion order
///
/// UNVERIFIED — not yet wired into the pipeline. The Latin compute_edges
/// with top_to_bottom sort is used instead.
#[allow(dead_code)]
pub fn cjk_compute_edges(hints: &mut GlyphHints, dim: Dimension, top_to_bottom: bool) {
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz { hints.x_scale } else { hints.y_scale };
    axis.edges.clear();

    // afcjk.c:1032-1037 — edge_distance_threshold in font units
    let edge_dist_thresh = {
        let raw = hints.metrics.as_ref()
            .map_or(50, |m| m.axis[dim as usize].edge_distance_threshold);
        let mut edt = ft_mul_fix(raw, scale);
        if edt > 16 { edt = ft_mul_div(16, 0x10000, scale); }
        else { edt = raw; }
        edt
    };

    // afcjk.c:1040-1120 — create edges from segments
    for seg_idx in 0..axis.segments.len() {
        let seg = &axis.segments[seg_idx];
        let seg_pos = seg.pos as i32;
        let mut best_edge: Option<usize> = None;
        let mut best_dist = i32::MAX;

        // afcjk.c:1050-1085 — find best-matching edge
        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir != seg.dir { continue; }
            let dist = (edge.fpos as i32 - seg_pos).abs();
            if dist < edge_dist_thresh && dist < best_dist {
                // afcjk.c:1065-1085 — linked segment compatibility
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
            // afcjk.c:1112-1116 — add segment to existing edge
            let e = &mut axis.edges[e_idx];
            axis.segments[seg_idx].edge_next = e.first;
            let pl = e.last;
            axis.segments[pl].edge_next = seg_idx;
            e.last = seg_idx;
        } else {
            // afcjk.c:1088-1109 — create new edge with sorted insertion
            let fpos = i16_from_i32(seg_pos);
            let opos = ft_mul_fix(fpos as i32, scale);
            let new_edge = AFEdge {
                fpos, opos, pos: opos, flags: 0, dir: seg.dir,
                link: usize::MAX, serif: usize::MAX,
                first: seg_idx, last: seg_idx, blue_edge: None,
            };
            // afcjk.c:1089 — af_axis_hints_new_edge with tb=0 for CJK
            // (CJK always uses af_axis_hints_new_edge(..., 0, ...)
            // because afcjk.c doesn't pass top_to_bottom — it's only
            // used in the BOUND checks in hint_edges)
            let insert_at = if top_to_bottom {
                let mut p = 0;
                while p < axis.edges.len() && axis.edges[p].fpos > fpos { p += 1; }
                p
            } else {
                axis.edges.len() // append to end
            };
            axis.edges.insert(insert_at, new_edge);
        }
    }

    // afcjk.c:1156-1168 — set segment→edge references
    for e_idx in 0..axis.edges.len() {
        let mut s = axis.edges[e_idx].first;
        loop {
            axis.segments[s].edge = e_idx;
            if s == axis.edges[e_idx].last { break; }
            s = axis.segments[s].edge_next;
        }
    }
    // afcjk.c:1170-1193 — edge flags (simplified for non-Hani scripts)
    for e in &mut axis.edges { e.flags = AF_EDGE_NORMAL; }
}
