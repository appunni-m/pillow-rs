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
        let this = u16::from_be_bytes([*loca.get(off)?, *loca.get(off + 1)?]) as u32 * 2;
        let next = u16::from_be_bytes([*loca.get(off + 2)?, *loca.get(off + 3)?]) as u32 * 2;
        (this, next)
    } else {
        let off = idx * 4;
        let this = u32::from_be_bytes([
            *loca.get(off)?,
            *loca.get(off + 1)?,
            *loca.get(off + 2)?,
            *loca.get(off + 3)?,
        ]);
        let next = u32::from_be_bytes([
            *loca.get(off + 4)?,
            *loca.get(off + 5)?,
            *loca.get(off + 6)?,
            *loca.get(off + 7)?,
        ]);
        (this, next)
    };
    Some(GlyphLocation {
        offset: this,
        length: next.saturating_sub(this),
    })
}
