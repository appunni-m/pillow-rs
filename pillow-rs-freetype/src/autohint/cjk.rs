//! CJK auto-hinter — port of `src/autofit/afcjk.c`.
//!
//! Most Indic-style rows in FreeType (beng, deva, guru, knda, mong, goth)
//! still use AF_WRITING_SYSTEM_LATIN, with top_to_bottom_hinting=true where
//! configured.  The `STYLE_DEFAULT_INDIC` rows (`limb`, `orya`, `sylo`,
//! `tibt`) use afindic.c, which delegates metrics and hinting to the CJK path.
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

use super::blue_strings::BlueStringEntry;
use super::types::*;
use crate::casts::i16_from_i32;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::tables::FontData;

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
/// This function is part of the CJK metrics path; callers provide the already
/// scaled standard-character outline so width extraction can reuse the Latin
/// segment and link machinery.
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
        let d = if dim == 0 {
            Dimension::Horz
        } else {
            Dimension::Vert
        };

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
            if link_idx == usize::MAX {
                continue;
            }
            let link = &axis.segments[link_idx];
            // afcjk.c:225 — bidirectional check
            if link.link != seg_idx || link_idx <= seg_idx {
                continue;
            }

            // afcjk.c:230-235 — distance as stem width
            let dist = i32::from(seg.pos).wrapping_sub(i32::from(link.pos)).abs();
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
            let mut cur_org = widths[0].org;
            let mut cur_sum = widths[0].org;
            let mut cur_count: i32 = 1;
            for i in 1..num_widths {
                if (widths[i].org - cur_org).abs() <= threshold {
                    cur_sum += widths[i].org;
                    cur_count += 1;
                } else {
                    widths[out].org = cur_sum / cur_count;
                    out += 1;
                    cur_org = widths[i].org;
                    cur_sum = widths[i].org;
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
            widths[0].org
        } else {
            // afcjk.c:256 — AF_LATIN_CONSTANT(metrics, 50) = 50 * upem / 2048
            50 * upem / 2048
        };

        m_axis.standard_width = stdw;
        // afcjk.c:259 — "let's try 20% of the smallest width"
        m_axis.edge_distance_threshold = stdw / 5;
        m_axis.extra_light = false; // afcjk.c:260
        m_axis.width_count = num_widths;

        // Copy widths (max AF_LATIN_MAX_WIDTHS)
        m_axis.widths[..num_widths].copy_from_slice(&widths[..num_widths]);
    }
}

/// Find CJK blue zones.
///
/// Port of `af_cjk_metrics_init_blues` for the FreeType build used by this
/// crate. `afcjk.c` undefines `AF_CONFIG_OPTION_CJK_BLUE_HANI_VERT`, so Hani
/// uses vertical top/bottom blue zones only; horizontal left/right entries in
/// generated data are intentionally ignored here.
pub fn cjk_metrics_init_blues(
    metrics: &mut AfLatinMetrics,
    font_data: &FontData,
    script_strings: &[BlueStringEntry],
) {
    for axis in &mut metrics.axis {
        axis.blue_count = 0;
        axis.blues.clear();
    }

    for entry in script_strings {
        let horiz = entry.props & 0x2 != 0;
        if horiz {
            continue;
        }

        let is_top = entry.props & 0x1 != 0;
        let axis = &mut metrics.axis[Dimension::Vert as usize];
        let mut fills: Vec<i32> = Vec::new();
        let mut flats: Vec<i32> = Vec::new();
        let mut fill = true;

        for &ch in entry.chars {
            if ch == '|' {
                fill = false;
                continue;
            }

            let glyph_index = font_data.cmap.char_index(ch as u32).unwrap_or(0);
            if glyph_index == 0 {
                continue;
            }

            let Ok(outline) = crate::tt::glyf::load_glyph(
                &font_data.glyf_data,
                &font_data.loca_data,
                font_data.head.index_to_loc_format,
                glyph_index,
                &font_data.hmtx,
            ) else {
                continue;
            };
            if outline.num_contours == 0 || outline.points.len() <= 2 {
                continue;
            }

            let mut best_pos: Option<i32> = None;
            let mut last = -1i32;
            for &end_pt in outline
                .end_pts_of_contours
                .iter()
                .take(outline.num_contours as usize)
            {
                let first = last + 1;
                last = end_pt as i32;
                if last <= first {
                    continue;
                }

                for idx in first..=last {
                    let y = outline.points[idx as usize].y;
                    best_pos = Some(match best_pos {
                        None => y,
                        Some(best) if is_top => best.max(y),
                        Some(best) => best.min(y),
                    });
                }
            }

            let Some(best_pos) = best_pos else {
                continue;
            };
            if fill {
                fills.push(best_pos);
            } else {
                flats.push(best_pos);
            }
        }

        if fills.is_empty() && flats.is_empty() {
            continue;
        }
        fills.sort_unstable();
        flats.sort_unstable();

        let (mut ref_org, mut shoot_org) = if flats.is_empty() {
            let value = fills[fills.len() / 2];
            (value, value)
        } else if fills.is_empty() {
            let value = flats[flats.len() / 2];
            (value, value)
        } else {
            (fills[fills.len() / 2], flats[flats.len() / 2])
        };

        if shoot_org != ref_org {
            let under_ref = shoot_org < ref_org;
            if is_top ^ under_ref {
                let mean = (shoot_org + ref_org) / 2;
                ref_org = mean;
                shoot_org = mean;
            }
        }

        let mut blue = AfLatinBlue::default();
        blue.ref_width.org = ref_org;
        blue.shoot_width.org = shoot_org;
        if is_top {
            blue.flags |= AF_LATIN_BLUE_TOP;
        }
        axis.blues.push(blue);
        axis.blue_count = axis.blues.len();
    }
}

