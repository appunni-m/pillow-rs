//! Face-level global data for auto-hinting.
//!
//! Port of FreeType's `AF_FaceGlobals` and
//! `af_face_globals_compute_style_coverage` from `afglobal.c`.
//!
//! The global object owns the per-face style coverage table and lazily
//! constructs per-style Latin metrics. Coverage is computed from generated
//! Unicode ranges in the same order as FreeType's `afstyles.h`; the first style
//! whose range contains a glyph wins that glyph. Default native TrueType loads
//! do not need this coverage, so construction defers it until `get_metrics`.
//!
//! Parity-sensitive details:
//!
//! - Standard-character fallback follows each script's complete FreeType
//!   candidate chain so fonts missing the first candidate can still measure.
//! - Coverage scanning includes every generated non-base range from
//!   `STYLE_TABLE`.
//! - Hebrew blue-zone initialization accounts for outlines changed by
//!   `FT_LOAD_NO_SCALE` TrueType programs in `latin.rs`.
//!
//! Per-style metrics are cached behind `Rc<RefCell<_>>` because the public
//! global object is shared while metrics are initialized on demand.
//!
//! Full 52-script support via generated data from afranges.c + afstyles.h.

use super::cjk::{cjk_metrics_init_blues, cjk_metrics_init_widths, cjk_metrics_scale};
use super::globals_data::{
    STYLE_FALLBACK, STYLE_TABLE, STYLE_UNASSIGNED, standard_chars_for_script,
};
use super::latin::{metrics_init_blues_impl, metrics_init_widths};
use super::types::AfLatinMetrics;
use crate::tables::FontData;
use crate::tt::cmap::CmapTable;

use std::cell::RefCell;
use std::rc::Rc;

// ── FaceGlobals ───────────────────────────────────────────────────────────

/// Per-face global hinting data.
/// Mirrors `AF_FaceGlobalsRec` from afglobal.h.
/// Uses Rc<RefCell<>> so clones share the same metrics cache.
#[derive(Clone)]
pub struct FaceGlobals {
    /// Total number of glyphs in the font.
    pub glyph_count: u16,
    /// Lazily computed coverage data. Default/native TrueType loads do not
    /// need autohint style coverage, so this mirrors FreeType's pay-for-use
    /// behavior more closely than eager construction.
    coverage: Rc<RefCell<Option<FaceCoverage>>>,
    /// Per-style cached metrics. Index into STYLE_TABLE → shared metrics.
    ///
    /// C stores initialized `AF_LatinMetrics` on the face globals and passes
    /// pointers to glyph loads.  Store metrics behind `Rc` for the same cheap
    /// reuse: every render can borrow the cached object instead of cloning the
    /// full per-glyph `non_base_glyphs` table.
    pub metrics_cache: Rc<RefCell<Vec<Option<Rc<AfLatinMetrics>>>>>,
    /// Font data for lazy metric computation.
    pub font_data: std::sync::Arc<FontData>,
    /// Whether the font is italic.
    pub is_italic: bool,
}

struct FaceCoverage {
    glyph_styles: Vec<usize>,
    non_base_glyphs: Vec<bool>,
    digit_glyphs: Vec<bool>,
}

const AF_STYLE_MASK: u16 = 0x1FFF;
const AF_DIGIT: u16 = 0x8000;
const AF_NONBASE: u16 = 0x4000;
const AF_HAS_CMAP_ENTRY: u16 = 0x2000;
const PINNED_FT_STYLE_FALLBACK: u16 = 86;
// `STYLE_TABLE` is the compact Rust table of default styles only; C's
// `AF_Style` enum also contains Latin OpenType feature styles.  The public
// `glyph_styles` map stores C enum values, not compact Rust indexes.  Values
// are generated from pinned `src/autofit/afstyles.h` with AF_CONFIG_OPTION_CJK
// and AF_CONFIG_OPTION_INDIC enabled.
const PINNED_FT_PUBLIC_STYLE_VALUES: [u16; 59] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 22, 23, 24, 25, 26, 27, 28, 29, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 58, 59, 57, 60, 61, 62, 63, 64, 65, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77,
    78, 79, 80, 81, 82, 83, 84, 85, 86,
];

