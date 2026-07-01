//! Face-level global data for auto-hinting.
//!
//! Port of FreeType's `AF_FaceGlobals` and
//! `af_face_globals_compute_style_coverage` from `afglobal.c`.
//!
//! Architecture:
//! 1. Coverage scan: iterate style classes from `globals_data.rs`,
//!    scan each script's Unicode ranges in order (matching afstyles.h).
//!    The first script whose range contains a codepoint for a glyph
//!    "wins" that glyph.
//! 2. Lazy metrics: per-style AfLatinMetrics computed on first access,
//!    cached via Rc<RefCell<>> for interior mutability.
//! 3. Per-glyph lookup: `glyph_styles[gindex]` → style index → metrics.
//!
//! Full 52-script support via generated data from afranges.c + afstyles.h.

use super::globals_data::{
    STYLE_TABLE, STYLE_FALLBACK, STYLE_UNASSIGNED,
};
use super::blue_strings::{BlueStringEntry, SCRIPT_TABLE, SCRIPT_LATN};
use super::types::AfLatinMetrics;
use super::latin::{metrics_init_blues_impl, metrics_init_widths};
use crate::tt::cmap::CmapTable;
use crate::tables::FontData;

use std::cell::RefCell;

// ── FaceGlobals ───────────────────────────────────────────────────────────

/// Per-face global hinting data.
/// Mirrors `AF_FaceGlobalsRec` from afglobal.h.
/// Uses Rc<RefCell<>> so clones share the same metrics cache.
#[derive(Clone)]
pub struct FaceGlobals {
    /// Total number of glyphs in the font.
    pub glyph_count: u16,
    /// Per-glyph style assignment. glyph_styles[gi] gives the index
    /// into `STYLE_TABLE` (usize), or STYLE_UNASSIGNED if uncovered.
    pub glyph_styles: Vec<usize>,
    /// Non-base glyph flags (Latin diacritics, etc.), shared across styles.
    pub non_base_glyphs: std::rc::Rc<Vec<bool>>,
    /// Per-style cached metrics. Index into STYLE_TABLE → Option<AfLatinMetrics>.
    pub metrics_cache: std::rc::Rc<RefCell<Vec<Option<AfLatinMetrics>>>>,
    /// Font data for lazy metric computation.
    pub font_data: std::sync::Arc<FontData>,
    /// Whether the font is italic.
    pub is_italic: bool,
}

impl FaceGlobals {
    /// Create FaceGlobals and run the coverage scan over all 52+ scripts.
    pub fn new(font_data: std::sync::Arc<FontData>, is_italic: bool) -> Self {
        let ng = font_data.maxp.num_glyphs as usize;
        let num_styles = STYLE_TABLE.len();

        // Build non-base glyph table (Latin diacritics etc.)
        let nonbase_ranges: &[(u32, u32)] = &[
            (0x005E, 0x0060), (0x007E, 0x007E), (0x00A8, 0x00A9),
            (0x00AE, 0x00B0), (0x00B4, 0x00B4), (0x00B8, 0x00B8),
            (0x00BC, 0x00BE), (0x02B9, 0x02DF), (0x02E5, 0x02FF),
            (0x0300, 0x036F), (0x1AB0, 0x1AEB), (0x1DC0, 0x1DFF),
            (0x2017, 0x2017), (0x203E, 0x203E), (0xA788, 0xA788),
            (0xA7F8, 0xA7FA),
        ];
        let mut non_base = vec![false; ng];
        for &(first, last) in nonbase_ranges {
            let mut ch = first;
            loop {
                if let Some(gi) = font_data.cmap.char_index(ch) {
                    if (gi as usize) < ng { non_base[gi as usize] = true; }
                }
                if ch >= last { break; }
                ch += 1;
            }
        }

        let mut glyph_styles = vec![STYLE_UNASSIGNED; ng];
        let metrics_cache = std::rc::Rc::new(RefCell::new(vec![None; num_styles]));

        // Run coverage scan
        compute_style_coverage(&font_data.cmap, ng as u16, &mut glyph_styles);

        FaceGlobals {
            glyph_count: ng as u16,
            glyph_styles,
            non_base_glyphs: std::rc::Rc::new(non_base),
            metrics_cache,
            font_data,
            is_italic,
        }
    }