/// Scale CJK metrics and blue zones.
///
/// CJK has no Latin x-height scale adjustment; both axes are scaled directly.
pub fn cjk_metrics_scale(
    metrics: &mut AfLatinMetrics,
    x_scale: i32,
    y_scale: i32,
    x_delta: i32,
    y_delta: i32,
) -> (i32, i32) {
    for dim in 0..2 {
        let scale = if dim == Dimension::Horz as usize {
            x_scale
        } else {
            y_scale
        };
        let delta = if dim == Dimension::Horz as usize {
            x_delta
        } else {
            y_delta
        };
        let axis = &mut metrics.axis[dim];
        axis.org_scale = scale;
        axis.org_delta = delta;
        axis.scale = scale;
        axis.delta = delta;

        for width in axis.widths.iter_mut() {
            width.cur = ft_mul_fix(width.org, scale);
            width.fit = width.cur;
        }
        axis.extra_light = ft_mul_fix(axis.standard_width, scale) < 40;

        for blue in &mut axis.blues {
            blue.ref_width.cur = ft_mul_fix(blue.ref_width.org, scale) + delta;
            blue.ref_width.fit = blue.ref_width.cur;
            blue.shoot_width.cur = ft_mul_fix(blue.shoot_width.org, scale) + delta;
            blue.shoot_width.fit = blue.shoot_width.cur;
            blue.flags &= !AF_LATIN_BLUE_ACTIVE;

            let dist = ft_mul_fix(blue.ref_width.org - blue.shoot_width.org, scale);
            if (-48..=48).contains(&dist) {
                blue.ref_width.fit = ft_pix_round(blue.ref_width.cur);

                let delta1 = ft_div_fix(blue.ref_width.fit, scale) - blue.shoot_width.org;
                let mut delta2 = ft_mul_fix(delta1.abs(), scale);
                if delta2 < 32 {
                    delta2 = 0;
                } else {
                    delta2 = ft_pix_round(delta2);
                }
                if delta1 < 0 {
                    delta2 = -delta2;
                }

                blue.shoot_width.fit = blue.ref_width.fit - delta2;
                blue.flags |= AF_LATIN_BLUE_ACTIVE;
            }
        }
    }

    (x_scale, y_scale)
}

