//! Table parsing for the bytecode hinter: fpgm, prep, cvt.
//!
//! C reference: `tt_face_load_cvt`, `tt_face_load_fpgm`, `tt_face_load_prep`
//! in `ttpload.c:295-505`.
//!
//! These tables are required for TrueType bytecode hinting:
//! - `cvt` (Control Value Table): array of FWORD values, scaled to 26.6
//! - `fpgm` (Font Program): bytecode executed once at face load
//! - `prep` (CVT Program): bytecode executed when ppem changes

use crate::error::FontError;

/// Parsed 'cvt ' table — array of control values in 26.6 format.
///
/// Each entry is a 16-bit signed FWORD from the font file, multiplied by 64
/// to convert from font units to 26.6 fixed-point. FreeType stores these as
/// `FT_Int32` values in 26.6.
pub fn parse_cvt(data: &[u8]) -> Result<Vec<i32>, FontError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if !data.len().is_multiple_of(2) {
        return Err(FontError::InvalidOutline(
            "cvt: table length must be even".into(),
        ));
    }

    let count = data.len() / 2;
    let mut cvt = Vec::with_capacity(count);

    for i in 0..count {
        let off = i * 2;
        let val = i16::from_be_bytes([data[off], data[off + 1]]) as i32;
        // Scale to 26.6: multiply by 64 (FT_GET_SHORT() * 64 in C)
        cvt.push(val * 64);
    }

    Ok(cvt)
}

/// Returned from `parse_fpgm` — the font program bytecode.
///
/// `fpgm` is a raw bytecode stream executed once when the font is loaded.
/// It typically contains function definitions (FDEF/ENDF) and storage
/// area initialization.
pub fn parse_fpgm(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

/// Returned from `parse_prep` — the CVT program bytecode.
///
/// `prep` is a raw bytecode stream executed each time the pixel size changes.
/// It scales CVT values for the current ppem and may adjust the graphics state.
pub fn parse_prep(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cvt_empty() {
        let cvt = parse_cvt(&[]).unwrap();
        assert!(cvt.is_empty());
    }

    #[test]
    fn test_parse_cvt_single() {
        // Single FWORD value: 0x0135 = 309 FU → 309 * 64 = 19776 (26.6)
        let data = [0x01u8, 0x35];
        let cvt = parse_cvt(&data).unwrap();
        assert_eq!(cvt.len(), 1);
        assert_eq!(cvt[0], 309 * 64);
    }

    #[test]
    fn test_parse_cvt_negative() {
        // Negative FWORD: 0xFF9C = -100 FU → -100 * 64 = -6400
        let data = [0xFFu8, 0x9C];
        let cvt = parse_cvt(&data).unwrap();
        assert_eq!(cvt[0], -100 * 64);
    }

    #[test]
    fn test_parse_cvt_multiple() {
        // Two entries: 0x0064 (100) and 0xFFCE (-50)
        let data = [0x00, 0x64, 0xFF, 0xCE];
        let cvt = parse_cvt(&data).unwrap();
        assert_eq!(cvt.len(), 2);
        assert_eq!(cvt[0], 100 * 64);
        assert_eq!(cvt[1], -50 * 64);
    }
}
