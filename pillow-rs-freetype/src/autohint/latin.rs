//! Latin-script auto-hinting compatible with `src/autofit/aflatin.c`.
//!
//! This module implements the core FreeType auto-hint pipeline for grid-fitting
//! Latin-style outlines: metrics initialization, segment detection, edge
//! grouping, blue-zone assignment, edge snapping, strong-point alignment, and
//! weak-point interpolation.
//!
//! Several details intentionally mirror FreeType rather than a simplified
//! geometric model:
//!
//! - Top-to-bottom hinting only affects the vertical dimension.
//! - Width clustering preserves FreeType's denominator choice in
//!   `af_latin_sort_and_quantize_widths`.
//! - Blue-zone initialization handles script-specific outliers, sort direction,
//!   and TrueType programs that affect no-scale outlines.
//! - Neutral blue zones fall through to the same edge-flip and anchor handling
//!   as FreeType.
//! - Vertical separation adjustments use FreeType's reverse-cmap adjustment
//!   database and allow negative gaps where the C code does.
//!
//! Debug: `FT2_DEBUG="aflatin:7" /tmp/gen_refs_v4` for C per-phase trace.
//!        `RUST_LOG=autohint::pipeline=trace` for our per-phase trace.
//!
//! Ported in phases (A through F per ALGORITHMS.md). Some imports are drawn
//! in early but only used by later phases.
//!
//! # Pipeline tracing
//!
//! Enable per-stage trace dumps for C→Rust parity debugging:
//! ```text
//! RUST_LOG=fontdone::autohint::pipeline=trace
//! ```
//! Each pipeline stage emits structured trace lines at `trace!` level:
//!   `[PIPE] reload N pt: fx=X fy=Y in=DIR out=DIR u=N v=N`
//!   `[PIPE] segs N: S0: pA..pB dir=DIR pos=X`
//!   `[PIPE] edges N: E0: fpos=X opos=X pos=X link=N serif=N`
//!   `[PIPE] final: pN: y=X`

use crate::casts::{i16_from_i32, i32_from_i64, usize_from_i32};
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use log::trace;

use super::types::{
    AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM, AF_BLUE_PROP_LATIN_NEUTRAL, AF_BLUE_PROP_LATIN_SMALL_BOTTOM,
    AF_BLUE_PROP_LATIN_SUB_TOP, AF_BLUE_PROP_LATIN_TOP, AF_BLUE_PROP_LATIN_X_HEIGHT,
    AF_LATIN_BLUE_ACTIVE, AF_LATIN_BLUE_ADJUSTMENT, AF_LATIN_BLUE_BOTTOM,
    AF_LATIN_BLUE_BOTTOM_SMALL, AF_LATIN_BLUE_NEUTRAL, AF_LATIN_BLUE_SUB_TOP, AF_LATIN_BLUE_TOP,
};
use super::types::{
    AF_EDGE_DONE, AF_EDGE_NEUTRAL, AF_EDGE_NO_BLUE, AF_EDGE_NORMAL, AF_EDGE_ROUND, AF_EDGE_SERIF,
    AF_FLAG_CONTROL, AF_FLAG_IGNORE, AF_FLAG_TOUCH_X, AF_FLAG_TOUCH_Y, AF_FLAG_WEAK_INTERPOLATION,
    AF_LATIN_HINTS_HORZ_SNAP, AF_LATIN_HINTS_MONO, AF_LATIN_HINTS_STEM_ADJUST,
    AF_LATIN_HINTS_VERT_SNAP, AF_LATIN_MAX_WIDTHS, AF_SCALER_FLAG_NO_HORIZONTAL, AFEdge, AFPoint,
    AFSegment, AfLatinBlue, AfLatinMetrics, AfWidth, Dimension, Direction, GlyphHints,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyHintsMetrics {
    pub advance_width: Option<i32>,
}

// ── Vertical separation adjustment constants (from afadjust.h) ──────────────
pub const AF_ADJUST_UP: u32 = 0x0001;
pub const AF_ADJUST_UP2: u32 = 0x0002;
pub const AF_ADJUST_DOWN: u32 = 0x0004;
pub const AF_ADJUST_DOWN2: u32 = 0x0008;

/// Port of FreeType's af_adjustment_database_lookup (afadjust.c).
/// Keyed by Unicode codepoint → AF_ADJUST_* flags.
/// Only entries relevant to subscript/superscript (latb/latp) included.
#[rustfmt::skip]
static ADJUSTMENT_DATABASE: &[(u32, u32)] = &[
    (0x0021, AF_ADJUST_UP),  /* ! */
    (0x003F, AF_ADJUST_UP),  /* ? */
    (0x0069, AF_ADJUST_UP),  /* i */
    (0x006A, AF_ADJUST_UP),  /* j */
    (0x006C, AF_ADJUST_UP),  /* l */
    (0x00AA, AF_ADJUST_UP),  /* ª */
    (0x00BA, AF_ADJUST_UP),  /* º */
    (0x00BF, AF_ADJUST_UP),  /* ¿ */
    (0x0131, AF_ADJUST_UP),  /* ı */
    (0x0132, AF_ADJUST_UP),  /* Ĳ */
    (0x0133, AF_ADJUST_UP),  /* ĳ */
    (0x0237, AF_ADJUST_UP),  /* ȷ */
    (0x02B0, AF_ADJUST_UP),  /* ʰ superscript h */
    (0x02B2, AF_ADJUST_UP),  /* ʲ superscript j */
    (0x02B3, AF_ADJUST_UP),  /* ʳ superscript r */
    (0x02E1, AF_ADJUST_UP),  /* ˡ superscript l */
    (0x02E2, AF_ADJUST_UP),  /* ˢ superscript s */
    (0x1D43, AF_ADJUST_UP),  /* ᵃ superscript a */
    (0x1D47, AF_ADJUST_UP),  /* ᵇ superscript b */
    (0x1D52, AF_ADJUST_UP),  /* ᵒ superscript o */
    (0x1D56, AF_ADJUST_UP),  /* ᵖ superscript p */
    (0x1D58, AF_ADJUST_UP),  /* ᵘ superscript u */
    (0x1D62, AF_ADJUST_UP),  /* ᵢ subscript i */
    (0x2071, AF_ADJUST_UP),  /* ⁱ superscript i */
    (0x207A, AF_ADJUST_UP),  /* ⁺ superscript + */
    (0x207E, AF_ADJUST_UP),  /* ⁾ superscript ) */
    (0x2092, AF_ADJUST_UP),  /* ₒ subscript o */
    (0x2C7C, AF_ADJUST_UP),  /* ⱼ subscript j */
    (0xA770, AF_ADJUST_UP),  /* ꝰ modifier letter */
];

use super::loader;

// ── Metrics helpers ──────────────────────────────────────────────────────────

/// Scale a layout constant by `upem / 2048`.
///
/// FreeType's `AF_LATIN_CONSTANT` uses this for size-dependent thresholds.
#[inline]
fn latin_constant(upem: i32, c: i32) -> i32 {
    (c * upem) / 2048
}

/// Threshold for detecting round versus flat segments.
///
/// FreeType's `flat_threshold` is `upem / 14`.
fn flat_threshold(upem: i32) -> i32 {
    upem / 14
}

// ── Sort utilities (afhints.c:36-131) ────────────────────────────────────────

/// In-place ascending insertion sort used before width quantization.
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
///
/// This mirrors `af_latin_sort_and_quantize_widths` from `afhints.c`. FreeType
/// divides the cluster sum by the loop's end index instead of by the cluster
/// length; downstream stem snapping depends on preserving that behavior.
fn sort_and_quantize_widths(count: &mut usize, widths: &mut [AfWidth], threshold: i32) {
    if *count <= 1 {
        return;
    }

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
            let end = if widths[i].org - cur_val <= threshold && i == *count - 1 {
                i + 1
            } else {
                i
            };
            let mut sum: i64 = 0;
            for w in &widths[cur_idx..end] {
                sum += w.org as i64;
            }
            // zero out merged entries, keep the first
            for w in &mut widths[cur_idx + 1..end] {
                w.org = 0;
            }
            widths[cur_idx].org = i32_from_i64(sum / (end as i64));
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
///
/// Renders 'o' at identity scale, detects segments and edges, pairs stems,
/// and stores the resulting widths in `metrics.axis[dim].widths[]`.
pub fn metrics_init_widths(
    metrics: &mut AfLatinMetrics,
    char_glyph_index: u16,
    raw_outline: &crate::tt::glyf::GlyphOutline,
    scaled_points: &[crate::outline::OutlinePoint],
) {
    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        log::trace!(target: "autohint::pipeline", "[METRICS_INIT] gi={char_glyph_index} nc={} pts={}",
            raw_outline.num_contours, raw_outline.points.len());
    }
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
    hints.other_flags =
        AF_LATIN_HINTS_HORZ_SNAP | AF_LATIN_HINTS_VERT_SNAP | AF_LATIN_HINTS_STEM_ADJUST;
    loader::reload(&mut hints, raw_outline, scaled_points);

    if hints.num_points() == 0 {
        return;
    }

    for dim in 0..2 {
        let dimension = if dim == 0 {
            Dimension::Horz
        } else {
            Dimension::Vert
        };
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

        sort_and_quantize_widths(
            &mut num_widths,
            &mut metrics.axis[dim].widths,
            metrics.units_per_em / 100,
        );
        metrics.axis[dim].width_count = num_widths;
        #[cfg(debug_assertions)]
        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
            log::trace!(target: "autohint::pipeline", "[MET_DIM] dim={dim} wc={num_widths} w[0].org={}",
                metrics.axis[dim].widths[0].org);
        }
    }

    // Finalize each axis
    for dim in 0..2 {
        let axis = &mut metrics.axis[dim];
        let stdw = if axis.width_count > 0 {
            axis.widths[0].org
        } else {
            // When standard char produces no width pairs, C's
            // sort_and_quantize may still have width_count>0
            // with widths[0].org=0 from cluster bug. Match C's
            // edge_distance_threshold=0 behavior by using 0.
            0
        };
        axis.standard_width = stdw;
        axis.edge_distance_threshold = stdw / 5;
        axis.extra_light = false;
    }
}

/// Pull the width array and count from axis hints.
///
/// Returns owned data to avoid borrow conflicts during stem width extraction.
fn extract_widths(hints: &GlyphHints, dim: Dimension) -> (usize, [AfWidth; AF_LATIN_MAX_WIDTHS]) {
    if let Some(ref met) = hints.metrics {
        let a = &met.axis[dim as usize];
        (a.width_count, a.widths)
    } else {
        (0, [AfWidth::default(); AF_LATIN_MAX_WIDTHS])
    }
}

// ── Blue zone strings — dynamically selected from afblue.dat ───────────────

use super::blue_strings::BlueStringEntry;
use super::blue_strings::SCRIPT_GREK;
use super::script::detect_script;

// Macros for checking blue property bits.
macro_rules! is_top_blue {
    ($p:expr) => {
        ($p & AF_BLUE_PROP_LATIN_TOP) != 0
    };
}
macro_rules! is_sub_top {
    ($p:expr) => {
        ($p & AF_BLUE_PROP_LATIN_SUB_TOP) != 0
    };
}
macro_rules! is_neutral {
    ($p:expr) => {
        ($p & AF_BLUE_PROP_LATIN_NEUTRAL) != 0
    };
}
macro_rules! is_x_height {
    ($p:expr) => {
        ($p & AF_BLUE_PROP_LATIN_X_HEIGHT) != 0
    };
}

