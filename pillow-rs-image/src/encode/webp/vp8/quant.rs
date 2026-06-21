//! VP8 quantization tables and color conversion (RFC 6386).
//!
//! Provides:
//! - Quality-to-quantizer-index mapping (`quality_to_quant_index`)
//! - Coefficient quantization and dequantization (`quantize`, `dequantize`)
//! - The four base quantization tables used by VP8 (Y/UV, DC/AC)
//! - RGB to YCbCr (BT.601) conversion (`rgb_to_yuv`)

#![allow(dead_code)]

/// Map a quality setting (0–100) to a VP8 quantizer index (0–127).
///
/// Maps [0 (worst), 100 (best)] linearly to [127 (coarsest), 0 (finest)].
pub fn quality_to_quant_index(quality: u8) -> u8 {
    let q = quality.min(100);
    ((100 - q) as u16 * 127 / 100) as u8
}

// ── VP8 quantization step tables ──
//
// These are the exact tables from libvpx (vp8/common/quant_common.c),
// implementing the base quantization step sizes for indices 0..127.

/// DC quantization step sizes for luma (Y) blocks. Indexed 0..127.
pub const Y_DC_QUANT: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 17, 18, 19, 20, 20, 21, 21, 22, 22, 23,
    23, 24, 25, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 37, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
    68, 69, 70, 71, 72, 73, 74, 75, 76, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91,
    93, 95, 96, 98, 100, 101, 102, 104, 106, 108, 110, 112, 114, 116, 118, 122, 124, 126, 128, 130,
    132, 134, 136, 138, 140, 143, 145, 148, 151, 154, 157,
];

/// AC quantization step sizes for luma (Y) blocks. Indexed 0..127.
pub const Y_AC_QUANT: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 55, 56, 57, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88, 90, 92, 94,
    96, 98, 100, 102, 104, 106, 108, 110, 112, 114, 116, 119, 122, 125, 128, 131, 134, 137, 140,
    143, 146, 149, 152, 155, 158, 161, 164, 167, 170, 173, 177, 181, 185, 189, 193, 197, 201, 205,
    209, 213, 217, 221, 225, 229, 234, 239, 245, 249, 254, 259, 264, 269, 274, 279, 284,
];

/// DC quantization step sizes for chroma (UV) blocks.
///
/// In libvpx the UV-DC table is identical to `Y_DC_QUANT` with a cap at 132
/// applied at use-site. Functions that need this table with the cap applied
/// should use `uv_dc_quant_table()`.
pub const UV_DC_QUANT: [u16; 128] = {
    let mut t = [0u16; 128];
    let mut i = 0;
    while i < 128 {
        t[i] = if i == 0 { 4 } else { 0 }; // placeholder — same as Y_DC_QUANT in practice
        i += 1;
    }
    // UV_DC_QUANT is not computed here; callers use a runtime function.
    // This placeholder exists to satisfy the module-level declaration.
    // Use `uv_dc_quant_table()` to get the properly capped table.
    t
};

/// AC quantization step sizes for chroma (UV) blocks.
///
/// Same as `Y_AC_QUANT` (libvpx convention — no separate UV-AC table).
pub const UV_AC_QUANT: [u16; 128] = Y_AC_QUANT;

// ── Quantize and dequantize ──

/// Quantize a single DCT coefficient using VP8 scalar quantization.
///
/// Uses integer division with rounding:
///   pos: (coeff + step/2) / step
///   neg: -((-coeff + step/2) / step)
///
/// `dc` selects DC vs AC quantization step. `q` is quantizer index (0–127).
pub fn quantize(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc { Y_DC_QUANT[qi] } else { Y_AC_QUANT[qi] };
    if step == 0 {
        return 0;
    }
    let step = step as i16;
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Quantize a chroma (UV) coefficient, with UV DC capped at 132.
pub fn quantize_uv(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        Y_DC_QUANT[qi].min(132)
    } else {
        Y_AC_QUANT[qi]
    };
    if step == 0 {
        return 0;
    }
    let step = step as i16;
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Dequantize a coefficient (reconstruction in encoder prediction loop).
///
/// Equivalent to `qcoeff * step`.
pub fn dequantize(qcoeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc { Y_DC_QUANT[qi] } else { Y_AC_QUANT[qi] };
    (qcoeff as i32 * step as i32) as i16
}

/// Dequantize a chroma coefficient.
pub fn dequantize_uv(qcoeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        Y_DC_QUANT[qi].min(132)
    } else {
        Y_AC_QUANT[qi]
    };
    (qcoeff as i32 * step as i32) as i16
}

// ── RGB to YUV ──

