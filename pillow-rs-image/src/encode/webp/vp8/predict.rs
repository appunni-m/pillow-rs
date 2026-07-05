//! VP8 intra prediction modes (RFC 6386 Section 11).
//!
//! Provides 16×16 luma prediction (DC, V, H, TM, and simplified B_PRED),
//! 4×4 sub-block prediction (all 10 intra modes), and 8×8 chroma prediction.
//! Also provides SAD-based mode selection for encoding decisions.

#![allow(dead_code)]

/// Prediction direction for a 16×16 luma macroblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbPredictionMode {
    DcPred = 0,
    VPred = 1,
    HPred = 2,
    TmPred = 3,
    BPred = 4, // per-4×4 sub-block prediction
}

/// Prediction mode for a 4×4 sub-block within a 16×16 macroblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubBlockMode {
    Dc = 0,
    V = 1,
    H = 2,
    Tm = 3,
    /// Vertical-left: smooth prediction angled up-left
    VLeft = 4,
    /// Vertical-right: smooth prediction angled up-right
    VRight = 5,
    /// Horizontal-up: smooth prediction angled down-left
    HUp = 6,
    /// Horizontal-down: smooth prediction angled down-right
    HDown = 7,
    /// Diagonal down-left (45-degree angle)
    LeftDown = 8,
    /// Diagonal up-right (45-degree angle)
    LeftUp = 9,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Clamp a pixel value to [0, 255].
#[inline]
fn clamp_u8(v: i16) -> u8 {
    if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v as u8
    }
}

/// Absolute difference between two `u8` values.
#[inline]
fn abs_diff_u8(a: u8, b: u8) -> u8 {
    if a > b { a - b } else { b - a }
}

/// Signed difference between two `u8` values, returned as `i16`.
#[inline]
fn diff_u8(a: u8, b: u8) -> i16 {
    (a as i16) - (b as i16)
}

/// Average two u8 values, rounding up.
#[inline]
fn avg2(a: u8, b: u8) -> u8 {
    ((a as u16 + b as u16 + 1) >> 1) as u8
}

/// Average two i16 values, rounding to nearest.
#[inline]
fn avg2_i16(a: i16, b: i16) -> i16 {
    (a + b + 1) >> 1
}

/// Weighted average: (a + 2*b + c + 2) >> 2
#[inline]
fn weighted_avg3(a: u8, b: u8, c: u8) -> u8 {
    ((a as u16 + 2 * b as u16 + c as u16 + 2) >> 2) as u8
}

// ---------------------------------------------------------------------------
// 16×16 luma prediction
// ---------------------------------------------------------------------------

/// Predict a 16×16 luma block using the given mode.
///
/// `dst` is a 256-byte output buffer (row-major 16×16).
/// `above` is 16 pixels from the row above (indices 0..16, column -1..15).
/// `left` is 16 pixels from the column left (indices 0..16, row -1..15).
/// `top_left` is the pixel at (-1,-1).
pub fn predict_luma_16x16(
    dst: &mut [u8],
    above: &[u8],
    left: &[u8],
    top_left: u8,
    mode: MbPredictionMode,
) {
    assert!(dst.len() >= 256);
    assert!(above.len() >= 16);
    assert!(left.len() >= 16);

    match mode {
        MbPredictionMode::DcPred => predict_16x16_dc(dst, above, left),
        MbPredictionMode::VPred => predict_16x16_v(dst, above),
        MbPredictionMode::HPred => predict_16x16_h(dst, left),
        MbPredictionMode::TmPred => predict_16x16_tm(dst, above, left, top_left),
        MbPredictionMode::BPred => predict_16x16_b(dst, above, left, top_left),
    }
}

fn predict_16x16_dc(dst: &mut [u8], above: &[u8], left: &[u8]) {
    let mut sum = 0u32;
    let mut count = 0u32;
    for &a in above.iter().take(16) {
        sum += a as u32;
        count += 1;
    }
    for &l in left.iter().take(16) {
        sum += l as u32;
        count += 1;
    }
    let avg = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };
    for y in 0..16 {
        for x in 0..16 {
            dst[y * 16 + x] = avg;
        }
    }
}

