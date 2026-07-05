//! Face-level global data for auto-hinting.
//!
//! Port of FreeType's `AF_FaceGlobals` and
//! `af_face_globals_compute_style_coverage` from `afglobal.c`.
//!
//! ## Parity Fixes in this file
//!
//! 1. **Standard char fallback chain** (L135): C's standard_charstring is
//!    "o O 0". Try '0' when 'o'/'O' are missing (common in non-Latin fonts).
//!
//! 2. **Per-script non-base glyph detection** (L80): generated globals_data.rs
//!    pointed to RANGES_*_NONBASE (empty) instead of RANGES_*_NONBASE_UNI.
//!    Corrected + scan all STYLE_TABLE non_base_ranges. 111 failures fixed.
//!
//! 3. **cmap coverage scan skip bug** (L205): skip optimization jumped past
//!    valid codepoints between probe gaps. Removed skip → 10 fixed (cans).
//!
//! 4. **hebr bytecode blue zone correction**: TrueType font programs alter
//!    outline at FT_LOAD_NO_SCALE. Handle in latin.rs::metrics_init_blues_impl.
//!    48 failures fixed.
//!
//! Architecture:
//! 1. Coverage scan: lazily iterate style classes from `globals_data.rs`,
//!    scan each script's Unicode ranges in order (matching afstyles.h).
//!    The first script whose range contains a codepoint for a glyph
//!    "wins" that glyph. Default native TrueType loads do not need this
//!    coverage, so construction defers it until `get_metrics`.
//! 2. Lazy metrics: per-style AfLatinMetrics computed on first access,
//!    cached via Rc<RefCell<>> for interior mutability.
//! 3. Per-glyph lookup: `glyph_styles[gindex]` → style index → metrics.
//!
//! Full 52-script support via generated data from afranges.c + afstyles.h.

use super::blue_strings::{BlueStringEntry, SCRIPT_LATN, SCRIPT_TABLE};
use super::globals_data::{STYLE_FALLBACK, STYLE_TABLE, STYLE_UNASSIGNED};
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
        self.ensure_coverage();
        let coverage = self.coverage.borrow();
        let coverage = coverage.as_ref()?;
        let gi = glyph_index as usize;
        if gi >= coverage.glyph_styles.len() {
            return None;
        }

        let mut si = coverage.glyph_styles[gi];
        if si == STYLE_UNASSIGNED || si >= STYLE_TABLE.len() {
            si = STYLE_FALLBACK;
        }

        let mut cache = self.metrics_cache.borrow_mut();

        if cache[si].is_none() {
            let style = &STYLE_TABLE[si];
            let upem = self.font_data.head.units_per_em as i32;
            let mut m = AfLatinMetrics::new(upem, self.glyph_count);

            // Copy non-base flags
            for (i, &nb) in coverage.non_base_glyphs.iter().enumerate() {
                if nb {
                    m.non_base_glyphs[i] = true;
                }
            }

            // Set hinting direction from script tag
            m.top_to_bottom_hinting = top_to_bottom_hinting(style.script_tag);

            // Stem widths from the script's standard character
            // C's `script_class->standard_charstring` is a space-separated
            // list.  C iterates: first character that maps to a valid glyph
            // wins (af_latin_metrics_init_widths, aflatin.c:95-131).
            // Without HarfBuzz, shaper is a no-op — C just tries chars in
            // order. Our `standard_char_for_script` only returns the first
            // char. Track multiple fallback chars to match C.
            //
            // All scripts use the same Latin 'o'-based approach because
            // Indic scripts' standard characters (e.g., Bengali U+09E6)
            // have fundamentally different shapes that produce incorrect
            // stem widths when run through segment-based detection.
            let std_chars: &[char] = match style.script_tag {
                // C Latin: "o O 0" (afscript.h:216-220)
                "latn" => &['o', 'O', '0'],
                // C Latin subscript: "ₒ ₀" = U+2092 U+2080 (afscript.h)
                "latb" => &['\u{2092}', '\u{2080}'],
                // C Latin superscript: "ᵒ ᴼ ⁰" = U+1D52 U+1D3C U+2070
                "latp" => &['\u{1D52}', '\u{1D3C}', '\u{2070}'],
                // Most scripts have a single standard character.
                _ => &[
                    super::globals_data::standard_char_for_script(style.script_tag),
                    '\0',
                ],
            };
            let mut char_glyph: u16 = 0;
            for &sc in std_chars {
                if sc == '\0' {
                    break;
                }
                let g = self.font_data.cmap.char_index(sc as u32).unwrap_or(0);
                if g > 0 {
                    char_glyph = g;
                    break;
                }
            }
            if char_glyph > 0 {
                if let Ok(outline_raw) = crate::tt::glyf::load_glyph(
                    &self.font_data.glyf_data,
                    &self.font_data.loca_data,
                    self.font_data.head.index_to_loc_format,
                    char_glyph,
                    &self.font_data.hmtx,
                ) {
                    let sp: Vec<_> = outline_raw
                        .points
                        .iter()
                        .map(|p| crate::outline::OutlinePoint {
                            x: p.x,
                            y: p.y,
                            on_curve: p.on_curve,
                        })
                        .collect();
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
                self.font_data.size_pt,
                self.font_data.head.units_per_em,
            );
            let (_, ya) = super::latin::metrics_scale_dim(&mut m, bs.x_scale, bs.y_scale, 0, 0);
            m.axis[1].org_scale = ya;

            cache[si] = Some(Rc::new(m));
        }

        cache[si].clone()
    }
}