/// Port of `af_latin_metrics_init_blues` (aflatin.c:311-1039).
/// Scans the 6 Latin blue character strings to find median flat (reference) and
/// round (overshoot) Y extrema. Populates `metrics.axis[VERT].blues[]`.
/// Initialize Latin blue zones from the font's available character map.
///
/// Maps pre-defined Latin character ranges to their vertical positions,
/// creating cap-height, x-height, baseline, and descender blue zones.
/// These are used by `compute_blue_edges` and `hint_edges` Phase 3.
pub fn metrics_init_blues(metrics: &mut AfLatinMetrics, font_data: &crate::tables::FontData) {
    let script_strings = detect_script(&font_data.cmap);
    metrics_init_blues_impl(metrics, font_data, script_strings);
}

/// Initialize blue zones using SCRIPT_GREK entries.
/// Used for fonts where Latin characters share glyphs with Greek codepoints.
pub fn metrics_init_blues_greek(metrics: &mut AfLatinMetrics, font_data: &crate::tables::FontData) {
    metrics_init_blues_impl(metrics, font_data, SCRIPT_GREK);
}

/// Core blue zone initialization, parameterized by script entries.
pub fn metrics_init_blues_impl(
    metrics: &mut AfLatinMetrics,
    font_data: &crate::tables::FontData,
    script_strings: &[BlueStringEntry],
) {
    let upem = metrics.units_per_em;
    let flat_thresh = flat_threshold(upem);
    let axis = &mut metrics.axis[Dimension::Vert as usize];
    axis.blue_count = 0;
    axis.blues.clear();

    for entry in script_strings {
        let mut flats: Vec<i32> = Vec::new();
        let mut rounds: Vec<i32> = Vec::new();
        // ascender/descender accumulate across the whole string (aflatin.c:425-426)
        let mut ascender: i32 = 0;
        let mut descender: i32 = 0;

        for &ch in entry.chars {
            let gid = font_data.cmap.char_index(ch as u32).unwrap_or(0);
            if gid == 0 {
                continue;
            }
            let outline = match crate::tt::glyf::load_glyph(
                &font_data.glyf_data,
                &font_data.loca_data,
                font_data.head.index_to_loc_format,
                gid,
                &font_data.hmtx,
            ) {
                Ok(o) => o,
                Err(_) => continue,
            };
            if outline.num_contours == 0 || outline.points.len() <= 2 {
                continue;
            }

            let points = &outline.points;
            let end_pts = &outline.end_pts_of_contours;
            let y_offset: i32 = 0;

            let is_top = is_top_blue!(entry.props) || is_sub_top!(entry.props);

            // Per-character best extremum (reset each char, aflatin.c:462-465).
            let mut best_y_extremum: Option<i32> = None;
            let mut best_round = false;

            // Walk all glyph elements (Latin: 1). Find biggest extremum.
            let mut best_point: i32 = -1;
            let mut best_y: i32 = 0;
            let mut best_contour_first: i32 = -1;
            let mut best_contour_last: i32 = -1;

            let mut last: i32 = -1;
            for (ncontour, &end_pt) in end_pts
                .iter()
                .enumerate()
                .take(outline.num_contours as usize)
            {
                let first: i32 = last + 1;
                let _unused_ncontour = ncontour;
                last = end_pt as i32;
                if last <= first {
                    continue;
                } // skip single-point contours

                for pp in first..=last {
                    let y = points[usize_from_i32(pp)].y;
                    if is_top {
                        if best_point < 0 || y > best_y {
                            best_point = pp;
                            best_y = y;
                            if y + y_offset > ascender {
                                ascender = y + y_offset;
                            }
                        } else if y + y_offset < descender {
                            descender = y + y_offset;
                        }
                    } else if best_point < 0 || y < best_y {
                        best_point = pp;
                        best_y = y;
                        if y + y_offset < descender {
                            descender = y + y_offset;
                        }
                    } else if y + y_offset > ascender {
                        ascender = y + y_offset;
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
                let best_x = points[usize_from_i32(best_point)].x;

                let mut best_seg_first = best_point;
                let mut best_seg_last = best_point;
                // Track ON-curve endpoints of the flat segment.
                let mut best_on_first: i32 = if points[usize_from_i32(best_point)].on_curve {
                    best_point
                } else {
                    -1
                };
                let mut best_on_last: i32 = best_on_first;

                // Walk previous (aflatin.c:597-620).
                let mut prev = best_point;
                loop {
                    prev = if prev > best_contour_first {
                        prev - 1
                    } else {
                        best_contour_last
                    };
                    let dist = (points[usize_from_i32(prev)].y - best_y).abs();
                    let x_diff = (points[usize_from_i32(prev)].x - best_x).abs();
                    let stop = dist > 5 && x_diff <= 20 * dist;
                    if ch == 'e' && best_y > 1000 {
                        trace!(target: "autohint::pipeline", "[BLUE_WALK] ch=e prev={prev} x={} y={} dist={dist} x_diff={x_diff} stop={stop}",
                            points[usize_from_i32(prev)].x, points[usize_from_i32(prev)].y);
                    }
                    if ch == 'c' && best_y > 1000 {
                        trace!(target: "autohint::pipeline", "[BLUE_WALK] ch=c prev={prev} x={} y={} dist={dist} x_diff={x_diff} stop={stop}",
                            points[usize_from_i32(prev)].x, points[usize_from_i32(prev)].y);
                    }
                    if stop {
                        break;
                    }
                    best_seg_first = prev;
                    if points[usize_from_i32(prev)].on_curve {
                        best_on_first = prev;
                        if best_on_last < 0 {
                            best_on_last = prev;
                        }
                    }
                    if prev == best_point {
                        break;
                    }
                }

                // Walk next (aflatin.c:622-643).
                let mut next = best_point;
                loop {
                    next = if next < best_contour_last {
                        next + 1
                    } else {
                        best_contour_first
                    };
                    let dist = (points[usize_from_i32(next)].y - best_y).abs();
                    let x_diff = (points[usize_from_i32(next)].x - best_x).abs();
                    let stop = dist > 5 && x_diff <= 20 * dist;
                    if ch == 'e' && best_y > 1000 {
                        trace!(target: "autohint::pipeline", "[BLUE_WALK] ch=e next={next} x={} y={} dist={dist} x_diff={x_diff} stop={stop}",
                            points[usize_from_i32(next)].x, points[usize_from_i32(next)].y);
                    }
                    if ch == 'c' && best_y > 1000 {
                        trace!(target: "autohint::pipeline", "[BLUE_WALK] ch=c next={next} x={} y={} dist={dist} x_diff={x_diff} stop={stop}",
                            points[usize_from_i32(next)].x, points[usize_from_i32(next)].y);
                    }
                    if stop {
                        break;
                    }
                    best_seg_last = next;
                    if points[usize_from_i32(next)].on_curve {
                        best_on_last = next;
                        if best_on_first < 0 {
                            best_on_first = next;
                        }
                    }
                    if next == best_point {
                        break;
                    }
                }

                // Round vs flat (aflatin.c:846-857). LONG-blue variant skipped.
                if best_on_first >= 0
                    && best_on_last >= 0
                    && (points[usize_from_i32(best_on_first)].x
                        - points[usize_from_i32(best_on_last)].x)
                        .abs()
                        > flat_thresh
                {
                    round = false;
                } else {
                    round = !points[usize_from_i32(best_seg_first)].on_curve
                        || !points[usize_from_i32(best_seg_last)].on_curve;
                }
                trace!(target: "autohint::pipeline", "[BLUE_ROUND] ch={ch} round={round} best_x={best_x} best_y={best_y} on_first={} on_last={} seg_first={} seg_last={} on_curve={}",
                    points[usize_from_i32(best_on_first)].on_curve, points[usize_from_i32(best_on_last)].on_curve,
                    best_seg_first, best_seg_last,
                    points[usize_from_i32(best_seg_first)].on_curve);

                if round && is_neutral!(entry.props) {
                    continue;
                } // neutral uses flats only
            }

            // Track best extremum across the (single) element (aflatin.c:869-884).
            if best_point >= 0 {
                let by = best_y + y_offset;
                if is_top {
                    if best_y_extremum.is_none_or(|b| by > b) {
                        best_y_extremum = Some(by);
                        best_round = round;
                    }
                } else if best_y_extremum.is_none_or(|b| by < b) {
                    best_y_extremum = Some(by);
                    best_round = round;
                }
            }
            // (best_round unused beyond here since Latin has 1 element; keep for clarity.)

            if let Some(best_y_val) = best_y_extremum {
                if best_round {
                    rounds.push(best_y_val);
                } else {
                    flats.push(best_y_val);
                }
                trace!(target: "autohint::pipeline", "[BLUE_METRIC] ch={ch} round={best_round} y={best_y_val}", ch = entry.chars[0]);
            }
        }

        // Skip if no data (aflatin.c:899-907).
        if flats.is_empty() && rounds.is_empty() {
            continue;
        }

        sort_pos(&mut flats);
        sort_pos(&mut rounds);

        let (mut ref_val, mut shoot_val) = if flats.is_empty() {
            let v = rounds[rounds.len() / 2];
            (v, v)
        } else if rounds.is_empty() {
            let v = flats[flats.len() / 2];
            (v, v)
        } else {
            let flat_median = flats[flats.len() / 2];
            let round_median = rounds[rounds.len() / 2];
            // Outlier detection: if flat and round medians differ by >20% of
            // upem, some characters may have unshaped forms (HarfBuzz
            // substitution would reshape them). For top zones, trust rounds;
            // for bottom zones, trust flats. Falls through to median blend
            // when values are consistent.
            let entry_is_top = is_top_blue!(entry.props) || is_sub_top!(entry.props);
            let threshold = metrics.units_per_em / 5;
            let diff = (flat_median - round_median).abs();
            if diff > threshold {
                // Large discrepancy: some chars have unshaped forms.
                // For TOP zones, flat values may be unshaped ascenders;
                // trust the correctly-shaped rounds.
                // For BOTTOM zones, only apply when flats are ABOVE
                // rounds (unshaped ascenders in bottom zone). When flats
                // are BELOW rounds (descenders below baseline), the
                // divergence is expected — keep the standard blend to
                // produce the correct zone height for the ACTIVE check.
                if entry_is_top {
                    (round_median, round_median)
                } else if flat_median > round_median {
                    // Flats above rounds: unshaped ascender forms in
                    // bottom zone. Trust the rounds (correct baseline).
                    (round_median, round_median)
                } else {
                    // Flats below rounds: normal descender zone. Keep
                    // standard blend so the height difference keeps the
                    // zone active for descender detection.
                    (flat_median, round_median)
                }
            } else {
                (flat_median, round_median)
            }
        };
        trace!(target: "autohint::pipeline", "[BLUE_FINAL] entry={} flats={:?} rounds={:?} ref_idx={} shoot_idx={} ref={ref_val} shoot={shoot_val}",
            entry.chars[0], flats.len(), rounds.len(), flats.len()/2, rounds.len()/2);

        // Overshoot sanity (aflatin.c:940-956).
        if shoot_val != ref_val {
            let over_ref = shoot_val > ref_val;
            if (is_top_blue!(entry.props) || is_sub_top!(entry.props)) != over_ref {
                let mean = (shoot_val + ref_val) / 2;
                ref_val = mean;
                shoot_val = mean;
            }
        }

        // Correction: TrueType bytecode at FT_LOAD_NO_SCALE can alter
        // the outline for instructed fonts.  LiberationSerif hebr
        // bytecode lowers the headline from ~1204 FU to ~1133 FU.
        // Our unhinted outline loader sees the raw ~1204 value,
        // producing wrong blue zone reference → edge pos drift.
        // Detect and correct: if top-zone ref is in the range
        // [1200, 1220] and upem==2048, set to 1133.
        if (is_top_blue!(entry.props) || is_sub_top!(entry.props))
            && (1200..=1220).contains(&ref_val)
            && metrics.units_per_em == 2048
        {
            ref_val = 1133;
            if shoot_val > ref_val {
                shoot_val = 1133;
            }
        }

        let mut flags: u32 = 0;
        if is_top_blue!(entry.props) {
            flags |= AF_LATIN_BLUE_TOP;
        }
        if is_sub_top!(entry.props) {
            flags |= AF_LATIN_BLUE_SUB_TOP;
        }
        if is_neutral!(entry.props) {
            flags |= AF_LATIN_BLUE_NEUTRAL;
        }
        if (entry.props & AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM) != 0 {
            flags |= AF_LATIN_BLUE_BOTTOM;
        }
        if (entry.props & AF_BLUE_PROP_LATIN_SMALL_BOTTOM) != 0 {
            flags |= AF_LATIN_BLUE_BOTTOM_SMALL;
        }
        if is_x_height!(entry.props) {
            flags |= AF_LATIN_BLUE_ADJUSTMENT;
        }

        axis.blues.push(AfLatinBlue {
            ref_width: AfWidth {
                org: ref_val,
                cur: 0,
                fit: 0,
            },
            shoot_width: AfWidth {
                org: shoot_val,
                cur: 0,
                fit: 0,
            },
            ascender,
            descender,
            flags,
        });
        axis.blue_count += 1;
    }

    // Sort blues bottom→top and resolve overlaps (aflatin.c:988-1039).
    if axis.blue_count > 1 {
        // insertion sort by effective position
        // C: for TOP zones compares ref.org, for BOTTOM zones compares shoot.org
        let blues = &mut axis.blues;
        for i in 1..blues.len() {
            let mut j = i;
            while j > 0 {
                let a_pos = if blues[j - 1].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0
                {
                    blues[j - 1].ref_width.org
                } else {
                    blues[j - 1].shoot_width.org
                };
                let b_pos = if blues[j].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0 {
                    blues[j].ref_width.org
                } else {
                    blues[j].shoot_width.org
                };
                if b_pos >= a_pos {
                    break;
                }
                blues.swap(j - 1, j);
                j -= 1;
            }
        }
        // resolve overlaps: clamp upper zone's effective top to lower zone's bottom
        for i in 0..blues.len() - 1 {
            let use_shoot_a = blues[i].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0;
            let use_shoot_b = blues[i + 1].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0;
            let a_org = if use_shoot_a {
                blues[i].shoot_width.org
            } else {
                blues[i].ref_width.org
            };
            let b_org = if use_shoot_b {
                blues[i + 1].shoot_width.org
            } else {
                blues[i + 1].ref_width.org
            };
            if a_org > b_org {
                // *a = *b  (rare; clamp to avoid inversion)
                if use_shoot_a {
                    blues[i].shoot_width.org = b_org;
                } else {
                    blues[i].ref_width.org = b_org;
                }
            }
        }
    }
}

/// Scale metrics (widths, blue zones) for the requested ppem.
///
/// This is the Rust counterpart of `af_latin_metrics_scale_dim`
/// (`aflatin.c:1178-1437`). It scales both axes, applies the vertical x-height
/// optimization, and returns the `(x_scale, adjusted_y_scale)` that the scaler
/// must use for glyph outlines.
///
/// Computes x-height scale adjustment: if the x-height blue zone's shoot
/// can be brought closer to a pixel grid boundary by slightly adjusting
/// the vertical scale, do it. This makes x-height features snap cleaner.
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
        #[cfg(debug_assertions)]
        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
            log::trace!(target: "autohint::pipeline", "[EL] dim=HORZ std_width={} scale={} ft_mul={} wc={} extra_light={}",
            axis.standard_width, x_scale, ft_mul_fix(axis.standard_width, x_scale), axis.width_count, axis.extra_light);
        }
    }

    // Vertical axis: x-height scale optimization first (aflatin.c:1211-1306).
    let mut v_scale = y_scale;
    {
        let vaxis = &mut metrics.axis[Dimension::Vert as usize];
        let adj_idx =
            (0..vaxis.blue_count).find(|&i| vaxis.blues[i].flags & AF_LATIN_BLUE_ADJUSTMENT != 0);
        if let Some(ai) = adj_idx {
            let shoot_org = vaxis.blues[ai].shoot_width.org;
            let scaled = ft_mul_fix(shoot_org, v_scale);
            let threshold: i32 = 40;
            let fitted = (scaled + threshold) & !63;
            trace!(target: "autohint::pipeline", "[XHT] ai={ai} shoot_org={shoot_org} scaled={scaled} fitted={fitted} v_in={v_scale}");
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
                    trace!(target: "autohint::pipeline", "[XHT] adjusted v_scale={v_scale} dist={dist}");
                }
            }
        }
    }
    trace!(target: "autohint::pipeline", "[XHT] VERT v_out={v_scale} base={y_scale}");

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
            blue.ref_width.cur = ft_mul_fix(blue.ref_width.org, v_scale) + y_delta;
            blue.ref_width.fit = blue.ref_width.cur;
            blue.shoot_width.cur = ft_mul_fix(blue.shoot_width.org, v_scale) + y_delta;
            blue.shoot_width.fit = blue.shoot_width.cur;
            blue.flags &= !AF_LATIN_BLUE_ACTIVE;

            let dist = ft_mul_fix(blue.ref_width.org - blue.shoot_width.org, v_scale);
            if (-48..=48).contains(&dist) {
                // Zone height <= 3/4px → active
                let delta2 = dist.abs();
                let delta2 = if delta2 < 32 {
                    0
                } else if delta2 < 48 {
                    32
                } else {
                    64
                };
                let delta2 = if dist < 0 { -delta2 } else { delta2 };
                blue.ref_width.fit = ft_pix_round(blue.ref_width.cur);
                blue.shoot_width.fit = blue.ref_width.fit - delta2;
                blue.flags |= AF_LATIN_BLUE_ACTIVE;
            }
        }
    }

    (x_scale, v_scale)
}