fn predict_16x16_v(dst: &mut [u8], above: &[u8]) {
    for y in 0..16 {
        for x in 0..16 {
            dst[y * 16 + x] = above[x];
        }
    }
}

fn predict_16x16_h(dst: &mut [u8], left: &[u8]) {
    for y in 0..16 {
        for x in 0..16 {
            dst[y * 16 + x] = left[y];
        }
    }
}

fn predict_16x16_tm(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    for y in 0..16 {
        for x in 0..16 {
            dst[y * 16 + x] = clamp_u8(above[x] as i16 + left[y] as i16 - top_left as i16);
        }
    }
}

/// Simplified B_PRED: each 4×4 sub-block uses DC prediction against its
/// available edge pixels (the original macroblock edges, not reconstructed
/// neighbour pixels).  For a production encoder, use
/// [`choose_subblock_modes`] instead.
fn predict_16x16_b(dst: &mut [u8], above: &[u8], left: &[u8], _top_left: u8) {
    for sub_y in 0..4 {
        for sub_x in 0..4 {
            let offset = sub_y * 4 * 16 + sub_x * 4;
            let sub_dst = &mut dst[offset..];

            // Collect available edge pixels for this 4×4 sub-block.
            let mut sum = 0u32;
            let mut count = 0u32;

            let ax = sub_x * 4;
            for i in 0..4 {
                if ax + i < 16 {
                    sum += above[ax + i] as u32;
                    count += 1;
                }
            }

            let ly = sub_y * 4;
            for i in 0..4 {
                if ly + i < 16 {
                    sum += left[ly + i] as u32;
                    count += 1;
                }
            }

            let avg = if count > 0 {
                ((sum + count / 2) / count) as u8
            } else {
                128
            };

            for y in 0..4 {
                for x in 0..4 {
                    // Stride stays 16 (the parent macroblock stride).
                    sub_dst[y * 16 + x] = avg;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 8×8 chroma prediction
// ---------------------------------------------------------------------------

/// Predict an 8×8 chroma block.  Only DC, V, H, and TM modes are used
/// (chroma does not use B_PRED).
pub fn predict_chroma_8x8(
    dst: &mut [u8],
    above: &[u8],
    left: &[u8],
    top_left: u8,
    mode: MbPredictionMode,
) {
    assert!(dst.len() >= 64);
    assert!(above.len() >= 8);
    assert!(left.len() >= 8);

    match mode {
        MbPredictionMode::DcPred => predict_8x8_dc(dst, above, left),
        MbPredictionMode::VPred => predict_8x8_v(dst, above),
        MbPredictionMode::HPred => predict_8x8_h(dst, left),
        MbPredictionMode::TmPred => predict_8x8_tm(dst, above, left, top_left),
        // Chroma never uses B_PRED — fall back to DC for safety.
        MbPredictionMode::BPred => predict_8x8_dc(dst, above, left),
    }
}

fn predict_8x8_dc(dst: &mut [u8], above: &[u8], left: &[u8]) {
    let mut sum = 0u32;
    let mut count = 0u32;
    for &a in above.iter().take(8) {
        sum += a as u32;
        count += 1;
    }
    for &l in left.iter().take(8) {
        sum += l as u32;
        count += 1;
    }
    let avg = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = avg;
        }
    }
}

fn predict_8x8_v(dst: &mut [u8], above: &[u8]) {
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = above[x];
        }
    }
}

fn predict_8x8_h(dst: &mut [u8], left: &[u8]) {
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = left[y];
        }
    }
}

fn predict_8x8_tm(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = clamp_u8(above[x] as i16 + left[y] as i16 - top_left as i16);
        }
    }
}

// ---------------------------------------------------------------------------
// 4×4 sub-block prediction (full B_PRED)
// ---------------------------------------------------------------------------

