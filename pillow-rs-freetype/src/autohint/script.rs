//! Script detection for blue zone selection.
//!
//! Matches FreeType's `af_face_globals_compute_style_coverage`: scans
//! Unicode ranges per script to assign a script to each glyph. The first
//! script whose range contains a codepoint that maps to a glyph "wins"
//! that glyph. This means Latin characters whose glyphs are shared with
//! Greek codepoints (e.g., ';' sharing glyph with Greek Question Mark
//! U+037E in LiberationSansNarrow-BoldItalic) get Greek blue strings.
//!
//! Reference: `afglobal.c:137-230` (af_face_globals_compute_style_coverage)

use super::blue_strings::{BlueStringEntry, SCRIPT_GREK, SCRIPT_LATN, SCRIPT_TABLE};
use crate::tt::cmap::CmapTable;

/// A Unicode codepoint range.
#[derive(Debug, Clone, Copy)]
pub struct UniRange {
    pub first: u32,
    pub last: u32,
}

/// Unicode ranges for Greek script (matching afranges.c: grek_uniranges).
pub static GRK_RANGES: &[UniRange] = &[
    UniRange {
        first: 0x0370,
        last: 0x03FF,
    }, // Greek and Coptic
    UniRange {
        first: 0x1F00,
        last: 0x1FFF,
    }, // Greek Extended
];

/// Unicode ranges for Latin script (matching afranges.c: latn_uniranges).
pub static LATN_RANGES: &[UniRange] = &[
    UniRange {
        first: 0x0020,
        last: 0x007F,
    }, // Basic Latin
    UniRange {
        first: 0x00A0,
        last: 0x00A9,
    }, // Latin-1 Supplement
    UniRange {
        first: 0x00AB,
        last: 0x00B1,
    },
    UniRange {
        first: 0x00B4,
        last: 0x00B8,
    },
    UniRange {
        first: 0x00BB,
        last: 0x00FF,
    },
    UniRange {
        first: 0x0100,
        last: 0x017F,
    }, // Latin Extended-A
    UniRange {
        first: 0x0180,
        last: 0x024F,
    }, // Latin Extended-B
    UniRange {
        first: 0x0250,
        last: 0x02AF,
    }, // IPA Extensions
    UniRange {
        first: 0x02B9,
        last: 0x02DF,
    }, // Spacing Modifier Letters
    UniRange {
        first: 0x02E5,
        last: 0x02FF,
    },
    UniRange {
        first: 0x0300,
        last: 0x036F,
    }, // Combining Diacritical
    UniRange {
        first: 0x1AB0,
        last: 0x1ABE,
    },
    UniRange {
        first: 0x1D00,
        last: 0x1D2B,
    },
    UniRange {
        first: 0x1D6B,
        last: 0x1D77,
    },
    UniRange {
        first: 0x1D79,
        last: 0x1D7F,
    },
    UniRange {
        first: 0x1D80,
        last: 0x1D9A,
    },
    UniRange {
        first: 0x1DC0,
        last: 0x1DFF,
    },
    UniRange {
        first: 0x1E00,
        last: 0x1EFF,
    }, // Latin Extended Additional
    UniRange {
        first: 0x2000,
        last: 0x206F,
    }, // General Punctuation
    UniRange {
        first: 0x2070,
        last: 0x209F,
    }, // Superscripts/Subscripts
    UniRange {
        first: 0x20A0,
        last: 0x20CF,
    }, // Currency Symbols
    UniRange {
        first: 0x2100,
        last: 0x214F,
    }, // Letterlike Symbols
    UniRange {
        first: 0x2150,
        last: 0x218F,
    }, // Number Forms
    UniRange {
        first: 0x2C60,
        last: 0x2C7F,
    }, // Latin Extended-C
    UniRange {
        first: 0xA720,
        last: 0xA7FF,
    }, // Latin Extended-D
    UniRange {
        first: 0xAB30,
        last: 0xAB6F,
    }, // Latin Extended-E
    UniRange {
        first: 0xFB00,
        last: 0xFB06,
    }, // Alphabetic Presentation Forms
    UniRange {
        first: 0xFF00,
        last: 0xFFEF,
    }, // Halfwidth/Fullwidth Forms
    UniRange {
        first: 0x1DF00,
        last: 0x1DFFF,
    }, // Latin Extended-G
    UniRange {
        first: 0x10780,
        last: 0x107BF,
    }, // Latin Extended-F
];

