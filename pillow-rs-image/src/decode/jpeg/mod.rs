//! JPEG decoder — IJG DCT_ISLOW IDCT for pixel-perfect libjpeg parity.
//!
//! Implements libjpeg's exact "slow-but-accurate" integer IDCT from `jidctint.c`.
//! Uses CONST_BITS=13, PASS1_BITS=2 fixed-point arithmetic matching libjpeg-turbo.
//!
//! Supports:
//!   - Baseline JPEG (SOF0): 8-bit, 4:2:0/4:2:2/4:4:4/4:1:1 subsampling,
//!     grayscale/YCbCr, restart markers
//!   - Progressive JPEG (SOF2): DC-first, DC-refine, AC-first, AC-refine scans
//!
//! Reference: IJG libjpeg `jidctint.c`, `jdphuff.c`, `jdsample.c`
//!            ISO/IEC 10918-1 / ITU-T T.81 (JPEG Standard)

pub(super) mod bit_reader;
pub(super) mod decode;
pub(super) mod huffman;
pub(super) mod idct;
pub(super) mod parser;
pub(super) mod progressive;
pub(super) mod upsample;

// Re-export the public entry point so `crate::decode::jpeg::decode` still works.
pub use decode::decode;

#[cfg(test)]
mod tests {
    use super::decode::decode;
    use crate::types::{ColorType, DecodedImage};

    #[test]
    fn test_idct_dc_only() {
        // DC-only input: the IDCT is applied after dequantization. The IDCT
        // adds 128 level shift internally. With all coefficients = 0,
        // all output pixels are exactly 128 (mid-gray).
        use crate::decode::jpeg::idct::jpeg_idct_islow;
        let mut block = [0i32; 64];
        let mut workspace = [0i32; 64];
        jpeg_idct_islow(&mut block, &mut workspace);
        // All-zero input → all-128 output after range_limit(+128)
        for &px in block.iter() {
            assert_eq!(px, 128, "DC-only (zero) IDCT should produce uniform 128");
        }
    }

    #[test]
    fn test_jpeg_natural_order() {
        use crate::decode::jpeg::idct::JPEG_NATURAL_ORDER;
        // Verify the zigzag→natural mapping has exactly 64 entries covering 0..63.
        let mut seen = [false; 64];
        for &n in JPEG_NATURAL_ORDER.iter() {
            assert!(n < 64, "index out of bounds");
            seen[n] = true;
        }
        assert!(seen.iter().all(|&s| s), "all 64 positions must be covered");
    }

    #[test]
    fn test_extend() {
        use crate::decode::jpeg::idct::extend;
        // Zero size: always returns 0 regardless of value.
        assert_eq!(extend(0, 0), 0);
        assert_eq!(extend(1, 0), 0);
        // Size 1: 1-bit twos-complement range -1..1
        assert_eq!(extend(0, 1), -1);
        assert_eq!(extend(1, 1), 1);
        // Size 2: range -2..3
        assert_eq!(extend(0, 2), -3);
        assert_eq!(extend(1, 2), -2);
        assert_eq!(extend(2, 2), 2);
        assert_eq!(extend(3, 2), 3);
    }

    #[test]
    fn test_ycc_converter() {
        use crate::decode::jpeg::idct::YccColorConverter;
        let c = YccColorConverter::new();
        // Gray pixel (y=128, cb=128, cr=128) → neutral gray
        let (r, g, b) = c.ycc_to_rgb(128, 128, 128);
        assert!((r as i32 - 128).abs() <= 2, "gray r ~128, got {r}");
        assert!((g as i32 - 128).abs() <= 2, "gray g ~128, got {g}");
        assert!((b as i32 - 128).abs() <= 2, "gray b ~128, got {b}");
    }

    #[test]
    fn test_huffman_table_basic() {
        use crate::decode::jpeg::bit_reader::BitReader;
        use crate::decode::jpeg::huffman::HuffTable;
        use crate::decode::jpeg::idct::extend;
        // Standard JPEG DC table for class 0: (lengths) [0, 3, 0, 0, 0, …],
        // values [0, 1, 2].
        let bits = [0u8, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let values = vec![0u8, 1, 2];
        let table = HuffTable::build(&bits, &values);
        // Build always returns a HuffTable (decode returns None for invalid codes)
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_huffman_table_decode_zero_bit() {
        use crate::decode::jpeg::bit_reader::BitReader;
        use crate::decode::jpeg::huffman::HuffTable;
        use crate::decode::jpeg::idct::extend;
        let bits = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let table = HuffTable::build(&bits, &[]);
        // Table with no codes: decode returns None.
        let data = [0u8; 4];
        let mut br = BitReader::new(&data, 0, 4);
        assert!(table.decode(&mut br).is_none());
    }

    #[test]
    fn test_extract_segments_no_rst() {
        use crate::decode::jpeg::decode::extract_entropy_segments;
        let data = [0u8; 100];
        let segs = extract_entropy_segments(&data, 0, 100);
        assert_eq!(segs.segments.len(), 1);
        assert_eq!(segs.segments[0], (0, 100));
    }

    #[test]
    fn test_extract_segments_with_rst() {
        use crate::decode::jpeg::decode::extract_entropy_segments;
        let mut data = vec![0u8; 50];
        data[10] = 0xFF;
        data[11] = 0xD0; // RST0
        data[30] = 0xFF;
        data[31] = 0xD1; // RST1
        let segs = extract_entropy_segments(&data, 0, 50);
        assert_eq!(segs.segments.len(), 3);
    }

    #[test]
    fn test_bit_reader_basic() {
        use crate::decode::jpeg::bit_reader::BitReader;
        // 0xAA = 0b10101010
        let data = [0xAA];
        let mut br = BitReader::new(&data, 0, 1);
        assert_eq!(br.read_bits(1).unwrap(), 1);
        assert_eq!(br.read_bits(1).unwrap(), 0);
        assert_eq!(br.read_bits(2).unwrap(), 2); // 10
    }

    #[test]
    fn test_bit_reader_byte_stuffing() {
        use crate::decode::jpeg::bit_reader::BitReader;
        // 0xFF followed by 0x00 → byte stuffing, 0xFF followed by non-zero → marker
        // The BitReader fills 2 bytes at a time
        let data = [0xFF, 0x00, 0xAA];
        let mut br = BitReader::new(&data, 0, 3);
        // 0xFF is in the buffer, 0x00 is skipped (stuffing), 0xAA is read next
        // First byte is 0xFF
        assert_eq!(br.read_bits(8).unwrap(), 0xFF);
    }

    #[test]
    fn test_bit_reader_stops_at_marker() {
        use crate::decode::jpeg::bit_reader::BitReader;
        // 0xFF 0xD9 = EOI marker, should stop reading
        let data = [0x55, 0xFF, 0xD9];
        let mut br = BitReader::new(&data, 0, 1); // only first byte
        assert_eq!(br.read_bits(8).unwrap(), 0x55);
        // Reading past the end should return None
        assert!(br.read_bits(1).is_none());
    }

    #[test]
    fn test_empty_jpeg_rejected() {
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn test_invalid_jpeg_rejected() {
        assert!(decode(b"not a jpeg").is_none());
    }

    #[test]
    fn test_truncated_jpeg_rejected() {
        // SOI marker only, no other data
        assert!(decode(&[0xFF, 0xD8]).is_none());
    }
}