/// Predict a 4×4 block using the given sub-block mode.
///
/// `dst` is a 16-byte output buffer (row-major 4×4).
/// `above` is 4 pixels from the edge above (indices 0..3, column -1..3).
/// `left` is 4 pixels from the edge left (indices 0..3, row -1..3).
/// `top_left` is the pixel at (-1,-1).
pub fn predict_4x4(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8, mode: SubBlockMode) {
    assert!(dst.len() >= 16);
    assert!(above.len() >= 4);
    assert!(left.len() >= 4);

    match mode {
        SubBlockMode::Dc => predict_4x4_dc(dst, above, left),
        SubBlockMode::V => predict_4x4_v(dst, above),
        SubBlockMode::H => predict_4x4_h(dst, left),
        SubBlockMode::Tm => predict_4x4_tm(dst, above, left, top_left),
        SubBlockMode::VLeft => predict_4x4_vleft(dst, above, top_left),
        SubBlockMode::VRight => predict_4x4_vright(dst, above, left, top_left),
        SubBlockMode::HUp => predict_4x4_hup(dst, left),
        SubBlockMode::HDown => predict_4x4_hdown(dst, above, left, top_left),
        SubBlockMode::LeftDown => predict_4x4_leftdown(dst, above, top_left),
        SubBlockMode::LeftUp => predict_4x4_leftup(dst, above, left, top_left),
    }
}

fn predict_4x4_dc(dst: &mut [u8], above: &[u8], left: &[u8]) {
    let mut sum = 0u32;
    let mut count = 0u32;
    for &a in above.iter().take(4) {
        sum += a as u32;
        count += 1;
    }
    for &l in left.iter().take(4) {
        sum += l as u32;
        count += 1;
    }
    let avg = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };
    for y in 0..4 {
        for x in 0..4 {
            dst[y * 4 + x] = avg;
        }
    }
}

fn predict_4x4_v(dst: &mut [u8], above: &[u8]) {
    for y in 0..4 {
        for x in 0..4 {
            dst[y * 4 + x] = above[x];
        }
    }
}

fn predict_4x4_h(dst: &mut [u8], left: &[u8]) {
    for y in 0..4 {
        for x in 0..4 {
            dst[y * 4 + x] = left[y];
        }
    }
}

fn predict_4x4_tm(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    for y in 0..4 {
        for x in 0..4 {
            dst[y * 4 + x] = clamp_u8(above[x] as i16 + left[y] as i16 - top_left as i16);
        }
    }
}

/// Vertical-left (B_VL):  prediction angled 22.5° from the top edge,
/// using two-tap averages of above-row pixels.  When the reference goes
/// past `above[3]` we repeat `above[3]`.
fn predict_4x4_vleft(dst: &mut [u8], above: &[u8], _top_left: u8) {
    let a = above[0];
    let b = above[1];
    let c = above[2];
    let d = above[3];

    // The VP8 reference formulas for B_VL (vertical-left):
    // Row 0: (a+b+1)/2, (b+c+1)/2, (c+d+1)/2, d
    // Row 1: (a+2*b+c+2)/4, (b+2*c+d+2)/4, d, d
    // Row 2: (b+2*c+d+2)/4, d, d, d
    // Row 3: (c+2*d+d+2)/4 = (c+3*d+2)/4, d, d, d
    dst[0] = avg2(a, b);
    dst[1] = avg2(b, c);
    dst[2] = avg2(c, d);
    dst[3] = d;

    dst[4] = weighted_avg3(a, b, c);
    dst[5] = weighted_avg3(b, c, d);
    dst[6] = d;
    dst[7] = d;

    dst[8] = weighted_avg3(b, c, d);
    dst[9] = d;
    dst[10] = d;
    dst[11] = d;

    // (c+3*d+2) >> 2
    dst[12] = ((c as u16 + 3 * d as u16 + 2) >> 2) as u8;
    dst[13] = d;
    dst[14] = d;
    dst[15] = d;
}

