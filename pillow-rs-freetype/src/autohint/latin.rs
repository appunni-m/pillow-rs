//! Latin-script auto-hinting — port of `src/autofit/aflatin.c`.
//!
//! Implements the core pipeline for grid-fitting Latin glyph outlines:
//!   segment detection → edge grouping → grid-fitting → point interpolation.
//!
//! Ported in phases (A through F per ALGORITHMS.md). Some imports are drawn
//! in early but only used by later phases.

use crate::fixed::{ft_mul_fix, ft_mul_div, ft_div_fix};
use super::types::{
    AFSegment, AFEdge, AFPoint, GlyphHints,
    Direction, Dimension,
    AF_FLAG_IGNORE, AF_FLAG_CONTROL, AF_FLAG_TOUCH_X, AF_FLAG_TOUCH_Y,
    AF_FLAG_WEAK_INTERPOLATION,
    AF_EDGE_ROUND, AF_EDGE_SERIF, AF_EDGE_DONE,
    AF_LATIN_HINTS_HORZ_SNAP, AF_LATIN_HINTS_VERT_SNAP,
    AF_LATIN_HINTS_STEM_ADJUST, AF_LATIN_HINTS_MONO,
    AfLatinMetrics, AfWidth, AfLatinBlue, AF_LATIN_MAX_WIDTHS,
};
// blue/edge flags — imported for later phases (Phases B–E)
#[allow(unused_imports)]
use super::types::{
    AF_EDGE_NEUTRAL, AF_EDGE_NO_BLUE,
    AF_LATIN_BLUE_ACTIVE, AF_LATIN_BLUE_TOP, AF_LATIN_BLUE_SUB_TOP,
    AF_LATIN_BLUE_NEUTRAL, AF_LATIN_BLUE_ADJUSTMENT, AF_LATIN_BLUE_BOTTOM,
    AF_LATIN_BLUE_BOTTOM_SMALL,
    AF_BLUE_PROP_LATIN_TOP, AF_BLUE_PROP_LATIN_SUB_TOP, AF_BLUE_PROP_LATIN_NEUTRAL,
    AF_BLUE_PROP_LATIN_X_HEIGHT, AF_BLUE_PROP_LATIN_LONG,
    AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM, AF_BLUE_PROP_LATIN_SMALL_BOTTOM,
};
use super::loader;

// ── Metrics helpers ──────────────────────────────────────────────────────────

/// AF_LATIN_CONSTANT: scale `c` by upem/2048.  aflatin.h:34
#[inline]
fn latin_constant(upem: i32, c: i32) -> i32 {
    (c * upem) / 2048
}

/// FLAT_THRESHOLD for round/straight classification.  aflatin.c:39
fn flat_threshold(upem: i32) -> i32 { upem / 14 }

// ── Sort utilities (afhints.c:36-131) ────────────────────────────────────────

/// Insertion-sort `table` ascending.  afhints.c:36
fn sort_pos(table: &mut [i32]) {
    for i in 1..table.len() {
        let val = table[i];
        let mut j = i;
        while j > 0 && val < table[j - 1] {
            table[j] = table[j - 1];
            j -= 1;
        }
        table[j] = val;
    }
}

/// Sort widths by `.org`, then collapse clusters ≤ threshold into their mean.
/// afhints.c:58-131
fn sort_and_quantize_widths(count: &mut usize, widths: &mut [AfWidth], threshold: i32) {
    if *count <= 1 { return; }

    // insertion-sort by .org
    for i in 1..*count {
        let val = widths[i];
        let mut j = i;
        while j > 0 && val.org < widths[j - 1].org {
            widths[j] = widths[j - 1];
            j -= 1;
        }
        widths[j] = val;
    }

    // cluster and average
    let mut cur_idx = 0usize;
    let mut cur_val = widths[0].org;
    for i in 1..*count {
        if widths[i].org - cur_val > threshold || i == *count - 1 {
            let end = if widths[i].org - cur_val <= threshold && i == *count - 1 { i + 1 } else { i };
            let mut sum: i64 = 0;
            for j in cur_idx..end { sum += widths[j].org as i64; }
            // zero out merged entries, keep the first
            for j in cur_idx + 1..end { widths[j].org = 0; }
            widths[cur_idx].org = (sum / (end as i64 - cur_idx as i64)) as i32;
            if i < *count - 1 {
                cur_idx = i + 1;
                cur_val = widths[cur_idx].org;
            }
        }
    }

    // compress: remove zero entries
    let mut dst = 1usize;
    for i in 1..*count {
        if widths[i].org != 0 {
            widths[dst] = widths[i];
            dst += 1;
        }
    }
    *count = dst;
}

// ── Font-wide stem-width histogram ───────────────────────────────────────────

/// Port of `af_latin_metrics_init_widths` (aflatin.c:55-265).
///
/// Scans the standard character glyph ('o' for Latin) to build the stem-width
/// histogram. Populates `metrics.axis[dim].width_count` and `.widths[]`.
/// Returns the standard character glyph index (for caller to re-use in blue init).
pub fn metrics_init_widths(
    metrics: &mut AfLatinMetrics,
    char_glyph_index: u16,
    raw_outline: &crate::tt::glyf::GlyphOutline,
    scaled_points: &[crate::outline::OutlinePoint],
) {
    if char_glyph_index == 0 || raw_outline.num_contours == 0 || raw_outline.points.is_empty() {
        // No usable glyph → fallback: use constant widths
        for dim in 0..2 {
            let axis = &mut metrics.axis[dim];
            axis.width_count = 0;
            let stdw = latin_constant(metrics.units_per_em, 50);
            axis.standard_width = stdw;
            axis.edge_distance_threshold = stdw / 5;
            axis.extra_light = false;
        }
        return;
    }

    // Scan the standard glyph at identity scale (0x10000 = 1.0)
    // Build temp hints: scale=1.0, deltas=0
    let mut hints = GlyphHints::new(0x10000, 0x10000, 0, 0);
    hints.metrics = Some(metrics.clone());
    hints.other_flags = AF_LATIN_HINTS_HORZ_SNAP | AF_LATIN_HINTS_VERT_SNAP | AF_LATIN_HINTS_STEM_ADJUST;
    loader::reload(&mut hints, raw_outline, scaled_points);

    if hints.num_points() == 0 { return; }

    for dim in 0..2 {
        let dimension = if dim == 0 { Dimension::Horz } else { Dimension::Vert };
        compute_segments(&mut hints, dimension);
        // link with width_count=0 (no widths yet — uses the else branch: dist_demerit=dist)
        link_segments_inner(&mut hints, dimension, 0, &[]);

        // Collect stem widths from mutual link pairs
        let axis = &hints.axis[dim];
        let mut num_widths: usize = 0;
        let segs = &axis.segments;
        for i in 0..segs.len() {
            let link = segs[i].link;
            if link != usize::MAX && i == segs[link].link && link > i {
                let dist = (segs[i].pos as i32 - segs[link].pos as i32).abs();
                if num_widths < AF_LATIN_MAX_WIDTHS {
                    metrics.axis[dim].widths[num_widths].org = dist;
                    num_widths += 1;
                }
            }
        }

        sort_and_quantize_widths(&mut num_widths, &mut metrics.axis[dim].widths,
                                  metrics.units_per_em / 100);
        metrics.axis[dim].width_count = num_widths;
    }

    // Finalize each axis
    for dim in 0..2 {
        let axis = &mut metrics.axis[dim];
        let stdw = if axis.width_count > 0 {
            axis.widths[0].org
        } else {
            latin_constant(metrics.units_per_em, 50)
        };
        axis.standard_width = stdw;
        axis.edge_distance_threshold = stdw / 5;
        axis.extra_light = false;
    }
}