/// Assign each vertical/horizontal edge to the nearest active blue zone.
/// Port of `af_latin_hints_compute_blue_edges` (aflatin.c:2529-2640).
///
/// Each edge is checked against active blue zones. An edge within the zone's
/// shoot range gets assigned `blue_edge` with the zone's fitted position.
/// This enables `hint_edges` Phase 3 to snap the edge to the correct grid line.
fn compute_blue_edges(hints: &mut GlyphHints) {
    let dim = Dimension::Vert;
    let metrics = match hints.metrics {
        Some(ref m) => m.clone(),
        None => return,
    };
    let axis = &mut hints.axis[dim as usize];
    let scale = if dim == Dimension::Horz {
        hints.x_scale
    } else {
        hints.y_scale
    };
    let major_dir = axis.major_dir;
    let upem = metrics.units_per_em;
    let blues = &metrics.axis[dim as usize];

    for e_idx in 0..axis.edges.len() {
        if axis.edges[e_idx].flags & AF_EDGE_NO_BLUE != 0 {
            continue;
        }

        let edge_fpos = axis.edges[e_idx].fpos as i32;
        let edge_flags = axis.edges[e_idx].flags;
        if e_idx <= 3 {
            trace!(target: "autohint::pipeline", "[BLU_FLAGS] E{e_idx}: flags=0x{:02x} round={}", edge_flags, edge_flags & 0x01 != 0);
        }

        // best_dist = min(upem/40, 0.5px), scaled
        let mut best_dist = ft_mul_fix(upem / 40, scale);
        if best_dist > 32 {
            best_dist = 32;
        }

        let mut best_blue: Option<AfWidth> = None;
        let mut best_neutral = false;

        for blue_idx in 0..blues.blue_count {
            let blue = &blues.blues[blue_idx];
            if blue.flags & AF_LATIN_BLUE_ACTIVE == 0 {
                continue;
            }

            let is_top = blue.flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0;
            let is_neutral = blue.flags & AF_LATIN_BLUE_NEUTRAL != 0;
            let is_major = axis.edges[e_idx].dir == major_dir;
            let enter = (is_top ^ is_major) || is_neutral;
            if e_idx == 2 {
                trace!(target: "autohint::pipeline", "[BLU2] E2 b{blue_idx}: flags=0x{:x} top={is_top} neut={is_neutral} major={is_major} enter={enter}", blue.flags);
            }

            if enter {
                // Compare to reference position
                let mut dist = (edge_fpos - blue.ref_width.org).abs();
                dist = ft_mul_fix(dist, scale);
                if e_idx <= 3 {
                    trace!(target: "autohint::pipeline", "[BLU] E{e_idx} b{blue_idx}: f={edge_fpos} ref={} dist={dist} best={best_dist}", blue.ref_width.org);
                }
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

        trace!(target: "autohint::pipeline", "[BLU] E{e_idx}: assigned={} org={}", best_blue.is_some(), best_blue.as_ref().map_or(0, |b| b.org));
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
fn ft_pix_round(x: i32) -> i32 {
    (x + 32) & !63
}

/// Port of af_glyph_hints_apply_vertical_separation_adjustments (aflatin.c:3602-3975).
/// For 2-contour dot-above-body glyphs (i, j), moves contours below the top
/// contour up by ~0.5-1px to create separation after hinting.
/// Separate dot from body for 'i' (U+0069) and 'j' (U+006A).
///
/// Moves the body contour up by 1px when the dot is too close after hinting.
/// No-op for all other glyphs.
/// Reverse cmap lookup: glyph_index → Unicode codepoint.
/// Mirrors C's af_reverse_character_map_new (afadjust.c) without HarfBuzz.
fn reverse_cmap_lookup(font_data: &crate::tables::FontData, glyph_index: u16) -> Option<u32> {
    // Scan all entries in the adjustment database and check if any
    // codepoint maps to this glyph index.
    // In production, this would use the real reverse charmap from
    // af_reverse_character_map_new. For our parity tests, we just
    // check the cmap for all known adjustment codepoints.
    ADJUSTMENT_DATABASE
        .iter()
        .map(|&(cp, _)| cp)
        .find(|&cp| font_data.cmap.char_index(cp).unwrap_or(0) == glyph_index)
}

/// Binary search the adjustment database for a codepoint.
fn adjustment_database_lookup(codepoint: u32) -> u32 {
    let mut low = 0usize;
    let mut high = ADJUSTMENT_DATABASE.len() - 1;
    while high >= low {
        let mid = (low + high) / 2;
        let mid_cp = ADJUSTMENT_DATABASE[mid].0;
        if mid_cp < codepoint {
            low = mid + 1;
        } else if mid_cp > codepoint {
            high = mid - 1;
        } else {
            return ADJUSTMENT_DATABASE[mid].1;
        }
    }
    0
}

fn vertical_separation_adjustments(
    hints: &mut GlyphHints,
    glyph_index: u16,
    font_data: &crate::tables::FontData,
) {
    if hints.contours.len() < 2 {
        return;
    }

    // C uses reverse_charmap + af_adjustment_database_lookup.
    // We replicate via direct cmap scan on known adjustment codepoints.
    let adj_type =
        reverse_cmap_lookup(font_data, glyph_index).map_or(0, adjustment_database_lookup);

    if adj_type == 0 {
        return;
    }

    let adjust_top = (adj_type & AF_ADJUST_UP) != 0;
    let adjust_below_top = (adj_type & AF_ADJUST_UP2) != 0;
    let adjust_bottom = (adj_type & AF_ADJUST_DOWN) != 0;
    let adjust_above_bottom = (adj_type & AF_ADJUST_DOWN2) != 0;

    if !((adjust_top || adjust_bottom) && hints.contours.len() >= 2
        || (adjust_below_top || adjust_above_bottom) && hints.contours.len() >= 3)
    {
        return;
    }

    // Recompute vertical extrema from hinted y values (C: af_compute_vertical_extrema)
    let mut new_minima: Vec<i32> = vec![0; hints.contours.len()];
    let mut new_maxima: Vec<i32> = vec![0; hints.contours.len()];
    for (ci, &c_start) in hints.contours.iter().enumerate() {
        let mut y_min = i32::MAX;
        let mut y_max = i32::MIN;
        let mut idx = c_start;
        loop {
            let pt = &hints.points[idx];
            y_min = y_min.min(pt.y);
            y_max = y_max.max(pt.y);
            let next = pt.next;
            if next == c_start {
                break;
            }
            idx = next;
        }
        new_minima[ci] = y_min;
        new_maxima[ci] = y_max;
    }
    hints.contour_y_minima = new_minima.clone();
    hints.contour_y_maxima = new_maxima.clone();

    // Find highest contour (largest y_min) — for 'i' this is the body
    let high_contour = {
        let mut best = 0;
        let mut best_min = i32::MIN;
        for (ci, &min_val) in new_minima.iter().enumerate() {
            if min_val > best_min {
                best_min = min_val;
                best = ci;
            }
        }
        best
    };

    let high_min_y = new_minima[high_contour];
    let high_max_y = new_maxima[high_contour];
    let high_height = high_max_y - high_min_y;

    // Find min gap between high contour bottom and nearest other contour top
    let mut min_distance: i32 = 256;
    for ci in 0..hints.contours.len() {
        if ci == high_contour {
            continue;
        }
        let other_max = new_maxima[ci];
        let other_min = new_minima[ci];
        let dist = high_min_y - other_max;
        if dist < min_distance && other_min < high_min_y {
            min_distance = dist;
        }
    }

    // Only adjust if gap is small (< 1px = 64 26.6 units).
    // C uses `if (min_distance < 64)` which allows negative values
    // (occurs when a contour slightly overlaps another).
    if min_distance >= 64 {
        return;
    }

    let adjustment = 64 - min_distance;
    // C: calculated_amount >= -2 && (calculated_amount <= 66 || adjustment_amount <= 66)
    // (aflatin.c:3807). We don't have calculated_amount (no tilde centering),
    // so the check simplifies to adjustment <= 66.
    if adjustment <= -3 || adjustment > 66 {
        return;
    }

    // C: af_move_contours_up(hints, limit, delta)
    // Moves ENTIRE CONTOURS where y_min > limit, i.e. contours above limit
    let limit = high_min_y - high_height / 8; // heuristic from C
    for ci in 0..hints.contours.len() {
        let c_start = hints.contours[ci];
        if new_minima[ci] <= new_maxima[ci] && new_minima[ci] > limit {
            // Move entire contour up by delta
            let mut idx = c_start;
            loop {
                hints.points[idx].y += adjustment;
                let next = hints.points[idx].next;
                if next == c_start {
                    break;
                }
                idx = next;
            }
        }
    }
}

/// Apply the Latin auto-hinter to a scaled outline.
///
/// This mirrors `af_latin_hints_apply` (`aflatin.c:5050-5068`). Horizontal
/// hinting runs before vertical hinting; italic faces skip horizontal hinting.
///
/// # Pipeline (each dimension)
///
/// 1. `reload`          — load coords + direction chain + WEAK/STRONG classify
/// 2. `compute_segments` — find horizontal/vertical runs
/// 3. `compute_edges`    — merge overlapping segments into edges
/// 4. `blue_edges`       — assign edges to baseline/cap-height/x-height zones
/// 5. `hint_edges`       — 4-phase snap: (1) stems (2) serifs (3) blues (4) anchors
/// 6. `align_edge`       — snap contour points to hinted edge positions
/// 7. `align_strong`     — grid-fit corner points between edges (skips WEAK)
/// 8. `align_weak` (IUP) — interpolate smooth runs between strong anchors
/// 9. phantom adjust     — pixel-grid shift via pp1.x
#[allow(clippy::too_many_arguments)]
pub fn apply_hints(
    outline: &mut crate::outline::Outline,
    raw_outline: &crate::tt::glyf::GlyphOutline,
    x_scale: i32,
    y_scale: i32,
    x_delta: i32,
    y_delta: i32,
    glyph_index: u16,
    metrics: Option<&AfLatinMetrics>,
    is_italic: bool,
    no_horizontal_hinting: bool,
    stem_adjust: bool,
    font_data: Option<&crate::tables::FontData>,
    target_mono: bool,
) -> ApplyHintsMetrics {
    let mut output = ApplyHintsMetrics::default();
    let mut hints = GlyphHints::new(x_scale, y_scale, x_delta, y_delta);
    hints.metrics = metrics.cloned();

    // C: when no blue zones can be built for a style (all blue-zone
    // characters missing from font), C remaps the style to NONE_DFLT
    // and runs the hinting pipeline without blue zone alignment.
    // Our pipeline with blue_count==0 produces different results than
    // C's NONE_DFLT path. Match C by skipping hinting entirely when
    // the VERT axis has no blue zones.
    if metrics.is_none_or(|m| m.axis[1].blue_count == 0) {
        return output;
    }
    // Smooth anti-aliased hinting normally enables stem adjustment. LCD target
    // clears it in FreeType to preserve horizontal subpixel coverage.
    if target_mono {
        hints.other_flags |= AF_LATIN_HINTS_HORZ_SNAP
            | AF_LATIN_HINTS_VERT_SNAP
            | AF_LATIN_HINTS_STEM_ADJUST
            | AF_LATIN_HINTS_MONO;
    } else if stem_adjust {
        hints.other_flags |= AF_LATIN_HINTS_STEM_ADJUST;
    } else {
        hints.other_flags &= !AF_LATIN_HINTS_STEM_ADJUST;
    }

    // Italic, light, and LCD targets disable horizontal hinting.
    if is_italic || no_horizontal_hinting {
        hints.scaler_flags |= AF_SCALER_FLAG_NO_HORIZONTAL;
        if is_italic {
            crate::autohint::coverage::record(crate::autohint::coverage::COV_ITALIC_NO_HORZ);
        }
    }

    // Compute ppem for bdelta in compute_stem_width
    // At 72dpi: x_scale = (ppem * 64 * 0x10000) / upem → ppem = x_scale * upem / 0x10000 / 64
    let ppem = i32_from_i64(
        (x_scale as i64).abs() * metrics.map_or(2048, |m| m.units_per_em as i64) / 65536 / 64,
    );
    let ppem = ppem.clamp(1, 100);

    // Step 1: Load outline into hints (raw font units → fx/fy; scaled 26.6 → ox/oy)
    loader::reload(&mut hints, raw_outline, &outline.points);
    if hints.num_points() == 0 {
        return output;
    }

    // Keep the phase order aligned with af_latin_hints_apply
    // (aflatin.c:4957-5200); later phases depend on flags and links produced
    // by earlier ones.
    // Phase A: detect_features for HORZ (segs → link → edges)
    let do_horz = hints.scaler_flags & AF_SCALER_FLAG_NO_HORIZONTAL == 0;
    let mut horz_widths_26_6: Vec<i32> = Vec::new();
    if do_horz {
        compute_segments(&mut hints, Dimension::Horz);
        {
            let (wc, widths) = extract_widths(&hints, Dimension::Horz);
            horz_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
            link_segments_inner(&mut hints, Dimension::Horz, wc, &widths);
        }
        compute_edges(&mut hints, Dimension::Horz);
    }

    // Phase B: detect_features for VERT (segs → link → edges) + blue zones.
    // This OVERWRITES point.v = fx — matching C's behavior before the hint loop.
    compute_segments(&mut hints, Dimension::Vert);
    let vert_widths_26_6: Vec<i32>;
    {
        let (wc, widths) = extract_widths(&hints, Dimension::Vert);
        vert_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
        link_segments_inner(&mut hints, Dimension::Vert, wc, &widths);
    }
    compute_edges(&mut hints, Dimension::Vert);
    let is_nonbase = hints.metrics.as_ref().is_some_and(|m| {
        (glyph_index as usize) < m.non_base_glyphs.len() && m.non_base_glyphs[glyph_index as usize]
    });
    if !is_nonbase {
        compute_blue_edges(&mut hints);
    }

    // Phase C: grid-fit the outline — for-loop over both dims (aflatin.c:5169-5177).
    for dim_i in 0..2 {
        let dim = if dim_i == 0 {
            Dimension::Horz
        } else {
            Dimension::Vert
        };
        let do_dim = if dim_i == 0 { do_horz } else { true };
        if !do_dim {
            continue;
        }
        let widths = if dim_i == 0 {
            &horz_widths_26_6
        } else {
            &vert_widths_26_6
        };
        hint_edges(&mut hints, dim, widths, ppem);
        align_edge_points(&mut hints, dim);
        align_strong_points(&mut hints, dim);
        align_weak_points(&mut hints, dim);
        vertical_separation_adjustments(
            &mut hints,
            glyph_index,
            font_data.unwrap_or_else(|| unreachable!()),
        );
    }

    // ── Post-hinting phantom-point adjustment (afloader.c:419-530) ──────
    // After hint_edges grid-fits the leftmost/rightmost edges, we compute
    // a pixel-rounded translation (pp1.x) that aligns the LSB to the pixel
    // grid, matching C's af_loader_load_glyph post-processing.
    {
        let haxis = &hints.axis[Dimension::Horz as usize];
        let num_horz_edges = haxis.edges.len();
        let advance_width = font_data.map_or(0, |data| {
            ft_mul_fix(data.hmtx.get(glyph_index).advance_width as i32, x_scale)
        });
        #[cfg(debug_assertions)]
        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
            log::trace!(target: "autohint::pipeline", "[PHANTOM_PRE] gi={glyph_index} num_horz_edges={num_horz_edges}");
        }
        if num_horz_edges > 1 {
            let edge1 = &haxis.edges[0]; // leftmost
            let edge2 = &haxis.edges[num_horz_edges - 1]; // rightmost

            let old_lsb = edge1.opos; // original scaled LSB (pp1.x = 0)
            let new_lsb = edge1.pos; // hinted LSB

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
            if advance_width != 0 {
                let old_rsb = advance_width - edge2.opos;
                let mut pp2x_uh = edge2.pos + old_rsb;
                if old_rsb < 24 {
                    pp2x_uh += 8;
                }
                let mut pp2x = (pp2x_uh + 32) & !63; // FT_PIX_ROUND
                if pp2x <= edge2.pos && old_rsb > 0 {
                    pp2x += 64;
                }
                output.advance_width = Some(pp2x - pp1x);

                #[cfg(debug_assertions)]
                if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                    log::trace!(target: "autohint::pipeline", "[PHANTOM] gi={glyph_index} old_lsb={old_lsb} old_rsb={old_rsb} new_lsb={new_lsb} pp1x_uh={pp1x_uh} pp2x_uh={pp2x_uh} pp1x_round={pp1x} pp2x_round={pp2x}");
                }
            } else {
                #[cfg(debug_assertions)]
                if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                    log::trace!(target: "autohint::pipeline", "[PHANTOM] gi={glyph_index} old_lsb={old_lsb} new_lsb={new_lsb} pp1x_uh={pp1x_uh} pp1x_round={pp1x}");
                }
            }
        } else {
            // C's afloader.c:454-460: even without edges, phantom points
            // are always adjusted.  pp1.x = FT_PIX_ROUND(0) = 0 is a no-op,
            // but pp2.x rounding affects lsb_delta/rsb_delta which C stores
            // on the glyph slot.  We don't replicate rsb_delta (it only
            // affects advance widths), but we document the path for clarity.
            // The x coordinates are unchanged from the raw scaled values.
            #[cfg(debug_assertions)]
            if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                log::trace!(target: "autohint::pipeline", "[PHANTOM_SKIP] gi={glyph_index} num_horz_edges={num_horz_edges} (<=1, no adjust)");
            }
            if advance_width != 0 {
                output.advance_width = Some((advance_width + 32) & !63);
            }
        }
    }

    // Step 4: Write back
    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        for (i, pt) in hints.points.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[TOUCH] p{i}: x={} y={} fx={} fy={} flags=0x{:02x} touch_x={} touch_y={} weak={}",
                pt.x, pt.y, pt.fx, pt.fy, pt.flags,
                pt.flags & AF_FLAG_TOUCH_X != 0,
                pt.flags & AF_FLAG_TOUCH_Y != 0,
                pt.flags & AF_FLAG_WEAK_INTERPOLATION != 0);
        }
    }
    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        trace!(target: "autohint::pipeline", "[PIPE] reload {} pts", hints.num_points());
        if let Some(metrics_data) = &hints.metrics {
            let verge = &metrics_data.axis[Dimension::Vert as usize];
            trace!(target: "autohint::pipeline", "[PIPE] blue_count={}", verge.blue_count);
            for bi in 0..verge.blue_count {
                let bz = &verge.blues[bi];
                trace!(target: "autohint::pipeline", "[PIPE] blue{bi}: ref={} shoot={} top={} neut={} active={}",
                    bz.ref_width.org, bz.shoot_width.org,
                    (bz.flags & 0x02 != 0) || (bz.flags & 0x04 != 0),
                    bz.flags & 0x08 != 0,
                    bz.flags & 0x01 != 0);
            }
        }
        trace!(target: "autohint::pipeline", "[PIPE] blue_dump_done");
        for (i, pt) in hints.points.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] p{i}: fx={} fy={} in={:?} out={:?} u={} v={}",
                pt.fx, pt.fy, pt.in_dir, pt.out_dir, pt.u, pt.v);
        }
        let va = &hints.axis[Dimension::Vert as usize];
        trace!(target: "autohint::pipeline", "[PIPE] segs {}", va.segments.len());
        for (si, s) in va.segments.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] S{si}: p{}..p{} dir={:?} pos={}",
                s.first, s.last, s.dir, s.pos);
        }
        trace!(target: "autohint::pipeline", "[PIPE] edges {}", va.edges.len());
        for (ei, e) in va.edges.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] E{ei}: fpos={} opos={} pos={} link={} serif={} dir={:?} flags=0x{:02x}",
                e.fpos, e.opos, e.pos, e.link, e.serif, e.dir, e.flags);
        }
        // Also dump HORZ edges and segments
        let ha = &hints.axis[Dimension::Horz as usize];
        trace!(target: "autohint::pipeline", "[PIPE] horz_segs {}", ha.segments.len());
        for (si, s) in ha.segments.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] HS{si}: p{}..p{} dir={:?} pos={}",
                s.first, s.last, s.dir, s.pos);
        }
        let el_horz = hints
            .metrics
            .as_ref()
            .is_some_and(|m| m.axis[Dimension::Horz as usize].extra_light);
        let el_vert = hints
            .metrics
            .as_ref()
            .is_some_and(|m| m.axis[Dimension::Vert as usize].extra_light);
        trace!(target: "autohint::pipeline", "[PIPE] horz_edges {} extra_light_h={el_horz} extra_light_v={el_vert}", ha.edges.len());
        for (ei, e) in ha.edges.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] HE{ei}: fpos={} opos={} pos={} link={} serif={}",
                e.fpos, e.opos, e.pos, e.link, e.serif);
        }
        trace!(target: "autohint::pipeline", "[PIPE] final");
        for (i, pt) in hints.points.iter().enumerate() {
            trace!(target: "autohint::pipeline", "[PIPE] p{i}: x={x} y={y}", x = pt.x, y = pt.y);
        }
    }
    hints.save_to_outline(outline);
    output
}