fn build_coverage(font_data: &FontData, glyph_count: u16) -> FaceCoverage {
    let ng = glyph_count as usize;
    // Build non-base glyph table (Latin diacritics etc.)
    let nonbase_ranges: &[(u32, u32)] = &[
        (0x005E, 0x0060),
        (0x007E, 0x007E),
        (0x00A8, 0x00A9),
        (0x00AE, 0x00B0),
        (0x00B4, 0x00B4),
        (0x00B8, 0x00B8),
        (0x00BC, 0x00BE),
        (0x02B9, 0x02DF),
        (0x02E5, 0x02FF),
        (0x0300, 0x036F),
        (0x1AB0, 0x1AEB),
        (0x1DC0, 0x1DFF),
        (0x2017, 0x2017),
        (0x203E, 0x203E),
        (0xA788, 0xA788),
        (0xA7F8, 0xA7FA),
    ];
    let mut non_base = vec![false; ng];
    for &(first, last) in nonbase_ranges {
        let mut ch = first;
        loop {
            if let Some(gi) = font_data.cmap.char_index(ch) {
                if (gi as usize) < ng {
                    non_base[gi as usize] = true;
                }
            }
            if ch >= last {
                break;
            }
            ch += 1;
        }
    }

    let mut glyph_styles = vec![STYLE_UNASSIGNED; ng];

    // Run coverage scan
    compute_style_coverage(&font_data.cmap, glyph_count, &mut glyph_styles);

    // Per-script non-base ranges: C checks glyph_styles[gi] & AF_NONBASE
    // during coverage. Each style's non_base_ranges (RANGES_*_NONBASE
    // and RANGES_*_NONBASE_UNI) contain combining marks and diacritics
    // that should NOT get blue zone alignment (afglobal.c).
    for style in STYLE_TABLE {
        for range in style.non_base_ranges {
            let mut cp = range.first;
            while cp <= range.last {
                if let Some(gi) = font_data.cmap.char_index(cp) {
                    if (gi as usize) < ng {
                        non_base[gi as usize] = true;
                    }
                }
                cp += 1;
                if cp > range.last {
                    break;
                }
            }
        }
    }

    FaceCoverage {
        glyph_styles,
        non_base_glyphs: non_base,
    }
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
                    if gi < ng && glyph_styles[gi] == STYLE_UNASSIGNED {
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