/// Compute edges for CJK/Indic scripts using best-distance matching.
/// Port of af_cjk_hints_compute_edges (afcjk.c:993-1193).
///
/// Key differences from Latin edge detection:
///   1. Best-distance matching instead of first-match
///   2. Linked segment compatibility check
///   3. Top-to-bottom edge insertion order
///
pub fn cjk_compute_edges(hints: &mut GlyphHints, dim: Dimension, top_to_bottom: bool) {
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz {
        hints.x_scale
    } else {
        hints.y_scale
    };
    axis.edges.clear();

    // afcjk.c:1032-1037 — edge_distance_threshold in font units
    let edge_dist_thresh = {
        let raw = hints
            .metrics
            .as_ref()
            .map_or(50, |m| m.axis[dim as usize].edge_distance_threshold);
        let mut edt = ft_mul_fix(raw, scale);
        if edt > 16 {
            edt = ft_mul_div(16, 0x10000, scale);
        } else {
            edt = raw;
        }
        edt
    };

    // afcjk.c:1040-1120 — create edges from segments
    for seg_idx in 0..axis.segments.len() {
        let seg_pos = axis.segments[seg_idx].pos as i32;
        let seg_dir = axis.segments[seg_idx].dir;
        let seg_link = axis.segments[seg_idx].link;
        let mut best_edge: Option<usize> = None;
        let mut best_dist = i32::MAX;

        // afcjk.c:1050-1085 — find best-matching edge
        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir != seg_dir {
                continue;
            }
            let dist = (edge.fpos as i32 - seg_pos).abs();
            if dist < edge_dist_thresh && dist < best_dist {
                // afcjk.c:1065-1085 — linked segment compatibility
                let link = seg_link;
                if link != usize::MAX {
                    let mut ok = true;
                    let mut s1 = edge.first;
                    loop {
                        let link1 = axis.segments[s1].link;
                        if link1 != usize::MAX {
                            let d2 = (axis.segments[link].pos as i32
                                - axis.segments[link1].pos as i32)
                                .abs();
                            if d2 >= edge_dist_thresh {
                                ok = false;
                                break;
                            }
                        }
                        if s1 == edge.last {
                            break;
                        }
                        s1 = axis.segments[s1].edge_next;
                    }
                    if !ok {
                        continue;
                    }
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
                fpos,
                opos,
                pos: opos,
                dir: seg_dir,
                first: seg_idx,
                last: seg_idx,
                ..AFEdge::default()
            };
            axis.segments[seg_idx].edge_next = seg_idx;
            let mut insert_at = axis.edges.len();
            while insert_at > 0 {
                let prev = &axis.edges[insert_at - 1];
                let is_before = if top_to_bottom {
                    prev.fpos > fpos
                } else {
                    prev.fpos < fpos
                };
                if is_before {
                    break;
                }

                // FreeType's `af_axis_hints_new_edge` keeps same-position
                // major-direction edges after earlier peers; minor-direction
                // peers are inserted before them.  Duplicate CJK strokes rely
                // on this order when segment links are reduced to edge links.
                if prev.fpos == fpos && seg_dir == axis.major_dir {
                    break;
                }

                insert_at -= 1;
            }
            axis.edges.insert(insert_at, new_edge);
        }
    }

    // afcjk.c:1156-1168 — set segment→edge references
    for e_idx in 0..axis.edges.len() {
        let mut s = axis.edges[e_idx].first;
        loop {
            axis.segments[s].edge = e_idx;
            if s == axis.edges[e_idx].last {
                break;
            }
            s = axis.segments[s].edge_next;
        }
    }

    // afcjk.c:1170-1258 — compute edge properties from grouped segments.
    for e_idx in 0..axis.edges.len() {
        let mut is_round = 0i32;
        let mut is_straight = 0i32;
        let first_seg = axis.edges[e_idx].first;
        let mut seg_idx = first_seg;

        loop {
            let seg = axis.segments[seg_idx];
            if seg.flags & AF_EDGE_ROUND != 0 {
                is_round += 1;
            } else {
                is_straight += 1;
            }

            let is_serif = if seg.serif != usize::MAX {
                let serif_edge = axis.segments[seg.serif].edge;
                serif_edge != usize::MAX && serif_edge != e_idx
            } else {
                false
            };

            if (seg.link != usize::MAX && axis.segments[seg.link].edge != usize::MAX) || is_serif {
                let mut edge2_idx = if is_serif {
                    axis.edges[e_idx].serif
                } else {
                    axis.edges[e_idx].link
                };
                let linked_seg = if is_serif { seg.serif } else { seg.link };

                if edge2_idx != usize::MAX {
                    let edge_delta =
                        (axis.edges[e_idx].fpos as i32 - axis.edges[edge2_idx].fpos as i32).abs();
                    let seg_delta = (seg.pos as i32 - axis.segments[linked_seg].pos as i32).abs();
                    if seg_delta < edge_delta {
                        edge2_idx = axis.segments[linked_seg].edge;
                    }
                } else {
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

            if seg_idx == axis.edges[e_idx].last {
                break;
            }
            seg_idx = axis.segments[seg_idx].edge_next;
        }

        axis.edges[e_idx].flags = AF_EDGE_NORMAL;
        if is_round > 0 && is_round >= is_straight {
            axis.edges[e_idx].flags |= AF_EDGE_ROUND;
        }

        if axis.edges[e_idx].serif != usize::MAX && axis.edges[e_idx].link != usize::MAX {
            axis.edges[e_idx].serif = usize::MAX;
        }
    }
}

/// Link CJK segments into stems and serifs.
///
/// This is the CJK-specific counterpart to Latin `link_segments_inner`, ported
/// from `af_cjk_hints_link_segments` in `afcjk.c`. CJK uses direct distance
/// scoring plus a Hanzi serif pass instead of Latin width-demerit scoring.
pub fn cjk_link_segments(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    let n = axis.segments.len();
    let major_dir = axis.major_dir;
    let upem = hints.metrics.as_ref().map_or(2048, |m| m.units_per_em);
    let len_threshold = (8 * upem) / 2048;
    let scale = if dim == Dimension::Horz {
        hints.x_scale
    } else {
        hints.y_scale
    };
    let dist_threshold = ft_div_fix(64 * 3, scale);

    for seg in &mut axis.segments {
        seg.score = 32000;
        seg.link = usize::MAX;
        seg.serif = usize::MAX;
    }

    for i in 0..n {
        if axis.segments[i].dir != major_dir {
            continue;
        }

        let pos1 = axis.segments[i].pos as i32;
        for j in 0..n {
            if i == j {
                continue;
            }
            if axis.segments[i].dir.as_i8() + axis.segments[j].dir.as_i8() != 0 {
                continue;
            }

            let dist = axis.segments[j].pos as i32 - pos1;
            if dist < 0 {
                continue;
            }

            let min = (axis.segments[i].min_coord as i32).max(axis.segments[j].min_coord as i32);
            let max = (axis.segments[i].max_coord as i32).min(axis.segments[j].max_coord as i32);
            let len = max - min;
            if len < len_threshold {
                continue;
            }

            if dist * 8 < axis.segments[i].score * 9
                && (dist * 8 < axis.segments[i].score * 7 || (axis.segments[i].height as i32) < len)
            {
                axis.segments[i].score = dist;
                axis.segments[i].height = i16_from_i32(len);
                axis.segments[i].link = j;
            }

            if dist * 8 < axis.segments[j].score * 9
                && (dist * 8 < axis.segments[j].score * 7 || (axis.segments[j].height as i32) < len)
            {
                axis.segments[j].score = dist;
                axis.segments[j].height = i16_from_i32(len);
                axis.segments[j].link = i;
            }
        }
    }

    for i in 0..n {
        let link1 = axis.segments[i].link;
        if link1 == usize::MAX || axis.segments[link1].link != i {
            continue;
        }
        if axis.segments[link1].pos <= axis.segments[i].pos {
            continue;
        }
        if axis.segments[i].score >= dist_threshold {
            continue;
        }

        for j in 0..n {
            if axis.segments[j].pos > axis.segments[i].pos || i == j {
                continue;
            }

            let link2 = axis.segments[j].link;
            if link2 == usize::MAX || axis.segments[link2].link != j {
                continue;
            }
            if axis.segments[link2].pos < axis.segments[link1].pos {
                continue;
            }
            if axis.segments[i].pos == axis.segments[j].pos
                && axis.segments[link1].pos == axis.segments[link2].pos
            {
                continue;
            }
            if axis.segments[j].score <= axis.segments[i].score
                || axis.segments[i].score * 4 <= axis.segments[j].score
            {
                continue;
            }

            if axis.segments[i].height as i32 >= axis.segments[j].height as i32 * 3 {
                for k in 0..n {
                    let link = axis.segments[k].link;
                    if link == j {
                        axis.segments[k].link = usize::MAX;
                        axis.segments[k].serif = link1;
                    } else if link == link2 {
                        axis.segments[k].link = usize::MAX;
                        axis.segments[k].serif = i;
                    }
                }
            } else {
                axis.segments[i].link = usize::MAX;
                axis.segments[link1].link = usize::MAX;
                break;
            }
        }
    }

    for i in 0..n {
        let seg2 = axis.segments[i].link;
        if seg2 == usize::MAX {
            continue;
        }
        let seg2_link = axis.segments[seg2].link;
        if seg2_link != i {
            axis.segments[i].link = usize::MAX;
            if seg2_link != usize::MAX
                && (axis.segments[seg2].score < dist_threshold
                    || axis.segments[i].score < axis.segments[seg2].score * 4)
            {
                axis.segments[i].serif = seg2_link;
            }
        }
    }
}

/// Assign CJK edges to active blue zones for the current dimension.
///
/// CJK has horizontal and vertical blue zones and no Latin neutral/round
/// overshoot special cases. The nearest active reference/shoot blue wins.
pub fn cjk_compute_blue_edges(hints: &mut GlyphHints, dim: Dimension) {
    let Some(metrics) = hints.metrics.as_ref() else {
        return;
    };
    let axis_metrics = &metrics.axis[dim as usize];
    if axis_metrics.blue_count == 0 {
        return;
    }

    let axis = &mut hints.axis[dim as usize];
    let scale = axis_metrics.scale;
    let major_dir = axis.major_dir;
    let mut best_dist0 = ft_mul_fix(metrics.units_per_em / 40, scale);
    if best_dist0 > 32 {
        best_dist0 = 32;
    }

    for edge in &mut axis.edges {
        let mut best_blue = None;
        let mut best_dist = best_dist0;

        for blue in axis_metrics.blues.iter().take(axis_metrics.blue_count) {
            if blue.flags & AF_LATIN_BLUE_ACTIVE == 0 {
                continue;
            }

            let is_top_right = blue.flags & AF_LATIN_BLUE_TOP != 0;
            let is_major = edge.dir == major_dir;
            if !(is_top_right ^ is_major) {
                continue;
            }

            let compare = if (edge.fpos as i32 - blue.ref_width.org).abs()
                > (edge.fpos as i32 - blue.shoot_width.org).abs()
            {
                blue.shoot_width
            } else {
                blue.ref_width
            };
            let dist = ft_mul_fix((edge.fpos as i32 - compare.org).abs(), scale);
            if dist < best_dist {
                best_dist = dist;
                best_blue = Some(compare);
            }
        }

        if let Some(blue) = best_blue {
            edge.blue_edge = Some(blue);
        }
    }
}

fn ft_pix_floor(x: i32) -> i32 {
    x & !63
}

fn ft_pix_round(x: i32) -> i32 {
    (x + 32) & !63
}

fn cjk_snap_width(widths: &[i32], mut width: i32) -> i32 {
    let mut best = 64 + 32 + 2;
    let mut reference = width;

    for &w in widths {
        let dist = (width - w).abs();
        if dist < best {
            best = dist;
            reference = w;
        }
    }

    let scaled = (reference + 32) & !63;
    if width >= reference {
        if width < scaled + 48 {
            width = reference;
        }
    } else if width > scaled - 48 {
        width = reference;
    }

    width
}

fn compute_stem_width(
    other_flags: u32,
    dim: Dimension,
    width: i32,
    _base_flags: u8,
    _stem_flags: u8,
    std_widths: &[i32],
) -> i32 {
    let stem_adjust = other_flags & AF_LATIN_HINTS_STEM_ADJUST != 0;
    if !stem_adjust {
        return width;
    }

    let mut dist = width;
    let mut sign = false;
    if dist < 0 {
        dist = -width;
        sign = true;
    }

    let vertical = dim == Dimension::Vert;
    let vert_snap = other_flags & AF_LATIN_HINTS_VERT_SNAP != 0;
    let horz_snap = other_flags & AF_LATIN_HINTS_HORZ_SNAP != 0;

    if (vertical && !vert_snap) || (!vertical && !horz_snap) {
        if let Some(&stdw) = std_widths.first() {
            if (dist - stdw).abs() < 40 {
                dist = stdw;
                if dist < 48 {
                    dist = 48;
                }
                return if sign { -dist } else { dist };
            }
        }

        if dist < 54 {
            dist += (54 - dist) / 2;
        } else if dist < 3 * 64 {
            let delta = dist & 63;
            dist &= !63;

            if delta < 10 {
                dist += delta;
            } else if delta < 22 {
                dist += 10;
            } else if delta < 42 {
                dist += delta;
            } else if delta < 54 {
                dist += 54;
            } else {
                dist += delta;
            }
        }
    } else {
        dist = cjk_snap_width(std_widths, dist);

        if vertical {
            if dist >= 64 {
                dist = (dist + 16) & !63;
            } else {
                dist = 64;
            }
        } else if other_flags & AF_LATIN_HINTS_MONO != 0 {
            if dist < 64 {
                dist = 64;
            } else {
                dist = (dist + 32) & !63;
            }
        } else if dist < 48 {
            dist = (dist + 64) >> 1;
        } else if dist < 128 {
            dist = (dist + 22) & !63;
        } else {
            dist = (dist + 32) & !63;
        }
    }

    if sign { -dist } else { dist }
}

fn linked_edge_pos(
    other_flags: u32,
    dim: Dimension,
    base_edge: &AFEdge,
    stem_edge: &AFEdge,
    std_widths: &[i32],
) -> i32 {
    let dist = stem_edge.opos - base_edge.opos;
    let fitted_width = compute_stem_width(
        other_flags,
        dim,
        dist,
        base_edge.flags,
        stem_edge.flags,
        std_widths,
    );
    base_edge.pos + fitted_width
}

fn hint_normal_stem(
    other_flags: u32,
    dim: Dimension,
    edge: &AFEdge,
    edge2: &AFEdge,
    anchor: i32,
    std_widths: &[i32],
) -> (i32, i32, i32) {
    let mut threshold = 64;
    if other_flags & AF_LATIN_HINTS_STEM_ADJUST == 0 {
        if edge.flags & AF_EDGE_ROUND != 0 && edge2.flags & AF_EDGE_ROUND != 0 {
            threshold = if dim == Dimension::Vert {
                64 - 9
            } else {
                64 - 15
            };
        } else {
            threshold = if dim == Dimension::Vert {
                64 - 9 / 3
            } else {
                64 - 15 / 3
            };
        }
    }

    let org_len = edge2.opos - edge.opos;
    let cur_len = compute_stem_width(
        other_flags,
        dim,
        org_len,
        edge.flags,
        edge2.flags,
        std_widths,
    );
    let org_center = (edge.opos + edge2.opos) / 2 + anchor;
    let mut cur_pos1 = org_center - cur_len / 2;
    let cur_pos2 = cur_pos1 + cur_len;
    let mut d_off1 = cur_pos1 - ft_pix_floor(cur_pos1);
    let d_off2 = cur_pos2 - ft_pix_floor(cur_pos2);
    let mut u_off1 = 64 - d_off1;
    let mut u_off2 = 64 - d_off2;
    let mut delta = 0;

    if d_off1 != 0 && d_off2 != 0 {
        if cur_len <= threshold {
            if d_off2 < cur_len {
                delta = if u_off1 <= d_off2 { u_off1 } else { -d_off2 };
            }
        } else {
            let mut apply = true;
            if threshold < 64
                && (d_off1 >= threshold
                    || u_off1 >= threshold
                    || d_off2 >= threshold
                    || u_off2 >= threshold)
            {
                apply = false;
            }
            if apply {
                let mut offset = cur_len & 63;
                if offset < 32 {
                    if u_off1 <= offset || d_off2 <= offset {
                        apply = false;
                    }
                } else {
                    offset = 64 - threshold;
                }

                if apply {
                    d_off1 = threshold - u_off1;
                    u_off1 -= offset;
                    u_off2 = threshold - d_off2;
                    let d_off2 = d_off2 - offset;

                    if d_off1 <= u_off1 {
                        u_off1 = -d_off1;
                    }
                    if d_off2 <= u_off2 {
                        u_off2 = -d_off2;
                    }

                    delta = if u_off1.abs() <= u_off2.abs() {
                        u_off1
                    } else {
                        u_off2
                    };
                }
            }
        }
    }

    if other_flags & AF_LATIN_HINTS_STEM_ADJUST == 0 {
        delta = delta.clamp(-14, 14);
    }

    cur_pos1 += delta;

    if edge.opos < edge2.opos {
        (cur_pos1, cur_pos1 + cur_len, delta)
    } else {
        (cur_pos1 + cur_len, cur_pos1, delta)
    }
}

fn align_serif_edge(base: &AFEdge, serif: &mut AFEdge) {
    serif.pos = base.pos + (serif.opos - base.opos);
}

/// Position CJK edges using `af_cjk_hint_edges` behavior.
pub(super) fn hint_edges(hints: &mut GlyphHints, dim: Dimension, std_widths: &[i32]) {
    // C: afcjk.c:1495-1580,1690-1818,1818-2261 computes CJK stem
    // widths, normal-stem positions, and skipped-edge interpolation.
    let other_flags = hints.other_flags;
    let axis = &mut hints.axis[dim as usize];
    let num_edges = axis.edges.len();
    let mut anchor: Option<usize> = None;
    let mut delta = 0;
    let mut skipped = 0usize;
    let mut has_last_stem = false;
    let mut last_stem_pos = 0;

    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            continue;
        }

        let link = axis.edges[i].link;
        let mut edge1 = None;
        let mut edge2 = link;
        let mut blue = axis.edges[i].blue_edge;
        if blue.is_some() {
            edge1 = Some(i);
        } else if link != usize::MAX {
            blue = axis.edges[link].blue_edge;
            if blue.is_some() {
                edge1 = Some(link);
                edge2 = i;
            }
        }

        let Some(edge1_idx) = edge1 else {
            continue;
        };
        let Some(blue) = blue else {
            continue;
        };

        axis.edges[edge1_idx].pos = blue.fit;
        axis.edges[edge1_idx].flags |= AF_EDGE_DONE;

        if edge2 != usize::MAX && axis.edges[edge2].blue_edge.is_none() {
            let pos = linked_edge_pos(
                other_flags,
                dim,
                &axis.edges[edge1_idx],
                &axis.edges[edge2],
                std_widths,
            );
            axis.edges[edge2].pos = pos;
            axis.edges[edge2].flags |= AF_EDGE_DONE;
        }

        if anchor.is_none() {
            anchor = Some(i);
        }
    }

    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            continue;
        }

        let link = axis.edges[i].link;
        if link == usize::MAX {
            skipped += 1;
            continue;
        }

        if has_last_stem
            && (axis.edges[i].pos < last_stem_pos + 64 || axis.edges[link].pos < last_stem_pos + 64)
        {
            skipped += 1;
            continue;
        }

        if axis.edges[link].blue_edge.is_some() || link < i {
            let pos = linked_edge_pos(
                other_flags,
                dim,
                &axis.edges[link],
                &axis.edges[i],
                std_widths,
            );
            axis.edges[i].pos = pos;
            axis.edges[i].flags |= AF_EDGE_DONE;
            has_last_stem = true;
            last_stem_pos = pos;
            continue;
        }

        let update_delta = dim != Dimension::Vert && anchor.is_none();
        let stem_anchor = if update_delta { 0 } else { delta };
        let (pos1, pos2, new_delta) = hint_normal_stem(
            other_flags,
            dim,
            &axis.edges[i],
            &axis.edges[link],
            stem_anchor,
            std_widths,
        );
        axis.edges[i].pos = pos1;
        axis.edges[link].pos = pos2;
        if update_delta {
            // C `af_cjk_hint_edges` stores the returned delta only for the
            // first non-vertical anchor stem; all other `af_hint_normal_stem`
            // calls discard it (afcjk.c:1904-1947).
            delta = new_delta;
        }
        anchor = Some(i);
        axis.edges[i].flags |= AF_EDGE_DONE;
        axis.edges[link].flags |= AF_EDGE_DONE;
        has_last_stem = true;
        last_stem_pos = axis.edges[link].pos;
    }

    // CJK keeps symmetric repeated stems stable before interpolating skipped
    // edges.  FreeType applies this only to horizontal edge sets shaped like a
    // sans-serif `m` (6 edges) or serif `m` (12 edges); see afcjk.c:2036-2097.
    if dim == Dimension::Horz && (num_edges == 6 || num_edges == 12) {
        let (edge1, edge2, edge3) = if num_edges == 6 {
            (0usize, 2usize, 4usize)
        } else {
            (1usize, 5usize, 9usize)
        };
        let dist1 = axis.edges[edge2].opos - axis.edges[edge1].opos;
        let dist2 = axis.edges[edge3].opos - axis.edges[edge2].opos;

        if (dist1 - dist2).abs() < 8
            && axis.edges[edge1].link == edge1 + 1
            && axis.edges[edge2].link == edge2 + 1
            && axis.edges[edge3].link == edge3 + 1
        {
            let delta = axis.edges[edge3].pos - (2 * axis.edges[edge2].pos - axis.edges[edge1].pos);
            axis.edges[edge3].pos -= delta;
            let edge3_link = axis.edges[edge3].link;
            if edge3_link != usize::MAX {
                axis.edges[edge3_link].pos -= delta;
            }
            if num_edges == 12 {
                axis.edges[8].pos -= delta;
                axis.edges[11].pos -= delta;
            }
            axis.edges[edge3].flags |= AF_EDGE_DONE;
            if edge3_link != usize::MAX {
                axis.edges[edge3_link].flags |= AF_EDGE_DONE;
            }
        }
    }

    if skipped == 0 {
        return;
    }

    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            continue;
        }
        let serif = axis.edges[i].serif;
        if serif != usize::MAX {
            let base = axis.edges[serif];
            align_serif_edge(&base, &mut axis.edges[i]);
            axis.edges[i].flags |= AF_EDGE_DONE;
            skipped = skipped.saturating_sub(1);
        }
    }

    if skipped == 0 {
        return;
    }

    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            continue;
        }

        let before = (0..i)
            .rev()
            .find(|&idx| axis.edges[idx].flags & AF_EDGE_DONE != 0);
        let after = (i + 1..num_edges).find(|&idx| axis.edges[idx].flags & AF_EDGE_DONE != 0);

        match (before, after) {
            (None, Some(after)) => {
                let base = axis.edges[after];
                align_serif_edge(&base, &mut axis.edges[i]);
            }
            (Some(before), None) => {
                let base = axis.edges[before];
                align_serif_edge(&base, &mut axis.edges[i]);
            }
            (Some(before), Some(after)) => {
                if axis.edges[after].fpos == axis.edges[before].fpos {
                    axis.edges[i].pos = axis.edges[before].pos;
                } else {
                    axis.edges[i].pos = axis.edges[before].pos
                        + ft_mul_div(
                            axis.edges[i].fpos as i32 - axis.edges[before].fpos as i32,
                            axis.edges[after].pos - axis.edges[before].pos,
                            axis.edges[after].fpos as i32 - axis.edges[before].fpos as i32,
                        );
                }
            }
            (None, None) => {}
        }
    }
}

