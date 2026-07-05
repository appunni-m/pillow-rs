//! VP8 DCT coefficient tokenization (RFC 6386 Section 13).
//!
//! Converts quantized 4×4 DCT coefficients into (token, extra_bits) pairs
//! suitable for boolean entropy coding via the VP8 bool encoder.

#![allow(dead_code)]

// ── DCT Token Constants (Section 13.2) ──

/// End-of-block marker token.
pub const DCT_EOB: i8 = 11;
/// Token: coefficient value is 0.
pub const DCT_0: i8 = 0;
/// Token: coefficient value is ±1.
pub const DCT_1: i8 = 1;
/// Token: coefficient value is ±2.
pub const DCT_2: i8 = 2;
/// Token: coefficient value is ±3.
pub const DCT_3: i8 = 3;
/// Token: coefficient value is ±4.
pub const DCT_4: i8 = 4;
/// Token: coefficient value is 5–6 (category 1).
pub const DCT_CAT1: i8 = 5;
/// Token: coefficient value is 7–10 (category 2).
pub const DCT_CAT2: i8 = 6;
/// Token: coefficient value is 11–18 (category 3).
pub const DCT_CAT3: i8 = 7;
/// Token: coefficient value is 19–34 (category 4).
pub const DCT_CAT4: i8 = 8;
/// Token: coefficient value is 35–66 (category 5).
pub const DCT_CAT5: i8 = 9;
/// Token: coefficient value is 67+ (category 6).
pub const DCT_CAT6: i8 = 10;

// ── Token Category Metadata ──

/// Base values for each DCT value category.
/// Entry `i` gives the smallest absolute coefficient value that falls in category `i+5`.
pub const DCT_CAT_BASE: [u8; 6] = [5, 7, 11, 19, 35, 67];

/// Extra bits per category: number of additional bits to encode after the token
/// to disambiguate which value within the category range.
pub const DCT_CAT_EXTRA_BITS: [u8; 6] = [1, 2, 3, 4, 5, 11];

/// Maximum number of DCT tokens (excluding EOB).
pub const NUM_DCT_TOKENS: usize = 12;

// ── Coefficient Bands ──

/// Band assignment for each position in the 4×4 zigzag scan order.
/// Determines which probability context to use when coding each coefficient.
pub const COEFF_BANDS: [u8; 16] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7];

/// Number of coefficient bands.
pub const NUM_COEFF_BANDS: usize = 8;

// ── Scan Order ──

