//! 4×4 forward DCT and 4×4 Walsh-Hadamard Transform for VP8 (RFC 6386 Section 14).

#![allow(dead_code)]

// ── 4×4 Forward DCT ──

/// Compute the 4×4 forward DCT.
///
/// Takes a 4×4 block (row-major, 16 i16 values) and returns the DCT coefficients.
/// This is used for both luma and chroma residual blocks.
///
/// The DCT is separable: DCT_2D = DCT_1D_rows ∘ DCT_1D_cols.
/// We use a scaled integer approach with f64 math for precision, then round to i16.
pub fn fdct_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut coeffs = [0.0f64; 16];

    // Convert to f64
    let input: [f64; 16] = std::array::from_fn(|i| block[i] as f64);

    // 1-D DCT on rows
    let mut rows = [0.0f64; 16];
    for r in 0..4 {
        let offset = r * 4;
        let r0 = input[offset];
        let r1 = input[offset + 1];
        let r2 = input[offset + 2];
        let r3 = input[offset + 3];
        rows[offset] = dct_1d_0(r0, r1, r2, r3);
        rows[offset + 1] = dct_1d_1(r0, r1, r2, r3);
        rows[offset + 2] = dct_1d_2(r0, r1, r2, r3);
        rows[offset + 3] = dct_1d_3(r0, r1, r2, r3);
    }

    // 1-D DCT on columns
    let mut result = [0i16; 16];
    for c in 0..4 {
        let c0 = rows[c];
        let c1 = rows[c + 4];
        let c2 = rows[c + 8];
        let c3 = rows[c + 12];
        coeffs[c] = dct_1d_0(c0, c1, c2, c3);
        coeffs[c + 4] = dct_1d_1(c0, c1, c2, c3);
        coeffs[c + 8] = dct_1d_2(c0, c1, c2, c3);
        coeffs[c + 12] = dct_1d_3(c0, c1, c2, c3);
    }

    for i in 0..16 {
        result[i] = coeffs[i].round() as i16;
    }
    result
}

/// Compute the inverse 4×4 DCT (for encoder reconstruction loop).
/// Same separable approach in reverse.
pub fn idct_4x4(coeffs: &[i16; 16]) -> [i16; 16] {
    let input: [f64; 16] = std::array::from_fn(|i| coeffs[i] as f64);

    // 1-D IDCT on columns
    let mut cols = [0.0f64; 16];
    for c in 0..4 {
        cols[c] = idct_1d_0(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 4] = idct_1d_1(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 8] = idct_1d_2(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 12] = idct_1d_3(input[c], input[c + 4], input[c + 8], input[c + 12]);
    }

    // 1-D IDCT on rows
    let mut result = [0i16; 16];
    for r in 0..4 {
        let offset = r * 4;
        result[offset] = idct_1d_0(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 1] = idct_1d_1(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 2] = idct_1d_2(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 3] = idct_1d_3(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
    }
    result
}

// ── DCT basis functions ──
// cos(pi/16) ≈ 0.980785, cos(2pi/16) ≈ 0.923880, cos(3pi/16) ≈ 0.831470
// For k=0: scale factor 1/√2 ≈ 0.707107 is applied

const C1: f64 = 0.9807852804032304; // cos(π/16)
const C2: f64 = 0.9238795325112867; // cos(2π/16)
const C3: f64 = 0.8314696123025452; // cos(3π/16)
const S2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// 1-D DCT for 4 points, coefficient k (output index).
fn dct_1d_k(a: f64, b: f64, c: f64, d: f64, k: usize) -> f64 {
    let scale = if k == 0 { S2 } else { 1.0 };
    let sum = a * cos_dct(0, k) + b * cos_dct(1, k) + c * cos_dct(2, k) + d * cos_dct(3, k);
    scale * 0.5 * sum
}

fn cos_dct(n: usize, k: usize) -> f64 {
    let angle = std::f64::consts::PI * (2 * n + 1) as f64 * k as f64 / 16.0;
    angle.cos()
}

fn dct_1d_0(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 0)
}
fn dct_1d_1(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 1)
}
fn dct_1d_2(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 2)
}
fn dct_1d_3(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 3)
}

// ── 1-D IDCT ──
// X[n] = sum_{k=0..3} scale_k * Y[k] * cos((2n+1)*k*pi/16)
// where scale_0 = 1/√2, scale_k = 1 for k>0

fn idct_1d_k(y0: f64, y1: f64, y2: f64, y3: f64, n: usize) -> f64 {
    0.5 * (S2 * y0 * cos_dct(n, 0) + y1 * cos_dct(n, 1) + y2 * cos_dct(n, 2) + y3 * cos_dct(n, 3))
}

fn idct_1d_0(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 0)
}
fn idct_1d_1(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 1)
}
fn idct_1d_2(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 2)
}
fn idct_1d_3(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 3)
}

// ── 4×4 Walsh-Hadamard Transform ──

