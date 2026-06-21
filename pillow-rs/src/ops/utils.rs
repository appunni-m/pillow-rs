//! Utility functions for pixel format conversion, alignment, and preprocessing.
//!
//! These are pure-Rust implementations of logic that previously resided in the
//! Python binding layer (CLAUDE.md "thin wrapper" rule).  Each function performs
//! one well-defined transformation and returns Rust primitives.
//!
//! # Functions
//!
//! * `align_row_to_32` — BMP/Qt-compatible scanline padding to 4-byte boundary
//! * `flatten_pixel_list` — reduce nested Python-style lists to flat byte arrays

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

/// Flatten a nested list/tuple of integers into a flat byte array.
///
/// Accepts either a flat list `[0, 255, 128, …]` or a nested list of rows
/// `[[0, 1], [2, 3], …]` and returns a single `Vec<u8>`.  Returns an error
/// if any element is outside the 0..256 range.
///
/// This corresponds to PIL's `Image.fromarray` list-flattening path.
pub fn flatten_pixel_list(values: &[i32]) -> Result<Vec<u8>, PilError> {
    if values.is_empty() {
        return Err(PilError::ValueError(
            "flatten_pixel_list: empty list".into(),
        ));
    }
    let mut out = Vec::with_capacity(values.len());
    for &v in values {
        if v < 0 || v > 255 {
            return Err(PilError::ValueError(format!(
                "flatten_pixel_list: pixel value {} out of range [0, 255]",
                v
            )));
        }
        out.push(v as u8);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_row_to_32_no_padding() {
        // 8-bit, width 4 -> 4 bytes per row, already aligned
        let data = vec![0u8, 1, 2, 3];
        let result = align_row_to_32(&data, 4, 8).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_align_row_to_32_with_padding() {
        // 8-bit, width 3 -> 3 bytes per row, needs 1 byte padding
        let data = vec![10u8, 20, 30, 40, 50, 60]; // 2 rows of 3
        let result = align_row_to_32(&data, 3, 8).unwrap();
        assert_eq!(result.len(), 8); // 2 * 4 bytes
        assert_eq!(result[0..3], [10, 20, 30]);
        assert_eq!(result[4..7], [40, 50, 60]);
    }

    #[test]
    fn test_align_row_to_32_1bit() {
        // 1-bit, width 8 -> 1 byte per row -> needs 3 bytes padding
        let data = vec![0xAAu8, 0x55u8]; // 2 rows
        let result = align_row_to_32(&data, 8, 1).unwrap();
        assert_eq!(result.len(), 8); // 2 * 4 bytes
    }

    #[test]
    fn test_align_row_to_32_empty() {
        let result = align_row_to_32(&[], 4, 8).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_flatten_pixel_list_flat() {
        let result = flatten_pixel_list(&[0, 128, 255]).unwrap();
        assert_eq!(result, vec![0u8, 128, 255]);
    }

    #[test]
    fn test_flatten_pixel_list_out_of_range() {
        let result = flatten_pixel_list(&[0, 256]);
        assert!(result.is_err());
    }

    #[test]
    fn test_flatten_pixel_list_empty() {
        let result = flatten_pixel_list(&[]);
        assert!(result.is_err());
    }
}