    /// Get the metrics for a given glyph index, lazily computing if needed.
    pub fn get_metrics(&self, glyph_index: u16) -> Option<AfLatinMetrics> {
        let gi = glyph_index as usize;
        if gi >= self.glyph_styles.len() {
            return None;
        }

        let mut si = self.glyph_styles[gi];
        if si == STYLE_UNASSIGNED || si >= STYLE_TABLE.len() {
            si = STYLE_FALLBACK;
        }

        let mut cache = self.metrics_cache.borrow_mut();

        if cache[si].is_none() {
            let style = &STYLE_TABLE[si];
            let upem = self.font_data.head.units_per_em as i32;
            let mut m = AfLatinMetrics::new(upem, self.glyph_count);

            // Copy non-base flags
            for (i, &nb) in self.non_base_glyphs.iter().enumerate() {
                if nb { m.non_base_glyphs[i] = true; }
            }

            // Set hinting direction from script tag
            m.top_to_bottom_hinting = top_to_bottom_hinting(style.script_tag);

            // Stem widths from the script's standard character
            // All scripts use the same Latin 'o'-based approach because
            // Indic scripts' standard characters (e.g., Bengali U+09E6)
            // have fundamentally different shapes that produce incorrect
            // stem widths when run through segment-based detection.
            let std_char = super::globals_data::standard_char_for_script(style.script_tag);
            let char_glyph = self.font_data.cmap.char_index(std_char as u32).unwrap_or(0);
            if char_glyph > 0 {
                if let Ok(outline_raw) = crate::tt::glyf::load_glyph(
                    &self.font_data.glyf_data, &self.font_data.loca_data,
                    self.font_data.head.index_to_loc_format, char_glyph,
                ) {
                    let sp: Vec<_> = outline_raw.points.iter()
                        .map(|p| crate::outline::OutlinePoint {
                            x: p.x, y: p.y, on_curve: p.on_curve,
                        }).collect();
                    metrics_init_widths(&mut m, char_glyph, &outline_raw, &sp);
                }
            } else {
                for dim in 0..2 {
                    let ax = &mut m.axis[dim];
                    let stdw = (50 * upem) / 2048;
                    ax.standard_width = stdw;
                    ax.edge_distance_threshold = stdw / 5;
                }
            }

            // Blue zones for this script
            metrics_init_blues_impl(&mut m, &self.font_data, style.blue_entries);

            // Scale
            let bs = crate::scaler::ScaleMetrics::new(
                self.font_data.size_pt, self.font_data.head.units_per_em,
            );
            let (_, ya) = super::latin::metrics_scale_dim(
                &mut m, bs.x_scale, bs.y_scale, 0, 0,
            );
            m.axis[1].org_scale = ya;

            cache[si] = Some(m);
        }

        cache[si].clone()
    }
}

// ── Coverage scan ─────────────────────────────────────────────────────────

/// Scan all 52+ scripts' Unicode ranges through the cmap.
/// First script whose range covers a codepoint wins that glyph.
/// Matches `af_face_globals_compute_style_coverage` (afglobal.c:137-230).
fn compute_style_coverage(
    cmap: &CmapTable,
    num_glyphs: u16,
    glyph_styles: &mut [usize],
) {
    let ng = num_glyphs as usize;
    for g in glyph_styles.iter_mut() { *g = STYLE_UNASSIGNED; }

    for (si, style) in STYLE_TABLE.iter().enumerate() {
        for range in style.uni_ranges {
            let mut cp = range.first;
            while cp <= range.last {
                if let Some(gi) = cmap.char_index(cp) {
                    let gi = gi as usize;
                    if gi < ng && glyph_styles[gi] == STYLE_UNASSIGNED {
                        glyph_styles[gi] = si;
                    }
                }
                cp += 1;
                // Skip optimization: jump ahead through unmapped regions
                if cp > range.last { break; }
                if cp.saturating_sub(range.first) > 256 && cmap.char_index(cp).is_none() {
                    let mut skip = cp + 64;
                    while skip <= range.last && cmap.char_index(skip).is_none() {
                        skip += 64;
                    }
                    cp = if skip > range.last { range.last + 1 } else { skip.saturating_sub(64) };
                }
            }
        }
    }

    // Fill unassigned with fallback
    for g in glyph_styles.iter_mut() {
        if *g == STYLE_UNASSIGNED { *g = STYLE_FALLBACK; }
    }
}

// ── Public API ────────────────────────────────────────────────────────────

/// Quick script detection for fonts that don't need full FaceGlobals.
pub fn detect_script(cmap: &CmapTable) -> &'static [BlueStringEntry] {
    for (_tag, ch, entries) in SCRIPT_TABLE {
        if cmap.char_index(*ch as u32).unwrap_or(0) != 0 {
            return entries;
        }
    }
    SCRIPT_LATN
}

/// Per-script hinting direction: TOP_TO_BOTTOM for Indic/Mongolian/Gothic.
/// Matches afscript.h HINTING_TOP_TO_BOTTOM entries.
/// All other scripts use bottom-to-top (Latin default).
pub fn top_to_bottom_hinting(tag: &str) -> bool {
    matches!(tag, "beng" | "deva" | "goth" | "guru" | "mong")
}