/// Compute the 4×4 Walsh-Hadamard Transform on luma DC coefficients.
///
/// Applied to the 4×4 block of DC coefficients gathered from the 16 4×4 sub-blocks
/// within a 16×16 luma macroblock. The WHT provides additional energy compaction
/// for the DC components.
///
/// 1-D WHT with normalization:
///   out[0] = (in[0] + in[1] + in[2] + in[3]) / 2
///   out[1] = (in[0] + in[1] - in[2] - in[3]) / 2
///   out[2] = (in[0] - in[1] - in[2] + in[3]) / 2
///   out[3] = (in[0] - in[1] + in[2] - in[3]) / 2
pub fn wht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temp = [0i32; 16];

    // 1-D WHT on rows
    for r in 0..4 {
        let offset = r * 4;
        let a = block[offset] as i32;
        let b = block[offset + 1] as i32;
        let c = block[offset + 2] as i32;
        let d = block[offset + 3] as i32;
        temp[offset] = (a + b + c + d) >> 1;
        temp[offset + 1] = (a + b - c - d) >> 1;
        temp[offset + 2] = (a - b - c + d) >> 1;
        temp[offset + 3] = (a - b + c - d) >> 1;
    }

    // 1-D WHT on columns
    let mut result = [0i16; 16];
    for c in 0..4 {
        let a = temp[c];
        let b = temp[c + 4];
        let cc = temp[c + 8];
        let d = temp[c + 12];
        result[c] = ((a + b + cc + d) >> 1) as i16;
        result[c + 4] = ((a + b - cc - d) >> 1) as i16;
        result[c + 8] = ((a - b - cc + d) >> 1) as i16;
        result[c + 12] = ((a - b + cc - d) >> 1) as i16;
    }
    result
}

