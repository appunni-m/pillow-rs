//! Utility functions for pixel format conversion, alignment, and preprocessing.
//!
//! These are pure-Rust implementations of logic that previously resided in the
//! Python binding layer (CLAUDE.md "thin wrapper" rule).  Each function performs
//! one well-defined transformation and returns Rust primitives.
//!
//! # Functions
//!
//! * `align_row_to_32` — BMP/Qt-compatible scanline padding to 4-byte boundary
use crate::error::PilError;

/// Convert each scanline from 8-bit to 32-bit aligned (PIL / Qt compatibility).
///
/// PIL's `ImageQt._toqclass_helper` pads every row to a 4-byte boundary, which
/// is what QImage's raw-buffer constructor requires.  Without this padding,
/// QImage over-reads the buffer and picks up garbage bytes.
///
/// Formula:  `bytes_per_line = ((width * bits + 31) & !31) / 8`
///
/// # Parameters
/// * `data` — raw pixel data (rows densely packed, no padding)
/// * `width` — number of pixels per row
/// * `bits_per_pixel` — 1 for mode "1", 8 for "L"/"P"
pub fn align_row_to_32(data: &[u8], width: u32, bits_per_pixel: u8) -> Result<Vec<u8>, PilError> {
    let bits_per_line = bits_per_pixel as u64 * width as u64;
    let bytes_per_line = ((bits_per_line + 7) / 8) as usize;

    if bytes_per_line == 0 {
        return Err(PilError::ValueError(
            "align_row_to_32: zero bytes per line".into(),
        ));
    }

    let extra_padding = (4 - (bytes_per_line % 4)) % 4;
    if extra_padding == 0 {
        return Ok(data.to_vec());
    }

    if data.is_empty() {
        return Ok(Vec::new());
    }

    let rows = data.len() / bytes_per_line;
    if rows == 0 {
        return Err(PilError::ValueError(
            "align_row_to_32: data shorter than one row".into(),
        ));
    }

    let padded_len = rows * (bytes_per_line + extra_padding);
    let mut padded = vec![0u8; padded_len];

    for i in 0..rows {
        let src_start = i * bytes_per_line;
        let dst_start = i * (bytes_per_line + extra_padding);
        padded[dst_start..dst_start + bytes_per_line]
            .copy_from_slice(&data[src_start..src_start + bytes_per_line]);
    }

    Ok(padded)
}