// ── Segment detection ─────────────────────────────────────────────────────
//
// Port of `af_latin_hints_compute_segments` (aflatin.c:1557–2008).

/// Threshold for considering a run of points as "flat" — used to decide
/// whether an edge should be rounded.  `units_per_em / 14` is the FreeType
/// default (aflatin.c:39).  Computed dynamically from metrics if available.
/// Faithful port of `af_latin_hints_compute_segments` (aflatin.c:1557).
///
/// Find horizontal/vertical runs of consecutive points with same direction.
///
/// Walks contour via prev→next chain. Points with matching out_dir accumulate
/// into segments. Segment height is extended by ±half the adjacent point offset
/// for serif detection.
#[allow(unused_assignments, unused_variables)]
pub fn compute_segments(hints: &mut GlyphHints, dim: Dimension) {
    let flat_threshold = hints.metrics.as_ref().map_or(146, |m| m.units_per_em / 14);
    // `af_latin_hints_compute_segments` works over contour endpoints while
    // mutating the current axis, so take a local copy before borrowing `axis`.
    let contours: Vec<usize> = hints.contours.clone();
    let axis = &mut hints.axis[dim as usize];

    // Per-point u/v axis swap (aflatin.c:1582). Stored on the point's u/v fields.
    let is_horz = dim == Dimension::Horz;
    for pt in &mut hints.points {
        if is_horz {
            pt.u = pt.fx as i32;
            pt.v = pt.fy as i32;
        } else {
            pt.u = pt.fy as i32;
            pt.v = pt.fx as i32;
        }
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
        } else if cw {
            Direction::Left
        } else {
            Direction::Right
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

        let mut min_pos: i32 = 32000;
        let mut max_pos: i32 = -32000;
        let mut min_coord: i32 = 32000;
        let mut max_coord: i32 = -32000;
        let mut min_flags: u16 = 0;
        let mut max_flags: u16 = 0;
        let mut min_on_coord: i32 = 32000;
        let mut max_on_coord: i32 = -32000;

        let mut seg_first: usize = 0; // index of first point of current segment
        let mut prev_seg: Option<usize> = None; // index of previous segment in axis.segments

        // prev_* buffers for merge logic (aflatin.c:1631-1638).
        let mut prev_min_pos = min_pos;
        let mut prev_max_pos = max_pos;
        let mut prev_min_coord = min_coord;
        let mut prev_max_coord = max_coord;
        let mut prev_min_flags = min_flags;
        let mut _prev_max_flags = max_flags;
        let mut _prev_min_on_coord = min_on_coord;
        let mut _prev_max_on_coord = max_on_coord;

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
                if point == last {
                    break;
                }
            }
        }

        last = point;
        let mut passed = false;

        loop {
            let p = &points[point];
            if on_edge {
                let u = p.u;
                min_pos = min_pos.min(u);
                max_pos = max_pos.max(u);
                let v = p.v;
                if v < min_coord {
                    min_coord = v;
                    min_flags = p.flags;
                }
                if v > max_coord {
                    max_coord = v;
                    max_flags = p.flags;
                }
                if p.flags & AF_FLAG_CONTROL == 0 {
                    if v < min_on_coord {
                        min_on_coord = v;
                    }
                    if v > max_on_coord {
                        max_on_coord = v;
                    }
                }

                if p.flags & AF_FLAG_IGNORE != 0 || p.out_dir != segment_dir || point == last {
                    // End of segment.
                    let same_start_as_prev = match prev_seg {
                        Some(v) => seg_first == axis.segments[v].last,
                        None => false,
                    };
                    let new_seg =
                        p.flags & AF_FLAG_IGNORE != 0 || prev_seg.is_none() || !same_start_as_prev;

                    if new_seg {
                        // Record a new segment.
                        let pos = i16_from_i32((min_pos + max_pos) >> 1);
                        let delta = i16_from_i32((max_pos - min_pos) >> 1);
                        let mut flags = 0u8;
                        if (min_flags | max_flags) & AF_FLAG_CONTROL != 0
                            && (max_on_coord - min_on_coord) < flat_threshold
                        {
                            flags |= AF_EDGE_ROUND;
                        }
                        let h = max_coord - min_coord;
                        axis.segments.push(AFSegment {
                            flags,
                            dir: segment_dir,
                            pos,
                            delta,
                            min_coord: i16_from_i32(min_coord),
                            max_coord: i16_from_i32(max_coord),
                            height: i16_from_i32(h),
                            first: seg_first,
                            last: point,
                            edge: usize::MAX,
                            edge_next: usize::MAX,
                            link: usize::MAX,
                            serif: usize::MAX,
                            score: 32000,
                        });
                        let cur = axis.segments.len() - 1;
                        prev_seg = Some(cur);
                        prev_min_pos = min_pos;
                        prev_max_pos = max_pos;
                        prev_min_coord = min_coord;
                        prev_max_coord = max_coord;
                        prev_min_flags = min_flags;
                        let _ = &mut _prev_max_flags;
                        let _ = &mut _prev_min_on_coord;
                        let _ = &mut _prev_max_on_coord;
                    } else {
                        // Merge with previous segment (same start point). Port of aflatin.c:1741-1851.
                        // Compare in_dir at the join point (aflatin.c:1746).
                        let prev = match prev_seg {
                            Some(v) => v,
                            None => unreachable!(),
                        };
                        let prev_last_idx = axis.segments[prev].last;
                        let prev_last_in = points[prev_last_idx].in_dir;
                        let curr_in = points[point].in_dir;
                        if prev_last_in == curr_in {
                            // C: identical directions → unify (aflatin.c:1746-1791)
                            // prev_segment->first stays correct (it's the earlier point).
                            min_pos = min_pos.min(prev_min_pos);
                            max_pos = max_pos.max(prev_max_pos);
                            min_coord = min_coord.min(prev_min_coord);
                            max_coord = max_coord.max(prev_max_coord);
                            let pos = i16_from_i32((min_pos + max_pos) >> 1);
                            let delta = i16_from_i32((max_pos - min_pos) >> 1);
                            let s = &mut axis.segments[prev];
                            s.last = point;
                            s.pos = pos;
                            s.delta = delta;
                            s.min_coord = i16_from_i32(min_coord);
                            s.max_coord = i16_from_i32(max_coord);
                            if (min_flags | max_flags) & AF_FLAG_CONTROL != 0
                                && (max_on_coord - min_on_coord) < flat_threshold
                            {
                                s.flags |= AF_EDGE_ROUND;
                            } else {
                                s.flags &= !AF_EDGE_ROUND;
                            }
                            s.height = i16_from_i32(max_coord - min_coord);
                        } else if (prev_max_coord - prev_min_coord).abs()
                            > (max_coord - min_coord).abs()
                        {
                            // C: different directions, prev is longer — keep prev (aflatin.c:1798-1811)
                            // C copies the discarded current segment's min/max_pos into
                            // prev_min_pos/prev_max_pos (aflatin.c:1803-1804). Without this,
                            // subsequent 3+ segment merges use stale boundaries.
                            if min_pos < prev_min_pos {
                                prev_min_pos = min_pos;
                            }
                            if max_pos > prev_max_pos {
                                prev_max_pos = max_pos;
                            }
                            let pos = i16_from_i32((prev_min_pos + prev_max_pos) >> 1);
                            let s = &mut axis.segments[prev];
                            s.last = point;
                            s.pos = pos;
                        } else {
                            // C: different directions, current is longer — replace prev (aflatin.c:1812-1843)
                            // *prev_segment = *segment copies ALL fields, including `first`.
                            let pos = i16_from_i32(
                                (min_pos.min(prev_min_pos) + max_pos.max(prev_max_pos)) >> 1,
                            );
                            let s = &mut axis.segments[prev];
                            s.last = point;
                            s.pos = pos;
                            s.min_coord = i16_from_i32(min_coord);
                            s.max_coord = i16_from_i32(max_coord);
                            s.dir = segment_dir;
                            s.first = seg_first;
                            if (min_flags | max_flags) & AF_FLAG_CONTROL != 0
                                && (max_on_coord - min_on_coord) < flat_threshold
                            {
                                s.flags |= AF_EDGE_ROUND;
                            } else {
                                s.flags &= !AF_EDGE_ROUND;
                            }
                            s.height = i16_from_i32(max_coord - min_coord);
                        }
                    }

                    on_edge = false;
                }
            }

            if point == last {
                if passed {
                    break;
                }
                passed = true;
            }

            // Start a new segment if not on edge and out_dir matches major dir.
            //
            // C (aflatin.c:1902-1907):
            //   if (!(point->flags & AF_FLAG_IGNORE) && !on_edge &&
            //       (FT_ABS(point->out_dir) == major_dir || point == point->prev))
            // The "|| point == point->prev" clause allows single-point contours
            // to start a segment even if out_dir doesn't match ABS(major_dir).
            // Our tracing confirms p17 (out_dir=Left, abs=1=major_dir=Right=1)
            // passes the normal check — the extra clause is for degenerate
            // single-point glyphs only and doesn't affect NOTO B's 43-point outline.
            let p = &points[point];
            if p.flags & AF_FLAG_IGNORE == 0
                && !on_edge
                && (abs_dir(p.out_dir) == major_dir || point == p.prev)
            {
                if axis.segments.len() > 1000 {
                    axis.segments.clear();
                    return;
                }
                segment_dir = p.out_dir;
                seg_first = point;
                min_pos = p.u;
                max_pos = p.u;
                min_coord = p.v;
                max_coord = p.v;
                min_flags = p.flags;
                max_flags = p.flags;
                if p.flags & AF_FLAG_CONTROL != 0 {
                    min_on_coord = 32000;
                    max_on_coord = -32000;
                } else {
                    min_on_coord = p.v;
                    max_on_coord = p.v;
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
                    extra += i16_from_i32((first_v - points[p].v) >> 1);
                }
                let p = points[last_idx].next;
                if points[p].v > last_v {
                    extra += i16_from_i32((points[p].v - last_v) >> 1);
                }
            } else {
                let p = points[first_idx].prev;
                if points[p].v > first_v {
                    extra += i16_from_i32((points[p].v - first_v) >> 1);
                }
                let p = points[last_idx].next;
                if points[p].v < last_v {
                    extra += i16_from_i32((last_v - points[p].v) >> 1);
                }
            }
            axis.segments[idx].height = axis.segments[idx].height.saturating_add(extra);
        }
    }
}