/// Move CJK edge points after `af_cjk_hint_edges`.
///
/// C: `af_cjk_align_edge_points` in afcjk.c:2172-2261.  Unlike the Latin
/// `af_glyph_hints_align_edge_points`, unsnapped CJK edges translate their
/// member points by `edge.pos - edge.opos` instead of assigning `edge.pos`
/// directly.
pub(super) fn align_edge_points(hints: &mut GlyphHints, dim: Dimension) {
    let snapping = (dim == Dimension::Horz && hints.other_flags & AF_LATIN_HINTS_HORZ_SNAP != 0)
        || (dim == Dimension::Vert && hints.other_flags & AF_LATIN_HINTS_VERT_SNAP != 0);
    let is_vert = dim == Dimension::Vert;
    let axis = &hints.axis[dim as usize];

    for edge in &axis.edges {
        let pos = edge.pos;
        let delta = edge.pos - edge.opos;
        let mut seg_idx = edge.first;
        loop {
            if seg_idx == usize::MAX {
                break;
            }
            let seg = &axis.segments[seg_idx];
            let mut pt_idx = seg.first;
            loop {
                if is_vert {
                    if snapping {
                        hints.points[pt_idx].y = pos;
                    } else {
                        hints.points[pt_idx].y += delta;
                    }
                    hints.points[pt_idx].flags |= AF_FLAG_TOUCH_Y;
                } else {
                    if snapping {
                        hints.points[pt_idx].x = pos;
                    } else {
                        hints.points[pt_idx].x += delta;
                    }
                    hints.points[pt_idx].flags |= AF_FLAG_TOUCH_X;
                }
                if pt_idx == seg.last {
                    break;
                }
                pt_idx = hints.points[pt_idx].next;
            }
            if seg_idx == edge.last {
                break;
            }
            seg_idx = seg.edge_next;
        }
    }
}