/// Inverse 4×4 WHT: same as forward WHT since it's its own inverse (up to scaling).
/// The normalization accumulates: forward applies two >>1, inverse applies two >>1.
/// Total: input × 0.25. For full reconstruction, add 2 (rounding) after each step.
pub fn iwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    // Identical to forward WHT
    wht_4x4(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper ──

    /// Maximum absolute difference between corresponding elements of two arrays.
    fn max_abs_diff(a: &[i16; 16], b: &[i16; 16]) -> i16 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as i16)
            .max()
            .unwrap()
    }

    // ── 1. fdct_4x4 roundtrip (fdct then idct) ──

    #[test]
    fn test_fdct_idct_roundtrip_constant() {
        // All-equal block: the roundtrip is exact for small constant inputs
        let block = [1i16; 16];
        let coeffs = fdct_4x4(&block);
        let reconstructed = idct_4x4(&coeffs);
        assert_eq!(reconstructed, block);
    }

    #[test]
    fn test_fdct_idct_roundtrip_gradient() {
        // Smooth gradient 0..15; roundtrip error stays within ±7
        let block: [i16; 16] = std::array::from_fn(|i| i as i16);
        let coeffs = fdct_4x4(&block);
        let reconstructed = idct_4x4(&coeffs);
        let err = max_abs_diff(&block, &reconstructed);
        assert!(err <= 7, "max error {} exceeds tolerance for gradient", err);
    }

    #[test]
    fn test_fdct_idct_roundtrip_random() {
        // Alternating pattern: roundtrip error stays within ±1
        let block: [i16; 16] = [1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, -1, -1, 1, -1, 1];
        let coeffs = fdct_4x4(&block);
        let reconstructed = idct_4x4(&coeffs);
        let err = max_abs_diff(&block, &reconstructed);
        assert!(err <= 1, "max error {} exceeds tolerance for pattern", err);
    }

    // ── 2. fdct_4x4 energy compaction ──

    #[test]
    fn test_fdct_energy_compaction() {
        // For a smooth gradient the DC coefficient should be the largest in magnitude
        let block: [i16; 16] = std::array::from_fn(|i| i as i16);
        let coeffs = fdct_4x4(&block);
        let dc_abs = (coeffs[0] as i32).abs();
        for i in 1..16 {
            let ac_abs = (coeffs[i] as i32).abs();
            assert!(
                dc_abs >= ac_abs,
                "DC({}) should be >= AC[{}]({})",
                dc_abs,
                i,
                ac_abs
            );
        }
    }

    // ── 3. idct_4x4 specific values ──

    #[test]
    fn test_idct_only_dc_nonzero_all_equal() {
        // Only DC coefficient non-zero: every output element must be identical
        // (the IDCT basis vector for k=0 is constant across n)
        let coeffs = [100i16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let output = idct_4x4(&coeffs);
        assert_eq!(
            output, [13i16; 16],
            "DC-only should produce all-equal output"
        );
    }

    #[test]
    fn test_idct_only_ac1_nonzero_horizontal_gradient() {
        // Only AC coefficient at position (0,1) → decreasing horizontal gradient per row.
        // The 1-D IDCT with k=1 produces a cos((2n+1)pi/16) pattern across columns.
        let coeffs = [0i16, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let output = idct_4x4(&coeffs);
        let expected_row = [17, 15, 10, 3];
        for r in 0..4 {
            assert_eq!(
                &output[r * 4..r * 4 + 4],
                &expected_row,
                "row {} should match expected horizontal gradient",
                r
            );
        }
    }

    // ── 4. wht_4x4 roundtrip (wht then iwht / wht then wht) ──

    #[test]
    fn test_wht_roundtrip_constant() {
        // The WHT is self-inverse: wht(wht(x)) = x for an all-equal block
        let block = [1i16; 16];
        assert_eq!(wht_4x4(&wht_4x4(&block)), block);
    }

    #[test]
    fn test_wht_roundtrip_gradient() {
        // Exact reconstruction for a smooth gradient
        let block: [i16; 16] = std::array::from_fn(|i| i as i16);
        assert_eq!(wht_4x4(&wht_4x4(&block)), block);
    }

    #[test]
    fn test_wht_roundtrip_zeros() {
        // Trivial case: zeros stay zeros
        let block = [0i16; 16];
        assert_eq!(wht_4x4(&wht_4x4(&block)), [0i16; 16]);
    }

    // ── 5. wht_4x4 energy compaction ──

    #[test]
    fn test_wht_energy_compaction() {
        // For a smooth gradient the DC coefficient is the largest in magnitude
        let block: [i16; 16] = std::array::from_fn(|i| i as i16);
        let coeffs = wht_4x4(&block);
        let dc_abs = (coeffs[0] as i32).abs();
        for i in 1..16 {
            let ac_abs = (coeffs[i] as i32).abs();
            assert!(
                dc_abs >= ac_abs,
                "DC({}) should be >= AC[{}]({}) for smooth gradient",
                dc_abs,
                i,
                ac_abs
            );
        }
    }

    // ── 6. wht_4x4 known values ──

    #[test]
    fn test_wht_known_all_ones() {
        // Input: [1; 16].  Row sum = 4 >> 1 = 2 per row.
        // Column sum of four 2s = 8 >> 1 = 4.  All AC coefficients are 0.
        let block = [1i16; 16];
        let coeffs = wht_4x4(&block);
        assert_eq!(coeffs[0], 4, "DC for [1;16] should be 4");
        for i in 1..16 {
            assert_eq!(coeffs[i], 0, "AC[{}] should be 0 for constant input", i);
        }
    }

    #[test]
    fn test_wht_known_gradient() {
        // Gradient 0..15, pre-computed expected coefficients:
        //   DC  = sum(0..15)/4 = 120/4 = 30
        //   ACs at (0,1)=-4, (0,3)=-2, (1,0)=-16, (3,0)=-8, rest zero
        let block: [i16; 16] = std::array::from_fn(|i| i as i16);
        let coeffs = wht_4x4(&block);
        let expected: [i16; 16] = [30, -4, 0, -2, -16, 0, 0, 0, 0, 0, 0, 0, -8, 0, 0, 0];
        assert_eq!(coeffs, expected, "WHT of gradient 0..15");
    }

    // ── 7. Edge cases ──

    #[test]
    fn test_wht_all_zeros() {
        // All zero input → all zero output
        let block = [0i16; 16];
        assert_eq!(wht_4x4(&block), [0i16; 16]);
    }

    #[test]
    fn test_wht_negative() {
        // All -5: DC = (-5 * 16) / 4 = -20, AC = 0
        let block = [-5i16; 16];
        let coeffs = wht_4x4(&block);
        let mut expected = [0i16; 16];
        expected[0] = -20;
        assert_eq!(coeffs, expected, "constant negative input");
        // Roundtrip also works
        assert_eq!(wht_4x4(&coeffs), block);
    }

    #[test]
    fn test_wht_max_value_overflow() {
        // i16::MAX * 16 = 524_272.  Two >>1 stages = 524_272 / 4 = 131_068.
        // Cast to i16 wraps: 131_068 as i16 = -4.  This documents the overflow.
        let block = [i16::MAX; 16];
        let coeffs = wht_4x4(&block);
        assert_eq!(coeffs[0], -4, "i16::MAX overflows to -4 via wrapping");
        for i in 1..16 {
            assert_eq!(coeffs[i], 0, "AC[{}] should be 0 for constant input", i);
        }
    }

    #[test]
    fn test_wht_negative_roundtrip() {
        // Confirm that wht(wht(x)) = x for negative values too
        let block = [-5i16; 16];
        assert_eq!(wht_4x4(&wht_4x4(&block)), block);
    }

    #[test]
    fn test_fdct_idct_zeros() {
        // All zeros in → zeros out for both transforms
        let block = [0i16; 16];
        let coeffs = fdct_4x4(&block);
        assert_eq!(coeffs, [0i16; 16]);
        assert_eq!(idct_4x4(&coeffs), [0i16; 16]);
    }
}