/// VP8 4×4 zigzag scan order: maps scan position → linear index in the 4×4 block.
pub const ZIGZAG: [u8; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// Inverse zigzag: maps linear index → scan position.
pub const ZIGZAG_INV: [u8; 16] = [0, 1, 5, 6, 2, 4, 7, 12, 3, 8, 11, 13, 9, 10, 14, 15];

/// Number of coefficient types: Y, Y2 (WHT DC), U, V.
pub const NUM_COEFF_TYPES: usize = 4;

/// Index of each coefficient type.
pub const COEFF_TYPE_Y: usize = 0;
pub const COEFF_TYPE_U: usize = 1;
pub const COEFF_TYPE_V: usize = 2;
pub const COEFF_TYPE_Y2: usize = 3;

/// Number of probability contexts per band.
pub const NUM_COEFF_CONTEXTS: usize = 3;

// ── Token Probability Tables ──

/// Default coefficient token probabilities.
/// Indexed as: [coeff_type][band][context][token_prob_index].
/// Token prob index 0..10 maps to interior nodes of DCT_TOKEN_TREE.
#[rustfmt::skip]
pub const COEFF_PROBS: [[[[u8; 11]; 3]; 8]; 4] = [
    // ── Type Y ──
    [
        [
            [128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
            [128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
            [128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
        ],
        [
            [253, 136, 254, 255, 228, 219, 128, 128, 128, 128, 128],
            [189, 129, 242, 255, 227, 213, 255, 219, 128, 128, 128],
            [106, 126, 227, 252, 214, 209, 255, 255, 128, 128, 128],
        ],
        [
            [1, 98, 248, 255, 236, 226, 255, 255, 128, 128, 128],
            [181, 133, 238, 254, 221, 234, 255, 154, 128, 128, 128],
            [78, 134, 202, 247, 198, 180, 255, 219, 128, 128, 128],
        ],
        [
            [1, 185, 249, 255, 243, 255, 128, 128, 128, 128, 128],
            [184, 150, 247, 255, 236, 224, 128, 128, 128, 128, 128],
            [77, 110, 216, 255, 236, 230, 128, 128, 128, 128, 128],
        ],
        [
            [1, 101, 251, 255, 241, 255, 128, 128, 128, 128, 128],
            [170, 139, 241, 252, 236, 209, 255, 255, 128, 128, 128],
            [37, 116, 196, 243, 228, 255, 255, 255, 128, 128, 128],
        ],
        [
            [1, 204, 254, 255, 245, 255, 128, 128, 128, 128, 128],
            [207, 160, 250, 255, 238, 128, 128, 128, 128, 128, 128],
            [102, 103, 231, 255, 211, 171, 128, 128, 128, 128, 128],
        ],
        [
            [1, 152, 252, 255, 240, 255, 128, 128, 128, 128, 128],
            [177, 135, 243, 255, 234, 225, 128, 128, 128, 128, 128],
            [80, 129, 211, 255, 194, 224, 128, 128, 128, 128, 128],
        ],
        [
            [1, 1, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [246, 1, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [255, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
        ],
    ],
    // ── Type U ──
    [
        [
            [198, 35, 237, 223, 255, 128, 128, 128, 128, 128, 128],
            [39, 136, 200, 203, 255, 128, 128, 128, 128, 128, 128],
            [105, 45, 163, 195, 254, 128, 128, 128, 128, 128, 128],
        ],
        [
            [143, 189, 230, 252, 204, 209, 255, 128, 128, 128, 128],
            [141, 179, 217, 255, 168, 241, 255, 128, 128, 128, 128],
            [44, 43, 118, 248, 153, 181, 255, 255, 128, 128, 128],
        ],
        [
            [44, 237, 253, 254, 216, 209, 255, 255, 128, 128, 128],
            [98, 157, 219, 254, 194, 174, 255, 255, 128, 128, 128],
            [38, 77, 130, 247, 127, 160, 255, 255, 128, 128, 128],
        ],
        [
            [56, 223, 249, 255, 229, 231, 128, 128, 128, 128, 128],
            [125, 172, 239, 255, 215, 224, 128, 128, 128, 128, 128],
            [29, 100, 203, 255, 206, 220, 128, 128, 128, 128, 128],
        ],
        [
            [22, 234, 247, 255, 227, 233, 128, 128, 128, 128, 128],
            [113, 164, 234, 255, 214, 226, 128, 128, 128, 128, 128],
            [34, 101, 194, 255, 196, 213, 128, 128, 128, 128, 128],
        ],
        [
            [38, 175, 245, 255, 224, 233, 128, 128, 128, 128, 128],
            [131, 175, 244, 252, 214, 225, 128, 128, 128, 128, 128],
            [31, 84, 199, 255, 182, 199, 128, 128, 128, 128, 128],
        ],
        [
            [44, 174, 241, 255, 215, 216, 128, 128, 128, 128, 128],
            [123, 146, 230, 252, 214, 209, 128, 128, 128, 128, 128],
            [28, 70, 173, 249, 163, 177, 128, 128, 128, 128, 128],
        ],
        [
            [3, 95, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [211, 6, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [148, 64, 255, 128, 128, 128, 128, 128, 128, 128, 128],
        ],
    ],
    // ── Type V ──
    [
        [
            [252, 122, 237, 180, 255, 128, 128, 128, 128, 128, 128],
            [68, 130, 222, 233, 255, 128, 128, 128, 128, 128, 128],
            [109, 71, 184, 220, 255, 128, 128, 128, 128, 128, 128],
        ],
        [
            [194, 197, 234, 255, 208, 210, 255, 128, 128, 128, 128],
            [162, 188, 221, 255, 194, 224, 255, 128, 128, 128, 128],
            [57, 71, 146, 247, 168, 196, 255, 255, 128, 128, 128],
        ],
        [
            [92, 226, 249, 255, 197, 196, 255, 255, 128, 128, 128],
            [132, 172, 222, 255, 192, 183, 255, 255, 128, 128, 128],
            [46, 81, 148, 244, 139, 167, 255, 255, 128, 128, 128],
        ],
        [
            [86, 223, 249, 255, 219, 218, 128, 128, 128, 128, 128],
            [154, 184, 240, 255, 218, 222, 128, 128, 128, 128, 128],
            [49, 108, 222, 255, 206, 217, 128, 128, 128, 128, 128],
        ],
        [
            [62, 230, 249, 255, 225, 223, 128, 128, 128, 128, 128],
            [144, 184, 239, 255, 213, 224, 128, 128, 128, 128, 128],
            [42, 108, 212, 255, 191, 209, 128, 128, 128, 128, 128],
        ],
        [
            [75, 203, 247, 255, 224, 218, 128, 128, 128, 128, 128],
            [157, 194, 248, 252, 215, 221, 128, 128, 128, 128, 128],
            [42, 93, 218, 255, 185, 208, 128, 128, 128, 128, 128],
        ],
        [
            [69, 201, 245, 255, 222, 211, 128, 128, 128, 128, 128],
            [149, 169, 235, 255, 218, 215, 128, 128, 128, 128, 128],
            [38, 76, 192, 247, 171, 186, 128, 128, 128, 128, 128],
        ],
        [
            [5, 141, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [234, 22, 255, 128, 128, 128, 128, 128, 128, 128, 128],
            [204, 44, 255, 128, 128, 128, 128, 128, 128, 128, 128],
        ],
    ],
    // ── Type Y2 (WHT DC) ──
    [
        [
            [248, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [194, 245, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [239, 254, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [200, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [244, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [213, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [230, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [232, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [238, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [246, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [240, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [249, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [241, 255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
        [
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
            [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ],
    ],
];

// ── DCT Token Tree ──

/// VP8 DCT token binary tree (Section 13.3).
/// Pairs of entries define interior nodes: positive = go to another node (index),
/// negative = leaf token value.
// VP8 tree convention: leaf values use -(token_value+1) so all leaves are negative (RFC 6386 Section 8).
pub const DCT_TOKEN_TREE: [i8; 22] = [
    -(DCT_EOB + 1),
    2,
    -(DCT_0 + 1),
    4,
    -(DCT_1 + 1),
    6,
    8,
    12,
    -(DCT_2 + 1),
    10,
    -(DCT_3 + 1),
    -(DCT_4 + 1),
    14,
    16,
    -(DCT_CAT1 + 1),
    -(DCT_CAT2 + 1),
    18,
    20,
    -(DCT_CAT3 + 1),
    -(DCT_CAT4 + 1),
    -(DCT_CAT5 + 1),
    -(DCT_CAT6 + 1),
];

// ── Functions ──

/// Classify a quantized coefficient absolute value into a DCT token.
///
/// Returns the token (0–10) and any extra bits needed to encode the value
/// within its category. For ZERO_TOKEN through FOUR_TOKEN, extra_bits is 0.
pub fn classify_coefficient(abs_value: i16) -> (i8, u8, u8) {
    let v = abs_value as u16;

    // Check small fixed tokens first
    if v == 0 {
        (DCT_0, 0, 0)
    } else if v == 1 {
        (DCT_1, 0, 0)
    } else if v == 2 {
        (DCT_2, 0, 0)
    } else if v == 3 {
        (DCT_3, 0, 0)
    } else if v == 4 {
        (DCT_4, 0, 0)
    } else if v <= 6 {
        // Category 1: values 5-6
        let extra = v - DCT_CAT_BASE[0] as u16;
        (DCT_CAT1, extra as u8, 1)
    } else if v <= 10 {
        // Category 2: values 7-10
        let extra = v - DCT_CAT_BASE[1] as u16;
        (DCT_CAT2, extra as u8, 2)
    } else if v <= 18 {
        // Category 3: values 11-18
        let extra = v - DCT_CAT_BASE[2] as u16;
        (DCT_CAT3, extra as u8, 3)
    } else if v <= 34 {
        // Category 4: values 19-34
        let extra = v - DCT_CAT_BASE[3] as u16;
        (DCT_CAT4, extra as u8, 4)
    } else if v <= 66 {
        // Category 5: values 35-66
        let extra = v - DCT_CAT_BASE[4] as u16;
        (DCT_CAT5, extra as u8, 5)
    } else {
        // Category 6: values 67+
        let extra = v - DCT_CAT_BASE[5] as u16;
        (DCT_CAT6, extra as u8, 11)
    }
}

/// Get the sign bit for a non-zero coefficient (0 = positive, 1 = negative).
pub fn sign_bit(coeff: i16) -> bool {
    coeff < 0
}

/// Get the coefficient band for a given zigzag scan position.
pub fn band_for_position(scan_pos: usize) -> usize {
    if scan_pos >= 16 {
        return 7; // clamp
    }
    COEFF_BANDS[scan_pos] as usize
}

/// Get the token probability table entry for given (coeff_type, band, context).
pub fn token_prob(coeff_type: usize, band: usize, context: usize) -> &'static [u8; 11] {
    &COEFF_PROBS[coeff_type][band][context]
}

/// Map zigzag index to linear block index.
pub fn zigzag_to_linear(zigzag_pos: usize) -> usize {
    ZIGZAG[zigzag_pos.min(15)] as usize
}

/// Map linear block index to zigzag position.
pub fn linear_to_zigzag(linear_idx: usize) -> usize {
    ZIGZAG_INV[linear_idx.min(15)] as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_coefficient ──

    #[test]
    fn test_classify_coefficient_zero() {
        assert_eq!(classify_coefficient(0), (DCT_0, 0, 0));
    }

    #[test]
    fn test_classify_coefficient_one() {
        assert_eq!(classify_coefficient(1), (DCT_1, 0, 0));
    }

    #[test]
    fn test_classify_coefficient_two() {
        assert_eq!(classify_coefficient(2), (DCT_2, 0, 0));
    }

    #[test]
    fn test_classify_coefficient_three() {
        assert_eq!(classify_coefficient(3), (DCT_3, 0, 0));
    }

    #[test]
    fn test_classify_coefficient_four() {
        assert_eq!(classify_coefficient(4), (DCT_4, 0, 0));
    }

    #[test]
    fn test_classify_coefficient_cat1_low() {
        assert_eq!(classify_coefficient(5), (DCT_CAT1, 0, 1));
    }

    #[test]
    fn test_classify_coefficient_cat1_high() {
        assert_eq!(classify_coefficient(6), (DCT_CAT1, 1, 1));
    }

    #[test]
    fn test_classify_coefficient_cat2_low() {
        assert_eq!(classify_coefficient(7), (DCT_CAT2, 0, 2));
    }

    #[test]
    fn test_classify_coefficient_cat2_high() {
        assert_eq!(classify_coefficient(10), (DCT_CAT2, 3, 2));
    }

    #[test]
    fn test_classify_coefficient_cat3_low() {
        assert_eq!(classify_coefficient(11), (DCT_CAT3, 0, 3));
    }

    #[test]
    fn test_classify_coefficient_cat3_high() {
        assert_eq!(classify_coefficient(18), (DCT_CAT3, 7, 3));
    }

    #[test]
    fn test_classify_coefficient_cat4_low() {
        assert_eq!(classify_coefficient(19), (DCT_CAT4, 0, 4));
    }

    #[test]
    fn test_classify_coefficient_cat4_high() {
        assert_eq!(classify_coefficient(34), (DCT_CAT4, 15, 4));
    }

    #[test]
    fn test_classify_coefficient_cat5_low() {
        assert_eq!(classify_coefficient(35), (DCT_CAT5, 0, 5));
    }

    #[test]
    fn test_classify_coefficient_cat5_high() {
        assert_eq!(classify_coefficient(66), (DCT_CAT5, 31, 5));
    }

    #[test]
    fn test_classify_coefficient_cat6_low() {
        assert_eq!(classify_coefficient(67), (DCT_CAT6, 0, 11));
    }

    #[test]
    fn test_classify_coefficient_cat6_mid() {
        assert_eq!(classify_coefficient(100), (DCT_CAT6, 33, 11));
    }

    #[test]
    fn test_classify_coefficient_cat6_large() {
        assert_eq!(classify_coefficient(322), (DCT_CAT6, 255, 11));
    }

    // ── sign_bit ──

    #[test]
    fn test_sign_bit_positive() {
        assert!(!sign_bit(5));
        assert!(!sign_bit(1));
        assert!(!sign_bit(100));
    }

    #[test]
    fn test_sign_bit_negative() {
        assert!(sign_bit(-5));
        assert!(sign_bit(-1));
        assert!(sign_bit(-100));
    }

    #[test]
    fn test_sign_bit_zero() {
        assert!(!sign_bit(0));
    }

    // ── band_for_position ──

    #[test]
    fn test_band_for_position_dc() {
        assert_eq!(band_for_position(0), 0);
    }

    #[test]
    fn test_band_for_position_scan_1() {
        assert_eq!(band_for_position(1), 1);
    }

    #[test]
    fn test_band_for_position_scan_2() {
        assert_eq!(band_for_position(2), 2);
    }

    #[test]
    fn test_band_for_position_scan_3() {
        assert_eq!(band_for_position(3), 3);
    }

    #[test]
    fn test_band_for_position_scan_4() {
        assert_eq!(band_for_position(4), 6);
    }

    #[test]
    fn test_band_for_position_scan_5() {
        assert_eq!(band_for_position(5), 4);
    }

    #[test]
    fn test_band_for_position_scan_15() {
        assert_eq!(band_for_position(15), 7);
    }

    #[test]
    fn test_band_for_position_clamp() {
        assert_eq!(band_for_position(16), 7);
        assert_eq!(band_for_position(100), 7);
    }

    // ── zigzag_to_linear / linear_to_zigzag ──

    #[test]
    fn test_zigzag_roundtrip_all_positions() {
        for i in 0..16 {
            let linear = zigzag_to_linear(i);
            let back = linear_to_zigzag(linear);
            assert_eq!(back, i, "roundtrip failed for zigzag position {}", i);
        }
    }

    #[test]
    fn test_zigzag_to_linear_specific() {
        // ZIGZAG[0] = 0, ZIGZAG[1] = 1, ZIGZAG[2] = 4, ZIGZAG[3] = 8
        assert_eq!(zigzag_to_linear(0), 0);
        assert_eq!(zigzag_to_linear(1), 1);
        assert_eq!(zigzag_to_linear(2), 4);
        assert_eq!(zigzag_to_linear(3), 8);
        assert_eq!(zigzag_to_linear(4), 5);
        assert_eq!(zigzag_to_linear(5), 2);
    }

    #[test]
    fn test_linear_to_zigzag_specific() {
        // ZIGZAG_INV[0] = 0, ZIGZAG_INV[1] = 1, ZIGZAG_INV[2] = 5, ZIGZAG_INV[3] = 6
        assert_eq!(linear_to_zigzag(0), 0);
        assert_eq!(linear_to_zigzag(1), 1);
        assert_eq!(linear_to_zigzag(4), 2);
        assert_eq!(linear_to_zigzag(8), 3);
    }

    // ── ZIGZAG array ──

    #[test]
    fn test_zigzag_array_first_four() {
        assert_eq!(ZIGZAG[0], 0);
        assert_eq!(ZIGZAG[1], 1);
        assert_eq!(ZIGZAG[2], 4);
        assert_eq!(ZIGZAG[3], 8);
    }

    #[test]
    fn test_zigzag_array_length() {
        assert_eq!(ZIGZAG.len(), 16);
    }

    #[test]
    fn test_zigzag_array_all_values_present() {
        let mut seen = [false; 16];
        for &v in ZIGZAG.iter() {
            assert!(v < 16, "value {} out of range", v);
            assert!(!seen[v as usize], "duplicate value {} in ZIGZAG", v);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&x| x), "not all 0-15 appear in ZIGZAG");
    }

    // ── ZIGZAG_INV array ──

    #[test]
    fn test_zigzag_inv_is_inverse() {
        for i in 0..16 {
            let zig = ZIGZAG[i] as usize;
            assert_eq!(ZIGZAG_INV[zig], i as u8);
        }
    }

    #[test]
    fn test_zigzag_inv_all_values_present() {
        let mut seen = [false; 16];
        for &v in ZIGZAG_INV.iter() {
            assert!(v < 16, "value {} out of range", v);
            assert!(!seen[v as usize], "duplicate value {} in ZIGZAG_INV", v);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&x| x), "not all 0-15 appear in ZIGZAG_INV");
    }

    // ── token_prob ──

    #[test]
    fn test_token_prob_returns_correct_slice() {
        // token_prob for coeff_type=0 (Y), band=0, context=0 should return
        // &COEFF_PROBS[0][0][0] = [128; 11]
        let probs = token_prob(0, 0, 0);
        assert_eq!(probs.len(), 11);
        for &p in probs.iter() {
            assert_eq!(p, 128);
        }
    }

    #[test]
    fn test_token_prob_differs_per_type() {
        // Different coefficient types should have different probabilities
        let y_band1_ctx0 = token_prob(0, 1, 0);
        let u_band1_ctx0 = token_prob(1, 1, 0);
        assert_ne!(y_band1_ctx0, u_band1_ctx0);
    }

    #[test]
    fn test_token_prob_y2_high_probs() {
        // Y2 type has many 255 values
        let probs = token_prob(3, 0, 0);
        for &p in probs.iter() {
            assert!(p >= 248);
        }
    }

    // ── DCT token constants ──

    #[test]
    fn test_dct_token_constants_unique() {
        let tokens = [
            DCT_EOB, DCT_0, DCT_1, DCT_2, DCT_3, DCT_4, DCT_CAT1, DCT_CAT2, DCT_CAT3, DCT_CAT4,
            DCT_CAT5, DCT_CAT6,
        ];
        let mut seen = std::collections::HashSet::new();
        for &t in &tokens {
            assert!(seen.insert(t), "duplicate constant value {}", t);
        }
    }

    #[test]
    fn test_dct_cat_base_and_extra_bits_lengths() {
        assert_eq!(DCT_CAT_BASE.len(), 6);
        assert_eq!(DCT_CAT_EXTRA_BITS.len(), 6);
    }

    // ── COEFF_BANDS ──

    #[test]
    fn test_coeff_bands_length() {
        assert_eq!(COEFF_BANDS.len(), 16);
    }

    #[test]
    fn test_coeff_bands_all_in_range() {
        for &b in COEFF_BANDS.iter() {
            assert!(
                b < NUM_COEFF_BANDS as u8,
                "band {} >= NUM_COEFF_BANDS={}",
                b,
                NUM_COEFF_BANDS
            );
        }
    }

    // ── DCT_TOKEN_TREE ──

    #[test]
    fn test_dct_token_tree_length() {
        assert_eq!(DCT_TOKEN_TREE.len(), 22);
    }

    #[test]
    fn test_dct_token_tree_leaf_values_negative() {
        for (i, &val) in DCT_TOKEN_TREE.iter().enumerate() {
            // Even-indexed entries are the "left" child, odd-indexed are "right".
            // A negative value is a leaf (VP8 convention: -(token+1) so always negative).
            // Positive values are indices pointing to another node pair.
            if val < 0 {
                // Leaf: extract token = -(val + 1); verify in valid range
                let token = -val as i32 - 1;
                assert!(
                    token >= 0 && token <= DCT_EOB as i32,
                    "leaf value {} at index {} yields token {} out of range",
                    val,
                    i,
                    token
                );
            } else {
                // Interior node: should be an even index within bounds
                assert!(
                    val % 2 == 0,
                    "interior node {} at index {} is not even",
                    val,
                    i
                );
                assert!(
                    (val as usize) < DCT_TOKEN_TREE.len(),
                    "interior node {} at index {} out of bounds",
                    val,
                    i
                );
            }
        }
    }

    #[test]
    fn test_dct_token_tree_ends_with_cat5_cat6() {
        // Last pair should be -(CAT5+1), -(CAT6+1) = -(9+1), -(10+1)
        assert_eq!(DCT_TOKEN_TREE[20], -(DCT_CAT5 + 1));
        assert_eq!(DCT_TOKEN_TREE[21], -(DCT_CAT6 + 1));
    }

    // ── COEFF_PROBS dimensions ──

    #[test]
    fn test_coeff_probs_dimensions() {
        assert_eq!(COEFF_PROBS.len(), 4); // 4 coeff types
        for ct in 0..4 {
            assert_eq!(COEFF_PROBS[ct].len(), 8); // 8 bands
            for b in 0..8 {
                assert_eq!(COEFF_PROBS[ct][b].len(), 3); // 3 contexts
                for ctx in 0..3 {
                    assert_eq!(COEFF_PROBS[ct][b][ctx].len(), 11); // 11 prob entries
                }
            }
        }
    }

    #[test]
    fn test_coeff_probs_values_in_range() {
        for ct in 0..4 {
            for b in 0..8 {
                for ctx in 0..3 {
                    for &p in COEFF_PROBS[ct][b][ctx].iter() {
                        assert!(p > 0, "prob is zero at [{}{}{}]", ct, b, ctx);
                    }
                }
            }
        }
    }

    // ── Constants ──

    #[test]
    fn test_num_dct_tokens() {
        assert_eq!(NUM_DCT_TOKENS, 12);
    }

    #[test]
    fn test_num_coeff_bands() {
        assert_eq!(NUM_COEFF_BANDS, 8);
    }

    #[test]
    fn test_num_coeff_types() {
        assert_eq!(NUM_COEFF_TYPES, 4);
    }

    #[test]
    fn test_num_coeff_contexts() {
        assert_eq!(NUM_COEFF_CONTEXTS, 3);
    }

    #[test]
    fn test_coeff_type_indices_unique() {
        let indices = [COEFF_TYPE_Y, COEFF_TYPE_U, COEFF_TYPE_V, COEFF_TYPE_Y2];
        let mut seen = std::collections::HashSet::new();
        for &idx in &indices {
            assert!(seen.insert(idx), "duplicate coeff type index {}", idx);
        }
        for &idx in &indices {
            assert!(
                idx < NUM_COEFF_TYPES,
                "coeff type index {} >= NUM_COEFF_TYPES={}",
                idx,
                NUM_COEFF_TYPES
            );
        }
    }
}