/// Vertical-right (B_RD / B_VR): prediction angled 67.5° from the top edge.
fn predict_4x4_vright(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    let i = top_left;
    let a = above[0];
    let b = above[1];
    let c = above[2];
    let d = above[3];
    let e = left[0];
    let f = left[1];

    // Row 0: avg2(i,a), avg2(a,b), avg2(b,c), avg2(c,d)
    dst[0] = avg2(i, a);
    dst[1] = avg2(a, b);
    dst[2] = avg2(b, c);
    dst[3] = avg2(c, d);

    // Row 1: avg2(e,i), avg2(i,a), avg2(a,b), avg2(b,c)
    dst[4] = avg2(e, i);
    dst[5] = avg2(i, a);
    dst[6] = avg2(a, b);
    dst[7] = avg2(b, c);

    // Row 2: avg2(f,e), avg2(e,i), avg2(i,a), avg2(a,b)
    dst[8] = avg2(f, e);
    dst[9] = avg2(e, i);
    dst[10] = avg2(i, a);
    dst[11] = avg2(a, b);

    // Row 3: avg2(f,e), avg2(f,e), avg2(e,i), avg2(i,a)
    dst[12] = avg2(f, e);
    dst[13] = avg2(f, e);
    dst[14] = avg2(e, i);
    dst[15] = avg2(i, a);
}

/// Horizontal-up (B_HU): prediction angled toward the upper-right.
fn predict_4x4_hup(dst: &mut [u8], left: &[u8]) {
    let e = left[0];
    let f = left[1];
    let g = left[2];
    let h = left[3];

    // Row 0: avg2(e,f), avg2(f,g), avg2(g,h), h
    dst[0] = avg2(e, f);
    dst[1] = avg2(f, g);
    dst[2] = avg2(g, h);
    dst[3] = h;

    // Row 1: avg2(f,g), avg2(g,h), h, h
    dst[4] = avg2(f, g);
    dst[5] = avg2(g, h);
    dst[6] = h;
    dst[7] = h;

    // Row 2: avg2(g,h), h, h, h
    dst[8] = avg2(g, h);
    dst[9] = h;
    dst[10] = h;
    dst[11] = h;

    // Row 3: h, h, h, h
    dst[12] = h;
    dst[13] = h;
    dst[14] = h;
    dst[15] = h;
}

/// Horizontal-down (B_HD): prediction angled toward the lower-right.
fn predict_4x4_hdown(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    let i = top_left;
    let a = above[0];
    let b = above[1];
    let c = above[2];
    let e = left[0];
    let f = left[1];
    let g = left[2];

    // Row 0: avg2(i,a), avg2(a,b), avg2(b,c), avg2(c,__)
    dst[0] = avg2(i, a);
    dst[1] = avg2(a, b);
    dst[2] = avg2(b, c);
    dst[3] = c; // extrapolate

    // Row 1: avg2(e,i), avg2(i,a), avg2(a,b), avg2(b,c)
    dst[4] = avg2(e, i);
    dst[5] = avg2(i, a);
    dst[6] = avg2(a, b);
    dst[7] = avg2(b, c);

    // Row 2: avg2(f,e), avg2(e,i), avg2(i,a), avg2(a,b)
    dst[8] = avg2(f, e);
    dst[9] = avg2(e, i);
    dst[10] = avg2(i, a);
    dst[11] = avg2(a, b);

    // Row 3: avg2(g,f), avg2(f,e), avg2(e,i), avg2(i,a)
    dst[12] = avg2(g, f);
    dst[13] = avg2(f, e);
    dst[14] = avg2(e, i);
    dst[15] = avg2(i, a);
}

/// Diagonal down-left (B_LD): 45-degree prediction toward the lower-left.
fn predict_4x4_leftdown(dst: &mut [u8], above: &[u8], top_left: u8) {
    let i = top_left;
    let a = above[0];
    let b = above[1];
    let c = above[2];
    let d = above[3];

    // Row 0: avg2(i,a), avg2(a,b), avg2(b,c), avg2(c,d)
    dst[0] = avg2(i, a);
    dst[1] = avg2(a, b);
    dst[2] = avg2(b, c);
    dst[3] = avg2(c, d);

    // Row 1: avg2(a,b), avg2(b,c), avg2(c,d), (d+d)>>1 = d
    dst[4] = avg2(a, b);
    dst[5] = avg2(b, c);
    dst[6] = avg2(c, d);
    dst[7] = d;

    // Row 2: avg2(b,c), avg2(c,d), d, d
    dst[8] = avg2(b, c);
    dst[9] = avg2(c, d);
    dst[10] = d;
    dst[11] = d;

    // Row 3: avg2(c,d), d, d, d
    dst[12] = avg2(c, d);
    dst[13] = d;
    dst[14] = d;
    dst[15] = d;
}