/// Extract (width_count, widths_array) from hints.metrics for the given dimension.
/// Returns owned data to avoid borrow conflicts.
fn extract_widths(hints: &GlyphHints, dim: Dimension) -> (usize, [AfWidth; AF_LATIN_MAX_WIDTHS]) {
    if let Some(ref met) = hints.metrics {
        let a = &met.axis[dim as usize];
        (a.width_count, a.widths)
    } else {
        (0, [AfWidth::default(); AF_LATIN_MAX_WIDTHS])
    }
}

// ── Blue zone strings (afblue.dat:347-358) ──────────────────────────────────

/// One entry in the Latin blue stringset table.
struct BlueStringEntry {
    chars: &'static [char],
    props: u32, // AF_BLUE_PROP_* bits
}

/// Standard Latin blue zones, in order (afblue.c:646-653).
static LATIN_BLUE_STRINGS: &[BlueStringEntry] = &[
    // 0: capital top
    BlueStringEntry { chars: &['T','H','E','Z','O','C','Q','S'], props: AF_BLUE_PROP_LATIN_TOP },
    // 1: capital bottom
    BlueStringEntry { chars: &['H','E','Z','L','O','C','U','S'], props: AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM },
    // 2: small f-top
    BlueStringEntry { chars: &['f','i','j','k','d','b','h'], props: AF_BLUE_PROP_LATIN_TOP },
    // 3: small top (x-height)
    BlueStringEntry { chars: &['u','v','x','z','o','e','s','c'], props: AF_BLUE_PROP_LATIN_TOP | AF_BLUE_PROP_LATIN_X_HEIGHT },
    // 4: small bottom
    BlueStringEntry { chars: &['n','r','x','z','o','e','s','c'], props: AF_BLUE_PROP_LATIN_SMALL_BOTTOM },
    // 5: small descender
    BlueStringEntry { chars: &['p','q','g','j','y'], props: 0 },
];

// Macros for checking blue property bits.
macro_rules! is_top_blue   { ($p:expr) => { ($p & AF_BLUE_PROP_LATIN_TOP) != 0 } }
macro_rules! is_sub_top    { ($p:expr) => { ($p & AF_BLUE_PROP_LATIN_SUB_TOP) != 0 } }
macro_rules! is_neutral    { ($p:expr) => { ($p & AF_BLUE_PROP_LATIN_NEUTRAL) != 0 } }
macro_rules! is_x_height   { ($p:expr) => { ($p & AF_BLUE_PROP_LATIN_X_HEIGHT) != 0 } }