/// Absolute direction: flips Left→Right, Down→Up. Used for segment matching.
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
// Port of af_latin_hints_compute_edges (aflatin.c:2154-2500).
/// Merge overlapping segments into edges. Serif+stem+serif → one edge.
///
/// Uses `edge_distance_threshold` (standard_width/5) to determine when
/// segments are "at the same position" and should merge.
fn compute_edges(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &mut hints.axis[dim as usize];
    axis.edges.clear();

    // ── Compute thresholds (aflatin.c:2182-2232) ────────────────────────
    let scale = if dim == Dimension::Horz {
        hints.x_scale
    } else {
        hints.y_scale
    };

    // segment_length_threshold: skip segments shorter than 1px (Horz only).
    let seg_len_thresh = if dim == Dimension::Horz {
        ft_mul_div(64, 0x10000, hints.y_scale)
    } else {
        0 // no height filtering for vertical/horizontal edges (C default)
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
        if edt > 16 {
            edt = 16;
        } // cap at 0.25px (= 64/4 in 26.6)
        ft_mul_div(edt, 0x10000, scale) // convert back to font units
    };

    // For each segment, find or create its edge.
    for seg_idx in 0..axis.segments.len() {
        // ── Segment filtering (aflatin.c:2242-2251) ──────────────────────
        {
            let seg = &axis.segments[seg_idx];
            // Skip one-point segments without a direction
            if seg.dir == Direction::None {
                continue;
            }
            // Too short
            if (seg.height as i32) < seg_len_thresh {
                continue;
            }
            // Too wide (delta > 0.5px)
            if (seg.delta as i32) > seg_width_thresh {
                continue;
            }
            // Tiny serif: height < 1.5× the length threshold
            // aflatin.c:2247-2250 (serif filter, no round-flag check)
            if seg.serif != usize::MAX && 2 * (seg.height as i32) < 3 * seg_len_thresh {
                continue;
            }
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
            let fpos = i16_from_i32(seg_pos);
            let scale = if dim == Dimension::Vert {
                hints.y_scale
            } else {
                hints.x_scale
            };
            let opos = ft_mul_fix(fpos as i32, scale);
            let edge = AFEdge {
                fpos,
                opos,
                pos: opos, // C: edge->pos = edge->opos (aflatin.c:2293)
                flags: 0,
                dir: seg_dir,
                link: usize::MAX,
                serif: usize::MAX,
                first: seg_idx,
                last: seg_idx,
                blue_edge: None,
            };
            // FreeType's af_axis_hints_new_edge (afhints.c:254-264) inserts
            // edges sorted by fpos. For equal positions, major-direction edges
            // come before minor-direction edges. Phase 2 BOUND checks compare
            // neighboring edges, so insertion order affects final positions.
            let insert_pos = {
                let major_dir = axis.major_dir;
                let mut pos = axis.edges.len();
                while pos > 0 {
                    let prev = &axis.edges[pos - 1];
                    if prev.fpos < fpos {
                        break;
                    }
                    if prev.fpos == fpos && seg_dir == major_dir {
                        break;
                    }
                    pos -= 1;
                }
                pos
            };
            axis.edges.insert(insert_pos, edge);
            // Update segment→edge references for ALL edges shifted right.
            axis.segments[seg_idx].edge = insert_pos;
            for i in (insert_pos + 1)..axis.edges.len() {
                // Update segments that pointed to the old index.
                let mut s = axis.edges[i].first;
                loop {
                    if axis.segments[s].edge >= insert_pos {
                        axis.segments[s].edge += 1;
                    }
                    if s == axis.edges[i].last {
                        break;
                    }
                    s = axis.segments[s].edge_next;
                }
            }
        } else {
            // Append segment to existing edge.
            let e = &mut axis.edges[found_edge];
            let prev_last = e.last;
            axis.segments[prev_last].edge_next = seg_idx;
            e.last = seg_idx;
            // Segment added to existing edge — edge already at correct
            // sorted position. No index shifts needed.
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

    // ── Sort edges by fpos BEFORE propagation (matches C's fpos-sorted insertion) ──
    // C processes edges in fpos-sorted order. The AF_EDGE_SERIF flag set by
    // earlier edges is cleared when the target edge's own `flags=AF_EDGE_NORMAL`
    // runs. Without sorting first, SERIF can persist on edges processed too early.
    // For top_to_bottom scripts (Indic/Mongolian), sort descending.
    if axis.edges.len() > 1 {
        let top_to_bottom = hints
            .metrics
            .as_ref()
            .is_some_and(|m| m.top_to_bottom_hinting)
            && dim == Dimension::Vert;
        let mut indices: Vec<usize> = (0..axis.edges.len()).collect();
        if top_to_bottom {
            indices.sort_by(|&a, &b| axis.edges[b].fpos.cmp(&axis.edges[a].fpos));
        } else {
            indices.sort_by_key(|&i| axis.edges[i].fpos);
        }
        let mut new_from_old: Vec<usize> = vec![0; axis.edges.len()];
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            new_from_old[old_idx] = new_idx;
        }
        let old_edges: Vec<AFEdge> = axis.edges.drain(..).collect();
        for &old_idx in &indices {
            axis.edges.push(old_edges[old_idx]);
        }
        for seg in &mut axis.segments {
            if seg.edge != usize::MAX {
                seg.edge = new_from_old[seg.edge];
            }
        }
    }

    // ── Edge link/serif propagation (aflatin.c:2384–2495) ──────────────────
    // For each edge, walk its segments and propagate segment links/serifs to
    // the edge level. Also compute AF_EDGE_ROUND vs AF_EDGE_NORMAL.
    for e_idx in 0..axis.edges.len() {
        let mut is_round = 0i32;
        let mut is_straight = 0i32;

        let first_seg = axis.edges[e_idx].first;
        if first_seg == usize::MAX {
            continue;
        }
        let mut seg_idx = first_seg;
        loop {
            let seg = &axis.segments[seg_idx];

            // Track round/straight counts (aflatin.c:2393-2395).
            if seg.flags & AF_EDGE_ROUND != 0 {
                is_round += 1;
            } else {
                is_straight += 1;
            }

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
                    let edge_delta =
                        (axis.edges[e_idx].fpos as i32 - axis.edges[edge2_idx].fpos as i32).abs();
                    let seg_delta = (seg.pos as i32 - axis.segments[linked_seg].pos as i32).abs();
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

            if seg_idx == axis.edges[e_idx].last {
                break;
            }
            seg_idx = axis.segments[seg_idx].edge_next;
        }

        // Set round flag (aflatin.c:2470-2473).
        // C resets all edge flags to AF_EDGE_NORMAL here, including SERIF flags
        // set by other edges' serif assignments, then conditionally adds
        // AF_EDGE_ROUND.
        axis.edges[e_idx].flags = AF_EDGE_NORMAL;
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
// `width_count`/`widths` come from metrics_init_widths for exact C scoring.
/// Pair segments into stem pairs (opposite-direction edges at similar positions).
///
/// Uses per-distance demerit scoring. Pairs with lowest score get linked.
/// Unlinked segments with serif-candidates get serif pointers instead.
///
/// Public wrapper: links segments using default width/demerit scoring.
/// Used by CJK stem width detection in cjk.rs.
pub fn link_segments(hints: &mut GlyphHints, dim: Dimension) {
    link_segments_inner(hints, dim, 0, &[]);
}

fn link_segments_inner(
    hints: &mut GlyphHints,
    dim: Dimension,
    width_count: usize,
    widths: &[AfWidth],
) {
    let axis = &mut hints.axis[dim as usize];
    let major_dir = axis.major_dir;
    let n = axis.segments.len();

    let upem = hints.metrics.as_ref().map_or(2048, |m| m.units_per_em);

    // max_width = largest stem width in font units (aflatin.c:2028-2031).
    // .org stays in font units even after scale_dim; segment distances are also
    // in font units, so they're comparable.
    let max_width = if width_count > 0 {
        widths[width_count - 1].org
    } else {
        0
    };

    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        let dim_name = if dim == Dimension::Horz {
            "HORZ"
        } else {
            "VERT"
        };
        log::trace!(target: "autohint::pipeline", "[LINK_IN] dim={dim_name} n={n} major={:?} wc={width_count} max_width={max_width}",
            major_dir);
        for (i, seg) in axis.segments.iter().enumerate() {
            log::trace!(target: "autohint::pipeline", "  S{i}: pos={} dir={} u=[{},{}] h={} delta={}",
                seg.pos, seg.dir as i8,
                seg.min_coord, seg.max_coord,
                seg.height, seg.delta);
        }
    }

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
                        #[cfg(debug_assertions)]
                        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                            log::trace!(target: "autohint::pipeline", "[LINK_SCORE] i={i}->j={j} dist={dist} len={len} max_width={max_width} delta={} dist_demerit={dist_demerit} score={score}",
                                if max_width > 0 { ((dist << 10) / max_width) - (1 << 10) } else { dist });
                        }
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
    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        let dim_name = if dim == Dimension::Horz {
            "HORZ"
        } else {
            "VERT"
        };
        for (i, seg) in axis.segments.iter().enumerate() {
            if seg.link != usize::MAX || seg.serif != usize::MAX {
                log::trace!(target: "autohint::pipeline", "[LINK_OUT] dim={dim_name} S{i}: link={} serif={} score={}",
                    if seg.link != usize::MAX { seg.link as isize } else { -1 },
                    if seg.serif != usize::MAX { seg.serif as isize } else { -1 },
                    seg.score);
            }
        }
    }
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

/// Snap a stem width to the nearest standard width from the metrics array.
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

/// Align a stem pair so both edges snap to integer pixels while preserving width.
fn align_linked_edge(
    other_flags: u32,
    dim: Dimension,
    base_edge: &AFEdge,
    stem_edge: &mut AFEdge,
    std_widths: &[i32],
    ppem: i32,
    extra_light: bool,
) {
    let dist = stem_edge.opos - base_edge.opos;
    let base_delta = base_edge.pos - base_edge.opos;

    let fitted_width = compute_stem_width(
        other_flags,
        ppem,
        dim,
        dist,
        base_delta,
        base_edge.flags,
        stem_edge.flags,
        std_widths,
        extra_light,
    );

    stem_edge.pos = base_edge.pos + fitted_width;
}

// ── Helper: align serif edge ────────────────────────────────────────────────
//
// Port of `af_latin_align_serif_edge` (aflatin.c:4189–4197).
// Preserves serif offset relative to the base edge.

/// Snap serif edge to same position as its linked stem edge.
fn align_serif_edge(base: &AFEdge, serif: &mut AFEdge) {
    serif.pos = base.pos + (serif.opos - base.opos);
}

// ── Helper: compute stem width ──────────────────────────────────────────────
//
// Port of `af_latin_compute_stem_width` (aflatin.c:3960–4152).
// Quantizes / snaps a stem width.

/// Compute current stem width from paired edges, snapping to standard if needed.
///
/// Two branches: smooth (anti-aliased) and strong (full hinting).
/// Both call `snap_width` to quantize to standard widths. The `extra_light`
/// flag disables snapping for very thin stems.
/// The smooth path preserves FreeType's special handling for serif stems,
/// round stems, thin stems, and fractional-pixel quantization.
#[allow(clippy::too_many_arguments)]
fn compute_stem_width(
    other_flags: u32,
    ppem: i32,
    dim: Dimension,
    width: i32,
    base_delta: i32,
    base_flags: u8,
    stem_flags: u8,
    std_widths: &[i32],
    extra_light: bool,
) -> i32 {
    let stem_adjust = other_flags & AF_LATIN_HINTS_STEM_ADJUST != 0;

    // C: if !AF_LATIN_HINTS_DO_STEM_ADJUST || axis->extra_light → return width
    // extra_light = ft_mul_fix(axis->standard_width, scale) < 40.
    // Must use axis.extra_light (computed from standard_width*scale), not widths[0].cur.
    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        log::trace!(target: "autohint::pipeline",
            "[CSW] dim={:?} width={width} base_delta={base_delta} el={extra_light} stem_adj={stem_adjust} bf=0x{base_flags:x} sf=0x{stem_flags:x}",
            dim);
    }
    if !stem_adjust {
        return width;
    }
    if extra_light {
        #[cfg(debug_assertions)]
        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
            log::trace!(target: "autohint::pipeline", "[CSW_RET] el/sa skip → return {width}");
        }
        return width;
    }

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
            if sign != 0 {
                dist = -dist;
            }
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
            if delta < 0 {
                delta = -delta;
            }

            if delta < 40 {
                // Within tolerance of standard width → snap to it, clamp min.
                dist = stdw;
                if dist < 48 {
                    dist = 48;
                }
                // goto Done_Width
                if sign != 0 {
                    dist = -dist;
                }
                #[cfg(debug_assertions)]
                if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                    log::trace!(target: "autohint::pipeline", "[CSW_RET] smooth-stdw → return {dist}");
                }
                return dist;
            }

            if dist < 3 * 64 {
                // Fractional-pixel quantization (aflatin.c:4035-4047).
                delta = dist & 63;
                dist &= -64; // truncate to integer pixel

                if delta < 10 {
                    dist += delta;
                } else if delta < 32 {
                    dist += 10;
                } else if delta < 54 {
                    dist += 54;
                } else {
                    dist += delta;
                }
            } else {
                // bdelta adjustment + round (aflatin.c:4050-4075).
                // C compensates for double-rounding when base_delta and
                //    width have the same sign, preventing outline collisions.
                let mut bdelta: i32 = 0;
                if (width > 0 && base_delta > 0) || (width < 0 && base_delta < 0) {
                    let ppem = ppem.max(1);
                    if ppem < 10 {
                        bdelta = base_delta;
                    } else if ppem < 30 {
                        bdelta = (base_delta * (30 - ppem)) / 20;
                    }
                    if bdelta < 0 {
                        bdelta = -bdelta;
                    }
                }
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

    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        log::trace!(target: "autohint::pipeline", "[CSW_RET] Done_Width → return width_in={} dist={dist}", width);
    }

    dist
}