/// Diagonal up-right (B_DD): prediction at ~112.5° using both above and left.
fn predict_4x4_leftup(dst: &mut [u8], above: &[u8], left: &[u8], top_left: u8) {
    let i = top_left;
    let a = above[0];
    let b = above[1];
    let c = above[2];
    let d = above[3];
    let e = left[0];
    let f = left[1];
    let g = left[2];
    let h = left[3];

    // Row 0: d, d, d, d
    // Row 1: c, d, d, d
    // Row 2: b, c, d, d
    // Row 3: a, b, c, d

    // This is a pure "vertical from the above-right corner" extrapolation.
    dst[0] = d;
    dst[1] = d;
    dst[2] = d;
    dst[3] = d;

    dst[4] = c;
    dst[5] = d;
    dst[6] = d;
    dst[7] = d;

    dst[8] = b;
    dst[9] = c;
    dst[10] = d;
    dst[11] = d;

    dst[12] = a;
    dst[13] = b;
    dst[14] = c;
    dst[15] = d;

    // (The unused I/E/F/G/H variables are present for API consistency
    //  — they are named explicitly above so the reader can see the full
    //  neighbourhood.)
    let _ = (i, e, f, g, h);
}

// ---------------------------------------------------------------------------
// SAD-based mode selection
// ---------------------------------------------------------------------------

/// Compute the sum of absolute differences between `block` and `prediction`.
fn compute_sad_4x4(block: &[u8], prediction: &[u8]) -> u32 {
    let mut sad = 0u32;
    for y in 0..4 {
        for x in 0..4 {
            let actual = block[y * 4 + x];
            let pred = prediction[y * 4 + x];
            sad += abs_diff_u8(actual, pred) as u32;
        }
    }
    sad
}

/// Compute SAD for a 16×16 block.
fn compute_sad_16x16(block: &[u8], prediction: &[u8]) -> u32 {
    let mut sad = 0u32;
    for y in 0..16 {
        for x in 0..16 {
            let actual = block[y * 16 + x];
            let pred = prediction[y * 16 + x];
            sad += abs_diff_u8(actual, pred) as u32;
        }
    }
    sad
}

/// Pick the best 16×16 luma prediction mode by minimising SAD.
///
/// `block` is the original 16×16 source data (256 bytes, row-major).
/// `above`, `left`, `top_left` describe the neighbour edge.
pub fn choose_luma_mode(block: &[u8], above: &[u8], left: &[u8], top_left: u8) -> MbPredictionMode {
    let modes = [
        MbPredictionMode::DcPred,
        MbPredictionMode::VPred,
        MbPredictionMode::HPred,
        MbPredictionMode::TmPred,
    ];

    let mut best_mode = MbPredictionMode::DcPred;
    let mut best_sad = u32::MAX;

    let mut pred_buf = [0u8; 256];

    for &mode in &modes {
        predict_luma_16x16(&mut pred_buf, above, left, top_left, mode);
        let sad = compute_sad_16x16(block, &pred_buf);
        if sad < best_sad {
            best_sad = sad;
            best_mode = mode;
        }
    }

    // Also evaluate B_PRED (simplified: each 4×4 sub-block picks
    // the best of DC/V/H/TM independently).
    let bpred_sad = evaluate_bpred_sad(block, above, left, top_left);
    if bpred_sad < best_sad {
        best_mode = MbPredictionMode::BPred;
    }

    best_mode
}