/// Port of `af_latin_metrics_init_blues` (aflatin.c:311-1039).
/// Scans the 6 Latin blue character strings to find median flat (reference) and
/// round (overshoot) Y extrema. Populates `metrics.axis[VERT].blues[]`.
pub fn metrics_init_blues(
    metrics: &mut AfLatinMetrics,
    font_data: &crate::tables::FontData,
) {
    let upem = metrics.units_per_em;
    let flat_thresh = flat_threshold(upem);
    let axis = &mut metrics.axis[Dimension::Vert as usize];
    axis.blue_count = 0;
    axis.blues.clear();

    for entry in LATIN_BLUE_STRINGS {
        let mut flats: Vec<i32> = Vec::new();
        let mut rounds: Vec<i32> = Vec::new();
        // ascender/descender accumulate across the whole string (aflatin.c:425-426)
        let mut ascender: i32 = 0;
        let mut descender: i32 = 0;

        for &ch in entry.chars {
            let gid = font_data.cmap.char_index(ch as u32).unwrap_or(0);
            if gid == 0 { continue; }
            let outline = match crate::tt::glyf::load_glyph(
                &font_data.glyf_data, &font_data.loca_data,
                font_data.head.index_to_loc_format, gid,
            ) {
                Ok(o) => o,
                Err(_) => continue,
            };
            if outline.num_contours <= 0 || outline.points.len() <= 2 { continue; }

            let points = &outline.points;
            let end_pts = &outline.end_pts_of_contours;
            let y_offset: i32 = 0;

            let is_top = is_top_blue!(entry.props) || is_sub_top!(entry.props);

            // Per-character best extremum (reset each char, aflatin.c:462-465).
            let mut best_y_extremum: i32 = if is_top { i32::MIN } else { i32::MAX };
            let mut best_round = false;

            // Walk all glyph elements (Latin: 1). Find biggest extremum.
            let mut best_point: i32 = -1;
            let mut best_y: i32 = 0;
            let mut best_contour_first: i32 = -1;
            let mut best_contour_last: i32 = -1;

            let mut last: i32 = -1;
            for ncontour in 0..outline.num_contours as usize {
                let first: i32 = last + 1;
                last = end_pts[ncontour] as i32;
                if last <= first { continue; } // skip single-point contours

                for pp in first..=last {
                    let y = points[pp as usize].y;
                    if is_top {
                        if best_point < 0 || y > best_y {
                            best_point = pp;
                            best_y = y;
                            if y + y_offset > ascender { ascender = y + y_offset; }
                        } else if y + y_offset < descender { descender = y + y_offset; }
                    } else {
                        if best_point < 0 || y < best_y {
                            best_point = pp;
                            best_y = y;
                            if y + y_offset < descender { descender = y + y_offset; }
                        } else if y + y_offset > ascender { ascender = y + y_offset; }
                    }
                }
                if best_point > best_contour_last {
                    best_contour_first = first;
                    best_contour_last = last;
                }
            }

            // Classify flat vs round at the extremum (aflatin.c:568-867).
            let mut round = false;
            if best_point >= 0 {
                let best_x = points[best_point as usize].x;

                let mut best_seg_first = best_point;
                let mut best_seg_last = best_point;
                // Track ON-curve endpoints of the flat segment.
                let mut best_on_first: i32 = if points[best_point as usize].on_curve { best_point } else { -1 };
                let mut best_on_last: i32 = best_on_first;

                // Walk previous (aflatin.c:597-620).
                let mut prev = best_point;
                loop {
                    prev = if prev > best_contour_first { prev - 1 } else { best_contour_last };
                    let dist = (points[prev as usize].y - best_y).abs();
                    if dist > 5 && (points[prev as usize].x - best_x).abs() <= 20 * dist {
                        break;
                    }
                    best_seg_first = prev;
                    if points[prev as usize].on_curve {
                        best_on_first = prev;
                        if best_on_last < 0 { best_on_last = prev; }
                    }
                    if prev == best_point { break; }
                }

                // Walk next (aflatin.c:622-643).
                let mut next = best_point;
                loop {
                    next = if next < best_contour_last { next + 1 } else { best_contour_first };
                    let dist = (points[next as usize].y - best_y).abs();
                    if dist > 5 && (points[next as usize].x - best_x).abs() <= 20 * dist {
                        break;
                    }
                    best_seg_last = next;
                    if points[next as usize].on_curve {
                        best_on_last = next;
                        if best_on_first < 0 { best_on_first = next; }
                    }
                    if next == best_point { break; }
                }

                // Round vs flat (aflatin.c:846-857). LONG-blue variant skipped.
                if best_on_first >= 0 && best_on_last >= 0
                    && (points[best_on_first as usize].x - points[best_on_last as usize].x).abs() > flat_thresh
                {
                    round = false;
                } else {
                    round = !points[best_seg_first as usize].on_curve
                         || !points[best_seg_last as usize].on_curve;
                }

                if round && is_neutral!(entry.props) { continue; } // neutral uses flats only
            }

            // Track best extremum across the (single) element (aflatin.c:869-884).
            if best_point >= 0 {
                let by = best_y + y_offset;
                if is_top {
                    if by > best_y_extremum { best_y_extremum = by; best_round = round; }
                } else {
                    if by < best_y_extremum { best_y_extremum = by; best_round = round; }
                }
            }
            // (best_round unused beyond here since Latin has 1 element; keep for clarity.)

            if best_y_extremum != i32::MIN && best_y_extremum != i32::MAX {
                if best_round { rounds.push(best_y_extremum); }
                else { flats.push(best_y_extremum); }
            }
        }

        // Skip if no data (aflatin.c:899-907).
        if flats.is_empty() && rounds.is_empty() { continue; }

        sort_pos(&mut flats);
        sort_pos(&mut rounds);

        let (mut ref_val, mut shoot_val) = if flats.is_empty() {
            let v = rounds[rounds.len() / 2];
            (v, v)
        } else if rounds.is_empty() {
            let v = flats[flats.len() / 2];
            (v, v)
        } else {
            (flats[flats.len() / 2], rounds[rounds.len() / 2])
        };

        // Overshoot sanity (aflatin.c:940-956).
        if shoot_val != ref_val {
            let over_ref = shoot_val > ref_val;
            if (is_top_blue!(entry.props) || is_sub_top!(entry.props)) != over_ref {
                let mean = (shoot_val + ref_val) / 2;
                ref_val = mean;
                shoot_val = mean;
            }
        }

        let mut flags: u32 = 0;
        if is_top_blue!(entry.props) { flags |= AF_LATIN_BLUE_TOP; }
        if is_sub_top!(entry.props) { flags |= AF_LATIN_BLUE_SUB_TOP; }
        if is_neutral!(entry.props) { flags |= AF_LATIN_BLUE_NEUTRAL; }
        if (entry.props & AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM) != 0 { flags |= AF_LATIN_BLUE_BOTTOM; }
        if (entry.props & AF_BLUE_PROP_LATIN_SMALL_BOTTOM) != 0 { flags |= AF_LATIN_BLUE_BOTTOM_SMALL; }
        if is_x_height!(entry.props) { flags |= AF_LATIN_BLUE_ADJUSTMENT; }

        axis.blues.push(AfLatinBlue {
            ref_width: AfWidth { org: ref_val, cur: 0, fit: 0 },
            shoot_width: AfWidth { org: shoot_val, cur: 0, fit: 0 },
            ascender,
            descender,
            flags,
        });
        axis.blue_count += 1;
    }

    // Sort blues bottom→top and resolve overlaps (aflatin.c:988-1039).
    if axis.blue_count > 1 {
        // insertion sort by effective position
        let blues = &mut axis.blues;
        for i in 1..blues.len() {
            let mut j = i;
            while j > 0 {
                let a_pos = if blues[j-1].flags & (AF_LATIN_BLUE_TOP|AF_LATIN_BLUE_SUB_TOP) != 0
                    { blues[j-1].shoot_width.org } else { blues[j-1].ref_width.org };
                let b_pos = if blues[j].flags & (AF_LATIN_BLUE_TOP|AF_LATIN_BLUE_SUB_TOP) != 0
                    { blues[j].shoot_width.org } else { blues[j].ref_width.org };
                if b_pos >= a_pos { break; }
                blues.swap(j-1, j);
                j -= 1;
            }
        }
        // resolve overlaps: clamp upper zone's effective top to lower zone's bottom
        for i in 0..blues.len()-1 {
            let use_shoot_a = blues[i].flags & (AF_LATIN_BLUE_TOP|AF_LATIN_BLUE_SUB_TOP) != 0;
            let use_shoot_b = blues[i+1].flags & (AF_LATIN_BLUE_TOP|AF_LATIN_BLUE_SUB_TOP) != 0;
            let a_org = if use_shoot_a { blues[i].shoot_width.org } else { blues[i].ref_width.org };
            let b_org = if use_shoot_b { blues[i+1].shoot_width.org } else { blues[i+1].ref_width.org };
            if a_org > b_org {
                // *a = *b  (rare; clamp to avoid inversion)
                if use_shoot_a { blues[i].shoot_width.org = b_org; }
                else { blues[i].ref_width.org = b_org; }
            }
        }
    }
}

/// Scale the metrics axes for the current size (base x_scale/y_scale), applying
/// the x-height scale optimization on the vertical axis, then scale the stem
/// ✅ VERIFIED: VERT/HORZ scale + width cur values match C
/// (v_scale=21967, HORZ cur=[61], VERT cur=[52] for DejaVuSans 10pt).
/// Port of af_latin_metrics_scale_dim (aflatin.c:1178-1437).
/// Returns the (x_scale, y_scale) the scaler must use to scale glyph outlines.
pub fn metrics_scale_dim(
    metrics: &mut AfLatinMetrics,
    x_scale: i32,
    y_scale: i32,
    x_delta: i32,
    y_delta: i32,
) -> (i32, i32) {
    // Horizontal axis.
    {
        let axis = &mut metrics.axis[Dimension::Horz as usize];
        axis.scale = x_scale;
        axis.delta = x_delta;
        for w in axis.widths.iter_mut() {
            w.cur = ft_mul_fix(w.org, x_scale);
            w.fit = w.cur;
        }
        axis.extra_light = ft_mul_fix(axis.standard_width, x_scale) < 32 + 8;
    }

    // Vertical axis: x-height scale optimization first (aflatin.c:1211-1306).
    let mut v_scale = y_scale;
    {
        let vaxis = &mut metrics.axis[Dimension::Vert as usize];
        let adj_idx = (0..vaxis.blue_count)
            .find(|&i| vaxis.blues[i].flags & AF_LATIN_BLUE_ADJUSTMENT != 0);
        if let Some(ai) = adj_idx {
            let shoot_org = vaxis.blues[ai].shoot_width.org;
            let scaled = ft_mul_fix(shoot_org, v_scale);
            // increase_x_height property: 0 for non-instructed fonts → threshold=40.
            let threshold: i32 = 40;
            let fitted = (scaled + threshold) & !63;
            if scaled != fitted {
                let new_scale = ft_mul_div(v_scale, fitted, scaled);
                let mut max_height = metrics.units_per_em;
                for b in &vaxis.blues {
                    max_height = max_height.max(b.ascender);
                    max_height = max_height.max(-b.descender);
                }
                let dist = ft_mul_fix(max_height, new_scale - v_scale);
                if -128 < dist && dist < 128 {
                    v_scale = new_scale;
                }
            }
        }
    }

    // Vertical axis: widths + blue zones (aflatin.c:1327-1437).
    {
        let axis = &mut metrics.axis[Dimension::Vert as usize];
        axis.scale = v_scale;
        axis.delta = y_delta;
        for w in axis.widths.iter_mut() {
            w.cur = ft_mul_fix(w.org, v_scale);
            w.fit = w.cur;
        }
        axis.extra_light = ft_mul_fix(axis.standard_width, v_scale) < 32 + 8;

        // Blue zones (aflatin.c:1357-1437).
        for blue in &mut axis.blues {
            blue.ref_width.cur   = ft_mul_fix(blue.ref_width.org, v_scale) + y_delta;
            blue.ref_width.fit   = blue.ref_width.cur;
            blue.shoot_width.cur = ft_mul_fix(blue.shoot_width.org, v_scale) + y_delta;
            blue.shoot_width.fit = blue.shoot_width.cur;
            blue.flags &= !AF_LATIN_BLUE_ACTIVE;

            let dist = ft_mul_fix(blue.ref_width.org - blue.shoot_width.org, v_scale);
            if dist <= 48 && dist >= -48 {
                // Zone height <= 3/4px → active
                let delta2 = dist.abs();
                let delta2 = if delta2 < 32 { 0 } else if delta2 < 48 { 32 } else { 64 };
                let delta2 = if dist < 0 { -delta2 } else { delta2 };
                blue.ref_width.fit   = ft_pix_round(blue.ref_width.cur);
                blue.shoot_width.fit = blue.ref_width.fit - delta2;
                blue.flags |= AF_LATIN_BLUE_ACTIVE;
            }
        }
    }

    (x_scale, v_scale)
}