/// Build the public `AF_FaceGlobalsRec::glyph_styles` map returned by
/// `FT_Prop_GlyphToScriptMap`.
///
/// FreeType exposes this array directly from `af_property_get_face_globals`
/// (`src/autofit/afmodule.c:296-304`).  Values are not public
/// `FT_AUTOHINTER_SCRIPT_*` constants: they are internal style indexes ORed
/// with `AF_HAS_CMAP_ENTRY`, `AF_NONBASE`, and `AF_DIGIT` flags from
/// `src/autofit/afglobal.h:76-84`.
pub fn build_public_glyph_style_map(font_data: &FontData, glyph_count: u16) -> Vec<u16> {
    let ng = usize::from(glyph_count);
    let mut glyph_styles = vec![AF_STYLE_MASK; ng];

    for (si, style) in STYLE_TABLE.iter().enumerate() {
        let style_value = PINNED_FT_PUBLIC_STYLE_VALUES
            .get(si)
            .copied()
            .unwrap_or(AF_STYLE_MASK);
        for range in style.uni_ranges {
            for cp in range.first..=range.last {
                if let Some(gi) = font_data.cmap.char_index(cp) {
                    let gi = usize::from(gi);
                    if gi != 0 && gi < ng && (glyph_styles[gi] & AF_STYLE_MASK) == AF_STYLE_MASK {
                        glyph_styles[gi] = style_value | AF_HAS_CMAP_ENTRY;
                    }
                }
            }
        }

        for range in style.non_base_ranges {
            for cp in range.first..=range.last {
                if let Some(gi) = font_data.cmap.char_index(cp) {
                    let gi = usize::from(gi);
                    if gi != 0 && gi < ng && (glyph_styles[gi] & AF_STYLE_MASK) == style_value {
                        glyph_styles[gi] |= AF_NONBASE;
                    }
                }
            }
        }
    }

    for cp in b'0'..=b'9' {
        if let Some(gi) = font_data.cmap.char_index(u32::from(cp)) {
            let gi = usize::from(gi);
            if gi != 0 && gi < ng {
                glyph_styles[gi] |= AF_DIGIT;
            }
        }
    }

    for style in &mut glyph_styles {
        if (*style & AF_STYLE_MASK) == AF_STYLE_MASK {
            *style &= !AF_STYLE_MASK;
            *style |= PINNED_FT_STYLE_FALLBACK;
        }
    }

    glyph_styles
}

impl FaceGlobals {
    /// Create FaceGlobals. Script coverage is computed lazily on first
    /// auto-hint metrics access, so default native TrueType loads do not pay
    /// for 52-script coverage scans during font construction.
    pub fn new(font_data: std::sync::Arc<FontData>, is_italic: bool) -> Self {
        let ng = font_data.maxp.num_glyphs as usize;
        let num_styles = STYLE_TABLE.len();
        FaceGlobals {
            glyph_count: ng as u16,
            coverage: Rc::new(RefCell::new(None)),
            metrics_cache: Rc::new(RefCell::new(vec![None; num_styles])),
            font_data,
            is_italic,
        }
    }

    fn ensure_coverage(&self) {
        if self.coverage.borrow().is_some() {
            return;
        }
        let coverage = build_coverage(&self.font_data, self.glyph_count);
        *self.coverage.borrow_mut() = Some(coverage);
    }

    /// Get the metrics for a given glyph index, lazily computing if needed.
    pub fn get_metrics(&self, glyph_index: u16) -> Option<Rc<AfLatinMetrics>> {
        if glyph_index == 0 {
            // C `af_face_globals_compute_style_coverage` skips `gindex == 0`
            // while scanning cmap coverage (afglobal.c:187-203).  Callers
            // that actually enter the auto-hinter can request the fallback
            // style explicitly; native/no-autohint paths keep `None`.
            return None;
        }
        self.ensure_coverage();
        let coverage = self.coverage.borrow();
        let coverage = coverage.as_ref()?;
        let gi = glyph_index as usize;
        if gi >= coverage.glyph_styles.len() {
            return None;
        }

        // `build_coverage` replaces every unassigned entry with the fallback,
        // and all other entries originate from `STYLE_TABLE::enumerate`.
        self.get_metrics_for_style(coverage.glyph_styles[gi])
    }