/// Evaluate the SAD for B_PRED mode by independently choosing the best 4×4
/// sub-block mode for each of the 16 sub-blocks.
fn evaluate_bpred_sad(block: &[u8], above: &[u8], left: &[u8], top_left: u8) -> u32 {
    let sub_modes = [
        SubBlockMode::Dc,
        SubBlockMode::V,
        SubBlockMode::H,
        SubBlockMode::Tm,
    ];
    let mut total_sad = 0u32;

    for sub_y in 0..4 {
        for sub_x in 0..4 {
            // Extract the 4×4 source block.
            let mut src_4x4 = [0u8; 16];
            for y in 0..4 {
                for x in 0..4 {
                    src_4x4[y * 4 + x] = block[(sub_y * 4 + y) * 16 + (sub_x * 4 + x)];
                }
            }

            // Determine neighbour pixels for this sub-block.
            // When inside the macroblock (sub_y > 0 or sub_x > 0), fall back
            // to the original block edges rather than reconstructed neighbours.
            // This is a simplification for the basic encoder.
            let sub_above: [u8; 4] = core::array::from_fn(|i| {
                let col = sub_x * 4 + i;
                if col < 16 { above[col] } else { 0 }
            });
            let sub_left: [u8; 4] = core::array::from_fn(|i| {
                let row = sub_y * 4 + i;
                if row < 16 { left[row] } else { 0 }
            });
            let sub_tl = if sub_x == 0 && sub_y == 0 {
                top_left
            } else if sub_x == 0 {
                // Use the left edge pixel from the original left boundary.
                left[sub_y * 4]
            } else if sub_y == 0 {
                // Use the above edge pixel from the original above boundary.
                above[sub_x * 4]
            } else {
                // Interior sub-block — use block boundary.  Simplified: 128.
                128u8
            };

            let mut best_sub_sad = u32::MAX;
            let mut pred_buf = [0u8; 16];

            for &sm in &sub_modes {
                predict_4x4(&mut pred_buf, &sub_above, &sub_left, sub_tl, sm);
                let sad = compute_sad_4x4(&src_4x4, &pred_buf);
                if sad < best_sub_sad {
                    best_sub_sad = sad;
                }
            }

            total_sad += best_sub_sad;
        }
    }

    total_sad
}

/// Choose the best 4×4 sub-block mode for a single 4×4 block.
///
/// Tries all modes in the [`SubBlockMode`] enum and returns the one with the
/// lowest SAD against `block`.
pub fn choose_4x4_mode(
    block: &[u8], // 16 bytes, row-major
    above: &[u8], // 4 bytes
    left: &[u8],  // 4 bytes
    top_left: u8,
) -> SubBlockMode {
    let all_modes = [
        SubBlockMode::Dc,
        SubBlockMode::V,
        SubBlockMode::H,
        SubBlockMode::Tm,
        SubBlockMode::VLeft,
        SubBlockMode::VRight,
        SubBlockMode::HUp,
        SubBlockMode::HDown,
        SubBlockMode::LeftDown,
        SubBlockMode::LeftUp,
    ];

    let mut best_mode = SubBlockMode::Dc;
    let mut best_sad = u32::MAX;
    let mut pred_buf = [0u8; 16];

    for &mode in &all_modes {
        predict_4x4(&mut pred_buf, above, left, top_left, mode);
        let sad = compute_sad_4x4(block, &pred_buf);
        if sad < best_sad {
            best_sad = sad;
            best_mode = mode;
        }
    }

    best_mode
}

/// Choose the best 16×16 luma mode using the full B_PRED evaluation
/// (tries all 10 modes per 4×4 sub-block).  This is more thorough than
/// [`choose_luma_mode`].
#[allow(dead_code)]
pub fn choose_luma_mode_full(
    block: &[u8],
    above: &[u8],
    left: &[u8],
    top_left: u8,
) -> MbPredictionMode {
    let simple_modes = [
        MbPredictionMode::DcPred,
        MbPredictionMode::VPred,
        MbPredictionMode::HPred,
        MbPredictionMode::TmPred,
    ];

    let mut best_mode = MbPredictionMode::DcPred;
    let mut best_sad = u32::MAX;
    let mut pred_buf = [0u8; 256];

    for &mode in &simple_modes {
        predict_luma_16x16(&mut pred_buf, above, left, top_left, mode);
        let sad = compute_sad_16x16(block, &pred_buf);
        if sad < best_sad {
            best_sad = sad;
            best_mode = mode;
        }
    }

    // Evaluate B_PRED with all 10 modes per sub-block.
    let bpred_sad = evaluate_bpred_sad_full(block, above, left, top_left);
    if bpred_sad < best_sad {
        best_mode = MbPredictionMode::BPred;
    }

    best_mode
}