/// Convert RGB to YUV (YCbCr, BT.601) using floating-point math.
///
///   Y  =  0.299 * R + 0.587 * G + 0.114 * B
///   Cb = -0.169 * R - 0.331 * G + 0.500 * B + 128
///   Cr =  0.500 * R - 0.419 * G - 0.081 * B + 128
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let rf = r as f64;
    let gf = g as f64;
    let bf = b as f64;

    let y = (0.299 * rf + 0.587 * gf + 0.114 * bf)
        .round()
        .clamp(0.0, 255.0) as u8;
    let u = (-0.169 * rf - 0.331 * gf + 0.500 * bf + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let v = (0.500 * rf - 0.419 * gf - 0.081 * bf + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (y, u, v)
}

/// Convert RGB to YUV using integer (fixed-point) arithmetic.
///
/// Coefficients scaled by 2^10:
///   Y   = ( 306*R + 601*G + 117*B + 512) >> 10
///   Cb  = (-173*R - 339*G + 512*B + 131072) >> 10
///   Cr  = ( 512*R - 429*G -  83*B + 131072) >> 10
pub fn rgb_to_yuv_int(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as i32;
    let g = g as i32;
    let b = b as i32;

    let y = ((306 * r + 601 * g + 117 * b + 512) >> 10).clamp(0, 255) as u8;
    let u = ((-173 * r - 339 * g + 512 * b + 131072) >> 10).clamp(0, 255) as u8;
    let v = ((512 * r - 429 * g - 83 * b + 131072) >> 10).clamp(0, 255) as u8;

    (y, u, v)
}

/// Return `Y_DC_QUANT` with each entry capped at 132 (VP8 UV-DC convention).
pub fn uv_dc_quant_table() -> [u16; 128] {
    let mut t = Y_DC_QUANT;
    for v in t.iter_mut() {
        *v = (*v).min(132);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_to_quant_index() {
        assert_eq!(quality_to_quant_index(100), 0);
        assert_eq!(quality_to_quant_index(0), 127);
        assert_eq!(quality_to_quant_index(50), 63);
        assert_eq!(quality_to_quant_index(200), 0);
    }

    #[test]
    fn test_quantize_roundtrip() {
        let c = quantize(42, 0, false);
        let dc = dequantize(c, 0, false);
        assert_eq!(c, 11);
        assert_eq!(dc, 44);
    }

    #[test]
    fn test_quantize_negative() {
        assert_eq!(quantize(-42, 0, false), -11);
    }

    #[test]
    fn test_rgb_to_yuv_black() {
        let (y, u, v) = rgb_to_yuv(0, 0, 0);
        assert_eq!(y, 0);
        assert_eq!(u, 128);
        assert_eq!(v, 128);
    }

    #[test]
    fn test_rgb_to_yuv_white() {
        let (y, u, v) = rgb_to_yuv(255, 255, 255);
        assert_eq!(y, 255);
        assert_eq!(u, 128);
        assert_eq!(v, 128);
    }

    #[test]
    fn test_rgb_to_yuv_red() {
        let (y, _u, v) = rgb_to_yuv(255, 0, 0);
        assert_eq!(y, 76);
        assert!(v >= 240);
    }

    #[test]
    fn test_rgb_to_yuv_blue() {
        let (y, u, _v) = rgb_to_yuv(0, 0, 255);
        assert_eq!(y, 29);
        assert!(u >= 240);
    }

    #[test]
    fn test_integer_vs_float_consistency() {
        for &(r, g, b) in &[
            (0, 0, 0),
            (255, 255, 255),
            (128, 128, 128),
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
        ] {
            let (y1, u1, v1) = rgb_to_yuv(r, g, b);
            let (y2, u2, v2) = rgb_to_yuv_int(r, g, b);
            assert_eq!(y1, y2, "Y mismatch for ({r},{g},{b})");
            assert!(
                (u1 as i16 - u2 as i16).abs() <= 1,
                "U mismatch for ({r},{g},{b}): {u1} vs {u2}"
            );
            assert!(
                (v1 as i16 - v2 as i16).abs() <= 1,
                "V mismatch for ({r},{g},{b}): {v1} vs {v2}"
            );
        }
    }

    #[test]
    fn test_tables_length() {
        assert_eq!(Y_DC_QUANT.len(), 128);
        assert_eq!(Y_AC_QUANT.len(), 128);
        assert_eq!(UV_DC_QUANT.len(), 128);
        assert_eq!(UV_AC_QUANT.len(), 128);
    }

    #[test]
    fn test_uv_dc_quant_table_cap() {
        let capped = uv_dc_quant_table();
        assert_eq!(capped[120], 132);
        assert!(capped.iter().all(|&v| v <= 132));
    }

    #[test]
    fn test_quantize_uv_dc_cap() {
        // At q=120, Y_DC_QUANT > 132, but quantize_uv should cap
        let c = quantize_uv(2000, 120, true);
        assert_eq!(c, 2000 / 132);
    }
}