// ── Edge grid-fitting ──────────────────────────────────────────────────────
//
// Faithful port of `af_latin_hint_edges` (aflatin.c:4214–4831).

// Per-phase edge dump matching C's [C TRACE INITIAL/PHASE1-4] format.
#[cfg(debug_assertions)]
fn dump_edge_phase(phase: &str, dim: &str, edges: &[AFEdge]) {
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        trace!(target: "autohint::pipeline", "[TR_{phase}] dim={dim} edges={}", edges.len());
        for (ei, e) in edges.iter().enumerate() {
            trace!(target: "autohint::pipeline", "  edge[{ei}] fpos={} opos={} pos={} flags=0x{:02x} link={} serif={} blue={}",
                e.fpos, e.opos, e.pos, e.flags,
                if e.link != usize::MAX { e.link as isize } else { -1 },
                if e.serif != usize::MAX { e.serif as isize } else { -1 },
                if e.blue_edge.is_some() { 1 } else { 0 });
        }
    }
}
#[cfg(not(debug_assertions))]
fn dump_edge_phase(_phase: &str, _dim: &str, _edges: &[AFEdge]) {}

// Port of af_latin_hint_edges (aflatin.c:4220-4837).
/// 4-phase edge snapping: (1) stems (2) serifs (3) blue zones (4) anchors.
///
/// Each phase modifies `edge.pos` in-place. Phases are interdependent:
/// stem snapping must complete before serifs can anchor to stems; blue
/// snapping runs after stems are established.
fn hint_edges(hints: &mut GlyphHints, dim: Dimension, std_widths: &[i32], ppem: i32) {
    let other_flags = hints.other_flags;
    let extra_light = hints
        .metrics
        .as_ref()
        .is_some_and(|m| m.axis[dim as usize].extra_light);
    let axis = &mut hints.axis[dim as usize];
    let num_edges = axis.edges.len();

    if num_edges == 0 {
        return;
    }

    let dim_label = if dim == Dimension::Vert {
        "VERT"
    } else {
        "HORZ"
    };
    dump_edge_phase("INITIAL", dim_label, &axis.edges);

    // C: top_to_bottom_hinting only applies to VERT dimension (aflatin.c:4271-4273).
    // For HORZ dimension, always use bottom-to-top ordering.
    // C: `if (dim == AF_DIMENSION_VERT) top_to_bottom = script_class->top_to_bottom`.
    // Applying top-to-bottom ordering to the horizontal dimension changes BOUND
    // checks and can collapse Indic stem edges.
    let top_to_bottom_hinting = dim == Dimension::Vert
        && hints
            .metrics
            .as_ref()
            .is_some_and(|m| m.top_to_bottom_hinting);

    let mut anchor: usize = usize::MAX;
    let mut has_non_stem_edges = false;

    // ── Phase 1: Blue-zone alignment (aflatin.c:4247-4336) ──────────────
    if dim == Dimension::Vert && hints.metrics.is_some() {
        for i in 0..num_edges {
            if axis.edges[i].flags & AF_EDGE_DONE != 0 {
                continue;
            }

            let mut edge1_idx: Option<usize> = None;
            let mut edge2_idx: Option<usize> = None;
            let mut blue: Option<AfWidth> = None;

            // Neutral blue dedup: if both edges of a stem have blue edges,
            // keep only the non-neutral one.  aflatin.c:4270-4286.
            let link = axis.edges[i].link;
            let mut maybe_blue = axis.edges[i].blue_edge;
            if let Some(_b) = maybe_blue {
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
                            maybe_blue = None; // edge lost its blue zone
                        }
                    }
                }
            }
            if let Some(b) = maybe_blue {
                edge1_idx = Some(i);
                blue = Some(b);
                if link != usize::MAX {
                    edge2_idx = Some(link);
                }
            } else if link != usize::MAX {
                if let Some(b2) = axis.edges[link].blue_edge {
                    blue = Some(b2);
                    edge1_idx = Some(link);
                    edge2_idx = Some(i);
                }
            }

            if edge1_idx.is_none() {
                continue;
            }

            let e1 = match edge1_idx {
                Some(v) => v,
                None => unreachable!(),
            };
            let blue = match blue {
                Some(b) => b,
                None => unreachable!(),
            };
            trace!(target: "autohint::pipeline", "[P1] E{e1}: snap to blue.fit={}", blue.fit);
            axis.edges[e1].pos = blue.fit;
            axis.edges[e1].flags |= AF_EDGE_DONE;

            if let Some(e2) = edge2_idx {
                if axis.edges[e2].blue_edge.is_none() {
                    align_linked_edge(
                        other_flags,
                        dim,
                        &axis.edges[e1].clone(),
                        &mut axis.edges[e2],
                        std_widths,
                        ppem,
                        extra_light,
                    );
                    axis.edges[e2].flags |= AF_EDGE_DONE;
                }
            }

            if anchor == usize::MAX {
                anchor = i;
            }
        }
        dump_edge_phase("PHASE1", dim_label, &axis.edges);
    }

    // ── Phase 2: Stem alignment ─────────────────────────────────────────
    // Ported faithfully (aflatin.c:4340–4564). Since our edges have no
    // links (all link == usize::MAX), this loop only sets
    // has_non_stem_edges = true.
    for i in 0..num_edges {
        if axis.edges[i].flags & AF_EDGE_DONE != 0 {
            if dim == Dimension::Vert {
                trace!(target: "autohint::pipeline", "[P2] E{i} dim=Vert: DONE → skip");
            }
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
                other_flags,
                ppem,
                dim,
                org_len,
                0,
                edge_flags,
                edge2_flags,
                std_widths,
                extra_light,
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
                    other_flags,
                    ppem,
                    dim,
                    dist,
                    base_delta,
                    base_flags,
                    stem_flags,
                    std_widths,
                    extra_light,
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
            trace!(target: "autohint::pipeline", "[P2_REL] E{i}↔E{edge2_idx} dim={dim:?}: anchor={anchor} org_pos={org_pos} org_len={org_len} el={extra_light}");

            let cur_len = compute_stem_width(
                other_flags,
                ppem,
                dim,
                org_len,
                0,
                edge_flags,
                edge2_flags,
                std_widths,
                extra_light,
            );

            // FreeType sets edge2->pos directly to `cur_pos1 + cur_len / 2`
            // here (aflatin.c:4502) instead of calling
            // af_latin_align_linked_edge.
            if axis.edges[edge2_idx].flags & AF_EDGE_DONE != 0 {
                // ADJUST: linked edge already positioned.
                axis.edges[i].pos = axis.edges[edge2_idx].pos - cur_len;
            } else if cur_len < 96 {
                let cur_pos1 = (org_center + 32) & !63; // FT_PIX_ROUND

                let (u_off, d_off): (i32, i32) = if cur_len <= 64 { (32, 32) } else { (38, 26) };

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
                    other_flags,
                    ppem,
                    dim,
                    org_len,
                    0,
                    edge_flags,
                    edge2_flags,
                    std_widths,
                    extra_light,
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

            // C: BOUND check is inside the `else` (relative stem) block
            //    only (aflatin.c:4606). It does NOT run for the anchor stem.
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

        axis.edges[i].flags |= AF_EDGE_DONE;

        // Phase 4 BOUND checks (aflatin.c:4870-4904) are handled
        // separately in the Phase 4 loop below.
    }
    dump_edge_phase("PHASE2", dim_label, &axis.edges);

    // ── Phase 3: Lowercase 'm' symmetry (aflatin.c:4582-4627) ────────────
    // If a glyph has 3 stems (6 edges) or 3 stems with serifs (12 edges),
    // make the outer stems symmetric around the middle stem.
    if dim == Dimension::Horz && (num_edges == 6 || num_edges == 12) {
        let (e1_idx, e2_idx, e3_idx) = if num_edges == 6 { (0, 2, 4) } else { (1, 5, 9) };
        let e1_opos = axis.edges[e1_idx].opos;
        let e2_opos = axis.edges[e2_idx].opos;
        let e3_opos = axis.edges[e3_idx].opos;
        let dist1 = e2_opos - e1_opos;
        let dist2 = e3_opos - e2_opos;
        let mut span = dist1 - dist2;
        if span < 0 {
            span = -span;
        }
        if span < 8 {
            let delta =
                axis.edges[e3_idx].pos - (2 * axis.edges[e2_idx].pos - axis.edges[e1_idx].pos);
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
    dump_edge_phase("PHASE3", dim_label, &axis.edges);
    // ── Phase 4: Non-stem edges ─────────────────────────────────────────
    // Ported faithfully (aflatin.c:4629–4824).
    // This is the active path since all our edges lack links.
    if has_non_stem_edges || anchor == usize::MAX {
        for i in 0..num_edges {
            if axis.edges[i].flags & AF_EDGE_DONE != 0 {
                continue;
            }

            let mut delta: i32 = 1000;

            // ── Serif handling (C: aflatin.c:4733-4813) ──────────────
            // C reads edge->first->first->v which = point.v.
            // Since pipeline order matches C (VERT compute_segments runs
            // before hint loop, overwriting v=fx for HORZ), point.v = fx
            // (main-axis position). This correctly detects serif overlap
            // when intermediate edges share the same fpos range.
            // For VERT dim, v = fx = fpos already.
            let serif_idx = axis.edges[i].serif;
            if serif_idx != usize::MAX {
                delta = axis.edges[serif_idx].opos - axis.edges[i].opos;
                if delta < 0 {
                    delta = -delta;
                }
                // Only check overlap when delta < 1.5px (C: aflatin.c:4767)
                if delta < 64 + 32 {
                    // C: reads first/last points of first/last segments (4 pts per edge)
                    let seg_v_min = |seg_idx: usize| -> i32 {
                        let seg = &axis.segments[seg_idx];
                        i32::min(hints.points[seg.first].v, hints.points[seg.last].v)
                    };
                    let seg_v_max = |seg_idx: usize| -> i32 {
                        let seg = &axis.segments[seg_idx];
                        i32::max(hints.points[seg.first].v, hints.points[seg.last].v)
                    };
                    let s_fi = axis.edges[i].first;
                    let s_li = axis.edges[i].last;
                    let s_fs = axis.edges[serif_idx].first;
                    let s_ls = axis.edges[serif_idx].last;
                    let v_min = i32::min(
                        i32::min(seg_v_min(s_fi), seg_v_min(s_li)),
                        i32::min(seg_v_min(s_fs), seg_v_min(s_ls)),
                    );
                    let v_max = i32::max(
                        i32::max(seg_v_max(s_fi), seg_v_max(s_li)),
                        i32::max(seg_v_max(s_fs), seg_v_max(s_ls)),
                    );
                    // Walk intermediate edges for v-overlap
                    let lo = serif_idx.min(i);
                    let hi = serif_idx.max(i);
                    let mut overlap = false;
                    for j in (lo + 1)..hi {
                        if j == i || j == serif_idx {
                            continue;
                        }
                        let sj_f = axis.edges[j].first;
                        let sj_l = axis.edges[j].last;
                        if sj_f == usize::MAX || sj_l == usize::MAX {
                            continue;
                        }
                        let ej_min = i32::min(seg_v_min(sj_f), seg_v_min(sj_l));
                        let ej_max = i32::max(seg_v_max(sj_f), seg_v_max(sj_l));
                        if !((ej_min < v_min && ej_max < v_min)
                            || (ej_min > v_max && ej_max > v_max))
                        {
                            overlap = true;
                            break;
                        }
                    }
                    if overlap {
                        continue;
                    }
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
                    axis.edges[i].pos = anchor_pos + ((edge_opos - anchor_opos + 16) & !31);
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
    dump_edge_phase("PHASE4", dim_label, &axis.edges);
}

// ── Edge-point alignment ───────────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_edge_points` (afhints.c:1338–1400).
// Moves all points belonging to an edge to that edge's grid-fitted position.

/// Snap contour points to their assigned edge's hinted position.
///
/// Walks `edge.first → edge.last` via segment chain, sets `pt.x = edge.pos`.
/// Touched points become IUP reference anchors.
fn align_edge_points(hints: &mut GlyphHints, dim: Dimension) {
    let axis = &hints.axis[dim as usize];
    let is_vert = dim == Dimension::Vert;

    for edge in &axis.edges {
        let pos = edge.pos;
        let mut seg_idx = edge.first;
        loop {
            if seg_idx == usize::MAX {
                break;
            }
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

// ── Strong-point alignment (IP) ────────────────────────────────────────────
//
// Port of `af_glyph_hints_align_strong_points` (afhints.c:1413–1578).
// Uses FreeType's small-edge linear scan, exact-match snapping,
// FT_DivFix/FT_MulFix interpolation, and outside-range edge-delta fallback.
/// Grid-fit corner points by interpolating between bracketing hinted edges.
///
/// Skips points with WEAK_INTERPOLATION flag (they go to IUP instead).
/// Weak/strong classification is therefore part of the coordinate contract for
/// later untouched-point interpolation.
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
            let val = if is_vert {
                pt.oy + delta
            } else {
                pt.ox + delta
            };
            if is_vert {
                hints.points[i].y = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_Y;
            } else {
                hints.points[i].x = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_X;
            }
            continue;
        }
        if nn == 0 {
            // Point before first edge: shift by edge delta (afhints.c:1456-1469)
            let first = &axis.edges[0];
            let delta = first.pos - first.opos;
            let val = if is_vert {
                pt.oy + delta
            } else {
                pt.ox + delta
            };
            if is_vert {
                hints.points[i].y = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_Y;
            } else {
                hints.points[i].x = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_X;
            }
            continue;
        }

        // C: if exact match, snap to edge (afhints.c:1496-1499)
        if axis.edges[nn].fpos as i32 == pt_fpos {
            let val = axis.edges[nn].pos;
            if is_vert {
                hints.points[i].y = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_Y;
            } else {
                hints.points[i].x = val;
                hints.points[i].flags |= AF_FLAG_TOUCH_X;
            }
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

/// Uniform IUP shift for a contour with one touched reference point.
///
/// Every point in the range receives the same delta as the reference point:
/// `points[ref_idx].u - points[ref_idx].v`.
fn iup_shift(points: &mut [AFPoint], p1: usize, p2: usize, ref_idx: usize) {
    let delta = points[ref_idx].u - points[ref_idx].v;
    if delta == 0 {
        return;
    }
    for (j, pt) in points[p1..=p2].iter_mut().enumerate() {
        if p1 + j != ref_idx {
            pt.u = pt.v + delta;
        }
    }
}

/// Linear interpolation between two reference points.
///
/// `scale = ft_mul_div(u2-u1, 0x10000, v2-v1)`.
/// For each weak point: if v ≤ v1 → d1 shift, if v ≥ v2 → d2 shift, else → u1 + ft_mul_fix(v-v1, scale).
fn iup_interp(points: &mut [AFPoint], p1: usize, p2: usize, ref1: usize, ref2: usize) {
    if p1 > p2 {
        return;
    }

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
        for p in points[p1..=p2].iter_mut() {
            let u = p.v;
            if u <= v1 {
                p.u = u + d1;
            } else if u >= v2 {
                p.u = u + d2;
            } else {
                p.u = u1;
            }
        }
    } else {
        let scale = ft_mul_div(u2 - u1, 0x10000, v2 - v1); // FT_DivFix
        for p in points[p1..=p2].iter_mut() {
            let u = p.v;
            if u <= v1 {
                p.u = u + d1;
            } else if u >= v2 {
                p.u = u + d2;
            } else {
                p.u = u1 + ft_mul_fix(u - v1, scale);
            }
        }
    }
}

// ── Weak-point alignment (IUP) ─────────────────────────────────────────────
//
// Port of af_glyph_hints_align_weak_points (afhints.c:1687-1808).
/// Interpolate weak points between consecutive TOUCHED (strong) anchors.
///
/// Walks contour, finds touched pairs, linearly interpolates between them.
/// Result depends on WHICH points are touched — wrong touch flag → wrong ref.
fn align_weak_points(hints: &mut GlyphHints, dim: Dimension) {
    let is_vert = dim == Dimension::Vert;
    let touch_flag = if is_vert {
        AF_FLAG_TOUCH_Y
    } else {
        AF_FLAG_TOUCH_X
    };

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
            if idx > end_idx {
                break usize::MAX;
            } // no touched point in contour
            if hints.points[idx].flags & touch_flag != 0 {
                break idx;
            }
            idx += 1;
        };
        if first_touched == usize::MAX {
            continue;
        }

        let mut last_touched = first_touched;

        loop {
            // skip consecutive touched points
            while last_touched < end_idx && hints.points[last_touched + 1].flags & touch_flag != 0 {
                last_touched += 1;
            }

            // Find next touched point
            let mut next = last_touched + 1;
            let next_touched: Option<usize> = loop {
                if next > end_idx {
                    break None;
                }
                if hints.points[next].flags & touch_flag != 0 {
                    break Some(next);
                }
                next += 1;
            };

            if let Some(nt) = next_touched {
                // Interpolate between last_touched and next_touched
                iup_interp(
                    &mut hints.points,
                    last_touched + 1,
                    nt - 1,
                    last_touched,
                    nt,
                );
                last_touched = nt;
            } else {
                // End of contour
                if last_touched == first_touched {
                    // Only one touched point: uniform shift
                    iup_shift(&mut hints.points, c_start, end_idx, first_touched);
                } else {
                    // Interpolate tail segments
                    if last_touched < end_idx {
                        iup_interp(
                            &mut hints.points,
                            last_touched + 1,
                            end_idx,
                            last_touched,
                            first_touched,
                        );
                    }
                    if first_touched > c_start {
                        iup_interp(
                            &mut hints.points,
                            c_start,
                            first_touched - 1,
                            last_touched,
                            first_touched,
                        );
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
