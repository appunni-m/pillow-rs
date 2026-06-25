//! 'kern' table -- Kerning.
//!
//! Contains kerning pair adjustments for horizontal (and vertical) glyph
//! positioning. Stub: kerning is not required for PIL parity tests at the
//! font sizes and glyph combinations currently exercised.

/// Parse the 'kern' table from raw bytes.
///
/// Returns `None` -- kerning support is not yet implemented.
#[allow(dead_code)]
pub(crate) fn parse_kern(_data: &[u8]) -> Option<()> {
    None
}