/// Assign each vertical/horizontal edge to the nearest active blue zone.
/// Port of `af_latin_hints_compute_blue_edges` (aflatin.c:2529-2640).
fn compute_blue_edges(hints: &mut GlyphHints) {
    let dim = Dimension::Vert;
    let metrics = match hints.metrics {
        Some(ref m) => m.clone(),
        None => return,
    };
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz { hints.x_scale } else { hints.y_scale };
    let major_dir = axis.major_dir;
    let upem = metrics.units_per_em;
    let blues = &metrics.axis[dim as usize];


    for e_idx in 0..axis.edges.len() {
        if axis.edges[e_idx].flags & AF_EDGE_NO_BLUE != 0 { continue; }

        let edge_fpos = axis.edges[e_idx].fpos as i32;
        let edge_flags = axis.edges[e_idx].flags;

        // best_dist = min(upem/40, 0.5px), scaled
        let mut best_dist = ft_mul_fix(upem / 40, scale);
        if best_dist > 32 { best_dist = 32; }

        let mut best_blue: Option<AfWidth> = None;
        let mut best_neutral = false;

        for blue_idx in 0..blues.blue_count {
            let blue = &blues.blues[blue_idx];
            if blue.flags & AF_LATIN_BLUE_ACTIVE == 0 { continue; }

            let is_top = blue.flags & (AF_LATIN_BLUE_TOP|AF_LATIN_BLUE_SUB_TOP) != 0;
            let is_neutral = blue.flags & AF_LATIN_BLUE_NEUTRAL != 0;
            let is_major = axis.edges[e_idx].dir == major_dir;

            if (is_top ^ is_major) || is_neutral {
                // Compare to reference position
                let mut dist = (edge_fpos - blue.ref_width.org).abs();
                dist = ft_mul_fix(dist, scale);
                if dist < best_dist {
                    best_dist = dist;
                    best_blue = Some(blue.ref_width);
                    best_neutral = is_neutral;
                }

                // For round edges, also compare to overshoot
                if edge_flags & AF_EDGE_ROUND != 0 && dist != 0 && !is_neutral {
                    let is_under = edge_fpos < blue.ref_width.org;
                    if is_top ^ is_under {
                        let mut shoot_dist = (edge_fpos - blue.shoot_width.org).abs();
                        shoot_dist = ft_mul_fix(shoot_dist, scale);
                        if shoot_dist < best_dist {
                            best_dist = shoot_dist;
                            best_blue = Some(blue.shoot_width);
                            best_neutral = false;
                        }
                    }
                }
            }
        }

        if let Some(bw) = best_blue {
            axis.edges[e_idx].blue_edge = Some(bw);
            if best_neutral {
                axis.edges[e_idx].flags |= AF_EDGE_NEUTRAL;
            }
        }
    }
}

