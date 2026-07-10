//! 'loca' table — glyph offsets into 'glyf'.
//!
//! Mirrors `tt_face_get_location` in `src/sfnt/ttload.c`.

/// `(offset, length)` of a glyph's data inside the 'glyf' table.
#[derive(Debug, Clone, Copy)]
pub struct GlyphLocation {
    pub offset: u32,
    pub length: u32,
}

/// Resolve a glyph's location from the 'loca' table.
///
/// `index_to_loc_format` is from the 'head' table (0 = short, 1 = long).
/// Returns `Some` with `length == 0` for the empty (space-like) glyph slot.
pub fn get_glyph_location(
    loca: &[u8],
    glyph_index: u16,
    index_to_loc_format: i16,
) -> Option<GlyphLocation> {
    let idx = glyph_index as usize;
    let (this, next) = if index_to_loc_format == 0 {
        let off = idx * 2;
        let record = loca.get(off..off + 4)?;
        let this = u16::from_be_bytes([record[0], record[1]]) as u32 * 2;
        let next = u16::from_be_bytes([record[2], record[3]]) as u32 * 2;
        (this, next)
    } else {
        let off = idx * 4;
        let record = loca.get(off..off + 8)?;
        let this = u32::from_be_bytes([record[0], record[1], record[2], record[3]]);
        let next = u32::from_be_bytes([record[4], record[5], record[6], record[7]]);
        (this, next)
    };
    Some(GlyphLocation {
        offset: this,
        length: next.saturating_sub(this),
    })
}