/// Check if a codepoint falls within any of the given Unicode ranges.
fn in_ranges(cp: u32, ranges: &[UniRange]) -> bool {
    ranges.iter().any(|r| cp >= r.first && cp <= r.last)
}

/// Determine which script a codepoint belongs to.
/// Returns Greek entries if the codepoint is in the Greek range AND
/// Greek characters exist in the font. Falls back to Latin.
pub fn script_for_codepoint(cmap: &CmapTable, cp: u32) -> &'static [BlueStringEntry] {
    // Check Greek first (FreeType scans Greek before Latin)
    if in_ranges(cp, GRK_RANGES) {
        if cmap.char_index('\u{0393}' as u32).unwrap_or(0) != 0 {
            return SCRIPT_GREK;
        }
    }
    // Check Latin via SCRIPT_TABLE (LATN is first)
    for (_tag, ch, entries) in SCRIPT_TABLE {
        if cmap.char_index(*ch as u32).unwrap_or(0) != 0 {
            return entries;
        }
    }
    SCRIPT_LATN
}

/// Build a glyph→script preference mapping matching FreeType's coverage scan.
/// Scans Greek ranges first, then Latin. Returns which glyphs prefer Greek.
/// `true` = glyph should use Greek blue strings, `false` = use Latin.
pub fn build_glyph_script_map(cmap: &CmapTable, max_glyphs: usize) -> Vec<bool> {
    let mut glyph_is_greek = vec![false; max_glyphs];

    // Phase 1: Scan Greek ranges (first = higher priority)
    for range in GRK_RANGES {
        let mut cp = range.first;
        while cp <= range.last {
            if let Some(gi) = cmap.char_index(cp) {
                let gi = gi as usize;
                if gi < max_glyphs {
                    glyph_is_greek[gi] = true;
                }
            }
            cp += 1;
            // Skip large ranges that don't exist in font
            if cp > range.last || (cp - range.first > 512 && cmap.char_index(cp).is_none()) {
                // Advance faster through unmapped regions
                let mut skip = cp;
                while skip <= range.last && cmap.char_index(skip).is_none() {
                    skip += 64;
                }
                cp = if skip > range.last {
                    range.last + 1
                } else {
                    skip
                };
            }
        }
    }

    // Phase 2: Latin ranges fill remaining gaps (only affects unassigned glyphs)
    // We don't actually need this since UNASSIGNED defaults to Latin in our system

    glyph_is_greek
}

/// Detect the script for the font's default metrics.
/// Also returns a glyph→script preference map for per-glyph overrides.
pub fn detect_font_scripts(
    cmap: &CmapTable,
    num_glyphs: u16,
) -> (Vec<bool>, &'static [BlueStringEntry]) {
    let glyph_is_greek = build_glyph_script_map(cmap, num_glyphs as usize);

    // Default script: use LATN unless Greek is the only script
    let default_script = detect_script(cmap);

    (glyph_is_greek, default_script)
}

/// Basic script detection for fonts without glyph sharing.
pub fn detect_script(cmap: &CmapTable) -> &'static [BlueStringEntry] {
    for (_tag, ch, entries) in SCRIPT_TABLE {
        if cmap.char_index(*ch as u32).unwrap_or(0) != 0 {
            return entries;
        }
    }
    SCRIPT_LATN
}

/// Check if a specific codepoint's glyph should use Greek blue strings.
pub fn is_glyph_greek(glyph_is_greek: &[bool], glyph_index: u16) -> bool {
    let gi = glyph_index as usize;
    gi < glyph_is_greek.len() && glyph_is_greek[gi]
}