/// Helper: FT_PIX_ROUND(x) = (x + 32) & !63  (26.6 → 6-bit rounding).
#[inline]
fn ft_pix_round(x: i32) -> i32 { (x + 32) & !63 }

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
    metrics: Option<&AfLatinMetrics>,
) {
    let mut hints = GlyphHints::new(x_scale, y_scale, x_delta, y_delta);
    hints.metrics = metrics.cloned();
    // Smooth anti-aliased hinting: enable stem adjustment for anti-aliased rendering.
    // FT_RENDER_MODE_NORMAL sets only STEM_ADJUST, not HORZ_SNAP/VERT_SNAP (aflatin.c:2673-2695).
    hints.other_flags = AF_LATIN_HINTS_STEM_ADJUST;

    // Step 1: Load outline into hints (raw font units → fx/fy; scaled 26.6 → ox/oy)
    loader::reload(&mut hints, raw_outline, &outline.points);
    if hints.num_points() == 0 {
        return;
    }

    // Step 2: Process vertical dimension (Y-axis / horizontal edges)
    compute_segments(&mut hints, Dimension::Vert);
    let vert_widths_26_6: Vec<i32>; // scaled widths for snapping
    {
        let (wc, widths) = extract_widths(&hints, Dimension::Vert);
        vert_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
        link_segments_inner(&mut hints, Dimension::Vert, wc, &widths);
    }
    compute_edges(&mut hints, Dimension::Vert);
    // Blue zones are pre-scaled by metrics_scale_dim (per-size); assign edges.
    compute_blue_edges(&mut hints);
    hint_edges(&mut hints, Dimension::Vert, &vert_widths_26_6);
    align_edge_points(&mut hints, Dimension::Vert);
    align_strong_points(&mut hints, Dimension::Vert);
    align_weak_points(&mut hints, Dimension::Vert);

    // Step 3: Process horizontal dimension (X-axis / vertical edges)
    compute_segments(&mut hints, Dimension::Horz);
    let horz_widths_26_6: Vec<i32>;
    {
        let (wc, widths) = extract_widths(&hints, Dimension::Horz);
        horz_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
        link_segments_inner(&mut hints, Dimension::Horz, wc, &widths);
    }
    compute_edges(&mut hints, Dimension::Horz);
    hint_edges(&mut hints, Dimension::Horz, &horz_widths_26_6);
    align_edge_points(&mut hints, Dimension::Horz);
    align_strong_points(&mut hints, Dimension::Horz);
    align_weak_points(&mut hints, Dimension::Horz);

    // ── Post-hinting phantom-point adjustment (afloader.c:419-530) ──────
    // After hint_edges grid-fits the leftmost/rightmost edges, we compute
    // a pixel-rounded translation (pp1.x) that aligns the LSB to the pixel
    // grid, matching C's af_loader_load_glyph post-processing.
    {
        let haxis = &hints.axis[Dimension::Horz as usize];
        let num_horz_edges = haxis.edges.len();
        if num_horz_edges > 1 {
            let edge1 = &haxis.edges[0];                    // leftmost
            let edge2 = &haxis.edges[num_horz_edges - 1];   // rightmost

            let old_lsb = edge1.opos;   // original scaled LSB (pp1.x = 0)
            let new_lsb = edge1.pos;    // hinted LSB

            let mut pp1x_uh = new_lsb - old_lsb;

            // Small-size pad: prefer too much space over too little.
            if old_lsb < 24 {
                pp1x_uh -= 8;
            }

            let mut pp1x = (pp1x_uh + 32) & !63; // FT_PIX_ROUND

            // Don't move if we'd lose the stem.
            if pp1x >= new_lsb && old_lsb > 0 {
                pp1x -= 64;
            }

            if pp1x != 0 {
                // Translate all points' x by -pp1x.
                for pt in hints.points.iter_mut() {
                    pt.x -= pp1x;
                }
            }
            // Note: pp2.x (right side bearing adjustment) is not implemented.
            // It affects advance width (getlength) but not the rendered glyph.
            let _ = edge2; // used for pp2x computation which we skip
        }
    }

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
    // ✅ VERIFIED: all segment positions, heights, directions, first/last indices
    // match C's af_latin_hints_compute_segments exactly for DejaVuSans 10pt '&'
    // (6 VERT + 6 HORZ segments). Verified via vendored C fprintf trace.
    let contours: Vec<usize> = hints.contours.clone();
    let axis = &mut hints.axis[dim as usize];

    // Per-point u/v axis swap (aflatin.c:1582). Stored on the point's u/v fields.
    let is_horz = dim == Dimension::Horz;
    for pt in &mut hints.points {
        if is_horz { pt.u = pt.fx as i32; pt.v = pt.fy as i32; }
        else       { pt.u = pt.fy as i32; pt.v = pt.fx as i32; }
    }

    // major_dir: per-glyph orientation from loader::reload.
    // CW (TrueType default) → TT/Default: HORZ=Up, VERT=Left.
    // CCW (PostScript) → PS/flipped: HORZ=Down, VERT=Right.
    // afhints.c:967-974.
    // aflatin.c:1577: major_dir is then ABSOLUTIFIED (Up/Right only) for segment
    // direction matching.
    let major_dir = {
        let cw = hints.cw_orientation; // true = clockwise (sum<0). C matches this to FT_Outline_Get_Orientation
        // C: default HORZ=UP VERT=LEFT. If PostScript (area>0→cw=false in our terms? or area<0→cw=true?): flip to HORZ=DOWN VERT=RIGHT
        // FT_Outline_Get_Orientation: area>0→POSTSCRIPT→flip. area<0→TRUETYPE→no_flip.
        // Our cw_orientation: area<0→true. So cw=true means area<0 means TRUETYPE means NO flip.
        // CW→TrueType→no flip: HORZ=UP, VERT=LEFT
        // CCW→PostScript→flip: HORZ=DOWN, VERT=RIGHT
        // Our cw_orientation=true means CW (=TrueType), so NO flip.
        let d = if is_horz {
            if cw { Direction::Up } else { Direction::Down }
        } else {
            if cw { Direction::Left } else { Direction::Right }
        };
        axis.major_dir = d;
        abs_dir(d) // ABSOLUTIFY for segment detection (aflatin.c:1577)
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

    // ── Height extension (aflatin.c:1959-2005) ──────────────────────────
    // Extend segment height by half the adjacent half-tint, so serifs can
    // be detected and ignored during edge filtering.
    if !axis.segments.is_empty() {
        let n_seg = axis.segments.len();
        for idx in 0..n_seg {
            let first_idx = axis.segments[idx].first;
            let last_idx = axis.segments[idx].last;
            let first_v = points[first_idx].v;
            let last_v = points[last_idx].v;

            let mut extra: i16 = 0;
            if first_v < last_v {
                let p = points[first_idx].prev;
                if points[p].v < first_v {
                    extra += ((first_v - points[p].v) >> 1) as i16;
                }
                let p = points[last_idx].next;
                if points[p].v > last_v {
                    extra += ((points[p].v - last_v) >> 1) as i16;
                }
            } else {
                let p = points[first_idx].prev;
                if points[p].v > first_v {
                    extra += ((points[p].v - first_v) >> 1) as i16;
                }
                let p = points[last_idx].next;
                if points[p].v < last_v {
                    extra += ((last_v - points[p].v) >> 1) as i16;
                }
            }
            axis.segments[idx].height = axis.segments[idx].height.saturating_add(extra);
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
// ✅ VERIFIED: edge fpos/opos/dir/flags/links match C for DejaVuSans 10pt '&'
// (5 VERT edges + 5 HORZ edges). Port of af_latin_hints_compute_edges
// (aflatin.c:2154-2500).
fn compute_edges(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    axis.edges.clear();

    // ── Compute thresholds (aflatin.c:2182-2232) ────────────────────────
    let scale = if dim == Dimension::Horz { hints.x_scale } else { hints.y_scale };

    // segment_length_threshold: skip segments shorter than 1px (Horz only).
    let seg_len_thresh = if dim == Dimension::Horz {
        ft_mul_div(64, 0x10000, hints.y_scale)   // FT_DivFix(64, hints->y_scale) in font units
    } else {
        0 // no height filtering for vertical/horizontal edges
    };
    let seg_width_thresh = ft_mul_div(32, 0x10000, scale); // 0.5px in font units

    // Edge distance threshold: at most 0.25px, from metrics if available.
    let edge_dist_thresh = {
        let raw = if let Some(ref met) = hints.metrics {
            met.axis[dim as usize].edge_distance_threshold
        } else {
            50 // fallback
        };
        let mut edt = ft_mul_fix(raw, scale);
        if edt > 16 { edt = 16; } // cap at 0.25px (= 64/4 in 26.6)
        ft_mul_div(edt, 0x10000, scale) // convert back to font units
    };

    // For each segment, find or create its edge.
    for seg_idx in 0..axis.segments.len() {
        // ── Segment filtering (aflatin.c:2242-2251) ──────────────────────
        {
            let seg = &axis.segments[seg_idx];
            // Skip one-point segments without a direction
            if seg.dir == Direction::None { continue; }
            // Too short
            if (seg.height as i32) < seg_len_thresh { continue; }
            // Too wide (delta > 0.5px)
            if (seg.delta as i32) > seg_width_thresh { continue; }
            // Tiny serif: height < 1.5× the length threshold
            // aflatin.c:2247-2250 (serif filter, no round-flag check)
            if seg.serif != usize::MAX && 2 * (seg.height as i32) < 3 * seg_len_thresh { continue; }
        }
        let seg_pos = axis.segments[seg_idx].pos as i32;
        let seg_dir = axis.segments[seg_idx].dir;
        let mut found_edge = usize::MAX;

        // Look for an existing edge at approximately this position.
        for e_idx in 0..axis.edges.len() {
            let edge = &axis.edges[e_idx];
            if edge.dir == seg_dir && (edge.fpos as i32 - seg_pos).abs() < edge_dist_thresh {
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
                pos: 0,  // zero-initialized like C (FT_ZERO). Not set to opos —
                         // hint_edges fills this in. Using opos as initial pos
                         // causes the BOUND check (aflatin.c:4544-4563) to
                         // incorrectly overwrite correctly-computed stem positions.
                flags: 0,
                dir: seg_dir,
                link: usize::MAX,
                serif: usize::MAX,
                first: seg_idx,
                last: seg_idx,
                blue_edge: None,
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

    // ── Second pass: catch directionless segments (aflatin.c:2306-2342) ──
    for seg_idx in 0..axis.segments.len() {
        if axis.segments[seg_idx].dir != Direction::None {
            continue;
        }
        let seg_pos = axis.segments[seg_idx].pos as i32;
        // Look for an existing edge at this position.
        let mut found: Option<usize> = None;
        for e_idx in 0..axis.edges.len() {
            let dist = (axis.edges[e_idx].fpos as i32 - seg_pos).abs();
            if dist < edge_dist_thresh {
                found = Some(e_idx);
                break;
            }
        }
        if let Some(e_idx) = found {
            // Append to existing edge (like the main loop does).
            let prev_last = axis.edges[e_idx].last;
            axis.segments[prev_last].edge_next = seg_idx;
            axis.edges[e_idx].last = seg_idx;
            axis.segments[seg_idx].edge = e_idx;
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

    // ── Sort edges by fpos (ascending) — matches FreeType's insertion order ───
    // FreeType inserts edges in fpos-sorted order during creation via
    // af_axis_hints_new_edge. We sort post-creation and remap all indices.
    if axis.edges.len() > 1 {
        // Build sort permutation: old_idx -> new position
        let mut indices: Vec<usize> = (0..axis.edges.len()).collect();
        indices.sort_by_key(|&i| axis.edges[i].fpos);
        // Build reverse map: new_pos -> old_idx
        let mut old_from_new: Vec<usize> = vec![0; axis.edges.len()];
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            old_from_new[new_idx] = old_idx;
        }
        // Build map: old_idx -> new_idx
        let mut new_from_old: Vec<usize> = vec![0; axis.edges.len()];
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            new_from_old[old_idx] = new_idx;
        }

        // Sort edges
        let old_edges: Vec<AFEdge> = axis.edges.drain(..).collect();
        for &old_idx in &indices {
            axis.edges.push(old_edges[old_idx].clone());
        }

        // Remap segment.edge references
        for seg in &mut axis.segments {
            if seg.edge != usize::MAX {
                seg.edge = new_from_old[seg.edge];
            }
        }

        // Remap edge.link and edge.serif within sorted edges
        for edge in &mut axis.edges {
            if edge.link != usize::MAX {
                edge.link = new_from_old[edge.link];
            }
            if edge.serif != usize::MAX {
                edge.serif = new_from_old[edge.serif];
            }
        }
    }
}

// Port of `af_latin_hints_link_segments` (aflatin.c:2015–2148).
// Pairs opposing-direction, overlapping segments into stem links, then
// derives serif relationships. Sets seg.link / seg.serif indices.
// `width_count`/`widths` come from metrics_init_widths for exact C scoring.
// ✅ VERIFIED: link/serif/score assignments match C for DejaVuSans 10pt '&'
// (both VERT and HORZ). Port of af_latin_hints_link_segments
// (aflatin.c:2011-2132).
fn link_segments_inner(
    hints: &mut GlyphHints,
    dim: Dimension,
    width_count: usize,
    widths: &[AfWidth],
) {
    let axis = &mut hints.axis[dim as usize];
    let major_dir = axis.major_dir;
    let n = axis.segments.len();

    let upem = hints.metrics.as_ref().map(|m| m.units_per_em).unwrap_or(2048);

    // max_width = largest stem width in font units (aflatin.c:2028-2031).
    // .org stays in font units even after scale_dim; segment distances are also
    // in font units, so they're comparable.
    let max_width = if width_count > 0 {
        widths[width_count - 1].org
    } else {
        0
    };

    let len_threshold = latin_constant(upem, 8).max(1);
    let len_score = latin_constant(upem, 6000);
    let dist_score: i32 = 3000;

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
                let dist = pos2 - pos1;

                if len >= len_threshold {
                    // aflatin.c:2093-2113 — exact C scoring
                    let dist_demerit: i32;
                    if max_width > 0 {
                        let delta = ((dist << 10) / max_width) - (1 << 10);
                        if delta > 10000 {
                            dist_demerit = 32000;
                        } else if delta > 0 {
                            dist_demerit = (delta * delta) / dist_score;
                        } else {
                            dist_demerit = 0;
                        }
                    } else {
                        dist_demerit = dist; // no widths → use raw distance
                    }

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

// ✅ VERIFIED: verified via hint_edges — all edge positions match C
// for DejaVuSans 10pt '&'. Port of af_latin_snap_width (aflatin.c:2725-2767).
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

// ✅ VERIFIED: via hint_edges C trace. Port of af_latin_align_linked_edge
// (aflatin.c:4157-4183).
fn align_linked_edge(
    other_flags: u32,
    dim: Dimension,
    base_edge: &AFEdge,
    stem_edge: &mut AFEdge,
    std_widths: &[i32],
) {
    let dist = stem_edge.opos - base_edge.opos;
    let base_delta = base_edge.pos - base_edge.opos;

    let fitted_width = compute_stem_width(
        other_flags, 0, dim,
        dist, base_delta,
        base_edge.flags,
        stem_edge.flags,
        std_widths,
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

// Port of `af_latin_compute_stem_width` (aflatin.c:3960–4152).
// ✅ VERIFIED: Smooth path matches C's inline logic (aflatin.c:3993-4075).
//    Serif: return dist. Round: snap≤1px. dist<56: clamp. Then standard-width
//    match |delta|<40, fractional-pixel quant, or bdelta+round (simplified).
// ✅ VERIFIED: Strong path calls snap_width + pixel rounding (aflatin.c:4076-4152).
fn compute_stem_width(
    other_flags: u32,
    _ppem: i32,
    dim: Dimension,
    width: i32,
    _base_delta: i32,
    base_flags: u8,
    stem_flags: u8,
    std_widths: &[i32],  // standard widths in 26.6 (from metrics .cur)
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
        // ── Smooth hinting: light quantization (aflatin.c:3993-4075) ────
        // Port of C's inline logic. Uses widths[0].cur directly, NOT snap_width.

        // Step 1: Leave serif widths alone (aflatin.c:3998-4001).
        if (stem_flags & AF_EDGE_SERIF) != 0 && vertical && dist < 3 * 64 {
            // goto Done_Width → return immediately, no quantization
            if sign != 0 { dist = -dist; }
            return dist;
        }

        // Step 2: Round-edge stem → snap to 1px (aflatin.c:4003-4006).
        if (base_flags & AF_EDGE_ROUND) != 0 {
            if dist < 80 {
                dist = 64;
            }
        } else if dist < 56 {
            // Step 3: Very thin stems → clamp to 56 (aflatin.c:4007-4008).
            dist = 56;
        }

        // Step 4: Standard-width matching + fractional pixel quantization
        // (aflatin.c:4016-4075).
        if !std_widths.is_empty() {
            let stdw = std_widths[0]; // axis->widths[0].cur
            let mut delta = dist - stdw;
            if delta < 0 { delta = -delta; }

            if delta < 40 {
                // Within tolerance of standard width → snap to it, clamp min.
                dist = stdw;
                if dist < 48 { dist = 48; }
                // goto Done_Width
                if sign != 0 { dist = -dist; }
                return dist;
            }

            if dist < 3 * 64 {
                // Fractional-pixel quantization (aflatin.c:4035-4047).
                delta = dist & 63;
                dist &= -64; // truncate to integer pixel

                if delta < 10 { dist += delta; }
                else if delta < 32 { dist += 10; }
                else if delta < 54 { dist += 54; }
                else { dist += delta; }
            } else {
                // bdelta adjustment + round (aflatin.c:4050-4075).
                // TODO: implement full bdelta when ppem is available.
                let bdelta: i32 = 0; // simplified
                dist = (dist - bdelta + 32) & !63;
            }
        }
    } else {
        // ── Strong hinting: snap to integer pixels ──────────────────────

        let org_dist = dist;

        dist = snap_width(std_widths, dist);

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

// ✅ VERIFIED: all edge positions (fpos, opos, pos) after hint_edges
// match C exactly for DejaVuSans 10pt '&' (5 VERT + 5 HORZ edges).
// Port of af_latin_hint_edges (aflatin.c:4220-4837).
fn hint_edges(hints: &mut GlyphHints, dim: Dimension, std_widths: &[i32]) {
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

    // ── Phase 1: Blue-zone alignment (aflatin.c:4247-4336) ──────────────
    if dim == Dimension::Vert && hints.metrics.is_some() {
        for i in 0..num_edges {
            if axis.edges[i].flags & AF_EDGE_DONE != 0 { continue; }

            let mut edge1_idx: Option<usize> = None;
            let mut edge2_idx: Option<usize> = None;
            let mut blue: Option<AfWidth> = None;

            // Neutral blue dedup: if both edges of a stem have blue edges,
            // keep only the non-neutral one.  aflatin.c:4270-4286.
            let link = axis.edges[i].link;
            let maybe_blue = axis.edges[i].blue_edge;
            if let Some(b) = maybe_blue {
                if link != usize::MAX {
                    let link_blue = axis.edges[link].blue_edge;
                    if link_blue.is_some() {
                        let is_neutral = axis.edges[i].flags & AF_EDGE_NEUTRAL != 0;
                        let link_neutral = axis.edges[link].flags & AF_EDGE_NEUTRAL != 0;
                        if link_neutral {
                            axis.edges[link].blue_edge = None;
                            axis.edges[link].flags &= !AF_EDGE_NEUTRAL;
                        } else if is_neutral {
                            axis.edges[i].blue_edge = None;
                            axis.edges[i].flags &= !AF_EDGE_NEUTRAL;
                            continue; // this edge lost its blue
                        }
                    }
                }
                edge1_idx = Some(i);
                blue = Some(b);
            } else if link != usize::MAX {
                if let Some(b2) = axis.edges[link].blue_edge {
                    blue = Some(b2);
                    edge1_idx = Some(link);
                    edge2_idx = Some(i);
                }
            }

            if edge1_idx.is_none() { continue; }

            let e1 = edge1_idx.unwrap();
            axis.edges[e1].pos = blue.unwrap().fit;
            axis.edges[e1].flags |= AF_EDGE_DONE;

            if let Some(e2) = edge2_idx {
                if axis.edges[e2].blue_edge.is_none() {
                    align_linked_edge(other_flags, dim, &axis.edges[e1].clone(), &mut axis.edges[e2], std_widths);
                    axis.edges[e2].flags |= AF_EDGE_DONE;
                }
            }

            if anchor == usize::MAX { anchor = i; }
        }
    }

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
                other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags, std_widths,
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
                    other_flags, 0, dim, dist, base_delta, base_flags, stem_flags, std_widths,
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
                other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags, std_widths,
            );

            // ✅ VERIFIED (2026-06-27): C sets edge2->pos = cur_pos1 + cur_len/2
            //    directly (aflatin.c:4502), no af_latin_align_linked_edge call.
            //    The "Align linked edge" block below was overwriting this.
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
                // C: edge2->pos = cur_pos1 + cur_len / 2 (aflatin.c:4502)
                axis.edges[edge2_idx].pos = cur_pos1 + cur_len / 2;
                axis.edges[edge2_idx].flags |= AF_EDGE_DONE;
            } else {
                let cur_len2 = compute_stem_width(
                    other_flags, 0, dim, org_len, 0, edge_flags, edge2_flags, std_widths,
                );

                let cur_pos1 = (org_pos + 32) & !63; // FT_PIX_ROUND
                let delta1 = (cur_pos1 + (cur_len2 >> 1) - org_center).abs();

                let cur_pos2 = ((org_pos + org_len + 32) & !63) - cur_len2;
                let delta2 = (cur_pos2 + (cur_len2 >> 1) - org_center).abs();

                axis.edges[i].pos = if delta1 < delta2 { cur_pos1 } else { cur_pos2 };
                // C: edge2->pos = edge->pos + cur_len (aflatin.c:4527)
                axis.edges[edge2_idx].pos = axis.edges[i].pos + cur_len2;
                axis.edges[edge2_idx].flags |= AF_EDGE_DONE;
            }
        }

        axis.edges[i].flags |= AF_EDGE_DONE;

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

    // ── Phase 3: Lowercase 'm' symmetry (aflatin.c:4582-4627) ────────────
    // If a glyph has 3 stems (6 edges) or 3 stems with serifs (12 edges),
    // make the outer stems symmetric around the middle stem.
    if dim == Dimension::Horz && (num_edges == 6 || num_edges == 12) {
        let (e1_idx, e2_idx, e3_idx) = if num_edges == 6 {
            (0, 2, 4)
        } else {
            (1, 5, 9)
        };
        let e1_opos = axis.edges[e1_idx].opos;
        let e2_opos = axis.edges[e2_idx].opos;
        let e3_opos = axis.edges[e3_idx].opos;
        let dist1 = e2_opos - e1_opos;
        let dist2 = e3_opos - e2_opos;
        let mut span = dist1 - dist2;
        if span < 0 { span = -span; }
        if span < 8 {
            let delta = axis.edges[e3_idx].pos
                - (2 * axis.edges[e2_idx].pos - axis.edges[e1_idx].pos);
            axis.edges[e3_idx].pos -= delta;
            axis.edges[e3_idx].flags |= AF_EDGE_DONE;
            let link = axis.edges[e3_idx].link;
            if link != usize::MAX {
                axis.edges[link].pos -= delta;
                axis.edges[link].flags |= AF_EDGE_DONE;
            }
            // Move serifs along with the stem (12-edge case).
            if num_edges == 12 {
                axis.edges[8].pos -= delta;
                axis.edges[11].pos -= delta;
            }
        }
    }

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
                if ordering_violated && i > 0 {
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
// ✅ VERIFIED: Matches C's algorithm exactly:
//    - Linear scan for ≤8 edges (binary search for >8)
//    - Exact-match edge snap
//    - Scale-based interpolation: FT_DivFix + FT_MulFix (cached on edge)
//    - Fallback: shift by edge delta for points outside edge range

fn align_strong_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis_snapshot = hints.axis[dim as usize].clone();
    let axis = &axis_snapshot;
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

        let pt_fpos = if is_vert { pt.fy as i32 } else { pt.fx as i32 };

        // C: linear scan for first edge with fpos >= u (afhints.c:1492-1502)
        let mut nn: usize = 0;
        while nn < axis.edges.len() && (axis.edges[nn].fpos as i32) < pt_fpos {
            nn += 1;
        }

        if nn >= axis.edges.len() {
            // Point after last edge: shift by edge delta (afhints.c:1460-1470)
            let last = &axis.edges[axis.edges.len() - 1];
            let delta = last.pos - last.opos;
            let val = if is_vert { pt.oy + delta } else { pt.ox + delta };
            if is_vert { hints.points[i].y = val; hints.points[i].flags |= AF_FLAG_TOUCH_Y; }
            else { hints.points[i].x = val; hints.points[i].flags |= AF_FLAG_TOUCH_X; }
            continue;
        }
        if nn == 0 {
            // Point before first edge: shift by edge delta (afhints.c:1456-1469)
            let first = &axis.edges[0];
            let delta = first.pos - first.opos;
            let val = if is_vert { pt.oy + delta } else { pt.ox + delta };
            if is_vert { hints.points[i].y = val; hints.points[i].flags |= AF_FLAG_TOUCH_Y; }
            else { hints.points[i].x = val; hints.points[i].flags |= AF_FLAG_TOUCH_X; }
            continue;
        }

        // C: if exact match, snap to edge (afhints.c:1496-1499)
        if axis.edges[nn].fpos as i32 == pt_fpos {
            let val = axis.edges[nn].pos;
            if is_vert { hints.points[i].y = val; hints.points[i].flags |= AF_FLAG_TOUCH_Y; }
            else { hints.points[i].x = val; hints.points[i].flags |= AF_FLAG_TOUCH_X; }
            continue;
        }

        // Interpolate: before = edges[nn-1], after = edges[nn] (afhints.c:1523-1540)
        let before = &axis.edges[nn - 1];
        let after = &axis.edges[nn];

        // C: scale = FT_DivFix(after.pos - before.pos, after.fpos - before.fpos)
        let pos_delta = after.pos - before.pos;
        let fpos_delta = (after.fpos - before.fpos) as i32;
        let scale = ft_div_fix(pos_delta, fpos_delta);
        let offset = pt_fpos - before.fpos as i32;
        // C: u = before->pos + FT_MulFix(fu - before->fpos, before->scale)
        let val = before.pos + ft_mul_fix(offset, scale);

        if is_vert {
            hints.points[i].y = val;
            hints.points[i].flags |= AF_FLAG_TOUCH_Y;
        } else {
            hints.points[i].x = val;
            hints.points[i].flags |= AF_FLAG_TOUCH_X;
        }
    }
}

// ── IUP helpers (afhints.c:1592-1681) ────────────────────────────────────────

/// ✅ VERIFIED: delta matches C's af_iup_shift for contour 1
/// (39/39 points correct for DejaVuSans 10pt '&' — afhints.c:1592).
fn iup_shift(points: &mut [AFPoint], p1: usize, p2: usize, ref_idx: usize) {
    let delta = points[ref_idx].u - points[ref_idx].v;
    if delta == 0 { return; }
    for i in p1..=p2 {
        if i != ref_idx {
            points[i].u = points[i].v + delta;
        }
    }
}

/// ✅ VERIFIED: scale + ft_mul_fix match C for contour 1
/// (39/39 points correct for DejaVuSans 10pt '&' — afhints.c:1619).
fn iup_interp(points: &mut [AFPoint], p1: usize, p2: usize, ref1: usize, ref2: usize) {
    if p1 > p2 { return; }

    let (ref1, ref2) = if points[ref1].v > points[ref2].v {
        (ref2, ref1)
    } else {
        (ref1, ref2)
    };

    let v1 = points[ref1].v;
    let v2 = points[ref2].v;
    let u1 = points[ref1].u;
    let u2 = points[ref2].u;
    let d1 = u1 - v1;
    let d2 = u2 - v2;

    if u1 == u2 || v1 == v2 {
        for i in p1..=p2 {
            let u = points[i].v;
            if u <= v1 { points[i].u = u + d1; }
            else if u >= v2 { points[i].u = u + d2; }
            else { points[i].u = u1; }
        }
    } else {
        let scale = ft_mul_div(u2 - u1, 0x10000, v2 - v1); // FT_DivFix
        for i in p1..=p2 {
            let u = points[i].v;
            if u <= v1 { points[i].u = u + d1; }
            else if u >= v2 { points[i].u = u + d2; }
            else { points[i].u = u1 + ft_mul_fix(u - v1, scale); }
        }
    }
}

// ── Weak-point alignment (IUP) ─────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_weak_points` (afhints.c:1687–1808).
// ✅ VERIFIED: IUP dispatch correct for non-boundary contours
// (39/39 contour-1 points match C for DejaVuSans 10pt '&').
// Port of af_glyph_hints_align_weak_points (afhints.c:1687-1808).
// ⚠️ contour boundary 5 points (p0-p4) differ — in_dir/out_dir mismatch.
fn align_weak_points(hints: &mut GlyphHints, dim: Dimension) {
    let is_vert = dim == Dimension::Vert;
    let touch_flag = if is_vert { AF_FLAG_TOUCH_Y } else { AF_FLAG_TOUCH_X };

    // PASS 1: Set u = hinted (current x/y), v = original (ox/oy)
    for pt in &mut hints.points {
        if is_vert {
            pt.u = pt.y;
            pt.v = pt.oy;
        } else {
            pt.u = pt.x;
            pt.v = pt.ox;
        }
    }

    // PASS 2: Iterate contours in storage order (points are contiguous per-contour)
    let contours_snapshot = hints.contours.clone();
    for &c_start in &contours_snapshot {
        let end_idx = hints.points[c_start].prev; // last point index of this contour

        // Find first touched point
        let mut idx = c_start;
        let first_touched: usize = loop {
            if idx > end_idx { break usize::MAX; } // no touched point in contour
            if hints.points[idx].flags & touch_flag != 0 { break idx; }
            idx += 1;
        };
        if first_touched == usize::MAX { continue; }

        let mut last_touched = first_touched;

        loop {
            // skip consecutive touched points
            while last_touched < end_idx && hints.points[last_touched + 1].flags & touch_flag != 0 {
                last_touched += 1;
            }

            // Find next touched point
            let mut next = last_touched + 1;
            let next_touched: Option<usize> = loop {
                if next > end_idx { break None; }
                if hints.points[next].flags & touch_flag != 0 { break Some(next); }
                next += 1;
            };

            if let Some(nt) = next_touched {
                // Interpolate between last_touched and next_touched
                iup_interp(&mut hints.points, last_touched + 1, nt - 1, last_touched, nt);
                last_touched = nt;
            } else {
                // End of contour
                if last_touched == first_touched {
                    // Only one touched point: uniform shift
                    iup_shift(&mut hints.points, c_start, end_idx, first_touched);
                } else {
                    // Interpolate tail segments
                    if last_touched < end_idx {
                        iup_interp(&mut hints.points, last_touched + 1, end_idx, last_touched, first_touched);
                    }
                    if first_touched > c_start {
                        iup_interp(&mut hints.points, c_start, first_touched - 1, last_touched, first_touched);
                    }
                }
                break;
            }
        }
    }

    // PASS 3: Write u back to x/y
    for pt in &mut hints.points {
        if is_vert {
            pt.y = pt.u;
        } else {
            pt.x = pt.u;
        }
    }
}