    /// Return the configured fallback style metrics for render-only paths that
    /// need FreeType's fallback outline hinting without changing load metrics.
    pub(crate) fn get_fallback_metrics(&self) -> Option<Rc<AfLatinMetrics>> {
        self.get_metrics_for_style(STYLE_FALLBACK)
    }

    fn get_metrics_for_style(&self, si: usize) -> Option<Rc<AfLatinMetrics>> {
        self.ensure_coverage();

        let mut cache = self.metrics_cache.borrow_mut();

        if cache[si].is_none() {
            let coverage = self.coverage.borrow();
            let coverage = coverage.as_ref()?;
            let style = &STYLE_TABLE[si];
            let upem = self.font_data.head.units_per_em as i32;
            let mut m = AfLatinMetrics::new(upem, self.glyph_count);
            let cjk_writing_system = uses_cjk_writing_system(style.script_tag);
            let indic_writing_system = uses_indic_writing_system(style.script_tag);
            m.no_advance_hinting = cjk_writing_system;
            m.digits_have_same_width = digits_have_same_width(&self.font_data);
            m.fixed_width = self
                .font_data
                .post
                .as_ref()
                .is_some_and(|post| post.is_fixed_pitch != 0);

            // Copy non-base flags
            for (i, &nb) in coverage.non_base_glyphs.iter().enumerate() {
                if nb {
                    m.non_base_glyphs[i] = true;
                }
            }
            for (i, &digit) in coverage.digit_glyphs.iter().enumerate() {
                if digit {
                    m.digit_glyphs[i] = true;
                }
            }

            // Set hinting direction from script tag
            m.top_to_bottom_hinting = top_to_bottom_hinting(style.script_tag);

            // FreeType's no-HarfBuzz shaper consumes one candidate from the
            // space-separated `standard_charstring` per loop iteration.  Both
            // width initializers stop at the first mapped candidate
            // (`aflatin.c:95-138`, `afcjk.c:102-140`, `afshaper.c:631-653`).
            let mut char_glyph: u16 = 0;
            for &sc in standard_chars_for_script(style.script_tag) {
                let g = self.font_data.cmap.char_index(sc as u32).unwrap_or(0);
                if g > 0 {
                    char_glyph = g;
                    break;
                }
            }
            if char_glyph > 0 {
                if let Ok(outline_raw) = self.font_data.load_glyph_outline(char_glyph) {
                    let sp: Vec<_> = outline_raw
                        .points
                        .iter()
                        .map(|p| crate::outline::OutlinePoint {
                            x: p.x,
                            y: p.y,
                            on_curve: p.on_curve,
                        })
                        .collect();
                    if cjk_writing_system {
                        cjk_metrics_init_widths(&mut m, &outline_raw, &sp);
                    } else {
                        metrics_init_widths(&mut m, char_glyph, &outline_raw, &sp);
                    }
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
            if style.script_tag == "hani" {
                cjk_metrics_init_blues(&mut m, &self.font_data, style.blue_entries);
            } else if indic_writing_system {
                // FreeType's STYLE_DEFAULT_INDIC styles (`limb`, `orya`,
                // `sylo`, `tibt`) use afindic.c, which delegates metrics and
                // hinting to CJK but intentionally skips blue-zone setup.
            } else {
                metrics_init_blues_impl(&mut m, &self.font_data, style.blue_entries);
            }

            // Scale
            let bs = crate::scaler::ScaleMetrics::from_font_data(&self.font_data);
            let (_, ya) = if cjk_writing_system {
                cjk_metrics_scale(&mut m, bs.x_scale, bs.y_scale, 0, 0)
            } else {
                super::latin::metrics_scale_dim(&mut m, bs.x_scale, bs.y_scale, 0, 0)
            };
            m.axis[1].org_scale = ya;

            cache[si] = Some(Rc::new(m));
        }

        cache[si].clone()
    }
}

fn build_coverage(font_data: &FontData, glyph_count: u16) -> FaceCoverage {
    let ng = glyph_count as usize;
    let mut non_base = vec![false; ng];
    let mut glyph_styles = vec![STYLE_UNASSIGNED; ng];
    let mut digit_glyphs = vec![false; ng];

    // Run coverage scan
    compute_style_coverage(&font_data.cmap, glyph_count, &mut glyph_styles);
    for cp in b'0'..=b'9' {
        if let Some(gi) = font_data.cmap.char_index(u32::from(cp)) {
            if gi != 0 && (gi as usize) < ng {
                digit_glyphs[gi as usize] = true;
            }
        }
    }

    // Per-script non-base ranges: C checks glyph_styles[gi] & AF_NONBASE
    // during coverage. Each style's non_base_ranges (RANGES_*_NONBASE
    // and RANGES_*_NONBASE_UNI) contain combining marks and diacritics
    // that should NOT get blue zone alignment (afglobal.c).
    for (si, style) in STYLE_TABLE.iter().enumerate() {
        for range in style.non_base_ranges {
            for cp in range.first..=range.last {
                if let Some(gi) = font_data.cmap.char_index(cp) {
                    let gi = gi as usize;
                    if gi != 0 && gi < ng && glyph_styles[gi] == si {
                        non_base[gi] = true;
                    }
                }
            }
        }
    }

    FaceCoverage {
        glyph_styles,
        non_base_glyphs: non_base,
        digit_glyphs,
    }
}

fn digits_have_same_width(font_data: &FontData) -> bool {
    let mut old_advance = 0;
    let mut started = false;

    for cp in b'0'..=b'9' {
        let Some(glyph_index) = font_data.cmap.char_index(u32::from(cp)) else {
            continue;
        };
        if glyph_index == 0 {
            continue;
        }

        let advance = font_data.hmtx.get(glyph_index).advance_width;
        if started {
            if advance != old_advance {
                return false;
            }
        } else {
            old_advance = advance;
            started = true;
        }
    }

    true
}

fn uses_cjk_writing_system(tag: &str) -> bool {
    matches!(tag, "hani" | "limb" | "orya" | "sylo" | "tibt")
}

fn uses_indic_writing_system(tag: &str) -> bool {
    matches!(tag, "limb" | "orya" | "sylo" | "tibt")
}

// ── Coverage scan ─────────────────────────────────────────────────────────

/// Scan all 52+ scripts' Unicode ranges through the cmap.
/// First script whose range covers a codepoint wins that glyph.
/// Matches `af_face_globals_compute_style_coverage` (afglobal.c:137-230).
fn compute_style_coverage(cmap: &CmapTable, num_glyphs: u16, glyph_styles: &mut [usize]) {
    let ng = num_glyphs as usize;
    for g in glyph_styles.iter_mut() {
        *g = STYLE_UNASSIGNED;
    }

    for (si, style) in STYLE_TABLE.iter().enumerate() {
        for range in style.uni_ranges {
            let mut cp = range.first;
            while cp <= range.last {
                if let Some(gi) = cmap.char_index(cp) {
                    let gi = gi as usize;
                    if gi != 0 && gi < ng && glyph_styles[gi] == STYLE_UNASSIGNED {
                        glyph_styles[gi] = si;
                    }
                }
                cp += 1;
            }
        }
    }

    // Fill unassigned with fallback
    for g in glyph_styles.iter_mut() {
        if *g == STYLE_UNASSIGNED {
            *g = STYLE_FALLBACK;
        }
    }
}

/// Per-script hinting direction: TOP_TO_BOTTOM for Indic/Mongolian/Gothic.
/// Matches afscript.h HINTING_TOP_TO_BOTTOM entries.
/// All other scripts use bottom-to-top (Latin default).
pub fn top_to_bottom_hinting(tag: &str) -> bool {
    matches!(tag, "beng" | "deva" | "goth" | "guru" | "mong")
}
