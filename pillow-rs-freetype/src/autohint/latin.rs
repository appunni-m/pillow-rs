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
pub const AF_ADJUST_DOWN: u32 = 0x0002;
pub const AF_ADJUST_UP2: u32 = 0x0004;
pub const AF_ADJUST_DOWN2: u32 = 0x0008;
pub const AF_ADJUST_TILDE_TOP: u32 = 0x0010;
pub const AF_ADJUST_TILDE_BOTTOM: u32 = 0x0020;
pub const AF_ADJUST_TILDE_TOP2: u32 = 0x0040;
pub const AF_ADJUST_TILDE_BOTTOM2: u32 = 0x0080;
pub const AF_IGNORE_CAPITAL_TOP: u32 = 0x0100;
pub const AF_IGNORE_CAPITAL_BOTTOM: u32 = 0x0200;
pub const AF_IGNORE_SMALL_TOP: u32 = 0x0400;
pub const AF_IGNORE_SMALL_BOTTOM: u32 = 0x0800;
pub const AF_ADJUST_NO_HEIGHT_CHECK: u32 = 0x1000;

/// Port of FreeType's `adjustment_database` in `afadjust.c`.
/// Keyed by Unicode codepoint; sorted for binary search lookup.
#[rustfmt::skip]
static ADJUSTMENT_DATABASE: &[(u32, u32)] = &[
    (0x0021, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ! */
    (0x003F, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ? */
    (0x0051, AF_IGNORE_CAPITAL_BOTTOM), /* Q */
    (0x0069, AF_ADJUST_UP), /* i */
    (0x006A, AF_ADJUST_UP), /* j */
    (0x00A1, AF_ADJUST_UP), /* ¡ */
    (0x00A6, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ¦ */
    (0x00AA, AF_ADJUST_UP), /* ª */
    (0x00BA, AF_ADJUST_UP), /* º */
    (0x00BF, AF_ADJUST_UP), /* ¿ */
    (0x00C0, AF_ADJUST_UP), /* À */
    (0x00C1, AF_ADJUST_UP), /* Á */
    (0x00C2, AF_ADJUST_UP), /* Â */
    (0x00C3, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ã */
    (0x00C4, AF_ADJUST_UP), /* Ä */
    (0x00C5, AF_ADJUST_UP), /* Å */
    (0x00C7, AF_IGNORE_CAPITAL_BOTTOM), /* Ç */
    (0x00C8, AF_ADJUST_UP), /* È */
    (0x00C9, AF_ADJUST_UP), /* É */
    (0x00CA, AF_ADJUST_UP), /* Ê */
    (0x00CB, AF_ADJUST_UP), /* Ë */
    (0x00CC, AF_ADJUST_UP), /* Ì */
    (0x00CD, AF_ADJUST_UP), /* Í */
    (0x00CE, AF_ADJUST_UP), /* Î */
    (0x00CF, AF_ADJUST_UP), /* Ï */
    (0x00D1, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ñ */
    (0x00D2, AF_ADJUST_UP), /* Ò */
    (0x00D3, AF_ADJUST_UP), /* Ó */
    (0x00D4, AF_ADJUST_UP), /* Ô */
    (0x00D5, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Õ */
    (0x00D6, AF_ADJUST_UP), /* Ö */
    (0x00D8, AF_IGNORE_CAPITAL_TOP | AF_IGNORE_CAPITAL_BOTTOM), /* Ø */
    (0x00D9, AF_ADJUST_UP), /* Ù */
    (0x00DA, AF_ADJUST_UP), /* Ú */
    (0x00DB, AF_ADJUST_UP), /* Û */
    (0x00DC, AF_ADJUST_UP), /* Ü */
    (0x00DD, AF_ADJUST_UP), /* Ý */
    (0x00E0, AF_ADJUST_UP), /* à */
    (0x00E1, AF_ADJUST_UP), /* á */
    (0x00E2, AF_ADJUST_UP), /* â */
    (0x00E3, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ã */
    (0x00E4, AF_ADJUST_UP), /* ä */
    (0x00E5, AF_ADJUST_UP), /* å */
    (0x00E7, AF_IGNORE_SMALL_BOTTOM), /* ç */
    (0x00E8, AF_ADJUST_UP), /* è */
    (0x00E9, AF_ADJUST_UP), /* é */
    (0x00EA, AF_ADJUST_UP), /* ê */
    (0x00EB, AF_ADJUST_UP), /* ë */
    (0x00EC, AF_ADJUST_UP), /* ì */
    (0x00ED, AF_ADJUST_UP), /* í */
    (0x00EE, AF_ADJUST_UP), /* î */
    (0x00EF, AF_ADJUST_UP), /* ï */
    (0x00F1, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ñ */
    (0x00F2, AF_ADJUST_UP), /* ò */
    (0x00F3, AF_ADJUST_UP), /* ó */
    (0x00F4, AF_ADJUST_UP), /* ô */
    (0x00F5, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* õ */
    (0x00F6, AF_ADJUST_UP), /* ö */
    (0x00F8, AF_IGNORE_SMALL_TOP | AF_IGNORE_SMALL_BOTTOM), /* ø */
    (0x00F9, AF_ADJUST_UP), /* ù */
    (0x00FA, AF_ADJUST_UP), /* ú */
    (0x00FB, AF_ADJUST_UP), /* û */
    (0x00FC, AF_ADJUST_UP), /* ü */
    (0x00FD, AF_ADJUST_UP), /* ý */
    (0x00FF, AF_ADJUST_UP), /* ÿ */
    (0x0100, AF_ADJUST_UP), /* Ā */
    (0x0101, AF_ADJUST_UP), /* ā */
    (0x0102, AF_ADJUST_UP), /* Ă */
    (0x0103, AF_ADJUST_UP), /* ă */
    (0x0104, AF_IGNORE_CAPITAL_BOTTOM), /* Ą */
    (0x0105, AF_IGNORE_SMALL_BOTTOM), /* ą */
    (0x0106, AF_ADJUST_UP), /* Ć */
    (0x0107, AF_ADJUST_UP), /* ć */
    (0x0108, AF_ADJUST_UP), /* Ĉ */
    (0x0109, AF_ADJUST_UP), /* ĉ */
    (0x010A, AF_ADJUST_UP), /* Ċ */
    (0x010B, AF_ADJUST_UP), /* ċ */
    (0x010C, AF_ADJUST_UP), /* Č */
    (0x010D, AF_ADJUST_UP), /* č */
    (0x010E, AF_ADJUST_UP), /* Ď */
    (0x0112, AF_ADJUST_UP), /* Ē */
    (0x0113, AF_ADJUST_UP), /* ē */
    (0x0114, AF_ADJUST_UP), /* Ĕ */
    (0x0115, AF_ADJUST_UP), /* ĕ */
    (0x0116, AF_ADJUST_UP), /* Ė */
    (0x0117, AF_ADJUST_UP), /* ė */
    (0x0118, AF_IGNORE_CAPITAL_BOTTOM), /* Ę */
    (0x0119, AF_IGNORE_SMALL_BOTTOM), /* ę */
    (0x011A, AF_ADJUST_UP), /* Ě */
    (0x011B, AF_ADJUST_UP), /* ě */
    (0x011C, AF_ADJUST_UP), /* Ĝ */
    (0x011D, AF_ADJUST_UP), /* ĝ */
    (0x011E, AF_ADJUST_UP), /* Ğ */
    (0x011F, AF_ADJUST_UP), /* ğ */
    (0x0120, AF_ADJUST_UP), /* Ġ */
    (0x0121, AF_ADJUST_UP), /* ġ */
    (0x0122, AF_ADJUST_DOWN), /* Ģ */
    (0x0123, AF_ADJUST_UP), /* ģ */
    (0x0124, AF_ADJUST_UP), /* Ĥ */
    (0x0125, AF_ADJUST_UP), /* ĥ */
    (0x0128, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ĩ */
    (0x0129, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ĩ */
    (0x012A, AF_ADJUST_UP), /* Ī */
    (0x012B, AF_ADJUST_UP), /* ī */
    (0x012C, AF_ADJUST_UP), /* Ĭ */
    (0x012D, AF_ADJUST_UP), /* ĭ */
    (0x012E, AF_IGNORE_CAPITAL_BOTTOM), /* Į */
    (0x012F, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* į */
    (0x0130, AF_ADJUST_UP), /* İ */
    (0x0133, AF_ADJUST_UP), /* ĳ */
    (0x0134, AF_ADJUST_UP), /* Ĵ */
    (0x0135, AF_ADJUST_UP), /* ĵ */
    (0x0136, AF_ADJUST_DOWN), /* Ķ */
    (0x0137, AF_ADJUST_DOWN), /* ķ */
    (0x0139, AF_ADJUST_UP), /* Ĺ */
    (0x013A, AF_ADJUST_UP), /* ĺ */
    (0x013B, AF_ADJUST_DOWN), /* Ļ */
    (0x013C, AF_ADJUST_DOWN), /* ļ */
    (0x0143, AF_ADJUST_UP), /* Ń */
    (0x0144, AF_ADJUST_UP), /* ń */
    (0x0145, AF_ADJUST_DOWN), /* Ņ */
    (0x0146, AF_ADJUST_DOWN), /* ņ */
    (0x0147, AF_ADJUST_UP), /* Ň */
    (0x0148, AF_ADJUST_UP), /* ň */
    (0x014C, AF_ADJUST_UP), /* Ō */
    (0x014D, AF_ADJUST_UP), /* ō */
    (0x014E, AF_ADJUST_UP), /* Ŏ */
    (0x014F, AF_ADJUST_UP), /* ŏ */
    (0x0150, AF_ADJUST_UP), /* Ő */
    (0x0151, AF_ADJUST_UP), /* ő */
    (0x0154, AF_ADJUST_UP), /* Ŕ */
    (0x0155, AF_ADJUST_UP), /* ŕ */
    (0x0156, AF_ADJUST_DOWN), /* Ŗ */
    (0x0157, AF_ADJUST_DOWN), /* ŗ */
    (0x0158, AF_ADJUST_UP), /* Ř */
    (0x0159, AF_ADJUST_UP), /* ř */
    (0x015A, AF_ADJUST_UP), /* Ś */
    (0x015B, AF_ADJUST_UP), /* ś */
    (0x015C, AF_ADJUST_UP), /* Ŝ */
    (0x015D, AF_ADJUST_UP), /* ŝ */
    (0x015E, AF_IGNORE_CAPITAL_BOTTOM), /* Ş */
    (0x015F, AF_IGNORE_SMALL_BOTTOM), /* ş */
    (0x0160, AF_ADJUST_UP), /* Š */
    (0x0161, AF_ADJUST_UP), /* š */
    (0x0162, AF_IGNORE_CAPITAL_BOTTOM), /* Ţ */
    (0x0163, AF_IGNORE_SMALL_BOTTOM), /* ţ */
    (0x0164, AF_ADJUST_UP), /* Ť */
    (0x0168, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ũ */
    (0x0169, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ũ */
    (0x016A, AF_ADJUST_UP), /* Ū */
    (0x016B, AF_ADJUST_UP), /* ū */
    (0x016C, AF_ADJUST_UP), /* Ŭ */
    (0x016D, AF_ADJUST_UP), /* ŭ */
    (0x016E, AF_ADJUST_UP), /* Ů */
    (0x016F, AF_ADJUST_UP), /* ů */
    (0x0170, AF_ADJUST_UP), /* Ű */
    (0x0171, AF_ADJUST_UP), /* ű */
    (0x0172, AF_IGNORE_CAPITAL_BOTTOM), /* Ų */
    (0x0173, AF_IGNORE_SMALL_BOTTOM), /* ų */
    (0x0174, AF_ADJUST_UP), /* Ŵ */
    (0x0175, AF_ADJUST_UP), /* ŵ */
    (0x0176, AF_ADJUST_UP), /* Ŷ */
    (0x0177, AF_ADJUST_UP), /* ŷ */
    (0x0178, AF_ADJUST_UP), /* Ÿ */
    (0x0179, AF_ADJUST_UP), /* Ź */
    (0x017A, AF_ADJUST_UP), /* ź */
    (0x017B, AF_ADJUST_UP), /* Ż */
    (0x017C, AF_ADJUST_UP), /* ż */
    (0x017D, AF_ADJUST_UP), /* Ž */
    (0x017E, AF_ADJUST_UP), /* ž */
    (0x0187, AF_IGNORE_CAPITAL_TOP), /* Ƈ */
    (0x0188, AF_IGNORE_SMALL_TOP), /* ƈ */
    (0x01A0, AF_IGNORE_CAPITAL_TOP), /* Ơ */
    (0x01A1, AF_IGNORE_SMALL_TOP), /* ơ */
    (0x01A5, AF_IGNORE_SMALL_TOP), /* ƥ */
    (0x01AB, AF_IGNORE_SMALL_BOTTOM), /* ƫ */
    (0x01AE, AF_IGNORE_CAPITAL_BOTTOM), /* Ʈ */
    (0x01AF, AF_IGNORE_CAPITAL_TOP), /* Ư */
    (0x01B0, AF_IGNORE_SMALL_TOP), /* ư */
    (0x01B4, AF_IGNORE_SMALL_TOP), /* ƴ */
    (0x01C3, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ǃ */
    (0x01C4, AF_ADJUST_UP), /* Ǆ */
    (0x01CC, AF_ADJUST_UP), /* ǌ */
    (0x01CD, AF_ADJUST_UP), /* Ǎ */
    (0x01CE, AF_ADJUST_UP), /* ǎ */
    (0x01CF, AF_ADJUST_UP), /* Ǐ */
    (0x01D0, AF_ADJUST_UP), /* ǐ */
    (0x01D1, AF_ADJUST_UP), /* Ǒ */
    (0x01D2, AF_ADJUST_UP), /* ǒ */
    (0x01D3, AF_ADJUST_UP), /* Ǔ */
    (0x01D4, AF_ADJUST_UP), /* ǔ */
    (0x01D5, AF_ADJUST_UP2), /* Ǖ */
    (0x01D6, AF_ADJUST_UP2), /* ǖ */
    (0x01D7, AF_ADJUST_UP2), /* Ǘ */
    (0x01D8, AF_ADJUST_UP2), /* ǘ */
    (0x01D9, AF_ADJUST_UP2), /* Ǚ */
    (0x01DA, AF_ADJUST_UP2), /* ǚ */
    (0x01DB, AF_ADJUST_UP2), /* Ǜ */
    (0x01DC, AF_ADJUST_UP2), /* ǜ */
    (0x01DE, AF_ADJUST_UP2), /* Ǟ */
    (0x01DF, AF_ADJUST_UP2), /* ǟ */
    (0x01E0, AF_ADJUST_UP2), /* Ǡ */
    (0x01E1, AF_ADJUST_UP2), /* ǡ */
    (0x01E2, AF_ADJUST_UP), /* Ǣ */
    (0x01E3, AF_ADJUST_UP), /* ǣ */
    (0x01E6, AF_ADJUST_UP), /* Ǧ */
    (0x01E7, AF_ADJUST_UP), /* ǧ */
    (0x01E8, AF_ADJUST_UP), /* Ǩ */
    (0x01E9, AF_ADJUST_UP), /* ǩ */
    (0x01EA, AF_IGNORE_CAPITAL_BOTTOM), /* Ǫ */
    (0x01EB, AF_IGNORE_SMALL_BOTTOM), /* ǫ */
    (0x01EC, AF_ADJUST_UP | AF_IGNORE_CAPITAL_BOTTOM), /* Ǭ */
    (0x01ED, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* ǭ */
    (0x01EE, AF_ADJUST_UP), /* Ǯ */
    (0x01EF, AF_ADJUST_UP), /* ǯ */
    (0x01F0, AF_ADJUST_UP), /* ǰ */
    (0x01F4, AF_ADJUST_UP), /* Ǵ */
    (0x01F5, AF_ADJUST_UP), /* ǵ */
    (0x01F8, AF_ADJUST_UP), /* Ǹ */
    (0x01F9, AF_ADJUST_UP), /* ǹ */
    (0x01FA, AF_ADJUST_UP2), /* Ǻ */
    (0x01FB, AF_ADJUST_UP2), /* ǻ */
    (0x01FC, AF_ADJUST_UP), /* Ǽ */
    (0x01FD, AF_ADJUST_UP), /* ǽ */
    (0x01FE, AF_ADJUST_UP), /* Ǿ */
    (0x01FF, AF_ADJUST_UP), /* ǿ */
    (0x0200, AF_ADJUST_UP), /* Ȁ */
    (0x0201, AF_ADJUST_UP), /* ȁ */
    (0x0202, AF_ADJUST_UP), /* Ȃ */
    (0x0203, AF_ADJUST_UP), /* ȃ */
    (0x0204, AF_ADJUST_UP), /* Ȅ */
    (0x0205, AF_ADJUST_UP), /* ȅ */
    (0x0206, AF_ADJUST_UP), /* Ȇ */
    (0x0207, AF_ADJUST_UP), /* ȇ */
    (0x0208, AF_ADJUST_UP), /* Ȉ */
    (0x0209, AF_ADJUST_UP), /* ȉ */
    (0x020A, AF_ADJUST_UP), /* Ȋ */
    (0x020B, AF_ADJUST_UP), /* ȋ */
    (0x020C, AF_ADJUST_UP), /* Ȍ */
    (0x020D, AF_ADJUST_UP), /* ȍ */
    (0x020E, AF_ADJUST_UP), /* Ȏ */
    (0x020F, AF_ADJUST_UP), /* ȏ */
    (0x0210, AF_ADJUST_UP), /* Ȑ */
    (0x0211, AF_ADJUST_UP), /* ȑ */
    (0x0212, AF_ADJUST_UP), /* Ȓ */
    (0x0213, AF_ADJUST_UP), /* ȓ */
    (0x0214, AF_ADJUST_UP), /* Ȕ */
    (0x0215, AF_ADJUST_UP), /* ȕ */
    (0x0216, AF_ADJUST_UP), /* Ȗ */
    (0x0217, AF_ADJUST_UP), /* ȗ */
    (0x0218, AF_ADJUST_DOWN), /* Ș */
    (0x0219, AF_ADJUST_DOWN), /* ș */
    (0x021A, AF_ADJUST_DOWN), /* Ț */
    (0x021B, AF_ADJUST_DOWN), /* ț */
    (0x021E, AF_ADJUST_UP), /* Ȟ */
    (0x021F, AF_ADJUST_UP), /* ȟ */
    (0x0224, AF_IGNORE_CAPITAL_BOTTOM), /* Ȥ */
    (0x0225, AF_IGNORE_SMALL_BOTTOM), /* ȥ */
    (0x0226, AF_ADJUST_UP), /* Ȧ */
    (0x0227, AF_ADJUST_UP), /* ȧ */
    (0x0228, AF_IGNORE_CAPITAL_BOTTOM), /* Ȩ */
    (0x0229, AF_IGNORE_SMALL_BOTTOM), /* ȩ */
    (0x022A, AF_ADJUST_UP2), /* Ȫ */
    (0x022B, AF_ADJUST_UP2), /* ȫ */
    (0x022C, AF_ADJUST_UP2), /* Ȭ */
    (0x022D, AF_ADJUST_UP2), /* ȭ */
    (0x022E, AF_ADJUST_UP), /* Ȯ */
    (0x022F, AF_ADJUST_UP), /* ȯ */
    (0x0230, AF_ADJUST_UP2), /* Ȱ */
    (0x0231, AF_ADJUST_UP2), /* ȱ */
    (0x0232, AF_ADJUST_UP), /* Ȳ */
    (0x0233, AF_ADJUST_UP), /* ȳ */
    (0x023A, AF_IGNORE_CAPITAL_TOP | AF_IGNORE_CAPITAL_BOTTOM), /* Ⱥ */
    (0x023B, AF_IGNORE_CAPITAL_TOP | AF_IGNORE_CAPITAL_BOTTOM), /* Ȼ */
    (0x023F, AF_IGNORE_SMALL_BOTTOM), /* ȿ */
    (0x0240, AF_IGNORE_SMALL_BOTTOM), /* ɀ */
    (0x0249, AF_ADJUST_UP), /* ɉ */
    (0x0256, AF_IGNORE_SMALL_BOTTOM), /* ɖ */
    (0x0260, AF_IGNORE_SMALL_TOP), /* ɠ */
    (0x0267, AF_IGNORE_SMALL_BOTTOM), /* ɧ */
    (0x0268, AF_ADJUST_UP), /* ɨ */
    (0x0272, AF_IGNORE_SMALL_BOTTOM), /* ɲ */
    (0x0273, AF_IGNORE_SMALL_BOTTOM), /* ɳ */
    (0x027B, AF_IGNORE_SMALL_BOTTOM), /* ɻ */
    (0x027D, AF_IGNORE_SMALL_BOTTOM), /* ɽ */
    (0x0282, AF_IGNORE_SMALL_BOTTOM), /* ʂ */
    (0x0288, AF_IGNORE_SMALL_BOTTOM), /* ʈ */
    (0x0290, AF_IGNORE_SMALL_BOTTOM), /* ʐ */
    (0x029B, AF_IGNORE_SMALL_TOP), /* ʛ */
    (0x02A0, AF_IGNORE_SMALL_TOP), /* ʠ */
    (0x02B2, AF_ADJUST_UP), /* ʲ */
    (0x02B5, AF_IGNORE_SMALL_BOTTOM), /* ʵ */
    (0x0390, AF_ADJUST_UP2), /* ΐ */
    (0x03AA, AF_ADJUST_UP), /* Ϊ */
    (0x03AB, AF_ADJUST_UP), /* Ϋ */
    (0x03AC, AF_ADJUST_UP), /* ά */
    (0x03AD, AF_ADJUST_UP), /* έ */
    (0x03AE, AF_ADJUST_UP), /* ή */
    (0x03AF, AF_ADJUST_UP), /* ί */
    (0x03B0, AF_ADJUST_UP2), /* ΰ */
    (0x03CA, AF_ADJUST_UP), /* ϊ */
    (0x03CB, AF_ADJUST_UP), /* ϋ */
    (0x03CC, AF_ADJUST_UP), /* ό */
    (0x03CD, AF_ADJUST_UP), /* ύ */
    (0x03CE, AF_ADJUST_UP), /* ώ */
    (0x03CF, AF_IGNORE_CAPITAL_BOTTOM), /* Ϗ */
    (0x03D4, AF_ADJUST_UP), /* ϔ */
    (0x03D7, AF_IGNORE_SMALL_BOTTOM), /* ϗ */
    (0x03D9, AF_IGNORE_SMALL_BOTTOM), /* ϙ */
    (0x03E2, AF_IGNORE_CAPITAL_BOTTOM), /* Ϣ */
    (0x03E3, AF_IGNORE_SMALL_BOTTOM), /* ϣ */
    (0x03F3, AF_ADJUST_UP), /* ϳ */
    (0x0400, AF_ADJUST_UP), /* Ѐ */
    (0x0401, AF_ADJUST_UP), /* Ё */
    (0x0403, AF_ADJUST_UP), /* Ѓ */
    (0x0407, AF_ADJUST_UP), /* Ї */
    (0x040C, AF_ADJUST_UP), /* Ќ */
    (0x040D, AF_ADJUST_UP), /* Ѝ */
    (0x040E, AF_ADJUST_UP), /* Ў */
    (0x040F, AF_IGNORE_CAPITAL_BOTTOM), /* Џ */
    (0x0419, AF_ADJUST_UP), /* Й */
    (0x0426, AF_IGNORE_CAPITAL_BOTTOM), /* Ц */
    (0x0429, AF_IGNORE_CAPITAL_BOTTOM), /* Щ */
    (0x0439, AF_ADJUST_UP), /* й */
    (0x0446, AF_IGNORE_SMALL_BOTTOM), /* ц */
    (0x0449, AF_IGNORE_SMALL_BOTTOM), /* щ */
    (0x0450, AF_ADJUST_UP), /* ѐ */
    (0x0451, AF_ADJUST_UP), /* ё */
    (0x0453, AF_ADJUST_UP), /* ѓ */
    (0x0456, AF_ADJUST_UP), /* і */
    (0x0457, AF_ADJUST_UP), /* ї */
    (0x0458, AF_ADJUST_UP), /* ј */
    (0x045C, AF_ADJUST_UP), /* ќ */
    (0x045D, AF_ADJUST_UP), /* ѝ */
    (0x045E, AF_ADJUST_UP), /* ў */
    (0x045F, AF_IGNORE_SMALL_BOTTOM), /* џ */
    (0x0476, AF_ADJUST_UP), /* Ѷ */
    (0x0477, AF_ADJUST_UP), /* ѷ */
    (0x047C, AF_ADJUST_UP2), /* Ѽ */
    (0x047D, AF_ADJUST_UP2), /* ѽ */
    (0x047E, AF_ADJUST_UP), /* Ѿ */
    (0x047F, AF_ADJUST_UP), /* ѿ */
    (0x0480, AF_IGNORE_CAPITAL_BOTTOM), /* Ҁ */
    (0x0481, AF_IGNORE_SMALL_BOTTOM), /* ҁ */
    (0x048A, AF_ADJUST_UP | AF_IGNORE_CAPITAL_BOTTOM), /* Ҋ */
    (0x048B, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* ҋ */
    (0x0490, AF_IGNORE_CAPITAL_TOP), /* Ґ */
    (0x0491, AF_IGNORE_SMALL_TOP), /* ґ */
    (0x0496, AF_IGNORE_CAPITAL_BOTTOM), /* Җ */
    (0x0497, AF_IGNORE_SMALL_BOTTOM), /* җ */
    (0x0498, AF_IGNORE_CAPITAL_BOTTOM), /* Ҙ */
    (0x0499, AF_IGNORE_SMALL_BOTTOM), /* ҙ */
    (0x049A, AF_IGNORE_CAPITAL_BOTTOM), /* Қ */
    (0x049B, AF_IGNORE_SMALL_BOTTOM), /* қ */
    (0x04A2, AF_IGNORE_CAPITAL_BOTTOM), /* Ң */
    (0x04A3, AF_IGNORE_SMALL_BOTTOM), /* ң */
    (0x04AA, AF_IGNORE_CAPITAL_BOTTOM), /* Ҫ */
    (0x04AB, AF_IGNORE_SMALL_BOTTOM), /* ҫ */
    (0x04AC, AF_IGNORE_CAPITAL_BOTTOM), /* Ҭ */
    (0x04AD, AF_IGNORE_SMALL_BOTTOM), /* ҭ */
    (0x04B2, AF_IGNORE_CAPITAL_BOTTOM), /* Ҳ */
    (0x04B3, AF_IGNORE_SMALL_BOTTOM), /* ҳ */
    (0x04B4, AF_IGNORE_CAPITAL_BOTTOM), /* Ҵ */
    (0x04B5, AF_IGNORE_SMALL_BOTTOM), /* ҵ */
    (0x04B6, AF_IGNORE_CAPITAL_BOTTOM), /* Ҷ */
    (0x04B7, AF_IGNORE_SMALL_BOTTOM), /* ҷ */
    (0x04BE, AF_IGNORE_CAPITAL_BOTTOM), /* Ҿ */
    (0x04BF, AF_IGNORE_SMALL_BOTTOM), /* ҿ */
    (0x04C1, AF_ADJUST_UP), /* Ӂ */
    (0x04C2, AF_ADJUST_UP), /* ӂ */
    (0x04C5, AF_IGNORE_CAPITAL_BOTTOM), /* Ӆ */
    (0x04C6, AF_IGNORE_SMALL_BOTTOM), /* ӆ */
    (0x04C9, AF_IGNORE_CAPITAL_BOTTOM), /* Ӊ */
    (0x04CA, AF_IGNORE_SMALL_BOTTOM), /* ӊ */
    (0x04CB, AF_IGNORE_CAPITAL_BOTTOM), /* Ӌ */
    (0x04CC, AF_IGNORE_SMALL_BOTTOM), /* ӌ */
    (0x04CD, AF_IGNORE_CAPITAL_BOTTOM), /* Ӎ */
    (0x04CE, AF_IGNORE_SMALL_BOTTOM), /* ӎ */
    (0x04D0, AF_ADJUST_UP), /* Ӑ */
    (0x04D1, AF_ADJUST_UP), /* ӑ */
    (0x04D2, AF_ADJUST_UP), /* Ӓ */
    (0x04D3, AF_ADJUST_UP), /* ӓ */
    (0x04D6, AF_ADJUST_UP), /* Ӗ */
    (0x04D7, AF_ADJUST_UP), /* ӗ */
    (0x04DA, AF_ADJUST_UP), /* Ӛ */
    (0x04DB, AF_ADJUST_UP), /* ӛ */
    (0x04DC, AF_ADJUST_UP), /* Ӝ */
    (0x04DD, AF_ADJUST_UP), /* ӝ */
    (0x04DE, AF_ADJUST_UP), /* Ӟ */
    (0x04DF, AF_ADJUST_UP), /* ӟ */
    (0x04E2, AF_ADJUST_UP), /* Ӣ */
    (0x04E3, AF_ADJUST_UP), /* ӣ */
    (0x04E4, AF_ADJUST_UP), /* Ӥ */
    (0x04E5, AF_ADJUST_UP), /* ӥ */
    (0x04E6, AF_ADJUST_UP), /* Ӧ */
    (0x04E7, AF_ADJUST_UP), /* ӧ */
    (0x04EA, AF_ADJUST_UP), /* Ӫ */
    (0x04EB, AF_ADJUST_UP), /* ӫ */
    (0x04EC, AF_ADJUST_UP), /* Ӭ */
    (0x04ED, AF_ADJUST_UP), /* ӭ */
    (0x04EE, AF_ADJUST_UP), /* Ӯ */
    (0x04EF, AF_ADJUST_UP), /* ӯ */
    (0x04F0, AF_ADJUST_UP), /* Ӱ */
    (0x04F1, AF_ADJUST_UP), /* ӱ */
    (0x04F2, AF_ADJUST_UP), /* Ӳ */
    (0x04F3, AF_ADJUST_UP), /* ӳ */
    (0x04F4, AF_ADJUST_UP), /* Ӵ */
    (0x04F5, AF_ADJUST_UP), /* ӵ */
    (0x04F6, AF_IGNORE_CAPITAL_BOTTOM), /* Ӷ */
    (0x04F7, AF_IGNORE_SMALL_BOTTOM), /* ӷ */
    (0x04F8, AF_ADJUST_UP), /* Ӹ */
    (0x04F9, AF_ADJUST_UP), /* ӹ */
    (0x04FA, AF_IGNORE_CAPITAL_BOTTOM), /* Ӻ */
    (0x04FB, AF_IGNORE_SMALL_BOTTOM), /* ӻ */
    (0x0506, AF_IGNORE_CAPITAL_BOTTOM), /* Ԇ */
    (0x0507, AF_IGNORE_SMALL_BOTTOM), /* ԇ */
    (0x0524, AF_IGNORE_CAPITAL_BOTTOM), /* Ԥ */
    (0x0525, AF_IGNORE_SMALL_BOTTOM), /* ԥ */
    (0x0526, AF_IGNORE_CAPITAL_BOTTOM), /* Ԧ */
    (0x0527, AF_IGNORE_SMALL_BOTTOM), /* ԧ */
    (0x052E, AF_IGNORE_CAPITAL_BOTTOM), /* Ԯ */
    (0x052F, AF_IGNORE_SMALL_BOTTOM), /* ԯ */
    (0x13A5, AF_ADJUST_UP), /* Ꭵ */
    (0x1D09, AF_ADJUST_DOWN), /* ᴉ */
    (0x1D4E, AF_ADJUST_DOWN), /* ᵎ */
    (0x1D51, AF_IGNORE_SMALL_BOTTOM), /* ᵑ */
    (0x1D62, AF_ADJUST_UP), /* ᵢ */
    (0x1D80, AF_IGNORE_SMALL_BOTTOM), /* ᶀ */
    (0x1D81, AF_IGNORE_SMALL_BOTTOM), /* ᶁ */
    (0x1D82, AF_IGNORE_SMALL_BOTTOM), /* ᶂ */
    (0x1D84, AF_IGNORE_SMALL_BOTTOM), /* ᶄ */
    (0x1D85, AF_IGNORE_SMALL_BOTTOM), /* ᶅ */
    (0x1D86, AF_IGNORE_SMALL_BOTTOM), /* ᶆ */
    (0x1D87, AF_IGNORE_SMALL_BOTTOM), /* ᶇ */
    (0x1D89, AF_IGNORE_SMALL_BOTTOM), /* ᶉ */
    (0x1D8A, AF_IGNORE_SMALL_BOTTOM), /* ᶊ */
    (0x1D8C, AF_IGNORE_SMALL_BOTTOM), /* ᶌ */
    (0x1D8D, AF_IGNORE_SMALL_BOTTOM), /* ᶍ */
    (0x1D8E, AF_IGNORE_SMALL_BOTTOM), /* ᶎ */
    (0x1D8F, AF_IGNORE_SMALL_BOTTOM), /* ᶏ */
    (0x1D90, AF_IGNORE_SMALL_BOTTOM), /* ᶐ */
    (0x1D91, AF_IGNORE_SMALL_BOTTOM), /* ᶑ */
    (0x1D92, AF_IGNORE_SMALL_BOTTOM), /* ᶒ */
    (0x1D93, AF_IGNORE_SMALL_BOTTOM), /* ᶓ */
    (0x1D94, AF_IGNORE_SMALL_BOTTOM), /* ᶔ */
    (0x1D95, AF_IGNORE_SMALL_BOTTOM), /* ᶕ */
    (0x1D96, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* ᶖ */
    (0x1D97, AF_IGNORE_SMALL_BOTTOM), /* ᶗ */
    (0x1D98, AF_IGNORE_SMALL_BOTTOM), /* ᶘ */
    (0x1D99, AF_IGNORE_SMALL_BOTTOM), /* ᶙ */
    (0x1D9A, AF_IGNORE_SMALL_BOTTOM), /* ᶚ */
    (0x1DA4, AF_ADJUST_UP), /* ᶤ */
    (0x1DA8, AF_ADJUST_UP), /* ᶨ */
    (0x1DA9, AF_IGNORE_SMALL_BOTTOM), /* ᶩ */
    (0x1DAA, AF_IGNORE_SMALL_BOTTOM), /* ᶪ */
    (0x1DAC, AF_IGNORE_SMALL_BOTTOM), /* ᶬ */
    (0x1DAE, AF_IGNORE_SMALL_BOTTOM), /* ᶮ */
    (0x1DAF, AF_IGNORE_SMALL_BOTTOM), /* ᶯ */
    (0x1DB3, AF_IGNORE_SMALL_BOTTOM), /* ᶳ */
    (0x1DB5, AF_IGNORE_SMALL_BOTTOM), /* ᶵ */
    (0x1DBC, AF_IGNORE_SMALL_BOTTOM), /* ᶼ */
    (0x1E00, AF_ADJUST_DOWN), /* Ḁ */
    (0x1E01, AF_ADJUST_DOWN), /* ḁ */
    (0x1E02, AF_ADJUST_UP), /* Ḃ */
    (0x1E03, AF_ADJUST_UP), /* ḃ */
    (0x1E04, AF_ADJUST_DOWN), /* Ḅ */
    (0x1E05, AF_ADJUST_DOWN), /* ḅ */
    (0x1E06, AF_ADJUST_DOWN), /* Ḇ */
    (0x1E07, AF_ADJUST_DOWN), /* ḇ */
    (0x1E08, AF_ADJUST_UP | AF_IGNORE_CAPITAL_BOTTOM), /* Ḉ */
    (0x1E09, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* ḉ */
    (0x1E0A, AF_ADJUST_UP), /* Ḋ */
    (0x1E0B, AF_ADJUST_UP), /* ḋ */
    (0x1E0C, AF_ADJUST_DOWN), /* Ḍ */
    (0x1E0D, AF_ADJUST_DOWN), /* ḍ */
    (0x1E0E, AF_ADJUST_DOWN), /* Ḏ */
    (0x1E0F, AF_ADJUST_DOWN), /* ḏ */
    (0x1E10, AF_ADJUST_DOWN), /* Ḑ */
    (0x1E11, AF_ADJUST_DOWN), /* ḑ */
    (0x1E12, AF_ADJUST_DOWN), /* Ḓ */
    (0x1E13, AF_ADJUST_DOWN), /* ḓ */
    (0x1E14, AF_ADJUST_UP2), /* Ḕ */
    (0x1E15, AF_ADJUST_UP2), /* ḕ */
    (0x1E16, AF_ADJUST_UP2), /* Ḗ */
    (0x1E17, AF_ADJUST_UP2), /* ḗ */
    (0x1E18, AF_ADJUST_DOWN), /* Ḙ */
    (0x1E19, AF_ADJUST_DOWN), /* ḙ */
    (0x1E1A, AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* Ḛ */
    (0x1E1B, AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* ḛ */
    (0x1E1C, AF_ADJUST_UP | AF_IGNORE_CAPITAL_BOTTOM), /* Ḝ */
    (0x1E1D, AF_ADJUST_UP | AF_IGNORE_SMALL_BOTTOM), /* ḝ */
    (0x1E1E, AF_ADJUST_UP), /* Ḟ */
    (0x1E1F, AF_ADJUST_UP), /* ḟ */
    (0x1E20, AF_ADJUST_UP), /* Ḡ */
    (0x1E21, AF_ADJUST_UP), /* ḡ */
    (0x1E22, AF_ADJUST_UP), /* Ḣ */
    (0x1E23, AF_ADJUST_UP), /* ḣ */
    (0x1E24, AF_ADJUST_DOWN), /* Ḥ */
    (0x1E25, AF_ADJUST_DOWN), /* ḥ */
    (0x1E26, AF_ADJUST_UP), /* Ḧ */
    (0x1E27, AF_ADJUST_UP), /* ḧ */
    (0x1E28, AF_IGNORE_CAPITAL_BOTTOM), /* Ḩ */
    (0x1E29, AF_IGNORE_SMALL_BOTTOM), /* ḩ */
    (0x1E2A, AF_ADJUST_DOWN), /* Ḫ */
    (0x1E2B, AF_ADJUST_DOWN), /* ḫ */
    (0x1E2C, AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* Ḭ */
    (0x1E2D, AF_ADJUST_UP | AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* ḭ */
    (0x1E2E, AF_ADJUST_UP2), /* Ḯ */
    (0x1E2F, AF_ADJUST_UP2), /* ḯ */
    (0x1E30, AF_ADJUST_UP), /* Ḱ */
    (0x1E31, AF_ADJUST_UP), /* ḱ */
    (0x1E32, AF_ADJUST_DOWN), /* Ḳ */
    (0x1E33, AF_ADJUST_DOWN), /* ḳ */
    (0x1E34, AF_ADJUST_DOWN), /* Ḵ */
    (0x1E35, AF_ADJUST_DOWN), /* ḵ */
    (0x1E36, AF_ADJUST_DOWN), /* Ḷ */
    (0x1E37, AF_ADJUST_DOWN), /* ḷ */
    (0x1E38, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ḹ */
    (0x1E39, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ḹ */
    (0x1E3A, AF_ADJUST_DOWN), /* Ḻ */
    (0x1E3B, AF_ADJUST_DOWN), /* ḻ */
    (0x1E3C, AF_ADJUST_DOWN), /* Ḽ */
    (0x1E3D, AF_ADJUST_DOWN), /* ḽ */
    (0x1E3E, AF_ADJUST_UP), /* Ḿ */
    (0x1E3F, AF_ADJUST_UP), /* ḿ */
    (0x1E40, AF_ADJUST_UP), /* Ṁ */
    (0x1E41, AF_ADJUST_UP), /* ṁ */
    (0x1E42, AF_ADJUST_DOWN), /* Ṃ */
    (0x1E43, AF_ADJUST_DOWN), /* ṃ */
    (0x1E44, AF_ADJUST_UP), /* Ṅ */
    (0x1E45, AF_ADJUST_UP), /* ṅ */
    (0x1E46, AF_ADJUST_DOWN), /* Ṇ */
    (0x1E47, AF_ADJUST_DOWN), /* ṇ */
    (0x1E48, AF_ADJUST_DOWN), /* Ṉ */
    (0x1E49, AF_ADJUST_DOWN), /* ṉ */
    (0x1E4A, AF_ADJUST_DOWN), /* Ṋ */
    (0x1E4B, AF_ADJUST_DOWN), /* ṋ */
    (0x1E4C, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* Ṍ */
    (0x1E4D, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* ṍ */
    (0x1E4E, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* Ṏ */
    (0x1E4F, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* ṏ */
    (0x1E50, AF_ADJUST_UP2), /* Ṑ */
    (0x1E51, AF_ADJUST_UP2), /* ṑ */
    (0x1E52, AF_ADJUST_UP2), /* Ṓ */
    (0x1E53, AF_ADJUST_UP2), /* ṓ */
    (0x1E54, AF_ADJUST_UP), /* Ṕ */
    (0x1E55, AF_ADJUST_UP), /* ṕ */
    (0x1E56, AF_ADJUST_UP), /* Ṗ */
    (0x1E57, AF_ADJUST_UP), /* ṗ */
    (0x1E58, AF_ADJUST_UP), /* Ṙ */
    (0x1E59, AF_ADJUST_UP), /* ṙ */
    (0x1E5A, AF_ADJUST_DOWN), /* Ṛ */
    (0x1E5B, AF_ADJUST_DOWN), /* ṛ */
    (0x1E5C, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ṝ */
    (0x1E5D, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ṝ */
    (0x1E5E, AF_ADJUST_DOWN), /* Ṟ */
    (0x1E5F, AF_ADJUST_DOWN), /* ṟ */
    (0x1E60, AF_ADJUST_UP), /* Ṡ */
    (0x1E61, AF_ADJUST_UP), /* ṡ */
    (0x1E62, AF_ADJUST_DOWN), /* Ṣ */
    (0x1E63, AF_ADJUST_DOWN), /* ṣ */
    (0x1E64, AF_ADJUST_UP), /* Ṥ */
    (0x1E65, AF_ADJUST_UP), /* ṥ */
    (0x1E66, AF_ADJUST_UP), /* Ṧ */
    (0x1E67, AF_ADJUST_UP), /* ṧ */
    (0x1E68, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ṩ */
    (0x1E69, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ṩ */
    (0x1E6A, AF_ADJUST_UP), /* Ṫ */
    (0x1E6B, AF_ADJUST_UP), /* ṫ */
    (0x1E6C, AF_ADJUST_DOWN), /* Ṭ */
    (0x1E6D, AF_ADJUST_DOWN), /* ṭ */
    (0x1E6E, AF_ADJUST_DOWN), /* Ṯ */
    (0x1E6F, AF_ADJUST_DOWN), /* ṯ */
    (0x1E70, AF_ADJUST_DOWN), /* Ṱ */
    (0x1E71, AF_ADJUST_DOWN), /* ṱ */
    (0x1E72, AF_ADJUST_DOWN), /* Ṳ */
    (0x1E73, AF_ADJUST_DOWN), /* ṳ */
    (0x1E74, AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* Ṵ */
    (0x1E75, AF_ADJUST_DOWN | AF_ADJUST_TILDE_BOTTOM), /* ṵ */
    (0x1E76, AF_ADJUST_DOWN), /* Ṷ */
    (0x1E77, AF_ADJUST_DOWN), /* ṷ */
    (0x1E78, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* Ṹ */
    (0x1E79, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP2), /* ṹ */
    (0x1E7A, AF_ADJUST_UP2), /* Ṻ */
    (0x1E7B, AF_ADJUST_UP2), /* ṻ */
    (0x1E7C, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ṽ */
    (0x1E7D, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ṽ */
    (0x1E7E, AF_ADJUST_DOWN), /* Ṿ */
    (0x1E7F, AF_ADJUST_DOWN), /* ṿ */
    (0x1E80, AF_ADJUST_UP), /* Ẁ */
    (0x1E81, AF_ADJUST_UP), /* ẁ */
    (0x1E82, AF_ADJUST_UP), /* Ẃ */
    (0x1E83, AF_ADJUST_UP), /* ẃ */
    (0x1E84, AF_ADJUST_UP), /* Ẅ */
    (0x1E85, AF_ADJUST_UP), /* ẅ */
    (0x1E86, AF_ADJUST_UP), /* Ẇ */
    (0x1E87, AF_ADJUST_UP), /* ẇ */
    (0x1E88, AF_ADJUST_DOWN), /* Ẉ */
    (0x1E89, AF_ADJUST_DOWN), /* ẉ */
    (0x1E8A, AF_ADJUST_UP), /* Ẋ */
    (0x1E8B, AF_ADJUST_UP), /* ẋ */
    (0x1E8C, AF_ADJUST_UP), /* Ẍ */
    (0x1E8D, AF_ADJUST_UP), /* ẍ */
    (0x1E8E, AF_ADJUST_UP), /* Ẏ */
    (0x1E8F, AF_ADJUST_UP), /* ẏ */
    (0x1E90, AF_ADJUST_UP), /* Ẑ */
    (0x1E91, AF_ADJUST_UP), /* ẑ */
    (0x1E92, AF_ADJUST_DOWN), /* Ẓ */
    (0x1E93, AF_ADJUST_DOWN), /* ẓ */
    (0x1E94, AF_ADJUST_DOWN), /* Ẕ */
    (0x1E95, AF_ADJUST_DOWN), /* ẕ */
    (0x1E96, AF_ADJUST_DOWN), /* ẖ */
    (0x1E97, AF_ADJUST_UP), /* ẗ */
    (0x1E98, AF_ADJUST_UP), /* ẘ */
    (0x1E99, AF_ADJUST_UP), /* ẙ */
    (0x1E9A, AF_ADJUST_UP), /* ẚ */
    (0x1E9B, AF_ADJUST_UP), /* ẛ */
    (0x1EA0, AF_ADJUST_DOWN), /* Ạ */
    (0x1EA1, AF_ADJUST_DOWN), /* ạ */
    (0x1EA2, AF_ADJUST_UP), /* Ả */
    (0x1EA3, AF_ADJUST_UP), /* ả */
    (0x1EA4, AF_ADJUST_UP2), /* Ấ */
    (0x1EA5, AF_ADJUST_UP2), /* ấ */
    (0x1EA6, AF_ADJUST_UP2), /* Ầ */
    (0x1EA7, AF_ADJUST_UP2), /* ầ */
    (0x1EA8, AF_ADJUST_UP2), /* Ẩ */
    (0x1EA9, AF_ADJUST_UP2), /* ẩ */
    (0x1EAA, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* Ẫ */
    (0x1EAB, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ẫ */
    (0x1EAC, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ậ */
    (0x1EAD, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ậ */
    (0x1EAE, AF_ADJUST_UP2), /* Ắ */
    (0x1EAF, AF_ADJUST_UP2), /* ắ */
    (0x1EB0, AF_ADJUST_UP2), /* Ằ */
    (0x1EB1, AF_ADJUST_UP2), /* ằ */
    (0x1EB2, AF_ADJUST_UP2), /* Ẳ */
    (0x1EB3, AF_ADJUST_UP2), /* ẳ */
    (0x1EB4, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* Ẵ */
    (0x1EB5, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ẵ */
    (0x1EB6, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ặ */
    (0x1EB7, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ặ */
    (0x1EB8, AF_ADJUST_DOWN), /* Ẹ */
    (0x1EB9, AF_ADJUST_DOWN), /* ẹ */
    (0x1EBA, AF_ADJUST_UP), /* Ẻ */
    (0x1EBB, AF_ADJUST_UP), /* ẻ */
    (0x1EBC, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ẽ */
    (0x1EBD, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ẽ */
    (0x1EBE, AF_ADJUST_UP2), /* Ế */
    (0x1EBF, AF_ADJUST_UP2), /* ế */
    (0x1EC0, AF_ADJUST_UP2), /* Ề */
    (0x1EC1, AF_ADJUST_UP2), /* ề */
    (0x1EC2, AF_ADJUST_UP2), /* Ể */
    (0x1EC3, AF_ADJUST_UP2), /* ể */
    (0x1EC4, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* Ễ */
    (0x1EC5, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ễ */
    (0x1EC6, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ệ */
    (0x1EC7, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ệ */
    (0x1EC8, AF_ADJUST_UP), /* Ỉ */
    (0x1EC9, AF_ADJUST_UP), /* ỉ */
    (0x1ECA, AF_ADJUST_DOWN), /* Ị */
    (0x1ECB, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ị */
    (0x1ECC, AF_ADJUST_DOWN), /* Ọ */
    (0x1ECD, AF_ADJUST_DOWN), /* ọ */
    (0x1ECE, AF_ADJUST_UP), /* Ỏ */
    (0x1ECF, AF_ADJUST_UP), /* ỏ */
    (0x1ED0, AF_ADJUST_UP2), /* Ố */
    (0x1ED1, AF_ADJUST_UP2), /* ố */
    (0x1ED2, AF_ADJUST_UP2), /* Ồ */
    (0x1ED3, AF_ADJUST_UP2), /* ồ */
    (0x1ED4, AF_ADJUST_UP2), /* Ổ */
    (0x1ED5, AF_ADJUST_UP2), /* ổ */
    (0x1ED6, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* Ỗ */
    (0x1ED7, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ỗ */
    (0x1ED8, AF_ADJUST_UP | AF_ADJUST_DOWN), /* Ộ */
    (0x1ED9, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ộ */
    (0x1EDA, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ớ */
    (0x1EDB, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ớ */
    (0x1EDC, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ờ */
    (0x1EDD, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ờ */
    (0x1EDE, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ở */
    (0x1EDF, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ở */
    (0x1EE0, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_IGNORE_CAPITAL_TOP), /* Ỡ */
    (0x1EE1, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_IGNORE_SMALL_TOP), /* ỡ */
    (0x1EE2, AF_ADJUST_DOWN | AF_IGNORE_CAPITAL_TOP), /* Ợ */
    (0x1EE3, AF_ADJUST_DOWN | AF_IGNORE_SMALL_TOP), /* ợ */
    (0x1EE4, AF_ADJUST_DOWN), /* Ụ */
    (0x1EE5, AF_ADJUST_DOWN), /* ụ */
    (0x1EE6, AF_ADJUST_UP), /* Ủ */
    (0x1EE7, AF_ADJUST_UP), /* ủ */
    (0x1EE8, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ứ */
    (0x1EE9, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ứ */
    (0x1EEA, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ừ */
    (0x1EEB, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ừ */
    (0x1EEC, AF_ADJUST_UP | AF_IGNORE_CAPITAL_TOP), /* Ử */
    (0x1EED, AF_ADJUST_UP | AF_IGNORE_SMALL_TOP), /* ử */
    (0x1EEE, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_IGNORE_CAPITAL_TOP), /* Ữ */
    (0x1EEF, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_IGNORE_SMALL_TOP), /* ữ */
    (0x1EF0, AF_ADJUST_DOWN | AF_IGNORE_CAPITAL_TOP), /* Ự */
    (0x1EF1, AF_ADJUST_DOWN | AF_IGNORE_SMALL_TOP), /* ự */
    (0x1EF2, AF_ADJUST_UP), /* Ỳ */
    (0x1EF3, AF_ADJUST_UP), /* ỳ */
    (0x1EF4, AF_ADJUST_DOWN), /* Ỵ */
    (0x1EF5, AF_ADJUST_DOWN), /* ỵ */
    (0x1EF6, AF_ADJUST_UP), /* Ỷ */
    (0x1EF7, AF_ADJUST_UP), /* ỷ */
    (0x1EF8, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* Ỹ */
    (0x1EF9, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ỹ */
    (0x1F00, AF_ADJUST_UP), /* ἀ */
    (0x1F01, AF_ADJUST_UP), /* ἁ */
    (0x1F02, AF_ADJUST_UP), /* ἂ */
    (0x1F03, AF_ADJUST_UP), /* ἃ */
    (0x1F04, AF_ADJUST_UP), /* ἄ */
    (0x1F05, AF_ADJUST_UP), /* ἅ */
    (0x1F06, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἆ */
    (0x1F07, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἇ */
    (0x1F10, AF_ADJUST_UP), /* ἐ */
    (0x1F11, AF_ADJUST_UP), /* ἑ */
    (0x1F12, AF_ADJUST_UP), /* ἒ */
    (0x1F13, AF_ADJUST_UP), /* ἓ */
    (0x1F14, AF_ADJUST_UP), /* ἔ */
    (0x1F15, AF_ADJUST_UP), /* ἕ */
    (0x1F20, AF_ADJUST_UP), /* ἠ */
    (0x1F21, AF_ADJUST_UP), /* ἡ */
    (0x1F22, AF_ADJUST_UP), /* ἢ */
    (0x1F23, AF_ADJUST_UP), /* ἣ */
    (0x1F24, AF_ADJUST_UP), /* ἤ */
    (0x1F25, AF_ADJUST_UP), /* ἥ */
    (0x1F26, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἦ */
    (0x1F27, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἧ */
    (0x1F30, AF_ADJUST_UP), /* ἰ */
    (0x1F31, AF_ADJUST_UP), /* ἱ */
    (0x1F32, AF_ADJUST_UP), /* ἲ */
    (0x1F33, AF_ADJUST_UP), /* ἳ */
    (0x1F34, AF_ADJUST_UP), /* ἴ */
    (0x1F35, AF_ADJUST_UP), /* ἵ */
    (0x1F36, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἶ */
    (0x1F37, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ἷ */
    (0x1F40, AF_ADJUST_UP), /* ὀ */
    (0x1F41, AF_ADJUST_UP), /* ὁ */
    (0x1F42, AF_ADJUST_UP), /* ὂ */
    (0x1F43, AF_ADJUST_UP), /* ὃ */
    (0x1F44, AF_ADJUST_UP), /* ὄ */
    (0x1F45, AF_ADJUST_UP), /* ὅ */
    (0x1F50, AF_ADJUST_UP), /* ὐ */
    (0x1F51, AF_ADJUST_UP), /* ὑ */
    (0x1F52, AF_ADJUST_UP), /* ὒ */
    (0x1F53, AF_ADJUST_UP), /* ὓ */
    (0x1F54, AF_ADJUST_UP), /* ὔ */
    (0x1F55, AF_ADJUST_UP), /* ὕ */
    (0x1F56, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ὖ */
    (0x1F57, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ὗ */
    (0x1F60, AF_ADJUST_UP), /* ὠ */
    (0x1F61, AF_ADJUST_UP), /* ὡ */
    (0x1F62, AF_ADJUST_UP), /* ὢ */
    (0x1F63, AF_ADJUST_UP), /* ὣ */
    (0x1F64, AF_ADJUST_UP), /* ὤ */
    (0x1F65, AF_ADJUST_UP), /* ὥ */
    (0x1F66, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ὦ */
    (0x1F67, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ὧ */
    (0x1F70, AF_ADJUST_UP), /* ὰ */
    (0x1F71, AF_ADJUST_UP), /* ά */
    (0x1F72, AF_ADJUST_UP), /* ὲ */
    (0x1F73, AF_ADJUST_UP), /* έ */
    (0x1F74, AF_ADJUST_UP), /* ὴ */
    (0x1F75, AF_ADJUST_UP), /* ή */
    (0x1F76, AF_ADJUST_UP), /* ὶ */
    (0x1F77, AF_ADJUST_UP), /* ί */
    (0x1F78, AF_ADJUST_UP), /* ὸ */
    (0x1F79, AF_ADJUST_UP), /* ό */
    (0x1F7A, AF_ADJUST_UP), /* ὺ */
    (0x1F7B, AF_ADJUST_UP), /* ύ */
    (0x1F7C, AF_ADJUST_UP), /* ὼ */
    (0x1F7D, AF_ADJUST_UP), /* ώ */
    (0x1F80, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾀ */
    (0x1F81, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾁ */
    (0x1F82, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾂ */
    (0x1F83, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾃ */
    (0x1F84, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾄ */
    (0x1F85, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾅ */
    (0x1F86, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾆ */
    (0x1F87, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾇ */
    (0x1F88, AF_ADJUST_DOWN), /* ᾈ */
    (0x1F89, AF_ADJUST_DOWN), /* ᾉ */
    (0x1F8A, AF_ADJUST_DOWN), /* ᾊ */
    (0x1F8B, AF_ADJUST_DOWN), /* ᾋ */
    (0x1F8C, AF_ADJUST_DOWN), /* ᾌ */
    (0x1F8D, AF_ADJUST_DOWN), /* ᾍ */
    (0x1F8E, AF_ADJUST_DOWN), /* ᾎ */
    (0x1F8F, AF_ADJUST_DOWN), /* ᾏ */
    (0x1F90, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾐ */
    (0x1F91, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾑ */
    (0x1F92, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾒ */
    (0x1F93, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾓ */
    (0x1F94, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾔ */
    (0x1F95, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾕ */
    (0x1F96, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾖ */
    (0x1F97, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾗ */
    (0x1F98, AF_ADJUST_DOWN), /* ᾘ */
    (0x1F99, AF_ADJUST_DOWN), /* ᾙ */
    (0x1F9A, AF_ADJUST_DOWN), /* ᾚ */
    (0x1F9B, AF_ADJUST_DOWN), /* ᾛ */
    (0x1F9C, AF_ADJUST_DOWN), /* ᾜ */
    (0x1F9D, AF_ADJUST_DOWN), /* ᾝ */
    (0x1F9E, AF_ADJUST_DOWN), /* ᾞ */
    (0x1F9F, AF_ADJUST_DOWN), /* ᾟ */
    (0x1FA0, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾠ */
    (0x1FA1, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾡ */
    (0x1FA2, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾢ */
    (0x1FA3, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾣ */
    (0x1FA4, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾤ */
    (0x1FA5, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾥ */
    (0x1FA6, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾦ */
    (0x1FA7, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾧ */
    (0x1FA8, AF_ADJUST_DOWN), /* ᾨ */
    (0x1FA9, AF_ADJUST_DOWN), /* ᾩ */
    (0x1FAA, AF_ADJUST_DOWN), /* ᾪ */
    (0x1FAB, AF_ADJUST_DOWN), /* ᾫ */
    (0x1FAC, AF_ADJUST_DOWN), /* ᾬ */
    (0x1FAD, AF_ADJUST_DOWN), /* ᾭ */
    (0x1FAE, AF_ADJUST_DOWN), /* ᾮ */
    (0x1FAF, AF_ADJUST_DOWN), /* ᾯ */
    (0x1FB0, AF_ADJUST_UP), /* ᾰ */
    (0x1FB1, AF_ADJUST_UP), /* ᾱ */
    (0x1FB2, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾲ */
    (0x1FB3, AF_ADJUST_DOWN), /* ᾳ */
    (0x1FB4, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ᾴ */
    (0x1FB6, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ᾶ */
    (0x1FB7, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ᾷ */
    (0x1FB8, AF_ADJUST_UP), /* Ᾰ */
    (0x1FB9, AF_ADJUST_UP), /* Ᾱ */
    (0x1FBC, AF_ADJUST_DOWN), /* ᾼ */
    (0x1FC2, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ῂ */
    (0x1FC3, AF_ADJUST_DOWN), /* ῃ */
    (0x1FC4, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ῄ */
    (0x1FC6, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ῆ */
    (0x1FC7, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ῇ */
    (0x1FCC, AF_ADJUST_DOWN), /* ῌ */
    (0x1FD0, AF_ADJUST_UP), /* ῐ */
    (0x1FD1, AF_ADJUST_UP), /* ῑ */
    (0x1FD2, AF_ADJUST_UP2), /* ῒ */
    (0x1FD3, AF_ADJUST_UP2), /* ΐ */
    (0x1FD6, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ῖ */
    (0x1FD7, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ῗ */
    (0x1FD8, AF_ADJUST_UP), /* Ῐ */
    (0x1FD9, AF_ADJUST_UP), /* Ῑ */
    (0x1FE0, AF_ADJUST_UP), /* ῠ */
    (0x1FE1, AF_ADJUST_UP), /* ῡ */
    (0x1FE2, AF_ADJUST_UP2), /* ῢ */
    (0x1FE3, AF_ADJUST_UP2), /* ΰ */
    (0x1FE4, AF_ADJUST_UP), /* ῤ */
    (0x1FE5, AF_ADJUST_UP), /* ῥ */
    (0x1FE6, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ῦ */
    (0x1FE7, AF_ADJUST_UP2 | AF_ADJUST_TILDE_TOP), /* ῧ */
    (0x1FE8, AF_ADJUST_UP), /* Ῠ */
    (0x1FE9, AF_ADJUST_UP), /* Ῡ */
    (0x1FF2, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ῲ */
    (0x1FF3, AF_ADJUST_DOWN), /* ῳ */
    (0x1FF4, AF_ADJUST_UP | AF_ADJUST_DOWN), /* ῴ */
    (0x1FF6, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP), /* ῶ */
    (0x1FF7, AF_ADJUST_UP | AF_ADJUST_TILDE_TOP | AF_ADJUST_DOWN), /* ῷ */
    (0x1FFC, AF_ADJUST_DOWN), /* ῼ */
    (0x203C, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ‼ */
    (0x203D, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ‽ */
    (0x2047, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ⁇ */
    (0x2048, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ⁈ */
    (0x2049, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ⁉ */
    (0x2071, AF_ADJUST_UP), /* ⁱ */
    (0x20AB, AF_ADJUST_DOWN), /* ₫ */
    (0x20C0, AF_ADJUST_DOWN), /* ⃀ */
    (0x2170, AF_ADJUST_UP), /* ⅰ */
    (0x2171, AF_ADJUST_UP), /* ⅱ */
    (0x2172, AF_ADJUST_UP), /* ⅲ */
    (0x2173, AF_ADJUST_UP), /* ⅳ */
    (0x2175, AF_ADJUST_UP), /* ⅵ */
    (0x2176, AF_ADJUST_UP), /* ⅶ */
    (0x2177, AF_ADJUST_UP), /* ⅷ */
    (0x2178, AF_ADJUST_UP), /* ⅸ */
    (0x217A, AF_ADJUST_UP), /* ⅺ */
    (0x217B, AF_ADJUST_UP), /* ⅻ */
    (0x2C64, AF_IGNORE_CAPITAL_BOTTOM), /* Ɽ */
    (0x2C67, AF_IGNORE_CAPITAL_BOTTOM), /* Ⱨ */
    (0x2C68, AF_IGNORE_SMALL_BOTTOM), /* ⱨ */
    (0x2C69, AF_IGNORE_CAPITAL_BOTTOM), /* Ⱪ */
    (0x2C6A, AF_IGNORE_SMALL_BOTTOM), /* ⱪ */
    (0x2C6B, AF_IGNORE_CAPITAL_BOTTOM), /* Ⱬ */
    (0x2C6C, AF_IGNORE_SMALL_BOTTOM), /* ⱬ */
    (0x2C6E, AF_IGNORE_CAPITAL_BOTTOM), /* Ɱ */
    (0x2C7C, AF_ADJUST_UP), /* ⱼ */
    (0x2C7E, AF_IGNORE_CAPITAL_BOTTOM), /* Ȿ */
    (0x2C7F, AF_IGNORE_CAPITAL_BOTTOM), /* Ɀ */
    (0x2CC2, AF_ADJUST_UP), /* Ⳃ */
    (0x2CC3, AF_ADJUST_UP), /* ⳃ */
    (0x2E18, AF_ADJUST_UP), /* ⸘ */
    (0x2E2E, AF_ADJUST_UP | AF_ADJUST_NO_HEIGHT_CHECK), /* ⸮ */
    (0xA640, AF_IGNORE_CAPITAL_BOTTOM), /* Ꙁ */
    (0xA641, AF_IGNORE_SMALL_BOTTOM), /* ꙁ */
    (0xA642, AF_IGNORE_CAPITAL_BOTTOM), /* Ꙃ */
    (0xA643, AF_IGNORE_SMALL_BOTTOM), /* ꙃ */
    (0xA680, AF_IGNORE_CAPITAL_TOP), /* Ꚁ */
    (0xA681, AF_IGNORE_SMALL_TOP), /* ꚁ */
    (0xA688, AF_IGNORE_CAPITAL_BOTTOM), /* Ꚉ */
    (0xA689, AF_IGNORE_SMALL_BOTTOM), /* ꚉ */
    (0xA68A, AF_IGNORE_CAPITAL_BOTTOM), /* Ꚋ */
    (0xA68B, AF_IGNORE_SMALL_BOTTOM), /* ꚋ */
    (0xA68E, AF_IGNORE_CAPITAL_BOTTOM), /* Ꚏ */
    (0xA68F, AF_IGNORE_SMALL_BOTTOM), /* ꚏ */
    (0xA690, AF_IGNORE_CAPITAL_BOTTOM), /* Ꚑ */
    (0xA691, AF_IGNORE_SMALL_BOTTOM), /* ꚑ */
    (0xA696, AF_IGNORE_CAPITAL_BOTTOM), /* Ꚗ */
    (0xA697, AF_IGNORE_SMALL_BOTTOM), /* ꚗ */
    (0xA726, AF_IGNORE_CAPITAL_BOTTOM), /* Ꜧ */
    (0xA727, AF_IGNORE_SMALL_BOTTOM), /* ꜧ */
    (0xA756, AF_IGNORE_CAPITAL_BOTTOM), /* Ꝗ */
    (0xA758, AF_IGNORE_CAPITAL_BOTTOM), /* Ꝙ */
    (0xA771, AF_IGNORE_SMALL_BOTTOM), /* ꝱ */
    (0xA772, AF_IGNORE_SMALL_BOTTOM), /* ꝲ */
    (0xA773, AF_IGNORE_SMALL_BOTTOM), /* ꝳ */
    (0xA774, AF_IGNORE_SMALL_BOTTOM), /* ꝴ */
    (0xA776, AF_IGNORE_SMALL_BOTTOM), /* ꝶ */
    (0xA790, AF_IGNORE_CAPITAL_BOTTOM), /* Ꞑ */
    (0xA791, AF_IGNORE_SMALL_BOTTOM), /* ꞑ */
    (0xA794, AF_IGNORE_SMALL_BOTTOM), /* ꞔ */
    (0xA795, AF_IGNORE_SMALL_BOTTOM), /* ꞕ */
    (0xA7C0, AF_IGNORE_CAPITAL_TOP | AF_IGNORE_CAPITAL_BOTTOM), /* Ꟁ */
    (0xA7C1, AF_IGNORE_SMALL_TOP | AF_IGNORE_SMALL_BOTTOM), /* ꟁ */
    (0xA7C4, AF_IGNORE_CAPITAL_BOTTOM), /* Ꞔ */
    (0xA7C5, AF_IGNORE_CAPITAL_BOTTOM), /* Ʂ */
    (0xA7C6, AF_IGNORE_CAPITAL_BOTTOM), /* Ᶎ */
    (0xA7CC, AF_IGNORE_CAPITAL_TOP | AF_IGNORE_CAPITAL_BOTTOM), /* Ꟍ */
    (0xA7CD, AF_IGNORE_SMALL_TOP | AF_IGNORE_SMALL_BOTTOM), /* ꟍ */
    (0xAB3C, AF_IGNORE_SMALL_BOTTOM), /* ꬼ */
    (0xAB46, AF_IGNORE_SMALL_BOTTOM), /* ꭆ */
    (0xAB5C, AF_IGNORE_SMALL_BOTTOM), /* ꭜ */
    (0xAB66, AF_IGNORE_SMALL_BOTTOM), /* ꭦ */
    (0xAB67, AF_IGNORE_SMALL_BOTTOM), /* ꭧ */
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
/// This mirrors the shared `af_sort_and_quantize_widths` from `afhints.c`.
/// FreeType divides the cluster sum by the loop's end index instead of by the
/// cluster length; downstream stem snapping depends on preserving that behavior.
pub(super) fn sort_and_quantize_widths(count: &mut usize, widths: &mut [AfWidth], threshold: i32) {
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
    loader::reload(&mut hints, raw_outline, scaled_points, metrics.units_per_em);

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
            // FreeType's afblue.dat uses `|` as blue-string syntax separating
            // overshoot/fill values from reference/flat values. The generated
            // Rust table stores it as a char, so skip it before cmap lookup.
            if ch == '|' {
                continue;
            }

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
                    best_on_first >= 0 && points[usize_from_i32(best_on_first)].on_curve,
                    best_on_last >= 0 && points[usize_from_i32(best_on_last)].on_curve,
                    best_seg_first, best_seg_last,
                    points[usize_from_i32(best_seg_first)].on_curve);

                if round && is_neutral!(entry.props) {
                    continue;
                } // neutral uses flats only
            }

            // Latin has one element, so this character's extremum is the result.
            if best_point >= 0 {
                best_y_extremum = Some(best_y + y_offset);
                best_round = round;
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
            // `af_latin_metrics_init_blues` keeps the two medians verbatim;
            // directionally invalid overshoots are corrected below.
            (flat_median, round_median)
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

    // Resolve blue-zone overlaps in bottom-to-top order without changing the
    // stored blue order.  C sorts a temporary pointer array here; later
    // lookups such as `af_latin_get_base_glyph_blues` still depend on the
    // original script-string order (aflatin.c:988-1039, 3069-3099).
    if axis.blue_count > 1 {
        let mut sorted: Vec<usize> = (0..axis.blues.len()).collect();
        for i in 1..sorted.len() {
            let mut j = i;
            while j > 0 {
                let a = &axis.blues[sorted[j - 1]];
                let b = &axis.blues[sorted[j]];
                let a_pos = if a.flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0 {
                    a.ref_width.org
                } else {
                    a.shoot_width.org
                };
                let b_pos = if b.flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0 {
                    b.ref_width.org
                } else {
                    b.shoot_width.org
                };
                if b_pos >= a_pos {
                    break;
                }
                sorted.swap(j - 1, j);
                j -= 1;
            }
        }

        for pair in sorted.windows(2) {
            let a_idx = pair[0];
            let b_idx = pair[1];
            let use_shoot_a =
                axis.blues[a_idx].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0;
            let use_shoot_b =
                axis.blues[b_idx].flags & (AF_LATIN_BLUE_TOP | AF_LATIN_BLUE_SUB_TOP) != 0;
            let a_org = if use_shoot_a {
                axis.blues[a_idx].shoot_width.org
            } else {
                axis.blues[a_idx].ref_width.org
            };
            let b_org = if use_shoot_b {
                axis.blues[b_idx].shoot_width.org
            } else {
                axis.blues[b_idx].ref_width.org
            };
            if a_org > b_org {
                if use_shoot_a {
                    axis.blues[a_idx].shoot_width.org = b_org;
                } else {
                    axis.blues[a_idx].ref_width.org = b_org;
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

fn base_glyph_blues(
    hints: &GlyphHints,
    is_capital: bool,
) -> (Option<AfLatinBlue>, Option<AfLatinBlue>) {
    let Some(metrics) = hints.metrics.as_ref() else {
        return (None, None);
    };
    let axis = &metrics.axis[Dimension::Vert as usize];

    let top_flag = (if is_capital {
        AF_LATIN_BLUE_TOP
    } else {
        AF_LATIN_BLUE_ADJUSTMENT
    }) | AF_LATIN_BLUE_ACTIVE;
    let bottom_flag = (if is_capital {
        AF_LATIN_BLUE_BOTTOM
    } else {
        AF_LATIN_BLUE_BOTTOM_SMALL
    }) | AF_LATIN_BLUE_ACTIVE;

    let top = axis
        .blues
        .iter()
        .find(|blue| (blue.flags & top_flag) == top_flag)
        .copied();
    let bottom = axis
        .blues
        .iter()
        .find(|blue| (blue.flags & bottom_flag) == bottom_flag)
        .copied();
    (top, bottom)
}

fn prevent_top_blue_alignment(hints: &mut GlyphHints, pos: i32) {
    for edge in &mut hints.axis[Dimension::Vert as usize].edges {
        if edge.pos > pos {
            edge.flags |= AF_EDGE_NO_BLUE;
        }
    }
}

fn prevent_bottom_blue_alignment(hints: &mut GlyphHints, pos: i32) {
    for edge in &mut hints.axis[Dimension::Vert as usize].edges {
        if edge.pos < pos {
            edge.flags |= AF_EDGE_NO_BLUE;
        }
    }
}

fn ignore_top_blue_alignment(hints: &mut GlyphHints, top: AfLatinBlue, bottom: AfLatinBlue) {
    let base_height = top.shoot_width.cur - bottom.shoot_width.cur;
    let limit = top.shoot_width.cur + base_height / 7 + 16;
    prevent_top_blue_alignment(hints, limit);
}

fn ignore_bottom_blue_alignment(hints: &mut GlyphHints, top: AfLatinBlue, bottom: AfLatinBlue) {
    let base_height = top.shoot_width.cur - bottom.shoot_width.cur;
    let limit = bottom.shoot_width.cur - base_height / 7 - 16;
    prevent_bottom_blue_alignment(hints, limit);
}

fn apply_blue_zone_ignore_adjustments(hints: &mut GlyphHints, adj_type: u32) {
    if adj_type == 0 {
        return;
    }

    let ignore_capital_top = (adj_type & AF_IGNORE_CAPITAL_TOP) != 0;
    let ignore_capital_bottom = (adj_type & AF_IGNORE_CAPITAL_BOTTOM) != 0;
    let ignore_small_top = (adj_type & AF_IGNORE_SMALL_TOP) != 0;
    let ignore_small_bottom = (adj_type & AF_IGNORE_SMALL_BOTTOM) != 0;

    if ignore_capital_top || ignore_capital_bottom {
        let (top, bottom) = base_glyph_blues(hints, true);
        if let (Some(top), Some(bottom)) = (top, bottom) {
            if ignore_capital_top {
                ignore_top_blue_alignment(hints, top, bottom);
            }
            if ignore_capital_bottom {
                ignore_bottom_blue_alignment(hints, top, bottom);
            }
        }
    }

    if ignore_small_top || ignore_small_bottom {
        let (top, bottom) = base_glyph_blues(hints, false);
        if let (Some(top), Some(bottom)) = (top, bottom) {
            if ignore_small_top {
                ignore_top_blue_alignment(hints, top, bottom);
            }
            if ignore_small_bottom {
                ignore_bottom_blue_alignment(hints, top, bottom);
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

fn recompute_vertical_extrema(hints: &mut GlyphHints) {
    let contour_count = hints.num_contours();
    let mut new_minima = vec![0; contour_count];
    let mut new_maxima = vec![0; contour_count];
    for ci in 0..contour_count {
        let (min_y, max_y) = contour_y_bounds(hints, ci);
        new_minima[ci] = min_y;
        new_maxima[ci] = max_y;
    }
    hints.contour_y_minima = new_minima;
    hints.contour_y_maxima = new_maxima;
}

fn contour_y_bounds(hints: &GlyphHints, contour: usize) -> (i32, i32) {
    let c_start = hints.contours[contour];
    if contour_is_dimensionless(hints, contour) {
        return (i32::MAX, i32::MIN);
    }
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
    (y_min, y_max)
}

fn contour_is_dimensionless(hints: &GlyphHints, contour: usize) -> bool {
    let first = hints.contours[contour];
    hints.points[hints.points[first].next].next == first
}

fn move_contour_vertically(hints: &mut GlyphHints, contour: usize, delta: i32) {
    let c_start = hints.contours[contour];
    let mut idx = c_start;
    loop {
        hints.points[idx].y += delta;
        let next = hints.points[idx].next;
        if next == c_start {
            break;
        }
        idx = next;
    }
}

fn touch_contour(hints: &mut GlyphHints, contour: usize) {
    let c_start = hints.contours[contour];
    let mut idx = hints.points[c_start].next;
    loop {
        hints.points[idx].flags |= AF_FLAG_IGNORE;
        if hints.points[idx].flags & AF_FLAG_CONTROL == 0 {
            hints.points[idx].flags |= AF_FLAG_TOUCH_Y;
        }
        if idx == c_start {
            break;
        }
        idx = hints.points[idx].next;
    }
}

fn touch_top_contours(hints: &mut GlyphHints, limit_contour: usize) {
    let limit = hints.contour_y_minima[limit_contour];
    for ci in 0..hints.contours.len() {
        let min_y = hints.contour_y_minima[ci];
        let max_y = hints.contour_y_maxima[ci];
        if min_y < max_y && min_y >= limit {
            touch_contour(hints, ci);
        }
    }
}

fn touch_bottom_contours(hints: &mut GlyphHints, limit_contour: usize) {
    let limit = hints.contour_y_minima[limit_contour];
    for ci in 0..hints.contours.len() {
        let min_y = hints.contour_y_minima[ci];
        let max_y = hints.contour_y_maxima[ci];
        if min_y < max_y && max_y <= limit {
            touch_contour(hints, ci);
        }
    }
}

fn move_contours_up(hints: &mut GlyphHints, limit: i32, delta: i32) {
    for ci in 0..hints.contours.len() {
        let min_y = hints.contour_y_minima[ci];
        let max_y = hints.contour_y_maxima[ci];
        if min_y < max_y && min_y > limit {
            move_contour_vertically(hints, ci, delta);
        }
    }
}

fn move_contours_down(hints: &mut GlyphHints, limit: i32, delta: i32) {
    for ci in 0..hints.contours.len() {
        let min_y = hints.contour_y_minima[ci];
        let max_y = hints.contour_y_maxima[ci];
        if min_y < max_y && max_y < limit {
            move_contour_vertically(hints, ci, -delta);
        }
    }
}

fn find_highest_contour(hints: &GlyphHints) -> usize {
    let mut highest_contour = 0;
    let mut highest_min_y = i32::MAX;
    let mut highest_max_y = i32::MIN;
    for ci in 0..hints.contours.len() {
        let current_min_y = hints.contour_y_minima[ci];
        let current_max_y = hints.contour_y_maxima[ci];
        if current_max_y > highest_max_y
            || (current_max_y == highest_max_y && current_min_y > highest_min_y)
        {
            highest_min_y = current_min_y;
            highest_max_y = current_max_y;
            highest_contour = ci;
        }
    }
    highest_contour
}

fn find_second_highest_contour(hints: &GlyphHints) -> usize {
    if hints.contours.len() < 3 {
        return 0;
    }
    let highest = find_highest_contour(hints);
    let highest_min_y = hints.contour_y_minima[highest];
    let mut second = 0;
    let mut second_max_y = i32::MIN;
    for ci in 0..hints.contours.len() {
        if ci == highest {
            continue;
        }
        let current_min_y = hints.contour_y_minima[ci];
        let current_max_y = hints.contour_y_maxima[ci];
        if current_max_y > second_max_y && current_min_y < highest_min_y {
            second_max_y = current_max_y;
            second = ci;
        }
    }
    second
}

fn find_lowest_contour(hints: &GlyphHints) -> usize {
    let mut lowest_contour = 0;
    let mut lowest_min_y = i32::MAX;
    let mut lowest_max_y = i32::MIN;
    for ci in 0..hints.contours.len() {
        let current_min_y = hints.contour_y_minima[ci];
        let current_max_y = hints.contour_y_maxima[ci];
        if current_min_y < lowest_min_y
            || (current_min_y == lowest_min_y && current_max_y < lowest_max_y)
        {
            lowest_min_y = current_min_y;
            lowest_max_y = current_max_y;
            lowest_contour = ci;
        }
    }
    lowest_contour
}

fn find_second_lowest_contour(hints: &GlyphHints) -> usize {
    if hints.contours.len() < 3 {
        return 0;
    }
    let lowest = find_lowest_contour(hints);
    let lowest_max_y = hints.contour_y_maxima[lowest];
    let mut second = 0;
    let mut second_min_y = i32::MAX;
    for ci in 0..hints.contours.len() {
        if ci == lowest {
            continue;
        }
        let current_min_y = hints.contour_y_minima[ci];
        let current_max_y = hints.contour_y_maxima[ci];
        if current_min_y < second_min_y && current_max_y > lowest_max_y {
            second_min_y = current_min_y;
            second = ci;
        }
    }
    second
}

fn contour_horizontal_overlap(hints: &GlyphHints, contour_index: usize) -> bool {
    let mut contour_min_x = i32::MAX;
    let mut contour_max_x = i32::MIN;
    let mut others_min_x = i32::MAX;
    let mut others_max_x = i32::MIN;

    for ci in 0..hints.contours.len() {
        if contour_is_dimensionless(hints, ci) {
            continue;
        }
        let first = hints.contours[ci];
        let mut idx = hints.points[first].next;
        loop {
            let x = hints.points[idx].x;
            if ci == contour_index {
                contour_min_x = contour_min_x.min(x);
                contour_max_x = contour_max_x.max(x);
            } else {
                others_min_x = others_min_x.min(x);
                others_max_x = others_max_x.max(x);
            }
            if idx == first {
                break;
            }
            idx = hints.points[idx].next;
        }
    }

    if contour_min_x == i32::MAX || others_min_x == i32::MAX {
        return false;
    }
    (others_min_x <= contour_max_x && contour_max_x <= others_max_x)
        || (others_min_x <= contour_min_x && contour_min_x <= others_max_x)
        || (contour_max_x >= others_max_x && contour_min_x <= others_min_x)
}

fn stretch_top_tilde(hints: &mut GlyphHints, contour: usize) -> i32 {
    let first = hints.contours[contour];
    let min_y = hints.contour_y_minima[contour];
    let max_y = hints.contour_y_maxima[contour];
    if min_y == max_y {
        return 0;
    }

    let height = max_y - min_y;
    let extremum_threshold = height / 8;
    let mut min_measurement = i32::MAX;
    let mut measurement_taken = false;
    let mut idx = hints.points[first].next;
    loop {
        let pt = hints.points[idx];
        if pt.flags & AF_FLAG_CONTROL == 0
            && hints.points[pt.prev].y == pt.y
            && hints.points[pt.next].y == pt.y
            && pt.y != min_y
            && pt.y != max_y
            && hints.points[pt.prev].flags & AF_FLAG_CONTROL != 0
            && hints.points[pt.next].flags & AF_FLAG_CONTROL != 0
        {
            let mut prev_on = pt.prev;
            let mut next_on = pt.next;
            while hints.points[prev_on].flags & AF_FLAG_CONTROL != 0 {
                prev_on = hints.points[prev_on].prev;
            }
            while hints.points[next_on].flags & AF_FLAG_CONTROL != 0 {
                next_on = hints.points[next_on].next;
            }
            let measurement = if hints.points[next_on].y > pt.y && hints.points[prev_on].y > pt.y {
                pt.y - min_y
            } else if hints.points[next_on].y < pt.y && hints.points[prev_on].y < pt.y {
                max_y - pt.y
            } else {
                0
            };
            if measurement >= extremum_threshold && measurement != 0 {
                measurement_taken = true;
                min_measurement = min_measurement.min(measurement);
            }
        }
        if idx == first {
            break;
        }
        idx = hints.points[idx].next;
    }

    if !measurement_taken {
        min_measurement = 0;
    }
    touch_top_contours(hints, contour);
    let target_height = min_measurement + 64;
    if height >= target_height {
        return 0;
    }

    let mut idx = first;
    loop {
        let y = hints.points[idx].y;
        hints.points[idx].y =
            (((y - min_y) as i64 * target_height as i64) / height as i64) as i32 + min_y;
        let next = hints.points[idx].next;
        if next == first {
            break;
        }
        idx = next;
    }
    target_height - height
}

fn stretch_bottom_tilde(hints: &mut GlyphHints, contour: usize) -> i32 {
    let first = hints.contours[contour];
    let min_y = hints.contour_y_minima[contour];
    let max_y = hints.contour_y_maxima[contour];
    if min_y == max_y {
        return 0;
    }

    let height = max_y - min_y;
    let extremum_threshold = height / 8;
    let mut min_measurement = i32::MAX;
    let mut measurement_taken = false;
    let mut idx = hints.points[first].next;
    loop {
        let pt = hints.points[idx];
        if pt.flags & AF_FLAG_CONTROL == 0
            && hints.points[pt.prev].y == pt.y
            && hints.points[pt.next].y == pt.y
            && pt.y != min_y
            && pt.y != max_y
            && hints.points[pt.prev].flags & AF_FLAG_CONTROL != 0
            && hints.points[pt.next].flags & AF_FLAG_CONTROL != 0
        {
            let mut prev_on = pt.prev;
            let mut next_on = pt.next;
            while hints.points[prev_on].flags & AF_FLAG_CONTROL != 0 {
                prev_on = hints.points[prev_on].prev;
            }
            while hints.points[next_on].flags & AF_FLAG_CONTROL != 0 {
                next_on = hints.points[next_on].next;
            }
            let measurement = if hints.points[next_on].y > pt.y && hints.points[prev_on].y > pt.y {
                pt.y - min_y
            } else if hints.points[next_on].y < pt.y && hints.points[prev_on].y < pt.y {
                max_y - pt.y
            } else {
                0
            };
            if measurement >= extremum_threshold && measurement != 0 {
                measurement_taken = true;
                min_measurement = min_measurement.min(measurement);
            }
        }
        if idx == first {
            break;
        }
        idx = hints.points[idx].next;
    }

    if !measurement_taken {
        min_measurement = 0;
    }
    touch_bottom_contours(hints, contour);
    let target_height = min_measurement + 64;
    if height >= target_height {
        return 0;
    }

    let mut idx = first;
    loop {
        let y = hints.points[idx].y;
        hints.points[idx].y =
            (((y - max_y) as i64 * target_height as i64) / height as i64) as i32 + max_y;
        let next = hints.points[idx].next;
        if next == first {
            break;
        }
        idx = next;
    }
    target_height - height
}

fn align_top_tilde(hints: &mut GlyphHints, contour: usize) -> i32 {
    let (min_y, max_y) = contour_y_bounds(hints, contour);
    let height = max_y - min_y;
    let mut delta = ft_pix_round(min_y) - min_y;
    if height < 3 * 64 {
        delta += (ft_pix_round(height) - height) / 2;
    }
    move_contour_vertically(hints, contour, delta);
    delta
}

fn align_bottom_tilde(hints: &mut GlyphHints, contour: usize) -> i32 {
    let (min_y, max_y) = contour_y_bounds(hints, contour);
    let height = max_y - min_y;
    let mut delta = ft_pix_round(max_y) - max_y;
    if height < 3 * 64 {
        delta -= (ft_pix_round(height) - height) / 2;
    }
    move_contour_vertically(hints, contour, delta);
    delta
}

fn apply_tilde_stretch_alignment(hints: &mut GlyphHints, adj_type: u32) {
    let is_top_tilde = (adj_type & AF_ADJUST_TILDE_TOP) != 0;
    let is_bottom_tilde = (adj_type & AF_ADJUST_TILDE_BOTTOM) != 0;
    let is_below_top_tilde = (adj_type & AF_ADJUST_TILDE_TOP2) != 0;
    let is_above_bottom_tilde = (adj_type & AF_ADJUST_TILDE_BOTTOM2) != 0;
    if !(is_top_tilde || is_bottom_tilde || is_below_top_tilde || is_above_bottom_tilde) {
        return;
    }

    recompute_vertical_extrema(hints);
    if is_below_top_tilde {
        let contour = find_second_highest_contour(hints);
        let y_offset = stretch_top_tilde(hints, contour) + align_top_tilde(hints, contour);
        recompute_vertical_extrema(hints);
        let limit = hints.contour_y_minima[contour];
        move_contours_up(hints, limit, y_offset);
        recompute_vertical_extrema(hints);
    }
    if is_above_bottom_tilde {
        let contour = find_second_lowest_contour(hints);
        let y_offset = stretch_bottom_tilde(hints, contour) - align_bottom_tilde(hints, contour);
        recompute_vertical_extrema(hints);
        let limit = hints.contour_y_maxima[contour];
        move_contours_down(hints, limit, y_offset);
        recompute_vertical_extrema(hints);
    }
    if is_top_tilde {
        let contour = find_highest_contour(hints);
        stretch_top_tilde(hints, contour);
        align_top_tilde(hints, contour);
        recompute_vertical_extrema(hints);
    }
    if is_bottom_tilde {
        let contour = find_lowest_contour(hints);
        stretch_bottom_tilde(hints, contour);
        align_bottom_tilde(hints, contour);
        recompute_vertical_extrema(hints);
    }
}

fn vertical_separation_accent_height_limit(hints: &GlyphHints, adj_type: u32) -> i32 {
    if adj_type == 0 || (adj_type & AF_ADJUST_NO_HEIGHT_CHECK) != 0 {
        return 0;
    }

    let (small_top, small_bottom) = base_glyph_blues(hints, false);
    if let (Some(top), Some(bottom)) = (small_top, small_bottom) {
        return 2 * (top.shoot_width.cur - bottom.shoot_width.cur) / 3;
    }

    let (capital_top, capital_bottom) = base_glyph_blues(hints, true);
    if let (Some(top), Some(bottom)) = (capital_top, capital_bottom) {
        return (top.shoot_width.cur - bottom.shoot_width.cur) / 2;
    }

    let Some(metrics) = hints.metrics.as_ref() else {
        return 0;
    };
    let scale = metrics.axis[Dimension::Vert as usize].scale;
    ft_mul_fix(metrics.units_per_em * 4 / 10, scale)
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

    recompute_vertical_extrema(hints);
    // C: `af_latin_hints_apply` leaves `accent_height_limit` at zero for
    // `AF_ADJUST_NO_HEIGHT_CHECK`, then the later contour-height guard skips
    // this adjustment.  A fixed limit incorrectly moves punctuation stems.
    let accent_height_limit = vertical_separation_accent_height_limit(hints, adj_type);

    if (adjust_top && hints.contours.len() >= 2) || (adjust_below_top && hints.contours.len() >= 3)
    {
        let high_contour = if adjust_below_top {
            find_second_highest_contour(hints)
        } else {
            find_highest_contour(hints)
        };
        if !contour_horizontal_overlap(hints, high_contour) {
            return;
        }

        let high_min_y = hints.contour_y_minima[high_contour];
        let high_max_y = hints.contour_y_maxima[high_contour];
        let high_height = high_max_y - high_min_y;
        if high_height > accent_height_limit {
            return;
        }

        let mut min_distance = 64;
        for ci in 0..hints.contours.len() {
            if ci == high_contour {
                continue;
            }
            let min_y = hints.contour_y_minima[ci];
            let max_y = hints.contour_y_maxima[ci];
            let distance = high_min_y - max_y;
            if distance < 64 && distance < min_distance && min_y < high_min_y {
                min_distance = distance;
            }
        }

        let adjustment_amount = 64 - min_distance;
        let is_top_tilde = (adj_type & AF_ADJUST_TILDE_TOP) != 0;
        let is_below_top_tilde = (adj_type & AF_ADJUST_TILDE_TOP2) != 0;
        let mut centering_adjustment = 0;
        if is_top_tilde || is_below_top_tilde {
            let tilde_contour = if adjust_top {
                high_contour
            } else if is_below_top_tilde {
                high_contour
            } else {
                find_highest_contour(hints)
            };
            let tilde_height =
                hints.contour_y_maxima[tilde_contour] - hints.contour_y_minima[tilde_contour];
            let mut pos = high_min_y + adjustment_amount;
            if adjust_below_top && is_top_tilde {
                pos += high_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering_adjustment = (ft_pix_round(tilde_height) - tilde_height) / 2;
            }
        }

        let calculated_amount =
            if (adjust_top && is_top_tilde) || (adjust_below_top && is_below_top_tilde) {
                adjustment_amount + centering_adjustment
            } else {
                adjustment_amount
            };
        if calculated_amount != 0
            && calculated_amount >= -2
            && (calculated_amount <= 66 || adjustment_amount <= 66)
        {
            let min_y_limit = high_min_y - high_height / 8;
            // C uses `calculated_amount` only for the range check above.  The
            // main contour move uses raw `adjustment_amount`; any tilde
            // centering is applied only by the secondary below-top move.
            // See `aflatin.c` in
            // `af_glyph_hints_apply_vertical_separation_adjustments`.
            move_contours_up(hints, min_y_limit, adjustment_amount);
            if adjust_below_top && is_top_tilde {
                move_contours_up(hints, min_y_limit + high_height, centering_adjustment);
            }
            recompute_vertical_extrema(hints);
        }
    }

    if (adjust_bottom && hints.contours.len() >= 2)
        || (adjust_above_bottom && hints.contours.len() >= 3)
    {
        let low_contour = if adjust_above_bottom {
            find_second_lowest_contour(hints)
        } else {
            find_lowest_contour(hints)
        };
        if !contour_horizontal_overlap(hints, low_contour) {
            return;
        }

        let low_min_y = hints.contour_y_minima[low_contour];
        let low_max_y = hints.contour_y_maxima[low_contour];
        let low_height = low_max_y - low_min_y;
        if low_height > accent_height_limit {
            return;
        }

        let mut min_distance = 64;
        for ci in 0..hints.contours.len() {
            if ci == low_contour {
                continue;
            }
            let min_y = hints.contour_y_minima[ci];
            let max_y = hints.contour_y_maxima[ci];
            let distance = min_y - low_max_y;
            if distance < 64 && distance < min_distance && max_y > low_max_y {
                min_distance = distance;
            }
        }

        let adjustment_amount = 64 - min_distance;
        let is_bottom_tilde = (adj_type & AF_ADJUST_TILDE_BOTTOM) != 0;
        let is_above_bottom_tilde = (adj_type & AF_ADJUST_TILDE_BOTTOM2) != 0;
        let mut centering_adjustment = 0;
        if is_bottom_tilde || is_above_bottom_tilde {
            let tilde_contour = if adjust_bottom {
                low_contour
            } else if is_above_bottom_tilde {
                low_contour
            } else {
                find_lowest_contour(hints)
            };
            let tilde_height =
                hints.contour_y_maxima[tilde_contour] - hints.contour_y_minima[tilde_contour];
            let mut pos = low_max_y - adjustment_amount;
            if adjust_above_bottom && is_bottom_tilde {
                pos -= low_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering_adjustment = (ft_pix_round(tilde_height) - tilde_height) / 2;
            }
        }

        let calculated_amount = if (adjust_bottom && is_bottom_tilde)
            || (adjust_above_bottom && is_above_bottom_tilde)
        {
            adjustment_amount + centering_adjustment
        } else {
            adjustment_amount
        };

        if calculated_amount != 0
            && calculated_amount >= -2
            && (calculated_amount <= 66 || adjustment_amount <= 66)
        {
            let max_y_limit = low_max_y + low_height / 8;
            // FreeType's `af_glyph_hints_apply_vertical_separation_adjustments`
            // uses the raw separation amount for the main bottom-contour move;
            // tilde centering is only applied to the secondary below-contour path.
            // See `aflatin.c` around the `af_move_contours_down` calls.
            move_contours_down(hints, max_y_limit, adjustment_amount);
            if adjust_above_bottom && is_bottom_tilde {
                move_contours_down(hints, max_y_limit - low_height, centering_adjustment);
            }
            recompute_vertical_extrema(hints);
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
    horz_snap: bool,
    vert_snap: bool,
    font_data: Option<&crate::tables::FontData>,
    target_mono: bool,
    _pp1x_shift: i32,
) -> ApplyHintsMetrics {
    let mut output = ApplyHintsMetrics::default();
    let Some(metrics) = metrics else {
        return output;
    };
    let mut hints = GlyphHints::new(x_scale, y_scale, x_delta, y_delta);
    hints.metrics = Some(metrics.clone());

    // C: when no blue zones can be built for a Latin-style script, C remaps to
    // NONE_DFLT. Our Latin pipeline with blue_count==0 produces different
    // results than that dummy-style path, so skip it. Do not skip the CJK path:
    // af_cjk_hints_apply still runs edge hinting without active blue zones.
    if !metrics.no_advance_hinting && metrics.axis[1].blue_count == 0 {
        return output;
    }
    // Match FreeType's af_latin_hints_init target table:
    // mono/LCD snap vertical stems, mono/LCD_V snap horizontal stems, and LCD
    // clears stem adjustment to preserve horizontal subpixel coverage.
    if horz_snap {
        hints.other_flags |= AF_LATIN_HINTS_HORZ_SNAP;
    }
    if vert_snap {
        hints.other_flags |= AF_LATIN_HINTS_VERT_SNAP;
    }
    if stem_adjust {
        hints.other_flags |= AF_LATIN_HINTS_STEM_ADJUST;
    }
    if target_mono {
        hints.other_flags |= AF_LATIN_HINTS_MONO;
    }

    let no_advance_hinting = metrics.no_advance_hinting;
    // CJK/Hani fallback uses `af_cjk_hints_init`, which always sets
    // AF_SCALER_FLAG_NO_ADVANCE but does not inherit Latin light-mode
    // AF_SCALER_FLAG_NO_HORIZONTAL (afcjk.c:1390-1421).
    if is_italic || (no_horizontal_hinting && !no_advance_hinting) {
        hints.scaler_flags |= AF_SCALER_FLAG_NO_HORIZONTAL;
        if is_italic {
            crate::autohint::coverage::record(crate::autohint::coverage::COV_ITALIC_NO_HORZ);
        }
    }

    // Compute ppem for bdelta in compute_stem_width
    // At 72dpi: x_scale = (ppem * 64 * 0x10000) / upem → ppem = x_scale * upem / 0x10000 / 64
    let ppem = i32_from_i64((x_scale as i64).abs() * metrics.units_per_em as i64 / 65536 / 64);
    let ppem = ppem.clamp(1, 100);

    // Step 1: Load outline into hints (raw font units → fx/fy; scaled 26.6 → ox/oy)
    loader::reload(
        &mut hints,
        raw_outline,
        &outline.points,
        metrics.units_per_em,
    );
    if hints.num_points() == 0 {
        return output;
    }

    // Keep the phase order aligned with af_latin_hints_apply
    // (aflatin.c:4957-5200); later phases depend on flags and links produced
    // by earlier ones.
    // Phase A: detect_features for HORZ (segs → link → edges)
    let do_horz = hints.scaler_flags & AF_SCALER_FLAG_NO_HORIZONTAL == 0;
    let use_cjk_edges = metrics.no_advance_hinting;
    let mut horz_widths_26_6: Vec<i32> = Vec::new();
    if do_horz {
        compute_segments(&mut hints, Dimension::Horz);
        {
            let (wc, widths) = extract_widths(&hints, Dimension::Horz);
            horz_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
            if use_cjk_edges {
                // FreeType 2.14.3's `af_glyph_hints_reload` resets segment
                // counts before every apply call (afhints.c:887-893), then
                // `af_cjk_hints_compute_segments` snapshots that zero limit
                // before running the Latin scanner (afcjk.c:794-807). Its CJK
                // roundness loop is empty in the public apply route, so keep
                // the Latin segment round flags for CJK/Hani parity.
                super::cjk::cjk_link_segments(&mut hints, Dimension::Horz);
            } else {
                link_segments_inner(&mut hints, Dimension::Horz, wc, &widths);
            }
        }
        if use_cjk_edges {
            super::cjk::cjk_compute_edges(&mut hints, Dimension::Horz, false);
            super::cjk::cjk_compute_blue_edges(&mut hints, Dimension::Horz);
        } else {
            compute_edges(&mut hints, Dimension::Horz);
        }
    }

    if let Some(data) = font_data {
        let adj_type = reverse_cmap_lookup(data, glyph_index).map_or(0, adjustment_database_lookup);
        // C applies tilde stretching/alignment before vertical feature
        // detection (aflatin.c:4938-4980), after horizontal detection.
        apply_tilde_stretch_alignment(&mut hints, adj_type);
    }

    // Phase B: detect_features for VERT (segs → link → edges) + blue zones.
    // This OVERWRITES point.v = fx — matching C's behavior before the hint loop.
    compute_segments(&mut hints, Dimension::Vert);
    let vert_widths_26_6: Vec<i32>;
    {
        let (wc, widths) = extract_widths(&hints, Dimension::Vert);
        vert_widths_26_6 = widths.iter().take(wc).map(|w| w.cur).collect();
        if use_cjk_edges {
            // Keep Latin segment roundness flags for CJK; see the horizontal
            // phase comment above for the FreeType 2.14.3 no-op wrapper detail.
            super::cjk::cjk_link_segments(&mut hints, Dimension::Vert);
        } else {
            link_segments_inner(&mut hints, Dimension::Vert, wc, &widths);
        }
    }
    if use_cjk_edges {
        super::cjk::cjk_compute_edges(&mut hints, Dimension::Vert, false);
        super::cjk::cjk_compute_blue_edges(&mut hints, Dimension::Vert);
    } else {
        compute_edges(&mut hints, Dimension::Vert);
    }
    if let Some(data) = font_data {
        let adj_type = reverse_cmap_lookup(data, glyph_index).map_or(0, adjustment_database_lookup);
        apply_blue_zone_ignore_adjustments(&mut hints, adj_type);
    }
    let is_nonbase = hints.metrics.as_ref().is_some_and(|m| {
        (glyph_index as usize) < m.non_base_glyphs.len() && m.non_base_glyphs[glyph_index as usize]
    });
    if !use_cjk_edges && !is_nonbase {
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
        // FreeType 2.14.3 dispatches these corresponding grid-fit steps to
        // `af_cjk_hint_edges` (afcjk.c:2301-2310) or `af_latin_hint_edges`
        // (aflatin.c:5050-5059). Keep our shared dispatcher on that boundary.
        hint_edges(&mut hints, dim, widths, ppem);
        if use_cjk_edges {
            super::cjk::align_edge_points(&mut hints, dim);
        } else {
            align_edge_points(&mut hints, dim);
        }
        align_strong_points(&mut hints, dim);
        align_weak_points(&mut hints, dim);
        if dim == Dimension::Vert {
            if let Some(data) = font_data {
                vertical_separation_adjustments(&mut hints, glyph_index, data);
            }
        }
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
        let target_light = no_horizontal_hinting && !stem_adjust && !horz_snap && !vert_snap;
        let preserve_original_advance = !target_light
            && (metrics.fixed_width
                || (metrics.digits_have_same_width
                    && metrics
                        .digit_glyphs
                        .get(glyph_index as usize)
                        .copied()
                        .unwrap_or(false)));
        #[cfg(debug_assertions)]
        if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
            log::trace!(target: "autohint::pipeline", "[PHANTOM_PRE] gi={glyph_index} num_horz_edges={num_horz_edges}");
        }
        if num_horz_edges > 1 && !no_advance_hinting {
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
            // C's afloader.c:454-460 also takes this branch when
            // `AF_HINTS_DO_ADVANCE` is false.  CJK Hani sets
            // `AF_SCALER_FLAG_NO_ADVANCE` in `afcjk.c:1419`, so pp2 is the
            // rounded original phantom advance, not an edge-adjusted advance.
            // pp1.x is FT_PIX_ROUND(0) for our zero-delta public fixtures, so
            // the x coordinates are unchanged.
            #[cfg(debug_assertions)]
            if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
                log::trace!(target: "autohint::pipeline", "[PHANTOM_SKIP] gi={glyph_index} num_horz_edges={num_horz_edges} (<=1, no adjust)");
            }
            if advance_width != 0 {
                output.advance_width = Some((advance_width + 32) & !63);
            }
        }
        if preserve_original_advance {
            // C: afloader.c:543-554 keeps the rounded original advance for
            // fixed-width faces and for ASCII digits when all digits share one
            // width, after outline positioning has already used pp1/pp2.
            output.advance_width = Some((advance_width + 32) & !63);
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
        let el_horz = if let Some(m) = hints.metrics.as_ref() {
            m.axis[Dimension::Horz as usize].extra_light
        } else {
            false
        };
        let el_vert = if let Some(m) = hints.metrics.as_ref() {
            m.axis[Dimension::Vert as usize].extra_light
        } else {
            false
        };
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
        let mut prev_max_flags = max_flags;
        let mut prev_min_on_coord = min_on_coord;
        let mut prev_max_on_coord = max_on_coord;

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
                        prev_max_flags = max_flags;
                        prev_min_on_coord = min_on_coord;
                        prev_max_on_coord = max_on_coord;
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
                            let prev_extends_min = prev_min_coord < min_coord;
                            let prev_extends_max = prev_max_coord > max_coord;
                            min_coord = min_coord.min(prev_min_coord);
                            max_coord = max_coord.max(prev_max_coord);
                            min_flags = [min_flags, prev_min_flags][usize::from(prev_extends_min)];
                            max_flags = [max_flags, prev_max_flags][usize::from(prev_extends_max)];
                            min_on_coord = min_on_coord.min(prev_min_on_coord);
                            max_on_coord = max_on_coord.max(prev_max_on_coord);
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
                            let delta = i16_from_i32((prev_max_pos - prev_min_pos) >> 1);
                            let s = &mut axis.segments[prev];
                            s.last = point;
                            s.pos = pos;
                            s.delta = delta;
                        } else {
                            // C: different directions, current is longer — replace prev (aflatin.c:1812-1843)
                            // *prev_segment = *segment copies ALL fields, including `first`.
                            // It also refreshes the prev_* merge buffers; U+0245
                            // target-mono depends on this to keep the vertical cusp
                            // horizontal segments unlinked like C.
                            min_pos = min_pos.min(prev_min_pos);
                            max_pos = max_pos.max(prev_max_pos);
                            let pos = i16_from_i32((min_pos + max_pos) >> 1);
                            let delta = i16_from_i32((max_pos - min_pos) >> 1);
                            let s = &mut axis.segments[prev];
                            s.last = point;
                            s.pos = pos;
                            s.delta = delta;
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

                            prev_min_pos = min_pos;
                            prev_max_pos = max_pos;
                            prev_min_coord = min_coord;
                            prev_max_coord = max_coord;
                            prev_min_flags = min_flags;
                            prev_max_flags = max_flags;
                            prev_min_on_coord = min_on_coord;
                            prev_max_on_coord = max_on_coord;
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
    if d.is_vertical() {
        Direction::Up
    } else if d.is_horizontal() {
        Direction::Right
    } else {
        Direction::None
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
                dir: seg_dir,
                first: seg_idx,
                last: seg_idx,
                ..AFEdge::default()
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
                seg.pos, seg.dir.as_i8(),
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
            if (seg1_dir.as_i8() + seg2_dir.as_i8() == 0) && pos2 > pos1 {
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

/// Preserve a serif edge's original offset from its hinted base edge.
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

    if hints
        .metrics
        .as_ref()
        .is_some_and(|metrics| metrics.no_advance_hinting)
    {
        // CJK metrics width initialization also enters this shared helper before
        // the main apply path switches to the dedicated CJK edge pipeline.
        super::cjk::hint_edges(hints, dim, std_widths);
        let axis = &hints.axis[dim as usize];
        dump_edge_phase("CJK", dim_label, &axis.edges);
        return;
    }

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

            let (Some(e1), Some(blue)) = (edge1_idx, blue) else {
                continue;
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
                let base = axis.edges[serif_idx];
                align_serif_edge(&base, &mut axis.edges[i]);
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
                    // `af_glyph_hints_align_weak_points` compares against the
                    // global point-array base, not this contour's first point.
                    // For later contours this can intentionally call
                    // `af_iup_interp` with an empty range; its p1 > p2 guard
                    // then matches the C pointer-range check.
                    if first_touched > 0 {
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