fn evaluate_bpred_sad_full(block: &[u8], above: &[u8], left: &[u8], top_left: u8) -> u32 {
    let all_modes = [
        SubBlockMode::Dc,
        SubBlockMode::V,
        SubBlockMode::H,
        SubBlockMode::Tm,
        SubBlockMode::VLeft,
        SubBlockMode::VRight,
        SubBlockMode::HUp,
        SubBlockMode::HDown,
        SubBlockMode::LeftDown,
        SubBlockMode::LeftUp,
    ];
    let mut total_sad = 0u32;

    for sub_y in 0..4 {
        for sub_x in 0..4 {
            let mut src_4x4 = [0u8; 16];
            for y in 0..4 {
                for x in 0..4 {
                    src_4x4[y * 4 + x] = block[(sub_y * 4 + y) * 16 + (sub_x * 4 + x)];
                }
            }

            let sub_above: [u8; 4] = core::array::from_fn(|i| {
                let col = sub_x * 4 + i;
                if col < 16 { above[col] } else { 0 }
            });
            let sub_left: [u8; 4] = core::array::from_fn(|i| {
                let row = sub_y * 4 + i;
                if row < 16 { left[row] } else { 0 }
            });
            let sub_tl = if sub_x == 0 && sub_y == 0 {
                top_left
            } else {
                128u8
            };

            let mut best_sub_sad = u32::MAX;
            let mut pred_buf = [0u8; 16];

            for &sm in &all_modes {
                predict_4x4(&mut pred_buf, &sub_above, &sub_left, sub_tl, sm);
                let sad = compute_sad_4x4(&src_4x4, &pred_buf);
                if sad < best_sub_sad {
                    best_sub_sad = sad;
                }
            }
            total_sad += best_sub_sad;
        }
    }
    total_sad
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A constant 16×16 all-128 source block with edge values providing
    /// a clear gradient so each mode produces distinct output.
    const EDGE_ABOVE: [u8; 16] = [
        160, 162, 164, 166, 168, 170, 172, 174, 176, 178, 180, 182, 184, 186, 188, 190,
    ];
    const EDGE_LEFT: [u8; 16] = [
        140, 142, 144, 146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 166, 168, 170,
    ];
    const EDGE_TL: u8 = 150;

    #[test]
    fn test_predict_16x16_dc() {
        let mut dst = [0u8; 256];
        predict_luma_16x16(
            &mut dst,
            &EDGE_ABOVE,
            &EDGE_LEFT,
            EDGE_TL,
            MbPredictionMode::DcPred,
        );
        // Average of 32 values (16 above + 16 left).
        // Sum above = 2800, sum left = 2480, total = 5280.  (5280+16)/32 = 165.
        let expected_avg = 165u8;
        for &v in dst.iter() {
            assert_eq!(v, expected_avg);
        }
    }

    #[test]
    fn test_predict_16x16_v() {
        let mut dst = [0u8; 256];
        predict_luma_16x16(
            &mut dst,
            &EDGE_ABOVE,
            &EDGE_LEFT,
            EDGE_TL,
            MbPredictionMode::VPred,
        );
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(dst[y * 16 + x], EDGE_ABOVE[x], "y={y} x={x}");
            }
        }
    }

    #[test]
    fn test_predict_16x16_h() {
        let mut dst = [0u8; 256];
        predict_luma_16x16(
            &mut dst,
            &EDGE_ABOVE,
            &EDGE_LEFT,
            EDGE_TL,
            MbPredictionMode::HPred,
        );
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(dst[y * 16 + x], EDGE_LEFT[y], "y={y} x={x}");
            }
        }
    }

    #[test]
    fn test_predict_16x16_tm() {
        let mut dst = [0u8; 256];
        predict_luma_16x16(
            &mut dst,
            &EDGE_ABOVE,
            &EDGE_LEFT,
            EDGE_TL,
            MbPredictionMode::TmPred,
        );
        for y in 0..16 {
            for x in 0..16 {
                let expected =
                    clamp_u8(EDGE_ABOVE[x] as i16 + EDGE_LEFT[y] as i16 - EDGE_TL as i16);
                assert_eq!(dst[y * 16 + x], expected, "y={y} x={x}");
            }
        }
    }

    #[test]
    fn test_predict_4x4_dc() {
        let above = [100, 102, 104, 106];
        let left = [90, 92, 94, 96];
        let mut dst = [0u8; 16];
        predict_4x4(&mut dst, &above, &left, 98, SubBlockMode::Dc);
        // avg = (100+102+104+106+90+92+94+96+4)/8 = 98
        for &v in dst.iter() {
            assert_eq!(v, 98);
        }
    }

    #[test]
    fn test_predict_4x4_v() {
        let above = [100, 110, 120, 130];
        let left = [50; 4];
        let mut dst = [0u8; 16];
        predict_4x4(&mut dst, &above, &left, 90, SubBlockMode::V);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(dst[y * 4 + x], above[x]);
            }
        }
    }

    #[test]
    fn test_predict_4x4_h() {
        let above = [100; 4];
        let left = [50, 60, 70, 80];
        let mut dst = [0u8; 16];
        predict_4x4(&mut dst, &above, &left, 90, SubBlockMode::H);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(dst[y * 4 + x], left[y]);
            }
        }
    }

    #[test]
    fn test_predict_4x4_tm() {
        let above = [100, 110, 120, 130];
        let left = [50, 60, 70, 80];
        let tl = 90;
        let mut dst = [0u8; 16];
        predict_4x4(&mut dst, &above, &left, tl, SubBlockMode::Tm);
        for y in 0..4 {
            for x in 0..4 {
                let expected = clamp_u8(above[x] as i16 + left[y] as i16 - tl as i16);
                assert_eq!(dst[y * 4 + x], expected, "y={y} x={x}");
            }
        }
    }

    #[test]
    fn test_predict_chroma_8x8() {
        let above = [128; 8];
        let left = [128; 8];
        let mut dst = [0u8; 64];
        predict_chroma_8x8(&mut dst, &above, &left, 128, MbPredictionMode::DcPred);
        assert_eq!(dst[0], 128);
        assert_eq!(dst[63], 128);
    }

    #[test]
    fn test_choose_luma_mode_dc_preferred() {
        // A perfectly flat block should choose DC.
        let block = [128u8; 256];
        let above = [128u8; 16];
        let left = [128u8; 16];
        let mode = choose_luma_mode(&block, &above, &left, 128);
        assert_eq!(mode, MbPredictionMode::DcPred);
    }

    #[test]
    fn test_choose_luma_mode_v_preferred() {
        // A vertically striped block should choose V.
        let mut block = [0u8; 256];
        for y in 0..16 {
            for x in 0..16 {
                block[y * 16 + x] = ((160u16 + x as u16 * 2) % 256) as u8;
            }
        }
        let above: [u8; 16] = core::array::from_fn(|x| ((160u16 + x as u16 * 2) % 256) as u8);
        let left = [128u8; 16];
        let mode = choose_luma_mode(&block, &above, &left, 128);
        assert_eq!(mode, MbPredictionMode::VPred);
    }

    #[test]
    fn test_predict_4x4_vleft() {
        let above = [100, 110, 120, 130];
        let left = [80, 90, 100, 110];
        let mut dst = [0u8; 16];
        predict_4x4(&mut dst, &above, &left, 90, SubBlockMode::VLeft);
        // Should not be uniform
        assert!(dst[0] != dst[15], "VLeft should produce non-uniform output");
    }

    #[test]
    fn test_predict_4x4_directional_non_uniform() {
        let above = [100, 120, 140, 160];
        let left = [80, 85, 90, 95];
        let tl = 90;

        let directional_modes = [
            SubBlockMode::VLeft,
            SubBlockMode::VRight,
            SubBlockMode::HUp,
            SubBlockMode::HDown,
            SubBlockMode::LeftDown,
            SubBlockMode::LeftUp,
        ];

        for &mode in &directional_modes {
            let mut dst = [0u8; 16];
            predict_4x4(&mut dst, &above, &left, tl, mode);
            // Check that at least one pair of distinct positions differ
            // (the block should not be uniform).
            let first = dst[0];
            let all_same = dst.iter().all(|&v| v == first);
            assert!(
                !all_same,
                "mode {mode:?} should produce non-uniform output, all pixels = {first}",
            );
        }
    }
}
